use super::*;

fn kind(device: &&str) -> InputKind {
    if is_system_audio_name(device) {
        InputKind::SystemAudio
    } else if device.starts_with("Micrófono") {
        InputKind::Microphone
    } else {
        InputKind::Unknown
    }
}

#[test]
fn the_chosen_microphone_wins_while_it_is_connected() {
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
            kind
        ),
        Some("Micrófono de la cámara")
    );
    // Elegido a mano en Ajustes, se respeta aunque no sea un micrófono.
    assert_eq!(
        choose_input(
            Some("Stereo Mix"),
            Some("Stereo Mix"),
            inputs.clone(),
            name,
            kind
        ),
        Some("Stereo Mix")
    );
    assert_eq!(
        choose_input(
            Some("Auriculares desconectados"),
            Some("Stereo Mix"),
            inputs.clone(),
            name,
            kind
        ),
        Some("Micrófono integrado")
    );
    assert_eq!(
        choose_input(None, Some("Stereo Mix"), inputs, name, kind),
        Some("Micrófono integrado")
    );
}

#[test]
fn the_automatic_choice_never_records_the_system_sound() {
    let inputs = ["Stereo Mix", "Micrófono integrado", "Micrófono USB"];
    assert_eq!(
        pick_input(Some("Stereo Mix"), inputs, kind),
        Some("Micrófono integrado")
    );
    assert_eq!(
        pick_input(Some("Micrófono USB"), inputs, kind),
        Some("Micrófono USB")
    );
    // Aunque sea la única entrada y la predeterminada de Windows.
    assert_eq!(pick_input(Some("Stereo Mix"), ["Stereo Mix"], kind), None);
    assert_eq!(
        pick_input(
            Some("Mezcla estéreo (Realtek(R) Audio)"),
            ["Mezcla estéreo (Realtek(R) Audio)"],
            kind
        ),
        None
    );
    // Si el sistema no dice qué es nada, vale la primera entrada que no grabe el sonido del equipo.
    assert_eq!(
        pick_input(
            Some("Stereo Mix"),
            ["Stereo Mix", "Línea (Realtek)", "Entrada USB"],
            kind
        ),
        Some("Línea (Realtek)")
    );
    assert_eq!(
        pick_input(
            Some("Entrada USB"),
            ["Línea (Realtek)", "Entrada USB"],
            kind
        ),
        Some("Entrada USB")
    );
    assert_eq!(
        pick_input(None, ["Línea", "Micrófono USB"], kind),
        Some("Micrófono USB")
    );
}

#[test]
fn system_sound_inputs_are_recognized_by_name() {
    for name in [
        "Stereo Mix (Realtek High Definition Audio)",
        "Mezcla estéreo (Realtek(R) Audio)",
        "CABLE Output (VB-Audio Virtual Cable)",
        "Monitor of Built-in Audio Analog Stereo",
        "BlackHole 2ch",
    ] {
        assert!(is_system_audio_name(name), "{name}");
    }
    for name in [
        "Micrófono (Realtek(R) Audio)",
        "Headset Microphone (Jabra)",
        "Varios micrófonos (Intel® Smart Sound)",
    ] {
        assert!(!is_system_audio_name(name), "{name}");
    }
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

/// Lee un WAV PCM de 16 bits: frecuencia, canales y muestras intercaladas.
pub(crate) fn read_wav(bytes: &[u8]) -> (u32, usize, Vec<i16>) {
    let format = bytes
        .windows(4)
        .position(|w| w == b"fmt ")
        .expect("chunk fmt")
        + 8;
    let channels = usize::from(u16::from_le_bytes([bytes[format + 2], bytes[format + 3]]));
    let rate = u32::from_le_bytes(bytes[format + 4..format + 8].try_into().unwrap());
    let data = bytes
        .windows(4)
        .position(|w| w == b"data")
        .expect("chunk data")
        + 8;
    let samples = bytes[data..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| i16::from_le_bytes(*pair))
        .collect();
    (rate, channels, samples)
}

#[test]
fn a_48k_stereo_recording_reaches_whisper_as_16k_mono_voice() {
    let (rate, channels, data) = read_wav(include_bytes!(
        "../../../../tests/audio/bateria-48k-estereo.wav"
    ));
    assert_eq!((rate, channels), (48_000, 2));
    let buffer = Mutex::new(CaptureBuffer {
        samples: vec![],
        pending_level: vec![],
        channels,
        limit: rate as usize * MAX_RECORDING_SECONDS,
        limit_reached: false,
    });
    let readings = Arc::new(Mutex::new(0));
    let counter = readings.clone();
    let (level, limit): (LevelCallback, LimitCallback) = (
        Arc::new(move |_| *counter.lock().unwrap() += 1),
        Arc::new(|| {}),
    );
    // Como cpal: bloques de 10 ms con los dos canales intercalados.
    for block in data.chunks(480 * channels) {
        push_samples(
            &buffer,
            block,
            |s| f32::from(s) / 32768.0,
            &level,
            &limit,
            rate as usize / 20,
        );
    }
    let mono = std::mem::take(&mut buffer.lock().unwrap().samples);
    assert_eq!(mono.len(), data.len() / channels);
    let audio = super::super::prepare(&resample(&mono, rate));
    let seconds = audio.len() as f32 / super::super::SAMPLE_RATE as f32;
    assert!((1.0..=2.2).contains(&seconds), "{seconds} s");
    let peak = audio.iter().fold(0.0f32, |p, s| p.max(s.abs()));
    assert!((0.85..=0.95).contains(&peak), "{peak}");
    // Una lectura de nivel cada 50 ms de grabación.
    assert!((40..=44).contains(&*readings.lock().unwrap()));
}
