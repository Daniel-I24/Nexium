use super::*;
use crate::providers::speech::{SpeechError, SpeechProvider, WhisperSpeech};
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;
use wiremock::{matchers::path, Mock, MockServer, ResponseTemplate};

#[test]
fn the_model_is_ready_only_with_its_exact_size() {
    let directory = tempfile::tempdir().unwrap();
    let model = SpeechModel {
        id: "prueba",
        display_name: "Prueba",
        description: "",
        file: "modelo.bin",
        size_bytes: 4,
        sha256: "",
    };
    assert_eq!(model_state(&model, directory.path()), ModelState::Missing);
    std::fs::write(directory.path().join("modelo.bin.part"), b"12").unwrap();
    assert_eq!(model_state(&model, directory.path()), ModelState::Missing);
    std::fs::write(directory.path().join("modelo.bin"), b"123").unwrap();
    assert_eq!(model_state(&model, directory.path()), ModelState::Damaged);
    std::fs::write(directory.path().join("modelo.bin"), b"1234").unwrap();
    assert_eq!(model_state(&model, directory.path()), ModelState::Ready);
}

#[test]
fn the_catalog_sizes_match_the_published_files() {
    assert_eq!(find_model("small").unwrap().size_bytes, 190_085_487);
    assert_eq!(find_model("base").unwrap().size_bytes, 59_707_625);
}

#[test]
fn the_label_and_the_download_counter_show_the_same_size() {
    assert_eq!(megabytes(190_085_487), 181);
    assert_eq!(megabytes(59_707_625), 57);
    let directory = tempfile::tempdir().unwrap();
    let listed = statuses(directory.path(), "small");
    assert_eq!(
        listed.iter().map(|model| model.size_mb).collect::<Vec<_>>(),
        [181, 57]
    );
}

#[test]
fn only_a_complete_model_counts_as_downloaded() {
    let directory = tempfile::tempdir().unwrap();
    let small = find_model("small").unwrap();
    std::fs::write(
        directory.path().join(format!("{}.part", small.file)),
        b"a medias",
    )
    .unwrap();
    let listed = statuses(directory.path(), "small");
    assert!(!listed[0].downloaded && !listed[0].damaged);
    std::fs::write(directory.path().join(small.file), b"truncado").unwrap();
    let listed = statuses(directory.path(), "small");
    assert!(!listed[0].downloaded && listed[0].damaged);
}

#[tokio::test]
async fn a_missing_model_is_reported() {
    let error = WhisperSpeech::default()
        .transcribe(
            vec![0.0; 10],
            PathBuf::from("no-existe.bin"),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert_eq!(error, SpeechError::ModelMissing);
}

#[tokio::test]
async fn a_corrupt_download_is_discarded() {
    let server = MockServer::start().await;
    Mock::given(path("/ggml-base-q5_1.bin"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![1, 2, 3]))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let model = find_model("base").unwrap();
    assert!(
        download_model(model, dir.path(), |_, _| {}, Some(&server.uri()))
            .await
            .is_err()
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn a_verified_download_is_kept() {
    let server = MockServer::start().await;
    let body = b"modelo".to_vec();
    Mock::given(path("/m.bin"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body.clone()))
        .mount(&server)
        .await;
    let digest: String = Sha256::digest(&body)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let digest: &'static str = Box::leak(digest.into_boxed_str());
    let model = SpeechModel {
        id: "x",
        display_name: "x",
        description: "",
        file: "m.bin",
        size_bytes: body.len() as u64,
        sha256: digest,
    };
    let dir = tempfile::tempdir().unwrap();
    download_model(&model, dir.path(), |_, _| {}, Some(&server.uri()))
        .await
        .unwrap();
    assert!(dir.path().join("m.bin").exists());
}

#[test]
fn the_default_is_automatic_and_always_resolves() {
    assert_eq!(crate::services::settings::DEFAULT_SPEECH_MODEL, AUTO_MODEL);
    assert!(resolve_model(AUTO_MODEL, Hardware::detect()).is_some());
    assert!(catalog_ids().contains(&AUTO_MODEL));
}

fn hardware(memory_gib: f64, physical_cores: usize) -> Hardware {
    Hardware {
        memory_gib,
        physical_cores,
    }
}

#[test]
fn automatic_picks_precise_only_with_enough_memory_and_cores() {
    let pick = |memory, cores| auto_choice(hardware(memory, cores)).0.id;
    assert_eq!(pick(15.7, 4), "small");
    // Un equipo de 8 GB informa algo menos.
    assert_eq!(pick(7.4, 4), "small");
    assert_eq!(pick(15.7, 2), "base");
    assert_eq!(pick(3.8, 8), "base");
    assert_eq!(pick(5.9, 4), "base");
}

#[test]
fn automatic_explains_its_choice() {
    assert_eq!(
        auto_choice(hardware(15.7, 4)).1,
        "este equipo tiene 16 GB de RAM y 4 núcleos, suficiente para que Preciso responda en torno a 1 s."
    );
    assert!(auto_choice(hardware(15.7, 2)).1.contains("2 núcleos"));
    assert!(auto_choice(hardware(3.8, 8)).1.contains("4 GB de RAM"));
}

#[test]
fn a_chosen_model_is_kept() {
    let small_machine = hardware(3.8, 2);
    assert_eq!(resolve_model("small", small_machine).unwrap().id, "small");
    assert!(resolve_model("grande", small_machine).is_none());
}
