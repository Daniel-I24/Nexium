//! Transcripción con Whisper y caché del modelo en memoria.

use std::{
    ffi::c_void,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use super::{SpeechError, SpeechProvider};

struct CachedModel<T> {
    path: PathBuf,
    model: Arc<T>,
    last_used: Instant,
}

/// Conserva un solo modelo cargado. Se carga en el primer uso y se libera tras un periodo de inactividad.
pub struct ModelCache<T> {
    slot: Mutex<Option<CachedModel<T>>>,
}

impl<T> Default for ModelCache<T> {
    fn default() -> Self {
        Self {
            slot: Mutex::new(None),
        }
    }
}

impl<T> ModelCache<T> {
    pub fn get_or_load<E>(
        &self,
        path: &Path,
        now: Instant,
        load: impl FnOnce() -> Result<T, E>,
    ) -> Result<Arc<T>, E> {
        let mut slot = self
            .slot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(cached) = slot.as_mut().filter(|cached| cached.path == path) {
            cached.last_used = now;
            return Ok(cached.model.clone());
        }
        // El anterior se libera antes de cargar: nunca hay dos modelos en memoria.
        *slot = None;
        let model = Arc::new(load()?);
        *slot = Some(CachedModel {
            path: path.to_owned(),
            model: model.clone(),
            last_used: now,
        });
        Ok(model)
    }

    pub fn touch(&self, now: Instant) {
        if let Some(cached) = self
            .slot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_mut()
        {
            cached.last_used = now;
        }
    }

    /// Si el modelo se está cargando, no espera: lo revisará en la siguiente pasada.
    pub fn release_if_idle(&self, now: Instant, max_idle: Duration) -> bool {
        let Ok(mut slot) = self.slot.try_lock() else {
            return false;
        };
        let idle = slot
            .as_ref()
            .is_some_and(|cached| now.saturating_duration_since(cached.last_used) >= max_idle);
        if idle {
            *slot = None;
        }
        idle
    }
}

#[derive(Default)]
pub struct WhisperSpeech {
    models: Arc<ModelCache<WhisperContext>>,
}

fn load_whisper(model_path: &Path) -> Result<WhisperContext, SpeechError> {
    // Los mensajes de whisper.cpp (y los de depuración que activa `force-debug`) no se escriben en la salida
    // de errores: se descartan. Solo actúa la primera vez.
    whisper_rs::install_logging_hooks();
    let path = model_path
        .to_str()
        .ok_or_else(|| SpeechError::Failed("La ruta del modelo no es válida.".into()))?;
    WhisperContext::new_with_params(path, WhisperContextParameters::default())
        .map_err(|_| SpeechError::ModelUnloadable)
}

/// Whisper procesa 30 s aunque el audio dure menos. Ajustarlo a la duración real reduce la espera de 6,5 a 1,5 s
/// en frases cortas (`docs/PRUEBA_VOZ.md`). 1500 unidades equivalen a 30 s; el margen evita cortar el final.
pub fn audio_context(samples: usize) -> i32 {
    i32::try_from(samples / 320 + 128)
        .unwrap_or(1500)
        .clamp(256, 1500)
}

/// Conecta el indicador de cancelación de Nexium con Whisper. Es el único punto con código `unsafe` de la voz.
///
/// **Por qué no se usa `FullParams::set_abort_callback_safe`:** en whisper-rs 0.16.0 guarda el cierre como
/// `Box<Box<dyn FnMut() -> bool>>`, pero su trampolín lo lee como `*mut F`, el tipo concreto del cierre.
/// Whisper recibe así un valor arbitrario, casi siempre "abortar", y ninguna transcripción termina: devuelve los
/// errores -6 o -8. La 0.16.0 es la última versión publicada, y su rama principal tenía el mismo código el
/// 13 de noviembre de 2026. Por eso la versión está fijada con `=` en `Cargo.toml`: al actualizarla hay que
/// revisar si el fallo sigue y si esta función aún hace falta.
///
/// **Pruebas:** `the_abort_callback_reads_the_cancellation_flag` comprueba el callback, y
/// `silence_is_transcribed_and_not_reported_as_a_failure` (con un modelo real, `--ignored`) comprueba que una
/// transcripción completa ya no se aborta.
///
/// # Safety
/// `flag` debe seguir vivo hasta que termine la llamada a `state.full` que use estos parámetros.
unsafe fn watch_cancellation(params: &mut FullParams, flag: &AtomicBool) {
    // SAFETY: la persona que llama garantiza que `flag` sobrevive a la transcripción, que es quien lee el puntero.
    unsafe {
        params.set_abort_callback(Some(abort_requested));
        params.set_abort_callback_user_data(std::ptr::from_ref(flag).cast_mut().cast());
    }
}

/// Callback de `watch_cancellation`: Whisper lo consulta durante la transcripción.
///
/// # Safety
/// `user_data` debe apuntar a un `AtomicBool` vivo.
unsafe extern "C" fn abort_requested(user_data: *mut c_void) -> bool {
    // SAFETY: `watch_cancellation` solo pasa punteros a un `AtomicBool` vivo durante la transcripción.
    unsafe { (*user_data.cast::<AtomicBool>()).load(Ordering::Relaxed) }
}

/// Vocabulario de Nexium como contexto previo. Whisper escribe mejor las palabras que ya ha "oído": sin él,
/// "¿Cuánta RAM estoy usando?" llegó a transcribirse como "Cuentarrán estoy usando", y "Wi-Fi" como "fiesto".
/// Es corto a propósito: cada palabra del contexto alarga la transcripción. Con esta lista, unos 0,1 s
/// (`docs/RENDIMIENTO.md`); con una frase completa, medio segundo.
pub const VOCABULARY_PROMPT: &str =
    "RAM, CPU, batería, Wi-Fi, recordatorio, clima, pantalla, documento.";

/// Con audio casi mudo, Whisper puede devolver el propio contexto previo como si se hubiera dicho.
/// Una frase larga que sea un trozo literal del vocabulario no es una solicitud.
pub fn is_prompt_echo(text: &str) -> bool {
    let text = crate::core::rules::normalize(text);
    text.split(' ').count() >= 5 && crate::core::rules::normalize(VOCABULARY_PROMPT).contains(&text)
}

/// Lo transcrito y cuánto tardó cada parte: esperar al modelo (si aún se estaba cargando) y transcribir.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Transcript {
    pub text: String,
    pub model_load: Duration,
    pub transcription: Duration,
}

/// Un hilo por núcleo físico, hasta 8: los hilos lógicos de un mismo núcleo no aceleran Whisper y le restan
/// CPU al resto del equipo. Si el sistema no lo dice, la mitad de los lógicos.
pub fn whisper_threads() -> i32 {
    let physical = sysinfo::System::physical_core_count().ok().or_else(|| {
        std::thread::available_parallelism()
            .ok()
            .map(|n| (n.get() / 2).max(1))
    });
    physical
        .and_then(|cores| i32::try_from(cores).ok())
        .unwrap_or(4)
        .clamp(1, 8)
}

fn run_whisper(
    samples: &[f32],
    context: &WhisperContext,
    cancelled: Arc<AtomicBool>,
) -> Result<String, SpeechError> {
    let mut state = context
        .create_state()
        .map_err(|_| SpeechError::Failed("No se pudo preparar la transcripción.".into()))?;
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some("es"));
    params.set_audio_ctx(audio_context(samples.len()));
    params.set_n_threads(whisper_threads());
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_print_special(false);
    params.set_initial_prompt(VOCABULARY_PROMPT);
    // Sin reintentos a otra temperatura: con frases cortas, repetir la decodificación puede triplicar la espera
    // y no mejora la orden. Cada solicitud es independiente y cabe en un solo segmento.
    params.set_temperature_inc(0.0);
    params.set_no_context(true);
    params.set_single_segment(true);
    // SAFETY: `cancelled` se mantiene vivo en esta función hasta después de `state.full`.
    unsafe { watch_cancellation(&mut params, &cancelled) };
    let outcome = state.full(params, samples);
    if cancelled.load(Ordering::Relaxed) {
        return Err(SpeechError::Cancelled);
    }
    outcome.map_err(|error| match error {
        whisper_rs::WhisperError::NoSamples => SpeechError::NoAudio,
        other => SpeechError::Failed(format!(
            "Whisper no pudo transcribir el audio ({other:?}). Vuelve a intentarlo."
        )),
    })?;
    Ok(state
        .as_iter()
        .map(|segment| segment.to_string())
        .collect::<String>()
        .trim()
        .to_owned())
}

/// Quita las marcas que Whisper usa para el silencio, como "[Música]" o "(risas)".
pub fn clean_transcript(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut depth = 0u32;
    for c in text.chars() {
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => output.push(c),
            _ => {}
        }
    }
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[async_trait]
impl SpeechProvider for WhisperSpeech {
    async fn transcribe(
        &self,
        samples: Vec<f32>,
        model_path: PathBuf,
        cancel: CancellationToken,
    ) -> Result<Transcript, SpeechError> {
        if !model_path.exists() {
            return Err(SpeechError::ModelMissing);
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let models = self.models.clone();
        let task = tokio::task::spawn_blocking(move || {
            let started = Instant::now();
            // Si la precarga sigue en curso, aquí se espera a que termine: esa espera es la carga que se nota.
            let context = models.get_or_load(&model_path, Instant::now(), || {
                super::contained(|| load_whisper(&model_path))
            })?;
            let loaded = Instant::now();
            let text = super::contained(|| run_whisper(&samples, &context, flag))?;
            models.touch(Instant::now());
            let text = clean_transcript(&text);
            Ok(Transcript {
                text: if is_prompt_echo(&text) {
                    String::new()
                } else {
                    text
                },
                model_load: loaded - started,
                transcription: loaded.elapsed(),
            })
        });
        tokio::pin!(task);
        tokio::select! {
            outcome = &mut task => outcome.map_err(|_| SpeechError::Failed("La transcripción se interrumpió.".into()))?,
            () = cancel.cancelled() => {
                cancelled.store(true, Ordering::Relaxed);
                let _ = task.await;
                Err(SpeechError::Cancelled)
            }
        }
    }

    fn preload(&self, model_path: &Path) -> Result<(), SpeechError> {
        if !model_path.exists() {
            return Err(SpeechError::ModelMissing);
        }
        self.models
            .get_or_load(model_path, Instant::now(), || {
                super::contained(|| load_whisper(model_path))
            })
            .map(|_| ())
    }

    fn release_idle(&self, max_idle: Duration) {
        self.models.release_if_idle(Instant::now(), max_idle);
    }
}

#[cfg(test)]
#[path = "tests/whisper.rs"]
mod tests;
