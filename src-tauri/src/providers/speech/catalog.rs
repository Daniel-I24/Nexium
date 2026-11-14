//! Catálogo de modelos de Whisper: estado en disco y descarga verificada por hash.

use std::path::Path;

use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

const MODEL_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpeechModel {
    pub id: &'static str,
    pub display_name: &'static str,
    pub file: &'static str,
    /// Tamaño exacto del archivo: se comprueba antes de usarlo, además del hash al descargarlo.
    pub size_bytes: u64,
    pub sha256: &'static str,
}

pub const CATALOG: &[SpeechModel] = &[
    SpeechModel {
        id: "small",
        display_name: "Preciso (recomendado)",
        file: "ggml-small-q5_1.bin",
        size_bytes: 190_085_487,
        sha256: "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb",
    },
    SpeechModel {
        id: "base",
        display_name: "Rápido, para equipos modestos",
        file: "ggml-base-q5_1.bin",
        size_bytes: 59_707_625,
        sha256: "422f1ae452ade6f30a004d7e5c6a43195e4433bc370bf23fac9cc591f01a8898",
    },
];

/// En qué estado está un modelo en disco.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelState {
    Missing,
    /// Hay un archivo con su nombre pero no tiene el tamaño esperado: quedó a medias o se alteró.
    Damaged,
    Ready,
}

pub fn model_state(model: &SpeechModel, directory: &Path) -> ModelState {
    match std::fs::metadata(directory.join(model.file)) {
        Ok(metadata) if metadata.len() == model.size_bytes => ModelState::Ready,
        Ok(_) => ModelState::Damaged,
        Err(_) => ModelState::Missing,
    }
}

/// Megabytes como los muestra Windows (1 MB = 1 048 576 bytes). La etiqueta y el contador de descarga usan
/// siempre esta función, para que nunca muestren cifras distintas del mismo archivo.
pub fn megabytes(bytes: u64) -> u64 {
    (bytes + 524_288) / 1_048_576
}

pub fn find_model(id: &str) -> Option<&'static SpeechModel> {
    CATALOG.iter().find(|model| model.id == id)
}

pub fn catalog_ids() -> Vec<&'static str> {
    CATALOG.iter().map(|model| model.id).collect()
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeechModelStatus {
    pub id: String,
    pub display_name: String,
    pub size_mb: u64,
    pub path: String,
    /// Completo y con su tamaño exacto: se puede usar.
    pub downloaded: bool,
    /// Hay un archivo con su nombre, pero no tiene el tamaño esperado.
    pub damaged: bool,
    pub is_active: bool,
}

pub fn statuses(directory: &Path, active: &str) -> Vec<SpeechModelStatus> {
    CATALOG
        .iter()
        .map(|model| {
            let path = directory.join(model.file);
            let state = model_state(model, directory);
            SpeechModelStatus {
                id: model.id.into(),
                display_name: model.display_name.into(),
                size_mb: megabytes(model.size_bytes),
                downloaded: state == ModelState::Ready,
                damaged: state == ModelState::Damaged,
                path: path.to_string_lossy().into_owned(),
                is_active: model.id == active,
            }
        })
        .collect()
}

/// Descarga verificada por hash: un archivo incompleto o alterado nunca queda como modelo válido.
pub async fn download_model(
    model: &SpeechModel,
    directory: &Path,
    on_progress: impl Fn(u64, u64) + Send,
    base_url: Option<&str>,
) -> Result<(), String> {
    const FAILED: &str = "No se pudo descargar el modelo de voz. Comprueba la conexión.";
    tokio::fs::create_dir_all(directory)
        .await
        .map_err(|_| "No se pudo crear la carpeta del modelo.".to_owned())?;
    let url = format!("{}/{}", base_url.unwrap_or(MODEL_BASE_URL), model.file);
    let response = reqwest::get(url).await.map_err(|_| FAILED.to_owned())?;
    if !response.status().is_success() {
        return Err(FAILED.into());
    }
    let total = response.content_length().unwrap_or(model.size_bytes);
    let partial = directory.join(format!("{}.part", model.file));
    let mut file = tokio::fs::File::create(&partial)
        .await
        .map_err(|_| "No se pudo escribir el modelo.".to_owned())?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0u64;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| FAILED.to_owned())?;
        hasher.update(&chunk);
        file.write_all(&chunk)
            .await
            .map_err(|_| "No se pudo escribir el modelo.".to_owned())?;
        downloaded += chunk.len() as u64;
        on_progress(downloaded, total);
    }
    file.flush()
        .await
        .map_err(|_| "No se pudo escribir el modelo.".to_owned())?;
    drop(file);
    let digest: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if digest != model.sha256 {
        let _ = tokio::fs::remove_file(&partial).await;
        return Err("El modelo descargado no superó la verificación. Inténtalo de nuevo.".into());
    }
    tokio::fs::rename(&partial, directory.join(model.file))
        .await
        .map_err(|_| "No se pudo guardar el modelo.".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::speech::{SpeechError, SpeechProvider, WhisperSpeech};
    use std::path::PathBuf;
    use tokio_util::sync::CancellationToken;
    use wiremock::{matchers::path, Mock, MockServer, ResponseTemplate};

    #[test]
    fn the_model_is_ready_only_with_its_exact_size() {
        let directory = tempfile::tempdir().unwrap();
        let model = SpeechModel {
            id: "prueba",
            display_name: "Prueba",
            file: "modelo.bin",
            size_bytes: 4,
            sha256: "",
        };
        assert_eq!(model_state(&model, directory.path()), ModelState::Missing);
        std::fs::write(directory.path().join("modelo.bin.part"), b"12").unwrap();
        assert_eq!(model_state(&model, directory.path()), ModelState::Missing);
        std::fs::write(directory.path().join("modelo.bin"), b"123").unwrap();
        assert_eq!(model_state(&model, directory.path()), ModelState::Damaged);
        std::fs::write(directory.path().join("modelo.bin"), b"1234").unwrap();
        assert_eq!(model_state(&model, directory.path()), ModelState::Ready);
    }

    #[test]
    fn the_catalog_sizes_match_the_published_files() {
        assert_eq!(find_model("small").unwrap().size_bytes, 190_085_487);
        assert_eq!(find_model("base").unwrap().size_bytes, 59_707_625);
    }

    #[test]
    fn the_label_and_the_download_counter_show_the_same_size() {
        assert_eq!(megabytes(190_085_487), 181);
        assert_eq!(megabytes(59_707_625), 57);
        let directory = tempfile::tempdir().unwrap();
        let listed = statuses(directory.path(), "small");
        assert_eq!(
            listed.iter().map(|model| model.size_mb).collect::<Vec<_>>(),
            [181, 57]
        );
    }

    #[test]
    fn only_a_complete_model_counts_as_downloaded() {
        let directory = tempfile::tempdir().unwrap();
        let small = find_model("small").unwrap();
        std::fs::write(
            directory.path().join(format!("{}.part", small.file)),
            b"a medias",
        )
        .unwrap();
        let listed = statuses(directory.path(), "small");
        assert!(!listed[0].downloaded && !listed[0].damaged);
        std::fs::write(directory.path().join(small.file), b"truncado").unwrap();
        let listed = statuses(directory.path(), "small");
        assert!(!listed[0].downloaded && listed[0].damaged);
    }

    #[tokio::test]
    async fn a_missing_model_is_reported() {
        let error = WhisperSpeech::default()
            .transcribe(
                vec![0.0; 10],
                PathBuf::from("no-existe.bin"),
                CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert_eq!(error, SpeechError::ModelMissing);
    }

    #[tokio::test]
    async fn a_corrupt_download_is_discarded() {
        let server = MockServer::start().await;
        Mock::given(path("/ggml-base-q5_1.bin"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![1, 2, 3]))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let model = find_model("base").unwrap();
        assert!(
            download_model(model, dir.path(), |_, _| {}, Some(&server.uri()))
                .await
                .is_err()
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn a_verified_download_is_kept() {
        let server = MockServer::start().await;
        let body = b"modelo".to_vec();
        Mock::given(path("/m.bin"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body.clone()))
            .mount(&server)
            .await;
        let digest: String = Sha256::digest(&body)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let digest: &'static str = Box::leak(digest.into_boxed_str());
        let model = SpeechModel {
            id: "x",
            display_name: "x",
            file: "m.bin",
            size_bytes: body.len() as u64,
            sha256: digest,
        };
        let dir = tempfile::tempdir().unwrap();
        download_model(&model, dir.path(), |_, _| {}, Some(&server.uri()))
            .await
            .unwrap();
        assert!(dir.path().join("m.bin").exists());
    }

    #[test]
    fn the_catalog_has_the_default_model() {
        assert!(find_model(crate::services::settings::DEFAULT_SPEECH_MODEL).is_some());
    }
}
