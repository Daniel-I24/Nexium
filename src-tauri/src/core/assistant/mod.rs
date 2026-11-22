//! Núcleo: resuelve la intención, ejecuta su handler y registra la interacción.

use std::sync::Arc;

use chrono::Local;

use tokio_util::sync::CancellationToken;

use super::{
    handlers::{CommandHandler, HandlerContext, Outcome, ORIGIN_LOCAL},
    rules,
};
use crate::{
    error::AiError,
    models::{
        AssistantResponse, Category, FileQuery, Intent, InteractionStatus, ResponseBody, Stage,
    },
    providers::{
        ai::{AiProvider, AiStatusTracker, InterpretRequest, VisionRequest},
        capture::{CaptureError, ScreenCaptureProvider},
    },
    services::{
        history::HistoryService,
        performance::{PerformanceLog, Timings},
        settings::SettingsService,
        usage::UsageService,
    },
};

mod ai_errors;
#[cfg(test)]
use ai_errors::reset_sentence;

pub const MAX_REQUEST_LENGTH: usize = 500;

pub type StageSink = Arc<dyn Fn(Stage) + Send + Sync>;

pub struct Assistant {
    pub ai: Arc<dyn AiProvider>,
    pub ai_status: AiStatusTracker,
    pub capture: Arc<dyn ScreenCaptureProvider>,
    pub handlers: Vec<Box<dyn CommandHandler>>,
    pub settings: SettingsService,
    pub history: HistoryService,
    pub usage: UsageService,
    /// Tiempos de cada etapa (`rendimiento.log`). Sin ruta, no escribe nada.
    pub performance: PerformanceLog,
}

enum Resolved {
    Local(Intent),
    Ai(Intent),
    /// La IA no estaba disponible, pero la búsqueda de archivos puede seguir con reglas locales.
    Degraded(Intent, String),
    Failed(Outcome),
}

impl Assistant {
    async fn resolve(&self, text: &str, cancel: &CancellationToken, stage: &StageSink) -> Resolved {
        if let Some(intent) = rules::match_local(text) {
            return Resolved::Local(intent);
        }
        let attempt = match self.check_ai_available() {
            Err(error) => Err(error),
            Ok(()) => {
                stage(Stage::Interpreting);
                let request = InterpretRequest {
                    text: text.to_owned(),
                    now: Local::now(),
                };
                tokio::select! {
                    answer = self.ai.interpret(request) => answer,
                    () = cancel.cancelled() => Err(AiError::Cancelled),
                }
            }
        };
        self.ai_status.record(&attempt);
        match attempt {
            Ok(answer) => {
                let _ = self.usage.record(answer.tokens);
                // La fecha que elija el modelo no cuenta: si admite dos lecturas, se pregunta.
                let doubt = matches!(answer.value, Intent::CreateReminder(_))
                    .then(|| rules::ambiguous_date_question(text))
                    .flatten();
                Resolved::Ai(doubt.unwrap_or(answer.value))
            }
            Err(AiError::Cancelled) => Resolved::Failed(Outcome {
                status: InteractionStatus::Cancelled,
                ..Outcome::failed(Category::General, "cancelled", "Operación cancelada.", None)
            }),
            Err(error) if rules::is_search_request(text) => {
                let terms = rules::local_search_terms(text);
                if terms.is_empty() {
                    return Resolved::Failed(self.ai_failure(Category::Files, &error));
                }
                let (message, _) = self.describe_ai_error(&error);
                Resolved::Degraded(
                    Intent::SearchFiles(FileQuery {
                        exact_name: None,
                        terms,
                    }),
                    format!("Búsqueda básica sin IA. {message}"),
                )
            }
            Err(error) => Resolved::Failed(self.ai_failure(Category::General, &error)),
        }
    }

    /// Guarda el resultado en el historial y lo devuelve. Si continúa una solicitud anterior (`continues`),
    /// actualiza su entrada: una solicitud tiene una sola entrada, que llega a su estado final, y la respuesta
    /// conserva su texto original y su identificador.
    pub(crate) fn finish_in(
        &self,
        continues: Option<i64>,
        request: &str,
        outcome: Outcome,
        origin: &str,
    ) -> AssistantResponse {
        let continued = continues.filter(|id| *id > 0).and_then(|id| {
            self.history
                .update(id, outcome.category, outcome.status, &outcome.summary)
                .ok()
                .flatten()
                .map(|(request, created_at)| (id, request, created_at))
        });
        let (id, request, created_at) = continued.unwrap_or_else(|| {
            let (id, created_at) = self
                .history
                .record(request, outcome.category, outcome.status, &outcome.summary)
                .unwrap_or_else(|_| (0, Local::now().to_rfc3339()));
            (id, request.to_owned(), created_at)
        });
        let origin_label = origin_label(outcome.source.as_deref(), origin);
        AssistantResponse {
            id,
            request,
            created_at,
            category: outcome.category,
            status: outcome.status,
            origin_label,
            summary: outcome.summary,
            body: outcome.body,
        }
    }

    pub async fn process(
        &self,
        text: &str,
        cancel: CancellationToken,
        stage: StageSink,
    ) -> AssistantResponse {
        self.process_timed(text, None, cancel, stage, Timings::new("texto"))
            .await
    }

    /// Como `process`, continuando la medición de una solicitud que empezó antes (por ejemplo, por voz).
    /// `continues`: la entrada del historial de la solicitud que esta completa (un permiso, una aclaración).
    /// Se actualiza esa entrada en lugar de crear otra.
    pub async fn process_timed(
        &self,
        text: &str,
        continues: Option<i64>,
        cancel: CancellationToken,
        stage: StageSink,
        mut timings: Timings,
    ) -> AssistantResponse {
        let resolved = self.resolve(text.trim(), &cancel, &stage).await;
        timings.mark("interpretación");
        let resolved_by = match &resolved {
            Resolved::Ai(_) => "IA",
            Resolved::Failed(_) => "sin resolver",
            _ => "local",
        };
        let response = self
            .execute(text.trim(), continues, resolved, cancel, stage)
            .await;
        timings.mark("respuesta");
        self.performance
            .record(&timings, resolved_by, response.category.as_str());
        response
    }

    async fn execute(
        &self,
        text: &str,
        continues: Option<i64>,
        resolved: Resolved,
        cancel: CancellationToken,
        stage: StageSink,
    ) -> AssistantResponse {
        let provider = self.ai.display_name().to_owned();
        let (intent, origin, note) = match resolved {
            Resolved::Local(intent @ Intent::Clarify { .. }) => {
                (intent, "reglas locales, sin IA".to_owned(), None)
            }
            Resolved::Local(intent) => (intent, ORIGIN_LOCAL.to_owned(), None),
            Resolved::Ai(intent) => (
                intent,
                format!("solicitud interpretada con IA ({provider})"),
                None,
            ),
            Resolved::Degraded(intent, note) => {
                (intent, "reglas locales, sin IA".to_owned(), Some(note))
            }
            Resolved::Failed(outcome) => {
                return self.finish_in(continues, text, outcome, "sin datos")
            }
        };
        stage(Stage::Executing);
        let context = HandlerContext {
            request: text.to_owned(),
            now: Local::now(),
            cancel: cancel.clone(),
        };
        let mut handled = None;
        for handler in &self.handlers {
            handled = handler.handle(&intent, &context).await;
            if handled.is_some() {
                break;
            }
        }
        let Some(mut outcome) = handled else {
            return self.finish_in(
                continues,
                text,
                Outcome::failed(
                    Category::General,
                    "noHandler",
                    "No sé hacer esa operación.",
                    None,
                ),
                "sin datos",
            );
        };
        if cancel.is_cancelled() {
            outcome = Outcome {
                status: InteractionStatus::Cancelled,
                ..Outcome::failed(outcome.category, "cancelled", "Operación cancelada.", None)
            };
        }
        if let (
            Some(extra),
            ResponseBody::Files {
                note: file_note, ..
            },
        ) = (note, &mut outcome.body)
        {
            *file_note = Some(extra);
        }
        stage(Stage::Responding);
        self.finish_in(continues, text, outcome, &origin)
    }

    /// Una captura por confirmación expresa del usuario. La imagen solo vive en esta función.
    pub async fn analyze_screen(
        &self,
        question: &str,
        continues: Option<i64>,
        cancel: CancellationToken,
        stage: StageSink,
    ) -> AssistantResponse {
        let question = question.trim();
        let provider = self.ai.display_name().to_owned();
        if let Err(error) = self.check_ai_available() {
            return self.finish_in(
                continues,
                question,
                self.ai_failure(Category::Screen, &error),
                "sin datos",
            );
        }
        stage(Stage::Capturing);
        let image = match self.capture.capture_once().await {
            Ok(image) => image,
            Err(CaptureError::PermissionDenied(message) | CaptureError::Failed(message)) => {
                return self.finish_in(
                    continues,
                    question,
                    Outcome::failed(Category::Screen, "capture", message, None),
                    "sin datos",
                )
            }
        };
        if cancel.is_cancelled() {
            return self.finish_in(
                continues,
                question,
                Outcome {
                    status: InteractionStatus::Cancelled,
                    ..Outcome::failed(Category::Screen, "cancelled", "Operación cancelada.", None)
                },
                "sin datos",
            );
        }
        stage(Stage::Analyzing);
        let request = VisionRequest {
            image_jpeg: image,
            question: question.to_owned(),
        };
        let answer = tokio::select! {
            answer = self.ai.analyze_image(request) => answer,
            () = cancel.cancelled() => Err(AiError::Cancelled),
        };
        self.ai_status.record(&answer);
        match answer {
            Ok(answer) => {
                let _ = self.usage.record(answer.tokens);
                stage(Stage::Responding);
                let analysis = answer.value;
                let body = ResponseBody::ScreenAnalysis {
                    summary: analysis.summary.clone(),
                    cause: analysis.cause,
                    steps: analysis.steps,
                };
                self.finish_in(
                    continues,
                    question,
                    Outcome::done(Category::Screen, analysis.summary, body),
                    &format!(
                        "una captura analizada con IA ({provider}). Nexium ya descartó la imagen"
                    ),
                )
            }
            Err(AiError::InvalidResponse) => {
                let outcome = Outcome::failed(
                    Category::Screen,
                    AiError::InvalidResponse.code(),
                    "La IA devolvió un análisis con un formato que Nexium no puede mostrar.",
                    Some("Vuelve a intentarlo. La captura ya se descartó.".into()),
                );
                self.finish_in(continues, question, outcome, "sin datos")
            }
            Err(AiError::Cancelled) => self.finish_in(
                continues,
                question,
                Outcome {
                    status: InteractionStatus::Cancelled,
                    ..Outcome::failed(Category::Screen, "cancelled", "Operación cancelada.", None)
                },
                "sin datos",
            ),
            Err(error) => self.finish_in(
                continues,
                question,
                self.ai_failure(Category::Screen, &error),
                "sin datos",
            ),
        }
    }
}

/// Un dato externo no es una lectura del equipo: se nombra su fuente.
fn origin_label(source: Option<&str>, origin: &str) -> String {
    match source {
        Some(source) if origin == ORIGIN_LOCAL => format!("{source}, sin IA"),
        Some(source) => format!("{source}; {origin}"),
        None => origin.to_owned(),
    }
}

#[cfg(test)]
mod tests;
