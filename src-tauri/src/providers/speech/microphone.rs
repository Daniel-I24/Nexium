//! Micrófono: elección del dispositivo, grabación con cpal y nivel de la voz.

use std::{
    sync::{mpsc, Arc, Mutex},
    thread::JoinHandle,
    time::Duration,
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::Serialize;

use super::{level_of, resample, SpeechError, MAX_RECORDING_SECONDS};

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

/// Qué es una entrada de audio, para elegirla sin preguntar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKind {
    /// El sistema la declara micrófono (integrado, USB, auriculares con micrófono).
    Microphone,
    /// El sistema no dice qué es. Puede ser un micrófono.
    Unknown,
    /// Graba lo que suena en el equipo, no la voz: "Stereo Mix", "Mezcla estéreo", cables virtuales.
    SystemAudio,
}

/// Nombres de las entradas que graban el sonido del equipo. Algunos controladores no las marcan como
/// virtuales, así que se reconocen también por el nombre, sin tildes ni mayúsculas.
const SYSTEM_AUDIO_NAMES: &[&str] = &[
    "stereo mix",
    "stereomix",
    "mezcla estereo",
    "mezcla stereo",
    "mix estereo",
    "what u hear",
    "wave out",
    "loopback",
    "monitor of",
    "cable output",
    "vb-audio",
    "voicemeeter",
    "blackhole",
    "soundflower",
];

pub fn is_system_audio_name(name: &str) -> bool {
    let name = crate::core::rules::normalize(name);
    SYSTEM_AUDIO_NAMES
        .iter()
        .any(|candidate| name.contains(&crate::core::rules::normalize(candidate)))
}

/// Elección automática: el primer micrófono real, prefiriendo el predeterminado si lo es. Si el sistema no
/// identifica ninguno, la primera entrada que no grabe el sonido del equipo. **Nunca** una de esas, aunque sea
/// la predeterminada de Windows: con ella Nexium oiría lo que suena, no tu voz.
pub fn pick_input<D>(
    default: Option<D>,
    inputs: impl IntoIterator<Item = D>,
    kind_of: impl Fn(&D) -> InputKind,
) -> Option<D> {
    let inputs: Vec<D> = inputs.into_iter().collect();
    let default_kind = default.as_ref().map(&kind_of);
    if default_kind == Some(InputKind::Microphone) {
        return default;
    }
    let mut unknown = None;
    for device in inputs {
        match kind_of(&device) {
            InputKind::Microphone => return Some(device),
            InputKind::Unknown if unknown.is_none() => unknown = Some(device),
            _ => {}
        }
    }
    if default_kind == Some(InputKind::Unknown) {
        return default;
    }
    unknown
}

/// El micrófono elegido en Ajustes, si sigue conectado (se respeta aunque no lo parezca: lo eligió la persona);
/// si no, la elección automática.
pub fn choose_input<D>(
    preferred: Option<&str>,
    default: Option<D>,
    inputs: Vec<D>,
    name_of: impl Fn(&D) -> String,
    kind_of: impl Fn(&D) -> InputKind,
) -> Option<D> {
    if let Some(wanted) = preferred {
        if let Some(index) = inputs.iter().position(|device| name_of(device) == wanted) {
            return inputs.into_iter().nth(index);
        }
    }
    pick_input(default, inputs, kind_of)
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
                        is_microphone: input_kind(&device) != InputKind::SystemAudio,
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
        input_kind,
    )
}

/// Nombre del dispositivo que se usaría ahora mismo con la preferencia indicada.
pub fn active_input(preferred: Option<&str>) -> Option<String> {
    resolve_input(preferred).map(|device| device_name(&device))
}

fn input_kind(device: &cpal::Device) -> InputKind {
    if is_system_audio_name(&device_name(device)) {
        return InputKind::SystemAudio;
    }
    match device
        .description()
        .map(|description| description.device_type())
    {
        Ok(
            cpal::DeviceType::Microphone | cpal::DeviceType::Headset | cpal::DeviceType::Handset,
        ) => InputKind::Microphone,
        Ok(cpal::DeviceType::Virtual) => InputKind::SystemAudio,
        _ => InputKind::Unknown,
    }
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
#[path = "tests/microphone.rs"]
mod tests;
