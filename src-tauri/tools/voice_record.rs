//! Graba frases con el mismo flujo que la app (micrófono automático o el elegido, mezcla a un canal y paso a
//! 16 kHz) y las guarda como WAV para medirlas con `voice_params_bench`. Solo para diagnóstico: el audio
//! se guarda fuera del repositorio y se borra al terminar la medición.
//!
//!   cargo run --release --example voice_record -- <carpeta> [micrófono]
use std::{
    io::Write,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

use cpal::traits::{DeviceTrait, HostTrait};
use nexium_lib::providers::speech::{self, SAMPLE_RATE};

const PHRASES: [(&str, &str); 4] = [
    ("bateria", "¿Cuánta batería tengo?"),
    ("ram", "¿Cuánta RAM estoy usando?"),
    ("hora", "¿Qué hora es?"),
    ("wifi", "¿A qué red wifi estoy conectado?"),
];
const SECONDS: u64 = 5;

fn write_wav(path: &Path, samples: &[f32]) {
    let data: Vec<u8> = samples
        .iter()
        .flat_map(|sample| ((sample.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())
        .collect();
    let mut file = std::fs::File::create(path).expect("wav");
    let size = u32::try_from(data.len()).expect("tamaño");
    file.write_all(b"RIFF").unwrap();
    file.write_all(&(36 + size).to_le_bytes()).unwrap();
    file.write_all(b"WAVEfmt ").unwrap();
    file.write_all(&16u32.to_le_bytes()).unwrap();
    file.write_all(&1u16.to_le_bytes()).unwrap();
    file.write_all(&1u16.to_le_bytes()).unwrap();
    file.write_all(&SAMPLE_RATE.to_le_bytes()).unwrap();
    file.write_all(&(SAMPLE_RATE * 2).to_le_bytes()).unwrap();
    file.write_all(&2u16.to_le_bytes()).unwrap();
    file.write_all(&16u16.to_le_bytes()).unwrap();
    file.write_all(b"data").unwrap();
    file.write_all(&size.to_le_bytes()).unwrap();
    file.write_all(&data).unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let folder = Path::new(&args[1]);
    let preferred = args.get(2).cloned();
    std::fs::create_dir_all(folder).expect("carpeta");
    let device = speech::active_input(preferred.as_deref()).unwrap_or_default();
    let host = cpal::default_host();
    let config = host
        .input_devices()
        .ok()
        .and_then(|mut devices| {
            devices.find(|candidate| {
                candidate
                    .description()
                    .is_ok_and(|description| description.name() == device)
            })
        })
        .and_then(|found| found.default_input_config().ok());
    let format = config.map_or_else(
        || "desconocido".to_owned(),
        |config| {
            format!(
                "{} Hz, {} canales, {:?}",
                config.sample_rate(),
                config.channels(),
                config.sample_format()
            )
        },
    );
    println!("Micrófono: {device}\nFormato: {format}\n");
    let mut manifest = Vec::new();
    for (id, phrase) in PHRASES {
        println!("Prepárate para decir: «{phrase}»");
        for count in (1..=3).rev() {
            println!("  {count}…");
            std::thread::sleep(Duration::from_secs(1));
        }
        let recording =
            speech::start_recording(preferred.clone(), Arc::new(|_| {}), Arc::new(|| {}))
                .expect("micrófono");
        let started = Instant::now();
        println!(">>> HABLA AHORA: «{phrase}» (grabando {SECONDS} s)");
        std::thread::sleep(Duration::from_secs(SECONDS));
        let samples = recording.finish();
        let peak = samples.iter().fold(0.0f32, |p, s| p.max(s.abs()));
        let file = format!("{id}.wav");
        write_wav(&folder.join(&file), &samples);
        println!(
            "    Grabado: {:.1} s, pico {:.3} ({} ms)\n",
            samples.len() as f32 / SAMPLE_RATE as f32,
            peak,
            started.elapsed().as_millis()
        );
        manifest.push(serde_json::json!({ "id": id, "file": file, "reference": phrase }));
    }
    std::fs::write(
        folder.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
    std::fs::write(
        folder.join("dispositivo.txt"),
        format!("{device}\n{format}\n"),
    )
    .unwrap();
    println!("Listo. Las grabaciones están en {}", folder.display());
}
