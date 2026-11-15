//! Voz: grabar, transcribir en el equipo y procesar lo dicho, y descargar los modelos de Whisper.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime, State};

use super::{present_stage, stage_sink, validate_text, AppState, StagePayload};
use crate::{
    error::{AppError, AppResult},
    models::{AssistantResponse, Category, InteractionStatus, ResponseBody, Stage},
    providers::speech,
    services::performance::Timings,
};

#[tauri::command]
pub fn voice_start<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>) -> AppResult<()> {
    let settings = state.settings.get()?;
    let model = speech::find_model(&settings.speech_model)
        .ok_or_else(|| AppError::Validation("Ese modelo de voz no existe.".into()))?;
    state
        .ready_model(model)
        .map_err(|error| AppError::Operation(error.to_string()))?;
    let (level_app, limit_app) = (app.clone(), app.clone());
    let recording = speech::start_recording(
        settings.microphone.clone(),
        Arc::new(move |level| {
            let _ = level_app.emit("voice://level", level);
        }),
        Arc::new(move || {
            let _ = limit_app.emit("voice://limit", ());
        }),
    )
    .map_err(|error| AppError::Operation(error.to_string()))?;
    *state
        .recording
        .lock()
        .map_err(|_| AppError::Operation("El micrófono quedó bloqueado.".into()))? =
        Some(recording);
    // El modelo se carga mientras hablas: al terminar la grabación solo queda transcribir.
    let (speech, path) = (state.speech.clone(), state.models_dir.join(model.file));
    tauri::async_runtime::spawn_blocking(move || {
        let _ = speech.preload(&path);
    });
    let _ = app.emit(
        "assistant://stage",
        StagePayload {
            stage: Stage::Listening,
        },
    );
    Ok(())
}

#[tauri::command]
pub fn voice_cancel<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>) {
    // Soltar la grabación cierra el micrófono y descarta el audio.
    if let Ok(mut recording) = state.recording.lock() {
        recording.take();
    }
    let _ = app.emit("assistant://stage", StagePayload { stage: Stage::Idle });
}

/// Cómo se nombra una solicitud por voz que no llegó a transcribirse, igual en la conversación y en el historial.
pub const VOICE_REQUEST: &str = "Solicitud por voz";

#[tauri::command]
pub async fn voice_stop<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> AppResult<Option<AssistantResponse>> {
    let recording = state
        .recording
        .lock()
        .map_err(|_| AppError::Operation("El micrófono quedó bloqueado.".into()))?
        .take();
    let Some(recording) = recording else {
        return Ok(None);
    };
    let mut timings = Timings::new("voz");
    let stage = stage_sink(&app);
    stage(Stage::Transcribing);
    let device = recording.device().to_owned();
    let samples = tokio::task::spawn_blocking(move || recording.finish())
        .await
        .map_err(|_| AppError::Operation("La grabación se interrumpió.".into()))?;
    timings.mark("fin de grabación");
    let settings = state.settings.get()?;
    let model = speech::find_model(&settings.speech_model)
        .ok_or_else(|| AppError::Validation("Ese modelo de voz no existe.".into()))?;
    speech::check_audible(&samples, &device)
        .and_then(|()| state.ready_model(model))
        .map_err(|error| AppError::Operation(error.to_string()))?;
    let cancel = state.fresh_cancel();
    let text = state
        .speech
        .transcribe(samples, state.models_dir.join(model.file), cancel.clone())
        .await;
    let text = match text {
        Ok(transcript) => {
            timings.add("carga del modelo", transcript.model_load);
            timings.add("transcripción", transcript.transcription);
            transcript.text
        }
        Err(speech::SpeechError::Cancelled) => {
            present_stage(&app).await;
            return Ok(None);
        }
        Err(error) => return Err(AppError::Operation(error.to_string())),
    };
    if text.is_empty() {
        state
            .assistant
            .performance
            .record(&timings, "sin solicitud", "general");
        let (id, created_at) = state.history.record(
            VOICE_REQUEST,
            Category::General,
            InteractionStatus::Failed,
            "No se oyó ninguna solicitud",
        )?;
        present_stage(&app).await;
        return Ok(Some(AssistantResponse {
            id,
            request: VOICE_REQUEST.into(),
            created_at,
            category: Category::General,
            status: InteractionStatus::Failed,
            origin_label: "voz transcrita en este equipo".into(),
            summary: "No se oyó ninguna solicitud".into(),
            body: ResponseBody::Error {
                code: "noSpeech".into(),
                message: "No oí ninguna solicitud.".into(),
                hint: Some("Acércate al micrófono o escribe tu solicitud.".into()),
            },
        }));
    }
    // Lo transcrito cumple el mismo límite que lo escrito.
    let response = state
        .assistant
        .process_timed(validate_text(&text)?, cancel, stage, timings)
        .await;
    present_stage(&app).await;
    Ok(Some(response))
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadPayload {
    model_id: String,
    downloaded_mb: u64,
    total_mb: u64,
}

#[tauri::command]
pub async fn speech_model_download<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    model_id: String,
) -> AppResult<()> {
    let model = speech::find_model(&model_id)
        .ok_or_else(|| AppError::Validation("Ese modelo de voz no existe.".into()))?;
    {
        let mut downloading = state
            .downloading
            .lock()
            .map_err(|_| AppError::Operation("No se pudo iniciar la descarga.".into()))?;
        if downloading.is_some() {
            return Err(AppError::Operation(
                "Ya hay una descarga de modelo en curso.".into(),
            ));
        }
        *downloading = Some(model.id.to_owned());
    }
    let emitter = app.clone();
    let id = model.id.to_owned();
    let result = speech::download_model(
        model,
        &state.models_dir,
        move |downloaded, total| {
            let _ = emitter.emit(
                "speech://download",
                DownloadPayload {
                    model_id: id.clone(),
                    downloaded_mb: downloaded / 1_048_576,
                    total_mb: speech::megabytes(total),
                },
            );
        },
        None,
    )
    .await
    .map_err(AppError::Operation);
    if let Ok(mut downloading) = state.downloading.lock() {
        *downloading = None;
    }
    result
}
