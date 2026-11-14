//! Búsqueda de archivos en las carpetas autorizadas.

use std::sync::Arc;

use async_trait::async_trait;

use super::{CommandHandler, HandlerContext, Outcome};
use crate::{
    models::{Category, FileQuery, Intent, ResponseBody},
    providers::files::{AuthorizedFolder, FileSearchProvider},
    services::settings::SettingsService,
};

pub struct FilesHandler {
    pub search: Arc<dyn FileSearchProvider>,
    pub settings: SettingsService,
    pub known_folders: Vec<AuthorizedFolder>,
}

impl FilesHandler {
    fn folder_labels(&self) -> Vec<String> {
        self.known_folders
            .iter()
            .map(|folder| folder.path.to_string_lossy().into_owned())
            .collect()
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
        let settings = match self.settings.get() {
            Ok(settings) => settings,
            Err(error) => {
                return Outcome::failed(Category::Files, "settings", error.to_string(), None)
            }
        };
        if !settings.folders_confirmed {
            return Outcome::pending(
                Category::Files,
                "Pendiente de autorización",
                ResponseBody::FolderConsent {
                    folders: self.folder_labels(),
                    pending_request: context.request.clone(),
                },
            );
        }
        let allowed: Vec<AuthorizedFolder> = self
            .known_folders
            .iter()
            .filter(|folder| {
                settings
                    .authorized_folders
                    .iter()
                    .any(|path| *path == folder.path.to_string_lossy())
            })
            .cloned()
            .collect();
        if allowed.is_empty() {
            return Outcome::failed(
                Category::Files,
                "noFolders",
                "No hay carpetas autorizadas para buscar.",
                Some("Autorízalas en Ajustes.".into()),
            );
        }
        let outcome = self
            .search
            .search(query.clone(), allowed.clone(), context.cancel.clone())
            .await;
        let places = allowed
            .iter()
            .map(|folder| folder.label.as_str())
            .collect::<Vec<_>>()
            .join(" y ");
        if outcome.files.is_empty() && context.cancel.is_cancelled() {
            return Outcome::failed(Category::Files, "cancelled", "Búsqueda cancelada.", None);
        }
        if outcome.files.is_empty() {
            let text = format!("No encontré archivos con \u{201c}{}\u{201d} en {places}. Solo busco por nombre, sin abrir los archivos.", describe_query(query));
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
