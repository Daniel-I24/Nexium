//! Resumir o analizar un documento: se busca, se ofrece y se pide confirmación. El contenido solo se lee y se
//! envía después, con `Assistant::analyze_document`, cuando la persona confirma ese archivo concreto.

use std::{path::Path, sync::Arc};

use async_trait::async_trait;

use super::{CommandHandler, FilesHandler, HandlerContext, Outcome};
use crate::core::rules::normalize;
use crate::{
    models::{Category, DocumentCandidate, DocumentTask, FileQuery, Intent, ResponseBody},
    providers::documents::{is_supported, DocumentOffers, MAX_DOCUMENT_CHARS},
};

/// Archivos que se ofrecen a la vez, como mucho.
const MAX_CANDIDATES: usize = 5;

pub struct DocumentsHandler {
    pub files: FilesHandler,
    pub offers: Arc<DocumentOffers>,
    pub provider: String,
}

/// Ofrece un archivo: la interfaz recibe su nombre y un identificador, no la ruta.
pub fn candidate(offers: &DocumentOffers, path: &Path, location: &str) -> DocumentCandidate {
    DocumentCandidate {
        token: offers.offer(path.to_path_buf()),
        name: path
            .file_stem()
            .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned()),
        extension: path.extension().map_or_else(String::new, |extension| {
            format!(".{}", extension.to_string_lossy())
        }),
        location: location.to_owned(),
    }
}

impl DocumentsHandler {
    fn consent(
        &self,
        task: DocumentTask,
        candidates: Vec<DocumentCandidate>,
        note: Option<String>,
        preselected: Option<String>,
    ) -> Outcome {
        Outcome::pending(
            Category::Files,
            "Pendiente de confirmación",
            ResponseBody::DocumentConsent {
                task,
                provider: self.provider.clone(),
                candidates,
                max_characters: MAX_DOCUMENT_CHARS,
                note,
                preselected,
            },
        )
    }

    async fn answer(
        &self,
        task: DocumentTask,
        query: &FileQuery,
        context: &HandlerContext,
    ) -> Outcome {
        // "Analiza este archivo": no hay nada que buscar; se elige en el selector del sistema.
        if query.exact_name.is_none() && query.terms.is_empty() {
            return self.consent(task, Vec::new(), None, None);
        }
        let allowed = match self.files.allowed() {
            Ok(allowed) if allowed.is_empty() => return self.files.consent(&context.request),
            Ok(allowed) => allowed,
            Err(outcome) => return *outcome,
        };
        let found = self
            .files
            .search
            .search(query.clone(), allowed, context.cancel.clone())
            .await;
        if context.cancel.is_cancelled() {
            return Outcome::failed(Category::Files, "cancelled", "Búsqueda cancelada.", None);
        }
        let files: Vec<_> = found
            .files
            .iter()
            .filter(|file| file.exists && is_supported(Path::new(&file.path)))
            .take(MAX_CANDIDATES)
            .collect();
        let scores: Vec<usize> = files
            .iter()
            .map(|file| relevance(&file.name, query))
            .collect();
        let clear = clear_winner(&scores);
        let candidates: Vec<DocumentCandidate> = files
            .iter()
            .map(|file| candidate(&self.offers, Path::new(&file.path), &file.location))
            .collect();
        let preselected = clear.map(|index| candidates[index].token.clone());
        let note = candidates.is_empty().then(|| {
            "No encontré ningún documento con ese nombre en tus carpetas autorizadas. Puedes elegirlo tú.".to_owned()
        });
        self.consent(task, candidates, note, preselected)
    }
}

/// Cuántos términos de la búsqueda tiene el nombre del archivo.
fn relevance(name: &str, query: &FileQuery) -> usize {
    let name = normalize(name);
    if let Some(exact) = &query.exact_name {
        return usize::from(
            name == normalize(exact.rsplit_once('.').map_or(exact, |(stem, _)| stem)),
        );
    }
    query
        .terms
        .iter()
        .filter(|term| name.contains(&normalize(term)))
        .count()
}

/// La coincidencia es clara si solo hay un candidato, o si el primero tiene más términos que el segundo.
pub fn clear_winner(scores: &[usize]) -> Option<usize> {
    match scores {
        [_] => Some(0),
        [first, second, ..] if first > second => Some(0),
        _ => None,
    }
}

#[async_trait]
impl CommandHandler for DocumentsHandler {
    async fn handle(&self, intent: &Intent, context: &HandlerContext) -> Option<Outcome> {
        let Intent::Document { task, query } = intent else {
            return None;
        };
        Some(self.answer(*task, query, context).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_clear_match_is_preselected() {
        assert_eq!(clear_winner(&[1]), Some(0));
        assert_eq!(clear_winner(&[2, 1, 1]), Some(0));
        assert_eq!(clear_winner(&[1, 1]), None);
        assert_eq!(clear_winner(&[]), None);
    }

    #[test]
    fn relevance_counts_the_search_terms_in_the_name() {
        let query = FileQuery {
            exact_name: None,
            terms: vec!["bases".into(), "datos".into()],
        };
        assert_eq!(relevance("Guía de Bases de Datos", &query), 2);
        assert_eq!(relevance("datos-2025", &query), 1);
        let exact = FileQuery {
            exact_name: Some("acta.docx".into()),
            terms: vec![],
        };
        assert_eq!(relevance("acta", &exact), 1);
    }
}
