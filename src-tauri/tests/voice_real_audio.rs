//! Transcripción de una grabación real a 48 kHz y en estéreo, por la misma cadena que la app.
//! Necesita un modelo de Whisper; el tiempo solo es significativo en release:
//!
//!   NEXIUM_WHISPER_MODEL=ruta/ggml-small-q5_1.bin cargo test --release --test voice_real_audio -- --ignored

use std::{path::PathBuf, time::Instant};

use nexium_lib::{
    core::rules,
    models::{Intent, SystemMetric},
    providers::speech::{prepare, resample, SpeechProvider, WhisperSpeech},
};
use tokio_util::sync::CancellationToken;

fn stereo_48k_as_mono() -> (u32, Vec<f32>) {
    let bytes = include_bytes!("audio/bateria-48k-estereo.wav");
    let data = bytes
        .windows(4)
        .position(|w| w == b"data")
        .expect("chunk data")
        + 8;
    let samples: Vec<f32> = bytes[data..]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|frame| {
            let left = f32::from(i16::from_le_bytes([frame[0], frame[1]]));
            let right = f32::from(i16::from_le_bytes([frame[2], frame[3]]));
            (left + right) / 2.0 / 32768.0
        })
        .collect();
    (48_000, samples)
}

#[tokio::test]
#[ignore = "necesita un modelo de Whisper descargado"]
async fn a_short_order_recorded_at_48k_stereo_is_transcribed_quickly_and_resolved_locally() {
    let model = PathBuf::from(std::env::var("NEXIUM_WHISPER_MODEL").expect("NEXIUM_WHISPER_MODEL"));
    let speech = WhisperSpeech::default();
    speech.preload(&model).expect("modelo");
    let (rate, mono) = stereo_48k_as_mono();
    let audio = prepare(&resample(&mono, rate));
    let started = Instant::now();
    let transcript = speech
        .transcribe(audio, model, CancellationToken::new())
        .await
        .expect("transcripción");
    let elapsed = started.elapsed();
    assert_eq!(
        rules::match_local(&transcript.text),
        Some(Intent::System(SystemMetric::Battery)),
        "{}",
        transcript.text
    );
    if !cfg!(debug_assertions) {
        assert!(elapsed.as_secs_f32() < 2.0, "{elapsed:?}");
    }
}
