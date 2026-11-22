//! Lectura de los WAV de 16 bits que usan las herramientas de medición. Se incluye con `#[path]`.

use std::path::Path;

/// Las muestras del bloque `data`, de -1 a 1.
pub fn read_wav(path: &Path) -> Vec<f32> {
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
