//! Catálogo de modelos de Whisper: estado en disco y descarga verificada por hash.

use std::{path::Path, sync::OnceLock};

use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

const MODEL_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpeechModel {
    pub id: &'static str,
    pub display_name: &'static str,
    /// Qué gana y qué pierde, y para qué equipos conviene, en una línea.
    pub description: &'static str,
    pub file: &'static str,
    /// Tamaño exacto del archivo: se comprueba antes de usarlo, además del hash al descargarlo.
    pub size_bytes: u64,
    pub sha256: &'static str,
}

pub const CATALOG: &[SpeechModel] = &[
    SpeechModel {
        id: "small",
        display_name: "Preciso",
        description: "Entiende mejor nombres y frases largas, pero tarda unas tres veces más y usa unos 330 MB de memoria al transcribir. Para equipos con 8 GB de RAM y 4 núcleos o más.",
        file: "ggml-small-q5_1.bin",
        size_bytes: 190_085_487,
        sha256: "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb",
    },
    SpeechModel {
        id: "base",
        display_name: "Rápido",
        description: "Responde unas tres veces antes y usa unos 150 MB, pero se equivoca más (21 % de palabras frente a 16 %). Para equipos con menos de 8 GB de RAM o menos de 4 núcleos.",
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

/// Los valores válidos del ajuste: el modo automático y cada modelo.
pub fn catalog_ids() -> Vec<&'static str> {
    std::iter::once(AUTO_MODEL)
        .chain(CATALOG.iter().map(|model| model.id))
        .collect()
}

/// Ajuste en el que Nexium elige el modelo según el equipo.
pub const AUTO_MODEL: &str = "auto";
/// Umbrales del modo automático, justificados en `docs/PRUEBA_VOZ.md`. Preciso necesita unos 330 MB más
/// mientras transcribe, y su tiempo baja casi en proporción a los núcleos: con 4 tarda en torno a 1 s una orden
/// corta; con 2 pasaría de los 2 s. Un equipo de 8 GB informa algo menos (memoria reservada por el sistema y
/// la gráfica), por eso el umbral es de 7 GiB.
pub const PRECISE_MIN_MEMORY_GIB: f64 = 7.0;
pub const PRECISE_MIN_CORES: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hardware {
    pub memory_gib: f64,
    pub physical_cores: usize,
}

impl Hardware {
    /// Se mide una vez: no cambia mientras Nexium está abierto.
    pub fn detect() -> Self {
        static DETECTED: OnceLock<Hardware> = OnceLock::new();
        *DETECTED.get_or_init(|| {
            let system = sysinfo::System::new_with_specifics(
                sysinfo::RefreshKind::nothing()
                    .with_memory(sysinfo::MemoryRefreshKind::nothing().with_ram()),
            );
            Self {
                // Sin poder medirla se asume un equipo modesto: Rápido nunca deja la voz lenta.
                memory_gib: system
                    .map_or(0.0, |system| system.total_memory() as f64 / 1_073_741_824.0),
                physical_cores: usize::try_from(super::whisper_threads()).unwrap_or(1),
            }
        })
    }
}

/// Lo que elige el modo automático y por qué, para mostrarlo en Ajustes.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutoChoice {
    pub model_id: String,
    pub display_name: String,
    pub reason: String,
}

pub fn auto_choice(hardware: Hardware) -> (&'static SpeechModel, String) {
    let memory = hardware.memory_gib.round();
    let cores = hardware.physical_cores;
    let (id, reason) = if hardware.physical_cores < PRECISE_MIN_CORES {
        (
            "base",
            format!(
                "este equipo tiene {cores} núcleos, y con Preciso cada orden tardaría más de 2 s."
            ),
        )
    } else if hardware.memory_gib < PRECISE_MIN_MEMORY_GIB {
        ("base", format!("este equipo tiene {memory} GB de RAM, y Preciso necesita unos 330 MB más mientras transcribe."))
    } else {
        ("small", format!("este equipo tiene {memory} GB de RAM y {cores} núcleos, suficiente para que Preciso responda en torno a 1 s."))
    };
    (find_model(id).unwrap_or(&CATALOG[0]), reason)
}

/// El modelo que se usa con un ajuste guardado. `None` si el ajuste no existe.
pub fn resolve_model(setting: &str, hardware: Hardware) -> Option<&'static SpeechModel> {
    if setting == AUTO_MODEL {
        Some(auto_choice(hardware).0)
    } else {
        find_model(setting)
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeechModelStatus {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub size_mb: u64,
    pub path: String,
    /// Completo y con su tamaño exacto: se puede usar.
    pub downloaded: bool,
    /// Hay un archivo con su nombre, pero no tiene el tamaño esperado.
    pub damaged: bool,
    pub is_active: bool,
}

/// `active` es el modelo que se usa de verdad, ya resuelto el modo automático.
pub fn statuses(directory: &Path, active: &str) -> Vec<SpeechModelStatus> {
    CATALOG
        .iter()
        .map(|model| {
            let path = directory.join(model.file);
            let state = model_state(model, directory);
            SpeechModelStatus {
                id: model.id.into(),
                display_name: model.display_name.into(),
                description: model.description.into(),
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
#[path = "tests/catalog.rs"]
mod tests;
