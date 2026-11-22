//! Actualizaciones desde GitHub Releases, firmadas: el actualizador solo instala lo que firma la clave de Nexium.
//! Nexium no comprueba nada por su cuenta: solo cuando la persona lo pide en Ajustes.

use serde::Serialize;
use tauri::{AppHandle, Runtime};
use tauri_plugin_updater::{Error as UpdaterError, UpdaterExt};

use crate::error::{AppError, AppResult};

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    /// La versión instalada.
    pub current: String,
    /// Si hay alguna versión publicada. Antes de la primera publicación no la hay, y no es un error.
    pub published: bool,
    /// La versión nueva, si la hay.
    pub available: Option<String>,
}

const OFFLINE: &str =
    "No hay conexión con GitHub, donde se publican las versiones. Revisa tu conexión a Internet.";

/// Por qué falló la búsqueda. `None`: no hay ninguna versión publicada, lo que no es un fallo.
fn check_failure(error: &UpdaterError) -> Option<AppError> {
    match error {
        UpdaterError::ReleaseNotFound => None,
        UpdaterError::Reqwest(_) | UpdaterError::Network(_) => {
            Some(AppError::Operation(OFFLINE.into()))
        }
        _ => Some(AppError::Operation(
            "La versión publicada no tiene un formato válido para este equipo. Vuelve a intentarlo más tarde."
                .into(),
        )),
    }
}

fn install_failure(error: &UpdaterError) -> AppError {
    AppError::Operation(match error {
        UpdaterError::Minisign(_)
        | UpdaterError::Base64(_)
        | UpdaterError::SignatureUtf8(_)
        | UpdaterError::SignedVersionMismatch { .. }
        | UpdaterError::MissingSignedVersion => {
            "La firma de la versión descargada no es válida, así que no se instaló. Tu versión actual sigue igual."
                .into()
        }
        UpdaterError::Reqwest(_) | UpdaterError::Network(_) => {
            "Se cortó la conexión durante la descarga. Tu versión actual sigue igual; vuelve a intentarlo.".into()
        }
        _ => "No se pudo instalar la versión nueva. Tu versión actual sigue igual; vuelve a intentarlo más tarde."
            .into(),
    })
}

#[tauri::command]
pub async fn update_check<R: Runtime>(app: AppHandle<R>) -> AppResult<UpdateStatus> {
    let current = app.package_info().version.to_string();
    let checked = app
        .updater()
        .map_err(|error| {
            check_failure(&error).unwrap_or_else(|| AppError::Operation(OFFLINE.into()))
        })?
        .check()
        .await;
    match checked {
        Ok(update) => Ok(UpdateStatus {
            current,
            published: true,
            available: update.map(|update| update.version),
        }),
        Err(error) => match check_failure(&error) {
            None => Ok(UpdateStatus {
                current,
                published: false,
                available: None,
            }),
            Some(failure) => Err(failure),
        },
    }
}

/// Descarga, comprueba la firma, instala y reinicia Nexium.
#[tauri::command]
pub async fn update_install<R: Runtime>(app: AppHandle<R>) -> AppResult<()> {
    let update = app
        .updater()
        .map_err(|error| install_failure(&error))?
        .check()
        .await
        .map_err(|error| {
            check_failure(&error).unwrap_or_else(|| {
                AppError::Validation("Todavía no hay ninguna versión publicada.".into())
            })
        })?
        .ok_or_else(|| AppError::Validation("Ya tienes la última versión.".into()))?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|error| install_failure(&error))?;
    app.restart()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_published_version_is_not_a_failure() {
        assert!(check_failure(&UpdaterError::ReleaseNotFound).is_none());
    }

    #[tokio::test]
    async fn no_connection_is_told_apart() {
        let offline = reqwest::Client::new()
            .get("http://127.0.0.1:9")
            .send()
            .await
            .unwrap_err();
        let failure = check_failure(&UpdaterError::Reqwest(offline)).unwrap();
        assert!(failure.to_string().contains("No hay conexión con GitHub"));
        assert!(install_failure(&UpdaterError::Network("503".into()))
            .to_string()
            .contains("Se cortó la conexión"));
    }

    #[test]
    fn an_invalid_signature_is_never_installed() {
        for error in [
            UpdaterError::MissingSignedVersion,
            UpdaterError::SignatureUtf8("x".into()),
        ] {
            assert!(install_failure(&error)
                .to_string()
                .contains("La firma de la versión descargada no es válida"));
        }
    }

    #[test]
    fn a_malformed_release_is_explained() {
        let failure = check_failure(&UpdaterError::InvalidUpdaterFormat).unwrap();
        assert!(failure.to_string().contains("no tiene un formato válido"));
    }
}
