//! Voz local: micrófono con cpal y transcripción con Whisper. El audio nunca sale del equipo ni se guarda.

mod audio;
mod catalog;
mod cpu;
mod microphone;
mod whisper;

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

pub use audio::*;
pub use catalog::*;
pub use cpu::*;
pub use microphone::*;
pub use whisper::*;

pub const SAMPLE_RATE: u32 = 16_000;
pub const MAX_RECORDING_SECONDS: usize = 30;

/// Cada causa de fallo de la voz, con su mensaje en español.
#[derive(Debug, PartialEq)]
pub enum SpeechError {
    ModelMissing,
    ModelDownloading,
    ModelDamaged,
    /// El archivo es correcto pero Whisper no pudo cargarlo: casi siempre por falta de memoria.
    ModelUnloadable,
    NoMicrophone(String),
    /// La grabación no tiene ninguna muestra.
    NoAudio,
    /// Se grabó, pero sin sonido: micrófono silenciado o dispositivo equivocado.
    Silent {
        device: String,
    },
    Failed(String),
    /// Al procesador le faltan instrucciones que necesita whisper.cpp (ver `cpu.rs`).
    UnsupportedCpu(Vec<&'static str>),
    Cancelled,
}

impl std::fmt::Display for SpeechError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ModelMissing => f.write_str("Falta el modelo de voz. Descárgalo en Ajustes, sección Voz. Mientras tanto puedes escribir tu solicitud."),
            Self::ModelDownloading => f.write_str("El modelo de voz aún se está descargando. Espera a que termine en Ajustes, sección Voz. Mientras tanto puedes escribir tu solicitud."),
            Self::ModelDamaged => f.write_str("El modelo de voz está incompleto o dañado. Vuelve a descargarlo en Ajustes, sección Voz."),
            Self::ModelUnloadable => f.write_str("No se pudo cargar el modelo de voz. Suele deberse a falta de memoria: cierra otras aplicaciones y vuelve a intentarlo."),
            Self::NoAudio => f.write_str("La grabación quedó vacía. Pulsa Hablar, di tu solicitud y después pulsa Terminar."),
            Self::Silent { device } => write!(f, "El micrófono «{device}» no captó ningún sonido. Comprueba que no esté silenciado y que sea el que usas."),
            Self::NoMicrophone(reason) | Self::Failed(reason) => f.write_str(reason),
            Self::UnsupportedCpu(missing) => write!(
                f,
                "La voz no está disponible en este equipo: a su procesador le faltan instrucciones que necesita el reconocimiento de voz ({}). Suelen tenerlas los procesadores de 2013 en adelante. Mientras tanto puedes escribir tus solicitudes.",
                missing.iter().map(|feature| feature.to_uppercase()).collect::<Vec<_>>().join(", ")
            ),
            Self::Cancelled => f.write_str("Operación cancelada."),
        }
    }
}

/// Por debajo de este pico (unos -60 dBFS) la grabación es silencio: ni siquiera hay ruido de fondo.
const SILENCE_PEAK: f32 = 0.001;

/// Distingue una grabación vacía o muda antes de gastar tiempo en transcribirla.
pub fn check_audible(samples: &[f32], device: &str) -> Result<(), SpeechError> {
    if samples.is_empty() {
        return Err(SpeechError::NoAudio);
    }
    let peak = samples
        .iter()
        .fold(0.0f32, |peak, sample| peak.max(sample.abs()));
    if peak < SILENCE_PEAK {
        return Err(SpeechError::Silent {
            device: device.to_owned(),
        });
    }
    Ok(())
}

#[async_trait]
pub trait SpeechProvider: Send + Sync {
    async fn transcribe(
        &self,
        samples: Vec<f32>,
        model_path: PathBuf,
        cancel: CancellationToken,
    ) -> Result<Transcript, SpeechError>;
    /// Carga el modelo si aún no está en memoria. Se llama al pulsar Hablar, en segundo plano,
    /// para que la transcripción no tenga que esperar a la carga al terminar la grabación.
    fn preload(&self, model_path: &Path) -> Result<(), SpeechError>;
    /// Libera de la memoria un modelo que lleve al menos `max_idle` sin usarse.
    fn release_idle(&self, max_idle: Duration);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_recording_problem_has_its_own_message() {
        assert_eq!(check_audible(&[], "Micrófono"), Err(SpeechError::NoAudio));
        assert_eq!(
            check_audible(&[0.0; 16_000], "Stereo Mix"),
            Err(SpeechError::Silent {
                device: "Stereo Mix".into()
            })
        );
        assert_eq!(
            check_audible(&[0.0005, -0.0009], "Micrófono"),
            Err(SpeechError::Silent {
                device: "Micrófono".into()
            })
        );
        assert_eq!(check_audible(&[0.0, 0.02, -0.01], "Micrófono"), Ok(()));
        let messages = [
            SpeechError::ModelMissing,
            SpeechError::ModelDownloading,
            SpeechError::ModelDamaged,
            SpeechError::ModelUnloadable,
            SpeechError::NoAudio,
            SpeechError::Silent {
                device: "Stereo Mix".into(),
            },
        ]
        .map(|error| error.to_string());
        let unique: std::collections::HashSet<_> = messages.iter().collect();
        assert_eq!(unique.len(), messages.len());
        assert!(messages[1].contains("se está descargando"));
        assert!(messages[2].contains("incompleto o dañado"));
        assert!(messages[3].contains("memoria"));
        assert!(messages[5].contains("Stereo Mix") && messages[5].contains("silenciado"));
        assert!(messages
            .iter()
            .all(|message| message != "No se pudo transcribir el audio."));
    }
}
