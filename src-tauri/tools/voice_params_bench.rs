//! Mide la transcripción de clips de 16 kHz variando la configuración de Whisper, para diagnosticar la voz.
//!
//!   cargo run --release --example voice_params_bench -- <modelo.bin> <carpeta con manifest.json> [opciones]
//!
//! Opciones (por defecto, las de la app): `crudo` (sin recorte ni normalización), `hilos=N`,
//! `temperatura=0.2` (reintentos de temperatura), `sin-prompt`, `ventana-completa` (sin audio_ctx),
//! `idioma-auto` y `repeticiones=N`.

#[path = "wav.rs"]
mod wav;
use std::{path::Path, time::Instant};
use wav::read_wav;

use nexium_lib::{
    core::rules,
    providers::speech::{audio_context, prepare, whisper_threads, VOCABULARY_PROMPT},
};
use serde::Deserialize;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

#[derive(Deserialize)]
struct Clip {
    id: String,
    file: String,
    reference: String,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let model = &args[1];
    let folder = Path::new(&args[2]);
    let options = &args[3..];
    let has = |name: &str| options.iter().any(|option| option == name);
    let value = |name: &str| {
        options
            .iter()
            .find_map(|option| option.strip_prefix(&format!("{name}=")))
            .map(str::to_owned)
    };
    let threads: i32 = value("hilos").map_or_else(whisper_threads, |n| n.parse().unwrap());
    let temperature_inc: f32 = value("temperatura").map_or(0.0, |t| t.parse().unwrap());
    let repetitions: usize = value("repeticiones").map_or(2, |n| n.parse().unwrap());
    let clips: Vec<Clip> = serde_json::from_str(
        std::fs::read_to_string(folder.join("manifest.json"))
            .expect("manifest")
            .trim_start_matches('\u{feff}'),
    )
    .expect("manifest válido");
    let started = Instant::now();
    let context = WhisperContext::new_with_params(model, WhisperContextParameters::default())
        .expect("modelo");
    println!(
        "opciones={options:?} hilos={threads} carga_ms={}",
        started.elapsed().as_millis()
    );
    let mut times = Vec::new();
    for clip in &clips {
        let raw = read_wav(&folder.join(&clip.file));
        let audio = if has("crudo") {
            raw.clone()
        } else {
            prepare(&raw)
        };
        let mut text = String::new();
        let mut best = f64::MAX;
        for _ in 0..repetitions {
            let mut state = context.create_state().expect("estado");
            let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            params.set_language(Some(if has("idioma-auto") { "auto" } else { "es" }));
            if !has("ventana-completa") {
                params.set_audio_ctx(audio_context(audio.len()));
            }
            params.set_n_threads(threads);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);
            params.set_print_special(false);
            if !has("sin-prompt") {
                params.set_initial_prompt(VOCABULARY_PROMPT);
            }
            params.set_temperature_inc(temperature_inc);
            params.set_no_context(true);
            params.set_single_segment(true);
            let t = Instant::now();
            state.full(params, &audio).expect("transcripción");
            best = best.min(t.elapsed().as_secs_f64() * 1000.0);
            text = state
                .as_iter()
                .map(|segment| segment.to_string())
                .collect::<String>()
                .trim()
                .to_owned();
        }
        times.push(best);
        println!(
            "{:<8} audio={:.1}s transcripcion={best:>6.0}ms local={} | {} -> {}",
            clip.id,
            audio.len() as f64 / 16_000.0,
            if rules::match_local(&text).is_some() {
                "sí"
            } else {
                "no"
            },
            clip.reference,
            text
        );
    }
    times.sort_by(f64::total_cmp);
    println!(
        "TOTAL mediana_ms={:.0} max_ms={:.0}",
        times[times.len() / 2],
        times[times.len() - 1]
    );
}
