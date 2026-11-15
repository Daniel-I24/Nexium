//! Documentos: se resumen solo tras confirmar un archivo concreto, identificado por el `token` que Nexium
//! entregó al ofrecerlo. La interfaz nunca envía una ruta.

use tauri::{AppHandle, Runtime, State};
use tauri_plugin_dialog::DialogExt;

use super::{present_stage, stage_sink, AppState};
use crate::{
    core::handlers::candidate,
    error::{AppError, AppResult},
    models::{AssistantResponse, DocumentCandidate, DocumentTask},
    providers::documents::SUPPORTED_EXTENSIONS,
};

/// Lee el archivo confirmado y lo envía a resumir o analizar.
#[tauri::command]
pub async fn document_analyze<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    token: String,
    task: DocumentTask,
) -> AppResult<AssistantResponse> {
    let path = state.offers.path(&token).ok_or_else(|| {
        AppError::Validation("Ese archivo ya no está disponible. Vuelve a pedirlo.".into())
    })?;
    let response = state
        .assistant
        .analyze_document(&path, task, state.fresh_cancel(), stage_sink(&app))
        .await;
    present_stage(&app).await;
    Ok(response)
}

/// Abre el selector de archivos del sistema, solo con los formatos que Nexium sabe leer.
/// `None` si se cerró sin elegir. Elegir un archivo no envía nada: falta confirmarlo.
#[tauri::command]
pub async fn document_pick<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> AppResult<Option<DocumentCandidate>> {
    let picker = app
        .dialog()
        .file()
        .set_title("Elige el documento")
        .add_filter("Documentos", SUPPORTED_EXTENSIONS);
    let chosen = tauri::async_runtime::spawn_blocking(move || picker.blocking_pick_file())
        .await
        .map_err(|_| AppError::Operation("No se pudo abrir el selector de archivos.".into()))?;
    let Some(chosen) = chosen else {
        return Ok(None);
    };
    let path = chosen
        .into_path()
        .map_err(|_| AppError::Validation("Ese archivo no se puede usar.".into()))?;
    let location = path
        .parent()
        .and_then(|parent| parent.file_name())
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    Ok(Some(candidate(&state.offers, &path, &location)))
}
