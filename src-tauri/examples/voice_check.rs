//! Diagnóstico de voz con el mismo código que usa Nexium. El audio no se guarda ni sale del equipo.
//!
//!   cargo run --example voice_check                  Solo lista los dispositivos: no graba.
//!   cargo run --example voice_check -- silencio      Transcribe 4 s de silencio sintético, sin micrófono.
//!   cargo run --example voice_check -- vacio         Transcribe un audio sin muestras.
//!   cargo run --example voice_check -- grabar 4      Graba 4 s, muestra el nivel y transcribe con el modelo descargado.

use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use cpal::traits::{DeviceTrait, HostTrait};
use nexium_lib::providers::speech::{self, SpeechProvider, WhisperSpeech};
use tokio_util::sync::CancellationToken;

fn describe(device: &cpal::Device) -> String {
    let name = device.description().map_or_else(
        |_| "(sin nombre)".to_owned(),
        |description| {
            format!(
                "{} [{:?} {:?}]",
                description.name(),
                description.device_type(),
                description.interface_type()
            )
        },
    );
    match device.default_input_config() {
        Ok(config) => format!(
            "{name} | {} Hz, {} canales, {:?}",
            config.sample_rate(),
            config.channels(),
            config.sample_format()
        ),
        Err(error) => format!("{name} | sin configuración de entrada: {error}"),
    }
}

fn model_path() -> PathBuf {
    let base = std::env::var("APPDATA").expect("APPDATA");
    PathBuf::from(base)
        .join("io.github.daniel-i24.nexium")
        .join("models")
        .join(speech::CATALOG[0].file)
}

#[tokio::main]
async fn main() {
    let host = cpal::default_host();
    println!("Sistema de audio: {:?}", host.id());
    match host.default_input_device() {
        Some(device) => println!("Entrada predeterminada: {}", describe(&device)),
        None => println!("Entrada predeterminada: ninguna"),
    }
    if let Ok(devices) = host.input_devices() {
        for device in devices {
            println!("  Entrada disponible: {}", describe(&device));
        }
    }

    println!("Nexium usaría: {:?}", speech::active_input(None));
    let arguments: Vec<String> = std::env::args().collect();
    // Audio sintético, sin micrófono: silencio de 4 s o ninguna muestra.
    if let Some(kind @ ("silencio" | "vacio")) = arguments.get(1).map(String::as_str) {
        let samples = if kind == "silencio" {
            vec![0.0; speech::SAMPLE_RATE as usize * 4]
        } else {
            Vec::new()
        };
        let start = Instant::now();
        let result = WhisperSpeech::default()
            .transcribe(samples, model_path(), CancellationToken::new())
            .await;
        println!("{kind}: {} ms: {result:?}", start.elapsed().as_millis());
        return;
    }
    if arguments.get(1).map(String::as_str) != Some("grabar") {
        return;
    }
    let seconds: u64 = arguments
        .get(2)
        .and_then(|value| value.parse().ok())
        .unwrap_or(4);
    println!("\nGrabando {seconds} s: habla ahora.");
    let recording = speech::start_recording(None, Arc::new(|_| {}), Arc::new(|| {}))
        .unwrap_or_else(|error| panic!("{error}"));
    println!("Grabando de: {}", recording.device());
    std::thread::sleep(Duration::from_secs(seconds));
    let samples = recording.finish();
    let peak = samples
        .iter()
        .fold(0.0f32, |peak, sample| peak.max(sample.abs()));
    println!(
        "Muestras a 16 kHz: {} ({:.2} s) | pico {peak:.4} | nivel {:.3}",
        samples.len(),
        samples.len() as f32 / speech::SAMPLE_RATE as f32,
        speech::level_of(&samples)
    );

    let path = model_path();
    println!("Modelo: {} (existe: {})", path.display(), path.exists());
    let start = Instant::now();
    let result = WhisperSpeech::default()
        .transcribe(samples, path, CancellationToken::new())
        .await;
    println!(
        "Transcripción en {} ms: {result:?}",
        start.elapsed().as_millis()
    );
}
