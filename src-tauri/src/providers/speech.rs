//! Voz local: micrófono con cpal y transcripción con Whisper. El audio nunca sale del equipo ni se guarda.

use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub const SAMPLE_RATE: u32 = 16_000;
pub const MAX_RECORDING_SECONDS: usize = 30;
const MODEL_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpeechModel {
    pub id: &'static str,
    pub display_name: &'static str,
    pub file: &'static str,
    pub size_mb: u32,
    pub sha256: &'static str,
}

pub const CATALOG: &[SpeechModel] = &[
    SpeechModel { id: "small", display_name: "Preciso (recomendado)", file: "ggml-small-q5_1.bin", size_mb: 190, sha256: "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb" },
    SpeechModel { id: "base", display_name: "Rápido, para equipos modestos", file: "ggml-base-q5_1.bin", size_mb: 60, sha256: "422f1ae452ade6f30a004d7e5c6a43195e4433bc370bf23fac9cc591f01a8898" },
];

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
    pub size_mb: u32,
    pub path: String,
    pub downloaded: bool,
    pub is_active: bool,
}

pub fn statuses(directory: &Path, active: &str) -> Vec<SpeechModelStatus> {
    CATALOG
        .iter()
        .map(|model| {
            let path = directory.join(model.file);
            SpeechModelStatus {
                id: model.id.into(),
                display_name: model.display_name.into(),
                size_mb: model.size_mb,
                downloaded: path.exists(),
                path: path.to_string_lossy().into_owned(),
                is_active: model.id == active,
            }
        })
        .collect()
}

#[derive(Debug, PartialEq)]
pub enum SpeechError {
    ModelMissing,
    NoMicrophone(String),
    Failed(String),
    Cancelled,
}

impl std::fmt::Display for SpeechError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ModelMissing => f.write_str("Falta el modelo de voz. Descárgalo en Ajustes, sección Voz."),
            Self::NoMicrophone(reason) | Self::Failed(reason) => f.write_str(reason),
            Self::Cancelled => f.write_str("Operación cancelada."),
        }
    }
}

#[async_trait]
pub trait SpeechProvider: Send + Sync {
    async fn transcribe(&self, samples: Vec<f32>, model_path: PathBuf, cancel: CancellationToken) -> Result<String, SpeechError>;
    /// Libera de la memoria un modelo que lleve al menos `max_idle` sin usarse.
    fn release_idle(&self, max_idle: Duration);
}

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
        Self { slot: Mutex::new(None) }
    }
}

impl<T> ModelCache<T> {
    pub fn get_or_load<E>(&self, path: &Path, now: Instant, load: impl FnOnce() -> Result<T, E>) -> Result<Arc<T>, E> {
        let mut slot = self.slot.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(cached) = slot.as_mut().filter(|cached| cached.path == path) {
            cached.last_used = now;
            return Ok(cached.model.clone());
        }
        // El anterior se libera antes de cargar: nunca hay dos modelos en memoria.
        *slot = None;
        let model = Arc::new(load()?);
        *slot = Some(CachedModel { path: path.to_owned(), model: model.clone(), last_used: now });
        Ok(model)
    }

    pub fn touch(&self, now: Instant) {
        if let Some(cached) = self.slot.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_mut() {
            cached.last_used = now;
        }
    }

    /// Si el modelo se está cargando, no espera: lo revisará en la siguiente pasada.
    pub fn release_if_idle(&self, now: Instant, max_idle: Duration) -> bool {
        let Ok(mut slot) = self.slot.try_lock() else { return false };
        let idle = slot.as_ref().is_some_and(|cached| now.saturating_duration_since(cached.last_used) >= max_idle);
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
    let path = model_path.to_str().ok_or_else(|| SpeechError::Failed("La ruta del modelo no es válida.".into()))?;
    WhisperContext::new_with_params(path, WhisperContextParameters::default()).map_err(|_| SpeechError::Failed("No se pudo cargar el modelo de voz. Vuelve a descargarlo en Ajustes.".into()))
}

/// Whisper procesa 30 s aunque el audio dure menos. Ajustarlo a la duración real reduce la espera de 6,5 a 1,5 s
/// en frases cortas (`docs/PRUEBA_VOZ.md`). 1500 unidades equivalen a 30 s; el margen evita cortar el final.
pub fn audio_context(samples: usize) -> i32 {
    i32::try_from(samples / 320 + 128).unwrap_or(1500).clamp(256, 1500)
}

fn run_whisper(samples: &[f32], context: &WhisperContext, cancelled: Arc<AtomicBool>) -> Result<String, SpeechError> {
    let mut state = context.create_state().map_err(|_| SpeechError::Failed("No se pudo preparar la transcripción.".into()))?;
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some("es"));
    params.set_audio_ctx(audio_context(samples.len()));
    params.set_n_threads(std::thread::available_parallelism().map_or(4, |n| i32::try_from(n.get() / 2).unwrap_or(4).clamp(1, 8)));
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_print_special(false);
    let flag = cancelled.clone();
    params.set_abort_callback_safe(move || flag.load(Ordering::Relaxed));
    let outcome = state.full(params, samples);
    if cancelled.load(Ordering::Relaxed) {
        return Err(SpeechError::Cancelled);
    }
    outcome.map_err(|_| SpeechError::Failed("No se pudo transcribir el audio.".into()))?;
    Ok(state.as_iter().map(|segment| segment.to_string()).collect::<String>().trim().to_owned())
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
    async fn transcribe(&self, samples: Vec<f32>, model_path: PathBuf, cancel: CancellationToken) -> Result<String, SpeechError> {
        if !model_path.exists() {
            return Err(SpeechError::ModelMissing);
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let models = self.models.clone();
        let task = tokio::task::spawn_blocking(move || {
            let context = models.get_or_load(&model_path, Instant::now(), || load_whisper(&model_path))?;
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

    fn release_idle(&self, max_idle: Duration) {
        self.models.release_if_idle(Instant::now(), max_idle);
    }
}

/// Convierte a 16 kHz, el formato que exige Whisper.
pub fn resample(samples: &[f32], from_rate: u32) -> Vec<f32> {
    if from_rate == SAMPLE_RATE || samples.is_empty() {
        return samples.to_vec();
    }
    let ratio = f64::from(from_rate) / f64::from(SAMPLE_RATE);
    let length = (samples.len() as f64 / ratio) as usize;
    (0..length)
        .map(|index| {
            let position = index as f64 * ratio;
            let base = position as usize;
            let next = samples.get(base + 1).copied().unwrap_or(samples[base]);
            let fraction = (position - base as f64) as f32;
            samples[base] + (next - samples[base]) * fraction
        })
        .collect()
}

pub fn level_of(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let energy = samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32;
    (energy.sqrt() * 6.0).min(1.0)
}

struct CaptureBuffer {
    samples: Vec<f32>,
    pending_level: Vec<f32>,
    channels: usize,
    limit: usize,
    limit_reached: bool,
}

/// Grabación en curso. Soltarla sin llamar a `finish` cierra el micrófono y descarta el audio.
pub struct Recording {
    stop: Option<mpsc::Sender<()>>,
    thread: Option<JoinHandle<()>>,
    buffer: Arc<Mutex<CaptureBuffer>>,
    sample_rate: u32,
}

impl Recording {
    pub fn finish(mut self) -> Vec<f32> {
        self.close();
        let samples = self.buffer.lock().map(|mut buffer| std::mem::take(&mut buffer.samples)).unwrap_or_default();
        resample(&samples, self.sample_rate)
    }

    fn close(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        self.close();
    }
}

type LevelCallback = Arc<dyn Fn(f32) + Send + Sync>;
type LimitCallback = Arc<dyn Fn() + Send + Sync>;

fn no_microphone() -> SpeechError {
    SpeechError::NoMicrophone("No puedo usar el micrófono: no hay ninguno disponible o el sistema le ha negado el permiso a Nexium. Revísalo en la configuración de privacidad.".into())
}

fn push_samples<T: Copy>(buffer: &Mutex<CaptureBuffer>, data: &[T], convert: impl Fn(T) -> f32, on_level: &LevelCallback, on_limit: &LimitCallback, chunk: usize) {
    let Ok(mut buffer) = buffer.lock() else { return };
    if buffer.limit_reached {
        return;
    }
    let channels = buffer.channels.max(1);
    for frame in data.chunks(channels) {
        let mono = frame.iter().map(|sample| convert(*sample)).sum::<f32>() / channels as f32;
        buffer.samples.push(mono);
        buffer.pending_level.push(mono);
    }
    if buffer.pending_level.len() >= chunk {
        on_level(level_of(&buffer.pending_level));
        buffer.pending_level.clear();
    }
    if buffer.samples.len() >= buffer.limit {
        buffer.limit_reached = true;
        on_limit();
    }
}

/// Abre el micrófono en un hilo propio: el flujo de cpal no se puede mover entre hilos en todos los sistemas.
pub fn start_recording(on_level: LevelCallback, on_limit: LimitCallback) -> Result<Recording, SpeechError> {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(Arc<Mutex<CaptureBuffer>>, u32), SpeechError>>();
    let thread = std::thread::spawn(move || {
        let opened = (|| {
            let device = cpal::default_host().default_input_device().ok_or_else(no_microphone)?;
            let supported = device.default_input_config().map_err(|_| no_microphone())?;
            let format = supported.sample_format();
            let config: cpal::StreamConfig = supported.into();
            let rate: u32 = config.sample_rate;
            let buffer = Arc::new(Mutex::new(CaptureBuffer {
                samples: Vec::new(),
                pending_level: Vec::new(),
                channels: usize::from(config.channels),
                limit: rate as usize * MAX_RECORDING_SECONDS,
                limit_reached: false,
            }));
            let chunk = rate as usize / 20;
            let target = buffer.clone();
            let (level, limit) = (on_level.clone(), on_limit.clone());
            let on_error = |_| {};
            let stream = match format {
                cpal::SampleFormat::F32 => device.build_input_stream(config, move |data: &[f32], _: &_| push_samples(&target, data, |s| s, &level, &limit, chunk), on_error, None),
                cpal::SampleFormat::I16 => device.build_input_stream(config, move |data: &[i16], _: &_| push_samples(&target, data, |s| f32::from(s) / 32768.0, &level, &limit, chunk), on_error, None),
                cpal::SampleFormat::U16 => device.build_input_stream(config, move |data: &[u16], _: &_| push_samples(&target, data, |s| (f32::from(s) - 32768.0) / 32768.0, &level, &limit, chunk), on_error, None),
                _ => return Err(SpeechError::NoMicrophone("El micrófono usa un formato de audio que Nexium no admite.".into())),
            }
            .map_err(|_| no_microphone())?;
            stream.play().map_err(|_| no_microphone())?;
            Ok((stream, buffer, rate))
        })();
        match opened {
            Ok((stream, buffer, rate)) => {
                let _ = ready_tx.send(Ok((buffer, rate)));
                let _ = stop_rx.recv();
                drop(stream);
            }
            Err(error) => {
                let _ = ready_tx.send(Err(error));
            }
        }
    });
    let (buffer, sample_rate) = ready_rx.recv_timeout(Duration::from_secs(10)).map_err(|_| no_microphone())??;
    Ok(Recording { stop: Some(stop_tx), thread: Some(thread), buffer, sample_rate })
}

/// Descarga verificada por hash: un archivo incompleto o alterado nunca queda como modelo válido.
pub async fn download_model(model: &SpeechModel, directory: &Path, on_progress: impl Fn(u64, u64) + Send, base_url: Option<&str>) -> Result<(), String> {
    const FAILED: &str = "No se pudo descargar el modelo de voz. Comprueba la conexión.";
    tokio::fs::create_dir_all(directory).await.map_err(|_| "No se pudo crear la carpeta del modelo.".to_owned())?;
    let url = format!("{}/{}", base_url.unwrap_or(MODEL_BASE_URL), model.file);
    let response = reqwest::get(url).await.map_err(|_| FAILED.to_owned())?;
    if !response.status().is_success() {
        return Err(FAILED.into());
    }
    let total = response.content_length().unwrap_or(u64::from(model.size_mb) * 1_048_576);
    let partial = directory.join(format!("{}.part", model.file));
    let mut file = tokio::fs::File::create(&partial).await.map_err(|_| "No se pudo escribir el modelo.".to_owned())?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0u64;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| FAILED.to_owned())?;
        hasher.update(&chunk);
        file.write_all(&chunk).await.map_err(|_| "No se pudo escribir el modelo.".to_owned())?;
        downloaded += chunk.len() as u64;
        on_progress(downloaded, total);
    }
    file.flush().await.map_err(|_| "No se pudo escribir el modelo.".to_owned())?;
    drop(file);
    let digest: String = hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect();
    if digest != model.sha256 {
        let _ = tokio::fs::remove_file(&partial).await;
        return Err("El modelo descargado no superó la verificación. Inténtalo de nuevo.".into());
    }
    tokio::fs::rename(&partial, directory.join(model.file)).await.map_err(|_| "No se pudo guardar el modelo.".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{matchers::path, Mock, MockServer, ResponseTemplate};

    #[test]
    fn resampling_keeps_duration() {
        let one_second = vec![0.5; 48_000];
        let result = resample(&one_second, 48_000);
        assert_eq!(result.len(), 16_000);
        assert!((result[100] - 0.5).abs() < 1e-5);
    }

    #[test]
    fn resampling_at_the_same_rate_changes_nothing() {
        assert_eq!(resample(&[0.1, 0.2], SAMPLE_RATE), vec![0.1, 0.2]);
    }

    #[test]
    fn level_is_bounded() {
        assert_eq!(level_of(&[]), 0.0);
        assert!((0.0..=1.0).contains(&level_of(&[1.0; 100])));
        assert!(level_of(&[0.0; 100]) < 0.01);
    }

    #[test]
    fn silence_markers_are_removed() {
        assert_eq!(clean_transcript(" [Música] (risas) "), "");
        assert_eq!(clean_transcript("¿Qué hora es? [Música]"), "¿Qué hora es?");
    }

    #[test]
    fn a_full_buffer_stops_accepting_audio() {
        let buffer = Mutex::new(CaptureBuffer { samples: vec![], pending_level: vec![], channels: 2, limit: 2, limit_reached: false });
        let limit_hits = Arc::new(Mutex::new(0));
        let counter = limit_hits.clone();
        let (level, limit): (LevelCallback, LimitCallback) = (Arc::new(|_| {}), Arc::new(move || *counter.lock().unwrap() += 1));
        push_samples(&buffer, &[0.5f32, 0.5, 0.5, 0.5], |s| s, &level, &limit, 100);
        push_samples(&buffer, &[0.5f32, 0.5], |s| s, &level, &limit, 100);
        assert_eq!(buffer.lock().unwrap().samples.len(), 2);
        assert_eq!(*limit_hits.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn a_missing_model_is_reported() {
        let error = WhisperSpeech::default().transcribe(vec![0.0; 10], PathBuf::from("no-existe.bin"), CancellationToken::new()).await.unwrap_err();
        assert_eq!(error, SpeechError::ModelMissing);
    }

    #[tokio::test]
    async fn a_corrupt_download_is_discarded() {
        let server = MockServer::start().await;
        Mock::given(path("/ggml-base-q5_1.bin")).respond_with(ResponseTemplate::new(200).set_body_bytes(vec![1, 2, 3])).mount(&server).await;
        let dir = tempfile::tempdir().unwrap();
        let model = find_model("base").unwrap();
        assert!(download_model(model, dir.path(), |_, _| {}, Some(&server.uri())).await.is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn a_verified_download_is_kept() {
        let server = MockServer::start().await;
        let body = b"modelo".to_vec();
        Mock::given(path("/m.bin")).respond_with(ResponseTemplate::new(200).set_body_bytes(body.clone())).mount(&server).await;
        let digest: String = Sha256::digest(&body).iter().map(|byte| format!("{byte:02x}")).collect();
        let digest: &'static str = Box::leak(digest.into_boxed_str());
        let model = SpeechModel { id: "x", display_name: "x", file: "m.bin", size_mb: 1, sha256: digest };
        let dir = tempfile::tempdir().unwrap();
        download_model(&model, dir.path(), |_, _| {}, Some(&server.uri())).await.unwrap();
        assert!(dir.path().join("m.bin").exists());
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
        let first = cache.get_or_load(Path::new("small.bin"), now, || Ok::<_, ()>(1)).unwrap();
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
        assert_eq!(cache.get_or_load(Path::new("m.bin"), now, || Err("dañado")).unwrap_err(), "dañado");
        assert_eq!(*cache.get_or_load(Path::new("m.bin"), now, || Ok::<_, &str>(1)).unwrap(), 1);
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
        cache.get_or_load(Path::new("m.bin"), start, || Ok::<_, ()>(1)).unwrap();
        cache.touch(start + Duration::from_secs(500));
        assert!(!cache.release_if_idle(start + idle, idle));
    }

    #[test]
    fn a_model_in_use_survives_its_release() {
        let cache = ModelCache::default();
        let (start, idle) = (Instant::now(), Duration::from_secs(1));
        let in_use = cache.get_or_load(Path::new("m.bin"), start, || Ok::<_, ()>(9)).unwrap();
        assert!(cache.release_if_idle(start + idle, idle));
        assert_eq!(*in_use, 9);
    }

    #[test]
    fn the_catalog_has_the_default_model() {
        assert!(find_model(crate::services::settings::DEFAULT_SPEECH_MODEL).is_some());
    }
}
