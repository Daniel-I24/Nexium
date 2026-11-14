//! Lo que no consulta nada: pedir permiso de captura, aclarar una duda o decir que no se entendió.

use async_trait::async_trait;

use super::{CommandHandler, HandlerContext, Outcome};
use crate::models::{Category, Intent, InteractionStatus, ResponseBody, SuggestedRequest};

/// Cambiar de sección: lo hace la interfaz al recibir la respuesta.
pub struct NavigationHandler;

#[async_trait]
impl CommandHandler for NavigationHandler {
    async fn handle(&self, intent: &Intent, _: &HandlerContext) -> Option<Outcome> {
        let Intent::OpenSection(section) = intent else {
            return None;
        };
        Some(Outcome::done(
            Category::General,
            format!("Abrí {}", section.label()),
            ResponseBody::Navigate { section: *section },
        ))
    }
}

/// Solo pide permiso: la captura ocurre únicamente tras la confirmación expresa en la interfaz.
pub struct ScreenHandler;

#[async_trait]
impl CommandHandler for ScreenHandler {
    async fn handle(&self, intent: &Intent, _: &HandlerContext) -> Option<Outcome> {
        let Intent::AnalyzeScreen { question } = intent else {
            return None;
        };
        Some(Outcome::pending(
            Category::Screen,
            "Pendiente de tu permiso para capturar",
            ResponseBody::CaptureRequest {
                question: question.clone(),
            },
        ))
    }
}

pub struct ClarifyHandler;

#[async_trait]
impl CommandHandler for ClarifyHandler {
    async fn handle(&self, intent: &Intent, _: &HandlerContext) -> Option<Outcome> {
        match intent {
            Intent::Clarify { question, suggestions } => {
                let suggestions = suggestions.iter().map(|request| SuggestedRequest { label: request.clone(), request: request.clone() }).collect();
                Some(Outcome::pending(Category::General, question.clone(), ResponseBody::Clarification { question: question.clone(), suggestions }))
            }
            Intent::Unknown => Some(Outcome {
                status: InteractionStatus::Failed,
                ..Outcome::done(
                    Category::General,
                    "No entendí la solicitud",
                    ResponseBody::Text { text: "No entendí tu solicitud. Puedo decirte la hora, el clima o el estado de tu equipo, buscar archivos, crear recordatorios y analizar tu pantalla.".into() },
                )
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::handlers::{test_support::now, TimeHandler};
    use tokio_util::sync::CancellationToken;

    #[tokio::test]
    async fn a_handler_declines_the_intents_of_others() {
        let context = HandlerContext {
            request: "x".into(),
            now: now(),
            cancel: CancellationToken::new(),
        };
        assert!(TimeHandler
            .handle(&Intent::Unknown, &context)
            .await
            .is_none());
        assert!(ScreenHandler
            .handle(&Intent::Time, &context)
            .await
            .is_none());
        assert!(ClarifyHandler
            .handle(&Intent::ListReminders, &context)
            .await
            .is_none());
        assert!(ClarifyHandler
            .handle(&Intent::Unknown, &context)
            .await
            .is_some());
    }
}
