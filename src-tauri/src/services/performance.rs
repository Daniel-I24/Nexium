//! Tiempos de cada etapa de una solicitud, para medir la velocidad en el equipo de cada persona.
//!
//! Se guardan en `rendimiento.log`, en la carpeta de datos de Nexium: solo duraciones, el tipo de
//! solicitud, si se resolvió en local o con IA y su categoría. Nunca el texto pedido ni la respuesta: todo lo
//! que se escribe son textos fijos del código (`&'static str`), así que el compilador impide guardar otra cosa.
//!
//! Al pasar de `MAX_LOG_BYTES`, el registro se renombra a `rendimiento.1.log` (que sustituye al anterior) y se
//! empieza uno nuevo: en disco nunca hay más de dos archivos ni más de 512 KB.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    time::{Duration, Instant},
};

use chrono::Local;

/// Tamaño máximo de cada archivo del registro.
const MAX_LOG_BYTES: u64 = 256 * 1024;

/// Etapas de una solicitud, en orden, con lo que tardó cada una.
#[derive(Debug, Clone)]
pub struct Timings {
    kind: &'static str,
    started: Instant,
    last: Instant,
    stages: Vec<(&'static str, Duration)>,
}

impl Timings {
    /// `kind` dice cómo llegó la solicitud: "voz", "texto" o "captura".
    pub fn new(kind: &'static str) -> Self {
        let now = Instant::now();
        Self {
            kind,
            started: now,
            last: now,
            stages: Vec::new(),
        }
    }

    /// Cierra una etapa con el tiempo transcurrido desde la anterior.
    pub fn mark(&mut self, stage: &'static str) {
        let now = Instant::now();
        self.stages.push((stage, now - self.last));
        self.last = now;
    }

    /// Una etapa medida en otro sitio (por ejemplo, dentro de la transcripción).
    pub fn add(&mut self, stage: &'static str, duration: Duration) {
        self.stages.push((stage, duration));
        self.last = Instant::now();
    }

    pub fn stage(&self, name: &str) -> Option<Duration> {
        self.stages
            .iter()
            .find(|(stage, _)| *stage == name)
            .map(|(_, duration)| *duration)
    }

    pub fn total(&self) -> Duration {
        self.last - self.started
    }

    /// Una línea legible: `voz · local · system | fin de grabación 12 ms · … · total 1180 ms`.
    pub fn line(&self, resolved_by: &'static str, category: &'static str) -> String {
        let stages = self
            .stages
            .iter()
            .map(|(stage, duration)| format!("{stage} {} ms", duration.as_millis()))
            .collect::<Vec<_>>()
            .join(" · ");
        format!(
            "{} · {resolved_by} · {category} | {stages} · total {} ms",
            self.kind,
            self.total().as_millis()
        )
    }
}

/// Registro de tiempos en disco. Sin ruta (en pruebas y mediciones), no escribe nada.
#[derive(Debug, Clone, Default)]
pub struct PerformanceLog {
    path: Option<PathBuf>,
}

impl PerformanceLog {
    pub fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    /// Añade una línea con la fecha. Un fallo al escribir no afecta a la solicitud.
    pub fn record(&self, timings: &Timings, resolved_by: &'static str, category: &'static str) {
        let line = format!(
            "{} {}\n",
            Local::now().format("%Y-%m-%d %H:%M:%S"),
            timings.line(resolved_by, category)
        );
        #[cfg(debug_assertions)]
        eprint!("[rendimiento] {line}");
        let Some(path) = &self.path else { return };
        if fs::metadata(path).is_ok_and(|meta| meta.len() + line.len() as u64 > MAX_LOG_BYTES) {
            let _ = fs::rename(path, path.with_extension("1.log"));
        }
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = file.write_all(line.as_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_stage_is_measured_in_order_and_adds_up() {
        let mut timings = Timings::new("voz");
        timings.add("carga del modelo", Duration::from_millis(5));
        timings.mark("transcripción");
        let line = timings.line("local", "system");
        assert!(line.starts_with("voz · local · system | carga del modelo 5 ms · transcripción "));
        assert!(line.contains(" · total "));
        assert_eq!(
            timings.stage("carga del modelo"),
            Some(Duration::from_millis(5))
        );
    }

    #[test]
    fn a_full_log_rotates_into_one_older_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rendimiento.log");
        let rotated = dir.path().join("rendimiento.1.log");
        fs::write(&rotated, "el más antiguo\n").unwrap();
        fs::write(&path, "x".repeat(MAX_LOG_BYTES as usize - 10) + "\n").unwrap();
        let log = PerformanceLog::new(path.clone());
        log.record(&Timings::new("texto"), "local", "time");
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.starts_with(&Local::now().format("%Y-%m-%d").to_string()));
        assert!(content.contains("texto · local · time |"));
        // El anterior pasa a ser el antiguo, y el más antiguo desaparece.
        assert!(fs::read_to_string(&rotated).unwrap().starts_with("xxx"));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn the_log_never_grows_beyond_two_files_of_the_limit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rendimiento.log");
        let log = PerformanceLog::new(path.clone());
        let timings = Timings::new("voz");
        for _ in 0..10_000 {
            log.record(&timings, "local", "system");
        }
        for file in fs::read_dir(dir.path()).unwrap() {
            assert!(file.unwrap().metadata().unwrap().len() <= MAX_LOG_BYTES);
        }
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }
}
