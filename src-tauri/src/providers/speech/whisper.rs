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
    params.set_n_threads(
        std::thread::available_parallelism()
            .map_or(4, |n| i32::try_from(n.get() / 2).unwrap_or(4).clamp(1, 8)),
    );
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_print_special(false);
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
    ) -> Result<String, SpeechError> {
        if !model_path.exists() {
            return Err(SpeechError::ModelMissing);
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let models = self.models.clone();
        let task = tokio::task::spawn_blocking(move || {
            let context =
                models.get_or_load(&model_path, Instant::now(), || load_whisper(&model_path))?;
            let outcome = run_whisper(&samples, &context, flag);
            models.touch(Instant::now());
            outcome
        });
        tokio::pin!(task);
        tokio::select! {
            outcome = &mut task => outcome.map_err(|_| SpeechError::Failed("La transcripción se interrumpió.".into()))?.map(|text| clean_transcript(&text)),
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
            .get_or_load(model_path, Instant::now(), || load_whisper(model_path))
            .map(|_| ())
    }

    fn release_idle(&self, max_idle: Duration) {
        self.models.release_if_idle(Instant::now(), max_idle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::speech::SAMPLE_RATE;

    #[test]
    fn the_abort_callback_reads_the_cancellation_flag() {
        let flag = AtomicBool::new(false);
        let pointer = std::ptr::from_ref(&flag).cast_mut().cast();
        // SAFETY: `flag` vive durante toda la prueba.
        assert!(!unsafe { abort_requested(pointer) });
        flag.store(true, Ordering::Relaxed);
        assert!(unsafe { abort_requested(pointer) });
    }

    /// Necesita el modelo real: `NEXIUM_WHISPER_MODEL=<ruta de ggml-*.bin> cargo test -- --ignored`.
    #[tokio::test]
    #[ignore = "necesita un modelo de Whisper descargado"]
    async fn silence_is_transcribed_and_not_reported_as_a_failure() {
        let model =
            PathBuf::from(std::env::var("NEXIUM_WHISPER_MODEL").expect("NEXIUM_WHISPER_MODEL"));
        let speech = WhisperSpeech::default();
        let silence = vec![0.0; SAMPLE_RATE as usize * 4];
        assert_eq!(
            speech
                .transcribe(silence, model, CancellationToken::new())
                .await,
            Ok(String::new())
        );
    }

    #[test]
    fn silence_markers_are_removed() {
        assert_eq!(clean_transcript(" [Música] (risas) "), "");
        assert_eq!(clean_transcript("¿Qué hora es? [Música]"), "¿Qué hora es?");
    }

    #[test]
    fn the_audio_context_follows_the_duration_within_limits() {
        assert_eq!(audio_context(0), 256);
        assert_eq!(audio_context(16_000 * 10), 628);
        assert_eq!(audio_context(16_000 * 30), 1500);
        assert_eq!(audio_context(usize::MAX), 1500);
    }

    #[test]
    fn the_model_loads_once_and_stays_in_memory() {
        let cache = ModelCache::default();
        let (path, now, mut loads) = (Path::new("small.bin"), Instant::now(), 0);
        for _ in 0..3 {
            cache
                .get_or_load(path, now, || {
                    loads += 1;
                    Ok::<_, ()>(7)
                })
                .unwrap();
        }
        assert_eq!(loads, 1);
    }

    #[test]
    fn another_model_replaces_the_first_before_loading() {
        let cache = ModelCache::default();
        let now = Instant::now();
        let first = cache
            .get_or_load(Path::new("small.bin"), now, || Ok::<_, ()>(1))
            .unwrap();
        let weak = Arc::downgrade(&first);
        drop(first);
        let second = cache
            .get_or_load(Path::new("base.bin"), now, || {
                assert!(weak.upgrade().is_none());
                Ok::<_, ()>(2)
            })
            .unwrap();
        assert_eq!(*second, 2);
    }

    #[test]
    fn a_failed_load_leaves_nothing_cached() {
        let cache = ModelCache::<i32>::default();
        let now = Instant::now();
        assert_eq!(
            cache
                .get_or_load(Path::new("m.bin"), now, || Err("dañado"))
                .unwrap_err(),
            "dañado"
        );
        assert_eq!(
            *cache
                .get_or_load(Path::new("m.bin"), now, || Ok::<_, &str>(1))
                .unwrap(),
            1
        );
    }

    #[test]
    fn the_model_is_released_only_after_the_idle_period() {
        let cache = ModelCache::default();
        let (path, start, idle) = (Path::new("m.bin"), Instant::now(), Duration::from_secs(600));
        let model = cache.get_or_load(path, start, || Ok::<_, ()>(1)).unwrap();
        let weak = Arc::downgrade(&model);
        drop(model);
        assert!(!cache.release_if_idle(start + Duration::from_secs(599), idle));
        assert!(weak.upgrade().is_some());
        assert!(cache.release_if_idle(start + idle, idle));
        assert!(weak.upgrade().is_none());
        assert!(!cache.release_if_idle(start + idle, idle));
    }

    #[test]
    fn using_the_model_restarts_the_idle_period() {
        let cache = ModelCache::default();
        let (start, idle) = (Instant::now(), Duration::from_secs(600));
        cache
            .get_or_load(Path::new("m.bin"), start, || Ok::<_, ()>(1))
            .unwrap();
        cache.touch(start + Duration::from_secs(500));
        assert!(!cache.release_if_idle(start + idle, idle));
    }

    #[test]
    fn a_model_in_use_survives_its_release() {
        let cache = ModelCache::default();
        let (start, idle) = (Instant::now(), Duration::from_secs(1));
        let in_use = cache
            .get_or_load(Path::new("m.bin"), start, || Ok::<_, ()>(9))
            .unwrap();
        assert!(cache.release_if_idle(start + idle, idle));
        assert_eq!(*in_use, 9);
    }
}
