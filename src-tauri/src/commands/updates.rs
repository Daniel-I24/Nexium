//! Actualizaciones desde GitHub Releases, firmadas: el actualizador solo instala lo que firma la clave de Nexium.
//! Nexium no comprueba nada por su cuenta: solo cuando la persona lo pide en Ajustes.

use serde::Serialize;
use tauri::{AppHandle, Runtime};
use tauri_plugin_updater::UpdaterExt;

use crate::error::{AppError, AppResult};

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    /// La versión instalada.
    pub current: String,
    /// La versión nueva, si la hay.
    pub available: Option<String>,
}

fn unreachable() -> AppError {
    AppError::Operation(
        "No se pudo comprobar si hay una versión nueva. Revisa tu conexión a Internet.".into(),
    )
}

#[tauri::command]
pub async fn update_check<R: Runtime>(app: AppHandle<R>) -> AppResult<UpdateStatus> {
    let update = app
        .updater()
        .map_err(|_| unreachable())?
        .check()
        .await
        .map_err(|_| unreachable())?;
    Ok(UpdateStatus {
        current: app.package_info().version.to_string(),
        available: update.map(|update| update.version),
    })
}

/// Descarga, comprueba la firma, instala y reinicia Nexium.
#[tauri::command]
pub async fn update_install<R: Runtime>(app: AppHandle<R>) -> AppResult<()> {
    let update = app
        .updater()
        .map_err(|_| unreachable())?
        .check()
        .await
        .map_err(|_| unreachable())?
        .ok_or_else(|| AppError::Validation("Ya tienes la última versión.".into()))?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|_| {
            AppError::Operation(
                "No se pudo instalar la versión nueva. Vuelve a intentarlo más tarde.".into(),
            )
        })?;
    app.restart()
}
