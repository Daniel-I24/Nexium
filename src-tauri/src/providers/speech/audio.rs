//! Preparación del audio para Whisper: 16 kHz, un canal, sin el silencio de los extremos y a un volumen útil.
//!
//! El micrófono suele grabar a 44,1 o 48 kHz y en estéreo. La mezcla a un canal se hace al capturar
//! (`microphone::push_samples`); aquí se convierte la frecuencia, se recorta y se normaliza.

use super::SAMPLE_RATE;

/// Convierte a 16 kHz, el formato que exige Whisper. Cada muestra nueva es la media de las originales que
/// cubre: un filtro paso bajo sencillo que evita que las frecuencias por encima de 8 kHz se doblen sobre la voz.
pub fn resample(samples: &[f32], from_rate: u32) -> Vec<f32> {
    if from_rate == SAMPLE_RATE || samples.is_empty() {
        return samples.to_vec();
    }
    let ratio = f64::from(from_rate) / f64::from(SAMPLE_RATE);
    let length = (samples.len() as f64 / ratio) as usize;
    let half = (ratio / 2.0).max(0.5);
    (0..length)
        .map(|index| {
            let center = index as f64 * ratio;
            let start = (center - half).floor().max(0.0) as usize;
            let end = ((center + half).ceil() as usize).min(samples.len() - 1);
            let window = &samples[start..=end];
            window.iter().sum::<f32>() / window.len() as f32
        })
        .collect()
}

/// Nivel para el trazo de la barra: raíz de la energía media, escalada a 0–1.
pub fn level_of(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let energy = samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32;
    (energy.sqrt() * 6.0).min(1.0)
}

/// Tramos de 20 ms para medir dónde hay voz.
const FRAME: usize = SAMPLE_RATE as usize / 50;
/// Margen que se conserva antes y después de la voz: no corta la primera ni la última sílaba.
const MARGIN: usize = SAMPLE_RATE as usize * 3 / 10;
/// Un tramo con voz supera esta fracción del tramo más fuerte (y un mínimo absoluto, para el ruido).
const VOICE_RATIO: f32 = 0.08;
const VOICE_FLOOR: f32 = 0.004;

fn rms(frame: &[f32]) -> f32 {
    (frame.iter().map(|sample| sample * sample).sum::<f32>() / frame.len().max(1) as f32).sqrt()
}

/// Quita el silencio del principio y del final. Menos audio es menos trabajo para Whisper, y el silencio
/// largo es donde inventa texto. Si no encuentra voz, devuelve el audio tal cual.
pub fn trim_silence(samples: &[f32]) -> &[f32] {
    let levels: Vec<f32> = samples.chunks(FRAME).map(rms).collect();
    let loudest = levels.iter().copied().fold(0.0f32, f32::max);
    let threshold = (loudest * VOICE_RATIO).max(VOICE_FLOOR);
    let (Some(first), Some(last)) = (
        levels.iter().position(|level| *level >= threshold),
        levels.iter().rposition(|level| *level >= threshold),
    ) else {
        return samples;
    };
    let start = (first * FRAME).saturating_sub(MARGIN);
    let end = ((last + 1) * FRAME + MARGIN).min(samples.len());
    &samples[start..end]
}

/// Pico al que se lleva la voz.
const TARGET_PEAK: f32 = 0.9;
/// Ganancia máxima: un micrófono muy bajo se sube, pero no se convierte el ruido en voz.
const MAX_GAIN: f32 = 8.0;

/// Sube el volumen de una grabación baja hasta un pico útil. Nunca la baja ni la satura.
pub fn normalize(samples: &[f32]) -> Vec<f32> {
    let peak = samples
        .iter()
        .fold(0.0f32, |peak, sample| peak.max(sample.abs()));
    if peak <= f32::EPSILON {
        return samples.to_vec();
    }
    let gain = (TARGET_PEAK / peak).clamp(1.0, MAX_GAIN);
    samples.iter().map(|sample| sample * gain).collect()
}

/// Audio a 16 kHz listo para transcribir: sin silencio en los extremos y a volumen útil.
pub fn prepare(samples: &[f32]) -> Vec<f32> {
    normalize(trim_silence(samples))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(seconds: f32, amplitude: f32) -> Vec<f32> {
        (0..(seconds * SAMPLE_RATE as f32) as usize)
            .map(|index| (index as f32 * 0.07).sin() * amplitude)
            .collect()
    }

    #[test]
    fn resampling_keeps_duration() {
        let one_second = vec![0.5; 48_000];
        let result = resample(&one_second, 48_000);
        assert_eq!(result.len(), 16_000);
        assert!(result.iter().all(|sample| (sample - 0.5).abs() < 1e-6));
        assert_eq!(resample(&vec![0.0; 44_100], 44_100).len(), 16_000);
    }

    #[test]
    fn resampling_at_the_same_rate_changes_nothing() {
        assert_eq!(resample(&[0.1, 0.2], SAMPLE_RATE), vec![0.1, 0.2]);
    }

    #[test]
    fn resampling_removes_what_whisper_cannot_use() {
        // 24 kHz a 48 kHz de muestreo: por encima de lo que cabe en 16 kHz. Sin filtro se doblaría sobre la voz.
        let high: Vec<f32> = (0..48_000)
            .map(|index| if index % 2 == 0 { 0.5 } else { -0.5 })
            .collect();
        let result = resample(&high, 48_000);
        assert!(
            result.iter().all(|sample| sample.abs() < 0.2),
            "{:?}",
            &result[..6]
        );
    }

    #[test]
    fn level_is_bounded() {
        assert_eq!(level_of(&[]), 0.0);
        assert!((0.0..=1.0).contains(&level_of(&[1.0; 100])));
        assert!(level_of(&[0.0; 100]) < 0.01);
    }

    #[test]
    fn silence_at_both_ends_is_removed_with_a_margin() {
        let mut audio = vec![0.0005; SAMPLE_RATE as usize * 2];
        audio.extend(tone(1.0, 0.3));
        audio.extend(vec![0.0005; SAMPLE_RATE as usize * 3]);
        let trimmed = trim_silence(&audio);
        let seconds = trimmed.len() as f32 / SAMPLE_RATE as f32;
        assert!((1.5..1.7).contains(&seconds), "{seconds} s");
    }

    #[test]
    fn audio_without_voice_is_left_as_is() {
        let quiet = vec![0.0; 16_000];
        assert_eq!(trim_silence(&quiet).len(), quiet.len());
    }

    #[test]
    fn a_quiet_recording_is_raised_but_never_clipped_or_lowered() {
        let quiet = normalize(&tone(0.5, 0.05));
        let peak = quiet.iter().fold(0.0f32, |p, s| p.max(s.abs()));
        assert!((0.35..=0.41).contains(&peak), "{peak}");
        let loud = normalize(&tone(0.5, 0.95));
        assert!(loud.iter().all(|sample| sample.abs() <= 0.95));
        let normal = normalize(&tone(0.5, 0.2));
        let peak = normal.iter().fold(0.0f32, |p, s| p.max(s.abs()));
        assert!((0.85..=0.91).contains(&peak), "{peak}");
    }
}
