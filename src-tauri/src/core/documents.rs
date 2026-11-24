//! Resumen y análisis de documentos, tras la confirmación expresa de la persona para ese archivo.
//!
//! El texto se extrae en el equipo, se recorta a `MAX_DOCUMENT_CHARS` y solo vive en esta función. El historial
//! guarda el nombre del archivo y la frase de resumen, nunca el contenido.

use std::path::Path;

use tokio_util::sync::CancellationToken;

use super::{
    assistant::{Assistant, StageSink},
    handlers::Outcome,
};
use crate::{
    error::AiError,
    models::{AssistantResponse, Category, DocumentTask, InteractionStatus, ResponseBody, Stage},
    providers::{
        ai::DocumentRequest,
        documents::{extension_of, extract},
    },
    services::performance::Timings,
};

/// Cómo se nombra la solicitud en la conversación y en el historial.
pub fn document_request(task: DocumentTask, file_name: &str) -> String {
    match task {
        DocumentTask::Summarize => format!("Resumen de «{file_name}»"),
        DocumentTask::Analyze => format!("Análisis de «{file_name}»"),
    }
}

impl Assistant {
    pub async fn analyze_document(
        &self,
        path: &Path,
        task: DocumentTask,
        continues: impl Into<super::assistant::Continues>,
        cancel: CancellationToken,
        stage: StageSink,
    ) -> AssistantResponse {
        let continues = continues.into();
        let mut timings = Timings::new("documento");
        let file_name = path
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        let request = document_request(task, &file_name);
        let provider = self.ai.display_name().to_owned();
        if let Err(error) = self.check_ai_available() {
            return self.finish_in(
                continues,
                &request,
                self.ai_failure(Category::Files, &error),
                "sin datos",
            );
        }
        stage(Stage::Reading);
        let owned = path.to_path_buf();
        let extracted = match tokio::task::spawn_blocking(move || extract(&owned)).await {
            Ok(Ok(extracted)) => extracted,
            Ok(Err(error)) => {
                return self.finish_in(
                    continues,
                    &request,
                    Outcome::failed(Category::Files, "document", error.to_string(), None),
                    "sin datos",
                )
            }
            Err(_) => {
                return self.finish_in(
                    continues,
                    &request,
                    Outcome::failed(
                        Category::Files,
                        "document",
                        "No pude leer el archivo.",
                        None,
                    ),
                    "sin datos",
                )
            }
        };
        timings.mark("lectura");
        stage(Stage::Summarizing);
        let answer = tokio::select! {
            answer = self.ai.analyze_document(DocumentRequest {
                task,
                format: extension_of(path),
                text: extracted.text,
            }) => answer,
            () = cancel.cancelled() => Err(AiError::Cancelled),
        };
        timings.mark("análisis");
        self.ai_status.record(&answer);
        self.performance.record(&timings, "IA", "files");
        let answer = match answer {
            Ok(answer) => answer,
            Err(AiError::Cancelled) => {
                return self.finish_in(
                    continues,
                    &request,
                    Outcome {
                        status: InteractionStatus::Cancelled,
                        ..Outcome::failed(
                            Category::Files,
                            "cancelled",
                            "Operación cancelada.",
                            None,
                        )
                    },
                    "sin datos",
                )
            }
            Err(error) => {
                return self.finish_in(
                    continues,
                    &request,
                    self.ai_failure(Category::Files, &error),
                    "sin datos",
                )
            }
        };
        let _ = self.usage.record(answer.tokens);
        stage(Stage::Responding);
        let summary = answer.value.summary.clone();
        self.finish_in(continues,
            &request,
            Outcome::done(
                Category::Files,
                summary,
                ResponseBody::DocumentAnalysis {
                    file_name,
                    task,
                    summary: answer.value.summary,
                    points: answer.value.points,
                    details: answer.value.details,
                    truncated: extracted.truncated,
                },
            ),
            &format!("documento leído en este equipo y resumido con IA ({provider}). Su contenido no se guarda"),
        )
    }
}
