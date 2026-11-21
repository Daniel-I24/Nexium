//! Búsqueda de archivos en las carpetas autorizadas.

use std::sync::Arc;

use async_trait::async_trait;

use super::{CommandHandler, HandlerContext, Outcome};
use crate::{
    models::{Category, FileQuery, Intent, ResponseBody},
    providers::files::{authorized_folders, AuthorizedFolder, FileSearchProvider},
    services::settings::SettingsService,
};

pub struct FilesHandler {
    pub search: Arc<dyn FileSearchProvider>,
    pub settings: SettingsService,
    pub known_folders: Vec<AuthorizedFolder>,
}

impl FilesHandler {
    /// Permiso pendiente: la solicitud se guarda para continuarla al aceptar.
    pub fn consent(&self, request: &str) -> Outcome {
        Outcome::pending(
            Category::Files,
            "Pendiente de autorización",
            ResponseBody::FolderConsent {
                folders: self
                    .known_folders
                    .iter()
                    .map(|folder| folder.path.to_string_lossy().into_owned())
                    .collect(),
                pending_request: request.to_owned(),
            },
        )
    }

    /// Carpetas autorizadas, con su nombre para mostrar. Vacío si aún no hay ninguna.
    pub fn allowed(&self) -> Result<Vec<AuthorizedFolder>, Box<Outcome>> {
        let settings = self.settings.get().map_err(|error| {
            Outcome::failed(Category::Files, "settings", error.to_string(), None)
        })?;
        Ok(authorized_folders(
            &settings.authorized_folders,
            &self.known_folders,
        ))
    }
}

fn describe_query(query: &FileQuery) -> String {
    query
        .exact_name
        .clone()
        .unwrap_or_else(|| query.terms.join(", "))
}

impl FilesHandler {
    async fn answer(&self, query: &FileQuery, context: &HandlerContext) -> Outcome {
        let allowed = match self.allowed() {
            Ok(allowed) if allowed.is_empty() => return self.consent(&context.request),
            Ok(allowed) => allowed,
            Err(outcome) => return *outcome,
        };
        let outcome = self
            .search
            .search(query.clone(), allowed.clone(), context.cancel.clone())
            .await;
        let places = crate::core::format::list(
            &allowed
                .iter()
                .map(|folder| folder.label.as_str())
                .collect::<Vec<_>>(),
        );
        if outcome.files.is_empty() && context.cancel.is_cancelled() {
            return Outcome::failed(Category::Files, "cancelled", "Búsqueda cancelada.", None);
        }
        if outcome.files.is_empty() {
            let text = format!("No encontré archivos con «{}» en {places}. Busco por nombre y, en los archivos de texto, también por contenido.", describe_query(query));
            return Outcome::done(
                Category::Files,
                "Sin coincidencias",
                ResponseBody::Text { text },
            );
        }
        let count = outcome.files.len();
        let summary = format!(
            "{} en {places}",
            crate::core::format::count(count, "coincidencia", "coincidencias")
        );
        let note = outcome.truncated.then(|| {
            "Hay más coincidencias o carpetas muy grandes. Prueba con términos más concretos."
                .to_owned()
        });
        Outcome::done(
            Category::Files,
            summary.clone(),
            ResponseBody::Files {
                summary,
                files: outcome.files,
                note,
            },
        )
    }
}

#[async_trait]
impl CommandHandler for FilesHandler {
    async fn handle(&self, intent: &Intent, context: &HandlerContext) -> Option<Outcome> {
        let Intent::SearchFiles(query) = intent else {
            return None;
        };
        Some(self.answer(query, context).await)
    }
}
