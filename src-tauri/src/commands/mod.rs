//! Capa IPC: valida la entrada y delega. No existe ningún comando genérico ni acceso a rutas o procesos.

pub mod documents;
pub mod folders;
pub mod records;
pub mod settings;
pub mod updates;
pub mod voice;
pub mod weather;

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime, State};
use tokio_util::sync::CancellationToken;

use crate::{
    core::assistant::{Assistant, Continues, StageSink, MAX_REQUEST_LENGTH},
    error::{AppError, AppResult},
    models::{AssistantResponse, Stage},
    providers::{
        ai::AiProvider,
        documents::DocumentOffers,
        files::AuthorizedFolder,
        speech::{self, Recording, SpeechProvider},
        weather::{WeatherProvider, ATTRIBUTION_URL},
    },
    services::{
        history::HistoryService, performance::Timings, reminders::ReminderService,
        settings::SettingsService, usage::UsageService,
    },
};

pub struct AppState {
    pub assistant: Arc<Assistant>,
    pub ai: Arc<dyn AiProvider>,
    pub speech: Arc<dyn SpeechProvider>,
    pub settings: SettingsService,
    pub history: HistoryService,
    pub reminders: ReminderService,
    /// Open-Meteo, también para comprobar la ciudad de Ajustes.
    pub weather: Arc<dyn WeatherProvider>,
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
    pub(crate) fn ready_model(
        &self,
        model: &speech::SpeechModel,
    ) -> Result<(), speech::SpeechError> {
        // Antes que nada: sin las instrucciones que necesita whisper.cpp, cargarlo cerraría Nexium.
        let missing = speech::missing_cpu_features();
        if !missing.is_empty() {
            return Err(speech::SpeechError::UnsupportedCpu(missing));
        }
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
pub(crate) struct StagePayload {
    pub(crate) stage: Stage,
}

/// La solicitud terminó: la barra vuelve a En espera mientras la respuesta entra con su animación.
/// Sin esperas artificiales, que se sumarían a cada orden.
pub(crate) async fn present_stage<R: Runtime>(app: &AppHandle<R>) {
    let _ = app.emit("assistant://stage", StagePayload { stage: Stage::Idle });
}

pub(crate) fn validate_text(text: &str) -> AppResult<&str> {
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
    continues: Option<i64>,
    part: Option<usize>,
) -> AppResult<AssistantResponse> {
    let text = validate_text(&text)?;
    let response = state
        .assistant
        .process_timed(
            text,
            Continues {
                id: continues,
                part,
            },
            state.fresh_cancel(),
            stage_sink(&app),
            Timings::new("texto"),
        )
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
pub async fn screen_analyze<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    question: String,
    continues: Option<i64>,
    part: Option<usize>,
) -> AppResult<AssistantResponse> {
    let question = validate_text(&question)?;
    let response = state
        .assistant
        .analyze_screen(
            question,
            Continues {
                id: continues,
                part,
            },
            state.fresh_cancel(),
            stage_sink(&app),
        )
        .await;
    present_stage(&app).await;
    Ok(response)
}
