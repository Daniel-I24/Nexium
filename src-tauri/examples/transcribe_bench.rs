//! Mide Whisper con audio real: latencia, precisión (WER) y memoria.
//! Uso: cargo run --example transcribe_bench -- <modelo.bin> <carpeta con manifest.json> <hilos> <full|auto> <resultados.jsonl>
//!
//! Con "auto", el codificador procesa solo la duración real del audio en lugar de la ventana de 30 s.
//! Cada clip se guarda al terminar: al relanzar, se omiten los que ya están en el archivo.
//! Se detiene con el código 3 si la memoria libre del equipo baja de 2 GB.
use std::{
    collections::HashSet,
    fs::OpenOptions,
    io::Write,
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

const MIN_FREE_MB: u64 = 2048;
const STOPPED_FOR_MEMORY: i32 = 3;

#[derive(Deserialize)]
struct Clip {
    id: String,
    file: String,
    reference: String,
}

#[derive(Serialize, Deserialize)]
struct Measurement {
    id: String,
    seconds: f64,
    elapsed: f64,
    errors: usize,
    words: usize,
    load_ms: u128,
    rss_peak_mb: u64,
    min_free_mb: u64,
    text: String,
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

/// 1500 unidades equivalen a 30 s; se añade un margen para no cortar el final.
fn audio_context(samples: usize) -> i32 {
    i32::try_from(samples / 320 + 128).unwrap_or(1500).clamp(256, 1500)
}

/// Vigila la memoria durante la transcripción, no solo entre clips.
fn watch_memory(peak_rss: Arc<AtomicU64>, min_free: Arc<AtomicU64>) {
    let pid: Pid = sysinfo::get_current_pid().expect("pid");
    let mut sys = System::new().expect("sysinfo");
    std::thread::spawn(move || loop {
        sys.refresh_memory();
        sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[pid]), true, ProcessRefreshKind::nothing().with_memory());
        let free = sys.available_memory() / 1_048_576;
        min_free.fetch_min(free, Ordering::Relaxed);
        if let Some(process) = sys.process(pid) {
            peak_rss.fetch_max(process.memory() / 1_048_576, Ordering::Relaxed);
        }
        if free < MIN_FREE_MB {
            println!("DETENIDO: memoria libre {free} MB, por debajo de {MIN_FREE_MB} MB. Los clips terminados están guardados.");
            std::process::exit(STOPPED_FOR_MEMORY);
        }
        std::thread::sleep(Duration::from_millis(100));
    });
}

fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    let rank = (fraction * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

fn summarize(results: &Path, expected: usize) {
    let content = std::fs::read_to_string(results).unwrap_or_default();
    let rows: Vec<Measurement> = content.lines().map(|line| serde_json::from_str(line).expect("resultado")).collect();
    if rows.is_empty() {
        return;
    }
    let mut times: Vec<f64> = rows.iter().map(|row| row.elapsed).collect();
    times.sort_by(f64::total_cmp);
    let (errors, words) = rows.iter().fold((0, 0), |(e, w), row| (e + row.errors, w + row.words));
    let audio: f64 = rows.iter().map(|row| row.seconds).sum();
    println!(
        "TOTAL clips={}/{expected} audio={audio:.0}s wer={:.2}% latencia_media={:.2}s mediana={:.2}s p95={:.2}s max={:.2}s rtf={:.3} carga_ms={} rss_pico_mb={} libre_min_mb={}",
        rows.len(),
        100.0 * errors as f64 / words as f64,
        times.iter().sum::<f64>() / times.len() as f64,
        percentile(&times, 0.5),
        percentile(&times, 0.95),
        times[times.len() - 1],
        times.iter().sum::<f64>() / audio,
        rows.iter().map(|row| row.load_ms).max().unwrap_or(0),
        rows.iter().map(|row| row.rss_peak_mb).max().unwrap_or(0),
        rows.iter().map(|row| row.min_free_mb).min().unwrap_or(0),
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (model, dir) = (&args[1], Path::new(&args[2]));
    let threads: i32 = args[3].parse().expect("hilos");
    let auto_context = args[4] == "auto";
    let results = Path::new(&args[5]);
    let clips: Vec<Clip> = serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).expect("manifest")).expect("json");

    let done: HashSet<String> = std::fs::read_to_string(results)
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str::<Measurement>(line).expect("resultado").id)
        .collect();
    if clips.iter().all(|clip| done.contains(&clip.id)) {
        summarize(results, clips.len());
        return;
    }

    let peak_rss = Arc::new(AtomicU64::new(0));
    let min_free = Arc::new(AtomicU64::new(u64::MAX));
    watch_memory(peak_rss.clone(), min_free.clone());

    let load = Instant::now();
    let ctx = WhisperContext::new_with_params(model, WhisperContextParameters::default()).expect("modelo");
    let load_ms = load.elapsed().as_millis();
    println!("modelo={model} hilos={threads} auto={auto_context} carga_ms={load_ms} ya_medidos={}", done.len());

    let mut output = OpenOptions::new().create(true).append(true).open(results).expect("resultados");
    for clip in clips.iter().filter(|clip| !done.contains(&clip.id)) {
        let audio = read_wav(&dir.join(&clip.file));
        // La app crea un estado por transcripción, así que forma parte de su latencia.
        let start = Instant::now();
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
        state.full(params, &audio).expect("transcripción");
        let elapsed = start.elapsed().as_secs_f64();
        let text: String = state.as_iter().map(|s| s.to_string()).collect();
        let (hypothesis, reference) = (normalize(&text), normalize(&clip.reference));
        let row = Measurement {
            id: clip.id.clone(),
            seconds: audio.len() as f64 / 16000.0,
            elapsed,
            errors: edit_distance(&reference, &hypothesis),
            words: reference.len(),
            load_ms,
            rss_peak_mb: peak_rss.load(Ordering::Relaxed),
            min_free_mb: min_free.load(Ordering::Relaxed),
            text: text.trim().to_owned(),
        };
        writeln!(output, "{}", serde_json::to_string(&row).expect("json")).expect("escritura");
        output.flush().expect("escritura");
        println!("{} {:.1}s -> {elapsed:.2}s | {}", row.id, row.seconds, row.text);
    }
    summarize(results, clips.len());
}
