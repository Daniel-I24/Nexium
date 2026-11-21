use super::*;
use crate::providers::speech::SAMPLE_RATE;

#[test]
fn the_abort_callback_reads_the_cancellation_flag() {
    let flag = AtomicBool::new(false);
    let pointer = std::ptr::from_ref(&flag).cast_mut().cast();
    // SAFETY: `flag` vive durante toda la prueba.
    assert!(!unsafe { abort_requested(pointer) });
    flag.store(true, Ordering::Relaxed);
    assert!(unsafe { abort_requested(pointer) });
}

/// Necesita el modelo real: `NEXIUM_WHISPER_MODEL=<ruta de ggml-*.bin> cargo test -- --ignored`.
#[tokio::test]
#[ignore = "necesita un modelo de Whisper descargado"]
async fn silence_is_transcribed_and_not_reported_as_a_failure() {
    let model = PathBuf::from(std::env::var("NEXIUM_WHISPER_MODEL").expect("NEXIUM_WHISPER_MODEL"));
    let speech = WhisperSpeech::default();
    let silence = vec![0.0; SAMPLE_RATE as usize * 4];
    assert_eq!(
        speech
            .transcribe(silence, model, CancellationToken::new())
            .await
            .map(|transcript| transcript.text),
        Ok(String::new())
    );
}

#[test]
fn the_vocabulary_prompt_is_not_taken_as_a_request() {
    assert!(is_prompt_echo(
        "Wi-Fi, recordatorio, clima, pantalla, documento."
    ));
    assert!(!is_prompt_echo("¿Cuánta RAM estoy usando?"));
    assert!(!is_prompt_echo("Batería"));
    assert!(!is_prompt_echo(
        "Recuérdame revisar el clima de la pantalla del documento"
    ));
}

#[test]
fn whisper_uses_one_thread_per_physical_core_up_to_eight() {
    assert!((1..=8).contains(&whisper_threads()));
    if let Ok(cores) = sysinfo::System::physical_core_count() {
        assert_eq!(whisper_threads(), i32::try_from(cores.min(8)).unwrap());
    }
}

#[test]
fn silence_markers_are_removed() {
    assert_eq!(clean_transcript(" [Música] (risas) "), "");
    assert_eq!(clean_transcript("¿Qué hora es? [Música]"), "¿Qué hora es?");
}

#[test]
fn the_audio_context_follows_the_duration_within_limits() {
    assert_eq!(audio_context(0), 256);
    assert_eq!(audio_context(16_000 * 10), 628);
    assert_eq!(audio_context(16_000 * 30), 1500);
    assert_eq!(audio_context(usize::MAX), 1500);
}

#[test]
fn the_model_loads_once_and_stays_in_memory() {
    let cache = ModelCache::default();
    let (path, now, mut loads) = (Path::new("small.bin"), Instant::now(), 0);
    for _ in 0..3 {
        cache
            .get_or_load(path, now, || {
                loads += 1;
                Ok::<_, ()>(7)
            })
            .unwrap();
    }
    assert_eq!(loads, 1);
}

#[test]
fn another_model_replaces_the_first_before_loading() {
    let cache = ModelCache::default();
    let now = Instant::now();
    let first = cache
        .get_or_load(Path::new("small.bin"), now, || Ok::<_, ()>(1))
        .unwrap();
    let weak = Arc::downgrade(&first);
    drop(first);
    let second = cache
        .get_or_load(Path::new("base.bin"), now, || {
            assert!(weak.upgrade().is_none());
            Ok::<_, ()>(2)
        })
        .unwrap();
    assert_eq!(*second, 2);
}

#[test]
fn a_failed_load_leaves_nothing_cached() {
    let cache = ModelCache::<i32>::default();
    let now = Instant::now();
    assert_eq!(
        cache
            .get_or_load(Path::new("m.bin"), now, || Err("dañado"))
            .unwrap_err(),
        "dañado"
    );
    assert_eq!(
        *cache
            .get_or_load(Path::new("m.bin"), now, || Ok::<_, &str>(1))
            .unwrap(),
        1
    );
}

#[test]
fn the_model_is_released_only_after_the_idle_period() {
    let cache = ModelCache::default();
    let (path, start, idle) = (Path::new("m.bin"), Instant::now(), Duration::from_secs(600));
    let model = cache.get_or_load(path, start, || Ok::<_, ()>(1)).unwrap();
    let weak = Arc::downgrade(&model);
    drop(model);
    assert!(!cache.release_if_idle(start + Duration::from_secs(599), idle));
    assert!(weak.upgrade().is_some());
    assert!(cache.release_if_idle(start + idle, idle));
    assert!(weak.upgrade().is_none());
    assert!(!cache.release_if_idle(start + idle, idle));
}

#[test]
fn using_the_model_restarts_the_idle_period() {
    let cache = ModelCache::default();
    let (start, idle) = (Instant::now(), Duration::from_secs(600));
    cache
        .get_or_load(Path::new("m.bin"), start, || Ok::<_, ()>(1))
        .unwrap();
    cache.touch(start + Duration::from_secs(500));
    assert!(!cache.release_if_idle(start + idle, idle));
}

#[test]
fn a_model_in_use_survives_its_release() {
    let cache = ModelCache::default();
    let (start, idle) = (Instant::now(), Duration::from_secs(1));
    let in_use = cache
        .get_or_load(Path::new("m.bin"), start, || Ok::<_, ()>(9))
        .unwrap();
    assert!(cache.release_if_idle(start + idle, idle));
    assert_eq!(*in_use, 9);
}
