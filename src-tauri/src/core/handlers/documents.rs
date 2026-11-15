//! Resumir o analizar un documento: se busca, se ofrece y se pide confirmación. El contenido solo se lee y se
//! envía después, con `Assistant::analyze_document`, cuando la persona confirma ese archivo concreto.

use std::{path::Path, sync::Arc};

use async_trait::async_trait;

use super::{CommandHandler, FilesHandler, HandlerContext, Outcome};
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
            return self.consent(task, Vec::new(), None);
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
        let candidates: Vec<DocumentCandidate> = found
            .files
            .iter()
            .filter(|file| file.exists && is_supported(Path::new(&file.path)))
            .take(MAX_CANDIDATES)
            .map(|file| candidate(&self.offers, Path::new(&file.path), &file.location))
            .collect();
        let note = candidates.is_empty().then(|| {
            "No encontré ningún documento con ese nombre en tus carpetas autorizadas. Puedes elegirlo tú.".to_owned()
        });
        self.consent(task, candidates, note)
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
