//! Micrófono: elección del dispositivo, grabación con cpal y nivel de la voz.

use std::{
    sync::{mpsc, Arc, Mutex},
    thread::JoinHandle,
    time::Duration,
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::Serialize;

use super::{SpeechError, MAX_RECORDING_SECONDS, SAMPLE_RATE};

/// Convierte a 16 kHz, el formato que exige Whisper.
pub fn resample(samples: &[f32], from_rate: u32) -> Vec<f32> {
    if from_rate == SAMPLE_RATE || samples.is_empty() {
        return samples.to_vec();
    }
    let ratio = f64::from(from_rate) / f64::from(SAMPLE_RATE);
    let length = (samples.len() as f64 / ratio) as usize;
    (0..length)
        .map(|index| {
            let position = index as f64 * ratio;
            let base = position as usize;
            let next = samples.get(base + 1).copied().unwrap_or(samples[base]);
            let fraction = (position - base as f64) as f32;
            samples[base] + (next - samples[base]) * fraction
        })
        .collect()
}

pub fn level_of(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let energy = samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32;
    (energy.sqrt() * 6.0).min(1.0)
}

struct CaptureBuffer {
    samples: Vec<f32>,
    pending_level: Vec<f32>,
    channels: usize,
    limit: usize,
    limit_reached: bool,
}

/// Grabación en curso. Soltarla sin llamar a `finish` cierra el micrófono y descarta el audio.
pub struct Recording {
    stop: Option<mpsc::Sender<()>>,
    thread: Option<JoinHandle<()>>,
    buffer: Arc<Mutex<CaptureBuffer>>,
    sample_rate: u32,
    device: String,
}

impl Recording {
    /// Nombre del dispositivo que se está grabando, para explicar un fallo.
    pub fn device(&self) -> &str {
        &self.device
    }

    pub fn finish(mut self) -> Vec<f32> {
        self.close();
        let samples = self
            .buffer
            .lock()
            .map(|mut buffer| std::mem::take(&mut buffer.samples))
            .unwrap_or_default();
        resample(&samples, self.sample_rate)
    }

    fn close(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        self.close();
    }
}

/// Si la entrada predeterminada no es un micrófono (por ejemplo, "Stereo Mix", que graba lo que suena en el
/// equipo), se usa el primer micrófono disponible. Si no hay ninguno reconocible, se respeta la predeterminada.
pub fn pick_input<D>(
    default: Option<D>,
    inputs: impl IntoIterator<Item = D>,
    is_microphone: impl Fn(&D) -> bool,
) -> Option<D> {
    if default.as_ref().is_some_and(&is_microphone) {
        return default;
    }
    inputs
        .into_iter()
        .find(|device| is_microphone(device))
        .or(default)
}

/// El micrófono elegido en Ajustes, si sigue conectado; si no, la elección automática.
pub fn choose_input<D>(
    preferred: Option<&str>,
    default: Option<D>,
    inputs: Vec<D>,
    name_of: impl Fn(&D) -> String,
    is_microphone: impl Fn(&D) -> bool,
) -> Option<D> {
    if let Some(wanted) = preferred {
        if let Some(index) = inputs.iter().position(|device| name_of(device) == wanted) {
            return inputs.into_iter().nth(index);
        }
    }
    pick_input(default, inputs, is_microphone)
}

/// Un dispositivo de entrada tal como se muestra en Ajustes.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InputDevice {
    pub name: String,
    pub is_microphone: bool,
    pub is_default: bool,
}

pub fn input_devices() -> Vec<InputDevice> {
    let host = cpal::default_host();
    let default = host
        .default_input_device()
        .map(|device| device_name(&device));
    host.input_devices()
        .map(|devices| {
            devices
                .map(|device| {
                    let name = device_name(&device);
                    InputDevice {
                        is_microphone: is_microphone(&device),
                        is_default: default.as_deref() == Some(name.as_str()),
                        name,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn resolve_input(preferred: Option<&str>) -> Option<cpal::Device> {
    let host = cpal::default_host();
    let inputs = host
        .input_devices()
        .map(Iterator::collect::<Vec<_>>)
        .unwrap_or_default();
    choose_input(
        preferred,
        host.default_input_device(),
        inputs,
        device_name,
        is_microphone,
    )
}

/// Nombre del dispositivo que se usaría ahora mismo con la preferencia indicada.
pub fn active_input(preferred: Option<&str>) -> Option<String> {
    resolve_input(preferred).map(|device| device_name(&device))
}

fn is_microphone(device: &cpal::Device) -> bool {
    device.description().is_ok_and(|description| {
        matches!(
            description.device_type(),
            cpal::DeviceType::Microphone | cpal::DeviceType::Headset | cpal::DeviceType::Handset
        )
    })
}

fn device_name(device: &cpal::Device) -> String {
    device.description().map_or_else(
        |_| "micrófono".to_owned(),
        |description| description.name().to_owned(),
    )
}

type LevelCallback = Arc<dyn Fn(f32) + Send + Sync>;
type LimitCallback = Arc<dyn Fn() + Send + Sync>;

fn no_microphone() -> SpeechError {
    SpeechError::NoMicrophone("No puedo usar el micrófono: no hay ninguno disponible o el sistema le ha negado el permiso a Nexium. Revísalo en la configuración de privacidad.".into())
}

fn push_samples<T: Copy>(
    buffer: &Mutex<CaptureBuffer>,
    data: &[T],
    convert: impl Fn(T) -> f32,
    on_level: &LevelCallback,
    on_limit: &LimitCallback,
    chunk: usize,
) {
    let Ok(mut buffer) = buffer.lock() else {
        return;
    };
    if buffer.limit_reached {
        return;
    }
    let channels = buffer.channels.max(1);
    for frame in data.chunks(channels) {
        let mono = frame.iter().map(|sample| convert(*sample)).sum::<f32>() / channels as f32;
        buffer.samples.push(mono);
        buffer.pending_level.push(mono);
    }
    if buffer.pending_level.len() >= chunk {
        on_level(level_of(&buffer.pending_level));
        buffer.pending_level.clear();
    }
    if buffer.samples.len() >= buffer.limit {
        buffer.limit_reached = true;
        on_limit();
    }
}

/// Abre el micrófono en un hilo propio: el flujo de cpal no se puede mover entre hilos en todos los sistemas.
pub fn start_recording(
    preferred: Option<String>,
    on_level: LevelCallback,
    on_limit: LimitCallback,
) -> Result<Recording, SpeechError> {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let (ready_tx, ready_rx) =
        mpsc::channel::<Result<(Arc<Mutex<CaptureBuffer>>, u32, String), SpeechError>>();
    let thread = std::thread::spawn(move || {
        let opened = (|| {
            let device = resolve_input(preferred.as_deref()).ok_or_else(no_microphone)?;
            let name = device_name(&device);
            let supported = device.default_input_config().map_err(|_| no_microphone())?;
            let format = supported.sample_format();
            let config: cpal::StreamConfig = supported.into();
            let rate: u32 = config.sample_rate;
            let buffer = Arc::new(Mutex::new(CaptureBuffer {
                samples: Vec::new(),
                pending_level: Vec::new(),
                channels: usize::from(config.channels),
                limit: rate as usize * MAX_RECORDING_SECONDS,
                limit_reached: false,
            }));
            let chunk = rate as usize / 20;
            let target = buffer.clone();
            let (level, limit) = (on_level.clone(), on_limit.clone());
            let on_error = |_| {};
            let stream = match format {
                cpal::SampleFormat::F32 => device.build_input_stream(
                    config,
                    move |data: &[f32], _: &_| {
                        push_samples(&target, data, |s| s, &level, &limit, chunk)
                    },
                    on_error,
                    None,
                ),
                cpal::SampleFormat::I16 => device.build_input_stream(
                    config,
                    move |data: &[i16], _: &_| {
                        push_samples(
                            &target,
                            data,
                            |s| f32::from(s) / 32768.0,
                            &level,
                            &limit,
                            chunk,
                        )
                    },
                    on_error,
                    None,
                ),
                cpal::SampleFormat::U16 => device.build_input_stream(
                    config,
                    move |data: &[u16], _: &_| {
                        push_samples(
                            &target,
                            data,
                            |s| (f32::from(s) - 32768.0) / 32768.0,
                            &level,
                            &limit,
                            chunk,
                        )
                    },
                    on_error,
                    None,
                ),
                _ => {
                    return Err(SpeechError::NoMicrophone(
                        "El micrófono usa un formato de audio que Nexium no admite.".into(),
                    ))
                }
            }
            .map_err(|_| no_microphone())?;
            stream.play().map_err(|_| no_microphone())?;
            Ok((stream, buffer, rate, name))
        })();
        match opened {
            Ok((stream, buffer, rate, name)) => {
                let _ = ready_tx.send(Ok((buffer, rate, name)));
                let _ = stop_rx.recv();
                drop(stream);
            }
            Err(error) => {
                let _ = ready_tx.send(Err(error));
            }
        }
    });
    let (buffer, sample_rate, device) = ready_rx
        .recv_timeout(Duration::from_secs(10))
        .map_err(|_| no_microphone())??;
    Ok(Recording {
        stop: Some(stop_tx),
        thread: Some(thread),
        buffer,
        sample_rate,
        device,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chosen_microphone_wins_while_it_is_connected() {
        let is_mic = |device: &&str| device.starts_with("Micrófono");
        let name = |device: &&str| (*device).to_owned();
        let inputs = vec![
            "Stereo Mix",
            "Micrófono integrado",
            "Micrófono de la cámara",
        ];
        assert_eq!(
            choose_input(
                Some("Micrófono de la cámara"),
                Some("Stereo Mix"),
                inputs.clone(),
                name,
                is_mic
            ),
            Some("Micrófono de la cámara")
        );
        assert_eq!(
            choose_input(
                Some("Stereo Mix"),
                Some("Stereo Mix"),
                inputs.clone(),
                name,
                is_mic
            ),
            Some("Stereo Mix")
        );
        assert_eq!(
            choose_input(
                Some("Auriculares desconectados"),
                Some("Stereo Mix"),
                inputs.clone(),
                name,
                is_mic
            ),
            Some("Micrófono integrado")
        );
        assert_eq!(
            choose_input(None, Some("Stereo Mix"), inputs, name, is_mic),
            Some("Micrófono integrado")
        );
    }

    #[test]
    fn a_microphone_is_preferred_over_a_default_that_is_not_one() {
        let is_mic = |device: &&str| device.starts_with("Micrófono");
        let inputs = ["Stereo Mix", "Micrófono integrado", "Micrófono USB"];
        assert_eq!(
            pick_input(Some("Stereo Mix"), inputs, is_mic),
            Some("Micrófono integrado")
        );
        assert_eq!(
            pick_input(Some("Micrófono USB"), inputs, is_mic),
            Some("Micrófono USB")
        );
        assert_eq!(
            pick_input(Some("Stereo Mix"), ["Stereo Mix"], is_mic),
            Some("Stereo Mix")
        );
        assert_eq!(pick_input(None, ["Línea"], is_mic), None);
        assert_eq!(
            pick_input(None, ["Línea", "Micrófono USB"], is_mic),
            Some("Micrófono USB")
        );
    }

    #[test]
    fn resampling_keeps_duration() {
        let one_second = vec![0.5; 48_000];
        let result = resample(&one_second, 48_000);
        assert_eq!(result.len(), 16_000);
        assert!((result[100] - 0.5).abs() < 1e-5);
    }

    #[test]
    fn resampling_at_the_same_rate_changes_nothing() {
        assert_eq!(resample(&[0.1, 0.2], SAMPLE_RATE), vec![0.1, 0.2]);
    }

    #[test]
    fn level_is_bounded() {
        assert_eq!(level_of(&[]), 0.0);
        assert!((0.0..=1.0).contains(&level_of(&[1.0; 100])));
        assert!(level_of(&[0.0; 100]) < 0.01);
    }

    #[test]
    fn a_full_buffer_stops_accepting_audio() {
        let buffer = Mutex::new(CaptureBuffer {
            samples: vec![],
            pending_level: vec![],
            channels: 2,
            limit: 2,
            limit_reached: false,
        });
        let limit_hits = Arc::new(Mutex::new(0));
        let counter = limit_hits.clone();
        let (level, limit): (LevelCallback, LimitCallback) = (
            Arc::new(|_| {}),
            Arc::new(move || *counter.lock().unwrap() += 1),
        );
        push_samples(
            &buffer,
            &[0.5f32, 0.5, 0.5, 0.5],
            |s| s,
            &level,
            &limit,
            100,
        );
        push_samples(&buffer, &[0.5f32, 0.5], |s| s, &level, &limit, 100);
        assert_eq!(buffer.lock().unwrap().samples.len(), 2);
        assert_eq!(*limit_hits.lock().unwrap(), 1);
    }
}
