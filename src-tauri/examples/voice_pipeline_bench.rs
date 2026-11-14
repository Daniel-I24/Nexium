//! Mide una orden por voz completa con el código de la app: carga del modelo, transcripción,
//! interpretación local y respuesta. También mide la misma orden escrita.
//! Uso: cargo run --release --example voice_pipeline_bench -- <modelo.bin> <carpeta con manifest.json>
//!
//! El manifiesto lista clips WAV de 16 kHz, mono y 16 bits, con la frase de referencia de cada uno.
//! Sin servidor de IA: lo que no se resuelve en local se cuenta como "IA" y no se envía a ningún sitio.
use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

use nexium_lib::{
    core::{
        assistant::Assistant,
        handlers::{CommandHandler, SystemHandler, TimeHandler},
        rules,
    },
    database::Database,
    models::Reading,
    providers::{
        ai::{server::NexiumServer, AiStatusTracker},
        capture::{CaptureError, ScreenCaptureProvider},
        speech::{SpeechProvider, WhisperSpeech},
        system::NativeSystemProvider,
    },
    services::{history::HistoryService, settings::SettingsService, usage::UsageService},
};
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

#[derive(Deserialize)]
struct Clip {
    id: String,
    file: String,
    reference: String,
}

struct NoCapture;

#[async_trait::async_trait]
impl ScreenCaptureProvider for NoCapture {
    async fn capture_once(&self) -> Result<Vec<u8>, CaptureError> {
        Err(CaptureError::Failed("sin captura en la medición".into()))
    }
}

fn read_wav(path: &Path) -> Vec<f32> {
    let bytes = std::fs::read(path).expect("wav");
    let data = bytes
        .windows(4)
        .position(|w| w == b"data")
        .expect("chunk data")
        + 8;
    bytes[data..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|sample| f32::from(i16::from_le_bytes(*sample)) / 32768.0)
        .collect()
}

fn assistant() -> Assistant {
    let db = Database::in_memory().expect("base de datos");
    let system = Arc::new(NativeSystemProvider::new(Arc::new(|| {
        Reading::Value(false)
    })));
    let handlers: Vec<Box<dyn CommandHandler>> =
        vec![Box::new(TimeHandler), Box::new(SystemHandler { system })];
    Assistant {
        ai: Arc::new(NexiumServer::new(None, "bench".into())),
        ai_status: AiStatusTracker::default(),
        capture: Arc::new(NoCapture),
        handlers,
        settings: SettingsService::new(db.clone()),
        history: HistoryService::new(db.clone()),
        usage: UsageService::new(db),
    }
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let model = Path::new(&args[1]).to_path_buf();
    let folder = Path::new(&args[2]);
    let clips: Vec<Clip> = serde_json::from_str(
        std::fs::read_to_string(folder.join("manifest.json"))
            .expect("manifest")
            .trim_start_matches('\u{feff}'),
    )
    .expect("manifest válido");
    let speech = WhisperSpeech::default();
    let assistant = assistant();
    let stage: nexium_lib::core::assistant::StageSink = Arc::new(|_| {});

    let started = Instant::now();
    speech.preload(&model).expect("modelo");
    println!("carga_modelo_ms={:.0}", ms(started.elapsed()));

    let (mut voice_totals, mut typed, mut local_hits) = (Vec::new(), Vec::new(), 0);
    for clip in &clips {
        let samples = read_wav(&folder.join(&clip.file));
        let audio_s = samples.len() as f64 / 16_000.0;
        let t0 = Instant::now();
        let text = speech
            .transcribe(samples, model.clone(), CancellationToken::new())
            .await
            .expect("transcripción");
        let transcribe = t0.elapsed();
        let local = rules::match_local(&text).is_some();
        let t1 = Instant::now();
        let response = assistant
            .process(&text, CancellationToken::new(), stage.clone())
            .await;
        let answer = t1.elapsed();
        let t2 = Instant::now();
        let _ = assistant
            .process(&clip.reference, CancellationToken::new(), stage.clone())
            .await;
        typed.push(ms(t2.elapsed()));
        local_hits += usize::from(local);
        if local {
            voice_totals.push(ms(transcribe + answer));
        }
        println!(
            "{:<16} audio={audio_s:.1}s transcripcion={:>5.0}ms respuesta={:>4.0}ms local={} | {} -> {} [{}]",
            clip.id,
            ms(transcribe),
            ms(answer),
            if local { "sí" } else { "no" },
            clip.reference,
            text,
            response.summary
        );
    }
    println!(
        "TOTAL clips={} locales={local_hits} voz_local_mediana_ms={:.0} voz_local_max_ms={:.0} escrita_mediana_ms={:.1} escrita_max_ms={:.1}",
        clips.len(),
        median(&mut voice_totals.clone()),
        voice_totals.iter().copied().fold(0.0, f64::max),
        median(&mut typed.clone()),
        typed.iter().copied().fold(0.0, f64::max),
    );
}
