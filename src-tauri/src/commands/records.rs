//! Recordatorios e historial guardados en el equipo.

use chrono::Utc;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime, State};

use super::AppState;
use crate::{
    error::AppResult,
    models::{HistoryEntry, InteractionStatus, Reminder},
};

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

/// «Cancelar» en un permiso o una aclaración de la conversación: la solicitud termina como cancelada,
/// en su misma entrada.
#[tauri::command]
pub fn history_cancel(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    state.history.update(
        id,
        crate::models::Category::General,
        InteractionStatus::Cancelled,
        "Cancelada",
    )?;
    Ok(())
}

#[tauri::command]
pub fn history_clear(state: State<'_, AppState>) -> AppResult<()> {
    state.history.clear()
}
