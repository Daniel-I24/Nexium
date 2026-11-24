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
        Action, AssistantResponse, Category, FileQuery, Intent, InteractionStatus, PartResponse,
        PartSummary, ResponseBody, Stage,
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

impl Resolved {
    /// Cómo se resolvió, para `rendimiento.log`.
    fn label(&self) -> &'static str {
        match self {
            Self::Ai(_) => "IA",
            Self::Failed(_) => "sin resolver",
            _ => "local",
        }
    }
}

/// Qué entrada del historial continúa una operación y, si pertenece a una petición con varias solicitudes,
/// qué parte. Sin entrada, la operación crea la suya.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Continues {
    pub id: Option<i64>,
    pub part: Option<usize>,
}

impl From<Option<i64>> for Continues {
    fn from(id: Option<i64>) -> Self {
        Self { id, part: None }
    }
}

/// Una petición puede pedir una cosa o varias.
enum Resolution {
    Single(Resolved),
    /// Cada parte con su fragmento de la petición. `extra`: solicitudes que pasaban del máximo.
    Multiple {
        parts: Vec<(String, Resolved)>,
        extra: usize,
    },
}

enum Resolved {
    Local(Intent),
    Ai(Intent),
    /// La IA no estaba disponible, pero la búsqueda de archivos puede seguir con reglas locales.
    Degraded(Intent, String),
    Failed(Outcome),
}

impl Assistant {
    /// Una sola solicitud o varias. Las reglas locales dividen solo si reconocen cada parte; si alguna necesita
    /// la IA, la petición va entera a la IA, que devuelve sus acciones. Continuar una parte nunca la divide.
    async fn resolve(
        &self,
        text: &str,
        continuing_part: bool,
        cancel: &CancellationToken,
        stage: &StageSink,
    ) -> Resolution {
        let compound = if continuing_part {
            rules::Compound::Single
        } else {
            rules::split_compound(text)
        };
        match compound {
            rules::Compound::Local { actions, extra } => {
                return Resolution::Multiple {
                    parts: actions
                        .into_iter()
                        .map(|action| (action.request, Resolved::Local(action.intent)))
                        .collect(),
                    extra,
                }
            }
            rules::Compound::Single => {
                if let Some(intent) = rules::match_local(text) {
                    return Resolution::Single(Resolved::Local(intent));
                }
            }
            rules::Compound::NeedsAi => {}
        }
        match self.interpret(text, cancel, stage).await {
            Ok(mut actions) if actions.len() == 1 && !continuing_part => {
                Resolution::Single(Resolved::Ai(actions.remove(0).intent))
            }
            Ok(actions) if continuing_part => Resolution::Single(
                actions
                    .into_iter()
                    .next()
                    .map_or(Resolved::Ai(Intent::Unknown), |action| {
                        Resolved::Ai(action.intent)
                    }),
            ),
            Ok(actions) => Resolution::Multiple {
                parts: actions
                    .into_iter()
                    .map(|action| (action.request, Resolved::Ai(action.intent)))
                    .collect(),
                extra: 0,
            },
            Err(resolved) => Resolution::Single(*resolved),
        }
    }

    /// La IA interpreta la petición. Una fecha de recordatorio que admite dos lecturas se pregunta siempre,
    /// elija lo que elija el modelo.
    async fn interpret(
        &self,
        text: &str,
        cancel: &CancellationToken,
        stage: &StageSink,
    ) -> Result<Vec<Action>, Box<Resolved>> {
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
                Ok(answer
                    .value
                    .into_iter()
                    .map(|action| {
                        // La fecha que elija el modelo no cuenta: si admite dos lecturas, se pregunta.
                        let doubt = matches!(action.intent, Intent::CreateReminder(_))
                            .then(|| rules::ambiguous_date_question(&action.request))
                            .flatten();
                        Action {
                            intent: doubt.unwrap_or(action.intent),
                            request: action.request,
                        }
                    })
                    .collect())
            }
            Err(AiError::Cancelled) => Err(Box::new(Resolved::Failed(Outcome {
                status: InteractionStatus::Cancelled,
                ..Outcome::failed(Category::General, "cancelled", "Operación cancelada.", None)
            }))),
            Err(error) if rules::is_search_request(text) => {
                let terms = rules::local_search_terms(text);
                if terms.is_empty() {
                    return Err(Box::new(Resolved::Failed(
                        self.ai_failure(Category::Files, &error),
                    )));
                }
                let (message, _) = self.describe_ai_error(&error);
                Err(Box::new(Resolved::Degraded(
                    Intent::SearchFiles(FileQuery {
                        exact_name: None,
                        terms,
                    }),
                    format!("Búsqueda básica sin IA. {message}"),
                )))
            }
            Err(error) => Err(Box::new(Resolved::Failed(
                self.ai_failure(Category::General, &error),
            ))),
        }
    }

    /// Guarda el resultado en el historial y lo devuelve. Si continúa una solicitud anterior, actualiza su
    /// entrada: una solicitud tiene una sola entrada, que llega a su estado final, y la respuesta conserva su
    /// texto original y su identificador. Si continúa una parte de una petición con varias, solo cambia esa
    /// parte, y la respuesta lo indica para que la interfaz actualice solo su bloque.
    pub(crate) fn finish_in(
        &self,
        continues: impl Into<Continues>,
        request: &str,
        outcome: Outcome,
        origin: &str,
    ) -> AssistantResponse {
        let continues = continues.into();
        let origin_label = origin_label(outcome.source.as_deref(), origin);
        if let (Some(id), Some(part)) = (continues.id.filter(|id| *id > 0), continues.part) {
            let summary = PartSummary {
                request: request.to_owned(),
                category: outcome.category,
                status: outcome.status,
                summary: outcome.summary.clone(),
            };
            if let Ok(Some((_, created_at))) = self.history.update_part(id, part, summary) {
                return AssistantResponse {
                    id,
                    request: request.to_owned(),
                    created_at,
                    category: outcome.category,
                    status: outcome.status,
                    origin_label,
                    summary: outcome.summary,
                    body: outcome.body,
                    part: Some(part),
                };
            }
        }
        let continued = continues.id.filter(|id| *id > 0).and_then(|id| {
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
        AssistantResponse {
            id,
            request,
            created_at,
            category: outcome.category,
            status: outcome.status,
            origin_label,
            summary: outcome.summary,
            body: outcome.body,
            part: None,
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
    /// `continues`: la entrada del historial (y la parte, si la hay) que esta solicitud completa, por ejemplo
    /// tras un permiso o una aclaración. Se actualiza esa entrada en lugar de crear otra.
    pub async fn process_timed(
        &self,
        text: &str,
        continues: impl Into<Continues>,
        cancel: CancellationToken,
        stage: StageSink,
        mut timings: Timings,
    ) -> AssistantResponse {
        let continues = continues.into();
        let text = text.trim();
        let resolution = self
            .resolve(text, continues.part.is_some(), &cancel, &stage)
            .await;
        timings.mark("interpretación");
        let (response, resolved_by) = match resolution {
            Resolution::Single(resolved) => {
                let resolved_by = resolved.label();
                let (outcome, origin) = self.run(text, resolved, &cancel, &stage).await;
                stage(Stage::Responding);
                (
                    self.finish_in(continues, text, outcome, &origin),
                    resolved_by,
                )
            }
            Resolution::Multiple { parts, extra } => {
                let resolved_by = parts
                    .first()
                    .map_or("local", |(_, resolved)| resolved.label());
                (
                    self.run_parts(text, continues, parts, extra, &cancel, &stage)
                        .await,
                    resolved_by,
                )
            }
        };
        timings.mark("respuesta");
        self.performance
            .record(&timings, resolved_by, response.category.as_str());
        response
    }

    /// Cada parte en el orden pedido. Si una falla o espera una confirmación, las demás se responden igual.
    async fn run_parts(
        &self,
        text: &str,
        continues: Continues,
        parts: Vec<(String, Resolved)>,
        extra: usize,
        cancel: &CancellationToken,
        stage: &StageSink,
    ) -> AssistantResponse {
        let mut responses = Vec::with_capacity(parts.len() + 1);
        for (request, resolved) in parts {
            let (outcome, origin) = self.run(&request, resolved, cancel, stage).await;
            responses.push(PartResponse {
                origin_label: origin_label(outcome.source.as_deref(), &origin),
                request,
                category: outcome.category,
                status: outcome.status,
                summary: outcome.summary,
                body: outcome.body,
            });
        }
        if extra > 0 {
            let outcome = Outcome::failed(
                Category::General,
                "tooManyRequests",
                format!(
                    "Atiendo hasta {} solicitudes a la vez, y había {} más.",
                    rules::MAX_PARTS,
                    extra
                ),
                Some("Pide el resto aparte.".into()),
            );
            responses.push(PartResponse {
                request: "El resto de la petición".into(),
                origin_label: "sin datos".into(),
                category: outcome.category,
                status: outcome.status,
                summary: outcome.summary,
                body: outcome.body,
            });
        }
        stage(Stage::Responding);
        let summaries: Vec<PartSummary> = responses
            .iter()
            .map(|part| PartSummary {
                request: part.request.clone(),
                category: part.category,
                status: part.status,
                summary: part.summary.clone(),
            })
            .collect();
        let (category, status, summary) = crate::services::history::combine_parts(&summaries);
        let continued = continues.id.filter(|id| *id > 0).and_then(|id| {
            self.history
                .set_parts(id, &summaries)
                .ok()
                .flatten()
                .map(|(request, created_at)| (id, request, created_at))
        });
        let (id, request, created_at) = continued.unwrap_or_else(|| {
            let (id, created_at) = self
                .history
                .record_parts(text, &summaries)
                .unwrap_or_else(|_| (0, Local::now().to_rfc3339()));
            (id, text.to_owned(), created_at)
        });
        AssistantResponse {
            id,
            request,
            created_at,
            category,
            status,
            origin_label: "cada solicitud indica el suyo".into(),
            summary,
            body: ResponseBody::Multiple { parts: responses },
            part: None,
        }
    }

    /// Ejecuta una intención con su handler y dice de dónde salió. No guarda nada en el historial.
    async fn run(
        &self,
        text: &str,
        resolved: Resolved,
        cancel: &CancellationToken,
        stage: &StageSink,
    ) -> (Outcome, String) {
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
            Resolved::Failed(outcome) => return (outcome, "sin datos".into()),
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
            return (
                Outcome::failed(
                    Category::General,
                    "noHandler",
                    "No sé hacer esa operación.",
                    None,
                ),
                "sin datos".into(),
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
        (outcome, origin)
    }

    /// Una captura por confirmación expresa del usuario. La imagen solo vive en esta función.
    pub async fn analyze_screen(
        &self,
        question: &str,
        continues: impl Into<Continues>,
        cancel: CancellationToken,
        stage: StageSink,
    ) -> AssistantResponse {
        let continues = continues.into();
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
