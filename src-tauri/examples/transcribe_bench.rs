//! Mide Whisper con audio real: tiempo, precisión (WER) y memoria.
//! Uso: cargo run --release --example transcribe_bench -- <modelo.bin> <carpeta con manifest.json> [hilos] [auto]
//! Con "auto", el codificador procesa solo la duración real del audio en lugar de la ventana completa de 30 s.
use std::{path::Path, time::Instant};

use serde::Deserialize;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

#[derive(Deserialize)]
struct Clip {
    id: String,
    file: String,
    reference: String,
}

fn normalize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

fn edit_distance(a: &[String], b: &[String]) -> usize {
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, wa) in a.iter().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, wb) in b.iter().enumerate() {
            let next = (row[j + 1] + 1).min(row[j] + 1).min(prev + usize::from(wa != wb));
            prev = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}

fn read_wav(path: &Path) -> Vec<f32> {
    let bytes = std::fs::read(path).expect("wav");
    let data = bytes.windows(4).position(|w| w == b"data").expect("chunk data") + 8;
    bytes[data..].chunks_exact(2).map(|s| f32::from(i16::from_le_bytes([s[0], s[1]])) / 32768.0).collect()
}

fn rss_mb(sys: &mut System, pid: Pid) -> f64 {
    sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[pid]), true, ProcessRefreshKind::nothing().with_memory());
    sys.process(pid).map_or(0.0, |p| p.memory() as f64 / 1_048_576.0)
}

/// 1500 unidades equivalen a 30 s; se añade un margen para no cortar el final.
fn audio_context(samples: usize) -> i32 {
    i32::try_from(samples / 320 + 128).unwrap_or(1500).clamp(256, 1500)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (model, dir) = (&args[1], Path::new(&args[2]));
    let threads: i32 = args.get(3).and_then(|t| t.parse().ok()).unwrap_or(4);
    let auto_context = args.get(4).is_some_and(|value| value == "auto");
    let clips: Vec<Clip> = serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).expect("manifest")).expect("json");
    let pid = sysinfo::get_current_pid().expect("pid");
    let mut sys = System::new().expect("sysinfo");
    let base_rss = rss_mb(&mut sys, pid);

    let load = Instant::now();
    let ctx = WhisperContext::new_with_params(model, WhisperContextParameters::default()).expect("modelo");
    println!("modelo={model} hilos={threads} carga_ms={} rss_base_mb={base_rss:.0}", load.elapsed().as_millis());

    let (mut total_audio, mut total_time, mut errors, mut words, mut peak) = (0.0, 0.0, 0usize, 0usize, 0.0_f64);
    for clip in &clips {
        let audio = read_wav(&dir.join(&clip.file));
        let seconds = audio.len() as f64 / 16000.0;
        let mut state = ctx.create_state().expect("estado");
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some("es"));
        params.set_n_threads(threads);
        if auto_context {
            params.set_audio_ctx(audio_context(audio.len()));
        }
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_print_special(false);
        let start = Instant::now();
        state.full(params, &audio).expect("transcripción");
        let elapsed = start.elapsed().as_secs_f64();
        peak = peak.max(rss_mb(&mut sys, pid));
        let text: String = state.as_iter().map(|s| s.to_string()).collect();
        let (hyp, reference) = (normalize(&text), normalize(&clip.reference));
        let distance = edit_distance(&reference, &hyp);
        total_audio += seconds;
        total_time += elapsed;
        errors += distance;
        words += reference.len();
        println!("{} {seconds:.1}s -> {elapsed:.2}s wer={:.1}% | {}", clip.id, 100.0 * distance as f64 / reference.len() as f64, text.trim());
    }
    println!(
        "TOTAL clips={} audio={total_audio:.0}s tiempo={total_time:.1}s rtf={:.3} wer={:.2}% rss_pico_mb={peak:.0}",
        clips.len(), total_time / total_audio, 100.0 * errors as f64 / words as f64
    );
}
