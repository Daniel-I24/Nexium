//! Carpetas autorizadas. Las rutas solo llegan del selector de carpetas del sistema o de la lista fija de
//! Documentos y Escritorio: la interfaz nunca puede autorizar una ruta escrita por ella.

use tauri::{AppHandle, Runtime, State};
use tauri_plugin_dialog::DialogExt;

use super::AppState;
use crate::error::{AppError, AppResult};

/// "Permitir Documentos y Escritorio" del aviso de la conversación.
#[tauri::command]
pub fn folders_allow_known(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    let mut folders = state.settings.get()?.authorized_folders;
    for folder in &state.known_folders {
        // Una carpeta que no existe en este equipo (por ejemplo, sin Escritorio) se omite.
        if let Ok(list) = state.settings.add_folder(&folder.path) {
            folders = list;
        }
    }
    if folders.is_empty() {
        return Err(AppError::Operation(
            "No se encontraron las carpetas Documentos ni Escritorio. Elige otra carpeta.".into(),
        ));
    }
    Ok(folders)
}

/// Abre el selector de carpetas del sistema y autoriza la elegida. `None` si se cerró sin elegir.
#[tauri::command]
pub async fn folders_pick<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> AppResult<Option<Vec<String>>> {
    let picker = app
        .dialog()
        .file()
        .set_title("Elige una carpeta para que Nexium pueda buscar en ella");
    let chosen = tauri::async_runtime::spawn_blocking(move || picker.blocking_pick_folder())
        .await
        .map_err(|_| AppError::Operation("No se pudo abrir el selector de carpetas.".into()))?;
    let Some(chosen) = chosen else {
        return Ok(None);
    };
    let path = chosen
        .into_path()
        .map_err(|_| AppError::Validation("Esa carpeta no se puede usar.".into()))?;
    state.settings.add_folder(&path).map(Some)
}

/// Retira el permiso de una carpeta. No toca la carpeta ni su contenido.
#[tauri::command]
pub fn folders_remove(state: State<'_, AppState>, folder: String) -> AppResult<Vec<String>> {
    state.settings.remove_folder(&folder)
}
