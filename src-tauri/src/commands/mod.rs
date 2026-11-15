//! Capa IPC: valida la entrada y delega. No existe ningún comando genérico ni acceso a rutas o procesos.

pub mod documents;
pub mod folders;

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use chrono::Utc;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime, State};
use tokio_util::sync::CancellationToken;

use crate::{
    core::assistant::{Assistant, StageSink, MAX_REQUEST_LENGTH},
    error::{AppError, AppResult},
    models::{
        AssistantResponse, Category, HistoryEntry, InteractionStatus, Reminder, ResponseBody, Stage,
    },
    providers::{
        ai::{AiProvider, AiServiceStatus},
        documents::DocumentOffers,
        files::AuthorizedFolder,
        speech::{self, Recording, SpeechModelStatus, SpeechProvider},
        weather::ATTRIBUTION_URL,
    },
    services::{
        history::HistoryService,
        performance::Timings,
        reminders::ReminderService,
        settings::{Settings, SettingsService},
        usage::UsageService,
    },
};

pub struct AppState {
    pub assistant: Arc<Assistant>,
    pub ai: Arc<dyn AiProvider>,
    pub speech: Arc<dyn SpeechProvider>,
    pub settings: SettingsService,
    pub history: HistoryService,
    pub reminders: ReminderService,
    pub usage: UsageService,
    pub known_folders: Vec<AuthorizedFolder>,
    /// Archivos ofrecidos para resumir: la interfaz solo conoce su identificador.
    pub offers: Arc<DocumentOffers>,
    pub models_dir: PathBuf,
    pub cancel: Mutex<CancellationToken>,
    pub recording: Mutex<Option<Recording>>,
    /// Modelo que se está descargando: hasta que termine y se verifique, no se puede usar.
    pub downloading: Mutex<Option<String>>,
}

impl AppState {
    /// Un modelo solo se usa si está completo, verificado y no se está descargando.
    fn ready_model(&self, model: &speech::SpeechModel) -> Result<(), speech::SpeechError> {
        if self
            .downloading
            .lock()
            .is_ok_and(|current| current.as_deref() == Some(model.id))
        {
            return Err(speech::SpeechError::ModelDownloading);
        }
        match speech::model_state(model, &self.models_dir) {
            speech::ModelState::Ready => Ok(()),
            speech::ModelState::Damaged => Err(speech::SpeechError::ModelDamaged),
            speech::ModelState::Missing => Err(speech::SpeechError::ModelMissing),
        }
    }
}

impl AppState {
    pub(crate) fn fresh_cancel(&self) -> CancellationToken {
        let token = CancellationToken::new();
        if let Ok(mut current) = self.cancel.lock() {
            current.cancel();
            *current = token.clone();
        }
        token
    }
}

pub(crate) fn stage_sink<R: Runtime>(app: &AppHandle<R>) -> StageSink {
    let app = app.clone();
    Arc::new(move |stage: Stage| {
        let _ = app.emit("assistant://stage", StagePayload { stage });
    })
}

#[derive(Clone, Serialize)]
struct StagePayload {
    stage: Stage,
}

/// La solicitud terminó: la barra vuelve a En espera mientras la respuesta entra con su animación.
/// Sin esperas artificiales, que se sumarían a cada orden.
pub(crate) async fn present_stage<R: Runtime>(app: &AppHandle<R>) {
    let _ = app.emit("assistant://stage", StagePayload { stage: Stage::Idle });
}

fn validate_text(text: &str) -> AppResult<&str> {
    let text = text.trim();
    if text.is_empty() {
        return Err(AppError::Validation("La solicitud está vacía.".into()));
    }
    if text.chars().count() > MAX_REQUEST_LENGTH {
        return Err(AppError::Validation(
            "La solicitud es demasiado larga.".into(),
        ));
    }
    Ok(text)
}

#[tauri::command]
pub async fn assistant_submit<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    text: String,
) -> AppResult<AssistantResponse> {
    let text = validate_text(&text)?;
    let response = state
        .assistant
        .process(text, state.fresh_cancel(), stage_sink(&app))
        .await;
    present_stage(&app).await;
    Ok(response)
}

#[tauri::command]
pub fn assistant_cancel(state: State<'_, AppState>) {
    if let Ok(token) = state.cancel.lock() {
        token.cancel();
    }
}

/// Solo abre la dirección fija de la atribución: la interfaz no puede pedir otra.
#[tauri::command]
pub fn open_attribution() -> AppResult<()> {
    tauri_plugin_opener::open_url(ATTRIBUTION_URL, None::<&str>)
        .map_err(|_| AppError::Operation("No se pudo abrir el navegador.".into()))
}

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

#[tauri::command]
pub async fn screen_analyze<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    question: String,
) -> AppResult<AssistantResponse> {
    let question = validate_text(&question)?;
    let response = state
        .assistant
        .analyze_screen(question, state.fresh_cancel(), stage_sink(&app))
        .await;
    present_stage(&app).await;
    Ok(response)
}

#[tauri::command]
pub fn reminders_list(state: State<'_, AppState>) -> AppResult<Vec<Reminder>> {
    state.reminders.list()
}

#[tauri::command]
pub fn reminders_create<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    content: String,
    due_at: String,
    history_id: Option<i64>,
) -> AppResult<Reminder> {
    let reminder = state.reminders.create(&content, &due_at, Utc::now())?;
    if let Some(id) = history_id {
        state
            .history
            .complete(id, &format!("Guardado: {}", reminder.content))?;
    }
    let _ = app.emit("reminders://changed", ());
    Ok(reminder)
}

#[tauri::command]
pub fn reminders_update<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: i64,
    content: String,
    due_at: String,
) -> AppResult<Reminder> {
    let reminder = state.reminders.update(id, &content, &due_at, Utc::now())?;
    let _ = app.emit("reminders://changed", ());
    Ok(reminder)
}

#[tauri::command]
pub fn reminders_delete<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: i64,
) -> AppResult<()> {
    state.reminders.delete(id)?;
    let _ = app.emit("reminders://changed", ());
    Ok(())
}

#[tauri::command]
pub fn history_list(
    state: State<'_, AppState>,
    status: Option<InteractionStatus>,
) -> AppResult<Vec<HistoryEntry>> {
    state.history.list(status)
}

#[tauri::command]
pub fn history_delete(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    state.history.delete(id)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordedFailure {
    id: i64,
    created_at: String,
}

/// Registra un fallo que la interfaz muestra en la conversación pero que no llegó a pasar por el núcleo.
#[tauri::command]
pub fn history_record_failure(
    state: State<'_, AppState>,
    request: String,
    cause: String,
) -> AppResult<RecordedFailure> {
    let (id, created_at) = state.history.record_failure(&request, &cause)?;
    Ok(RecordedFailure { id, created_at })
}

#[tauri::command]
pub fn history_clear(state: State<'_, AppState>) -> AppResult<()> {
    state.history.clear()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    display_name: String,
    data_notice: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    settings: Settings,
    provider: ProviderInfo,
    ai_status: AiServiceStatus,
    ai_requests_today: u32,
    tokens_today: u64,
    known_folders: Vec<String>,
    speech_models: Vec<SpeechModelStatus>,
    microphones: Vec<speech::InputDevice>,
    /// El dispositivo que Nexium usará al pulsar Hablar con los ajustes guardados.
    active_microphone: Option<String>,
}

fn settings_view(state: &AppState) -> AppResult<SettingsView> {
    let settings = state.settings.get()?;
    let (ai_requests_today, tokens_today) = state.usage.today_totals()?;
    Ok(SettingsView {
        speech_models: speech::statuses(&state.models_dir, &settings.speech_model),
        microphones: speech::input_devices(),
        active_microphone: speech::active_input(settings.microphone.as_deref()),
        settings,
        provider: ProviderInfo {
            display_name: state.ai.display_name().into(),
            data_notice: state.ai.data_notice().into(),
        },
        ai_status: state.assistant.ai_status.current(state.ai.is_configured()),
        ai_requests_today,
        tokens_today,
        known_folders: state
            .known_folders
            .iter()
            .map(|folder| folder.path.to_string_lossy().into_owned())
            .collect(),
    })
}

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> AppResult<SettingsView> {
    settings_view(&state)
}

#[tauri::command]
pub fn settings_save(state: State<'_, AppState>, settings: Settings) -> AppResult<SettingsView> {
    settings.validate(&speech::catalog_ids())?;
    state.settings.save_editable(&settings)?;
    settings_view(&state)
}
