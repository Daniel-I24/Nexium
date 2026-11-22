//! Historial, cancelación, etapas y registro de tiempos de una solicitud.

use super::*;

#[tokio::test]
async fn a_request_keeps_a_single_history_entry_until_its_final_state() {
    let h = harness(None, true, false);
    let pending = ask(&h.assistant, "Resume mi informe de programación").await;
    assert_eq!(pending.status, InteractionStatus::Pending);
    authorize(&h);
    // Al permitir las carpetas, la misma solicitud sigue en su entrada.
    let offered = h
        .assistant
        .process_timed(
            "Resume mi informe de programación",
            Some(pending.id),
            CancellationToken::new(),
            silent(),
            Timings::new("texto"),
        )
        .await;
    assert_eq!(offered.id, pending.id);
    let ResponseBody::DocumentConsent { .. } = offered.body else {
        panic!("{:?}", offered.body)
    };
    let file = h._folder.path().join("informe programacion.txt");
    std::fs::write(&file, "Informe con texto suficiente para resumirlo.").unwrap();
    let done = h
        .assistant
        .analyze_document(
            &file,
            crate::models::DocumentTask::Summarize,
            Some(pending.id),
            CancellationToken::new(),
            silent(),
        )
        .await;
    assert_eq!(done.id, pending.id);
    assert_eq!(done.request, "Resume mi informe de programación");
    let history = h.assistant.history.list(None).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].status, InteractionStatus::Completed);
    assert_eq!(history[0].response, "Trata de bases de datos.");
}

#[tokio::test]
async fn a_continuation_of_a_deleted_entry_creates_a_new_one() {
    let h = harness(None, true, false);
    let response = h
        .assistant
        .process_timed(
            "¿Qué hora es?",
            Some(999),
            CancellationToken::new(),
            silent(),
            Timings::new("texto"),
        )
        .await;
    assert_ne!(response.id, 999);
    assert_eq!(h.assistant.history.list(None).unwrap().len(), 1);
}

#[tokio::test]
async fn every_interaction_is_recorded() {
    let h = harness(None, true, false);
    ask(&h.assistant, "¿Qué hora es?").await;
    let entries = h.assistant.history.list(None).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].category, Category::Time);
    assert_eq!(entries[0].status, InteractionStatus::Completed);
}

#[tokio::test]
async fn a_cancelled_request_is_recorded_as_cancelled() {
    let h = harness(None, true, false);
    let cancel = CancellationToken::new();
    cancel.cancel();
    let response = h.assistant.process("cuéntame algo", cancel, silent()).await;
    assert_eq!(response.status, InteractionStatus::Cancelled);
}

#[tokio::test]
async fn stages_are_reported_in_order() {
    let h = harness(None, true, false);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    h.assistant
        .process(
            "¿Qué hora es?",
            CancellationToken::new(),
            Arc::new(move |stage| sink.lock().unwrap().push(stage)),
        )
        .await;
    assert_eq!(
        *seen.lock().unwrap(),
        vec![Stage::Executing, Stage::Responding]
    );
}

#[tokio::test]
async fn the_performance_log_never_keeps_the_request() {
    let mut h = harness(None, true, false);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rendimiento.log");
    h.assistant.performance = crate::services::performance::PerformanceLog::new(path.clone());
    ask(&h.assistant, "¿Qué hora es? Secreto-7391").await;
    ask(&h.assistant, "Busca el informe Confidencial-5512").await;
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content.lines().count(), 2);
    assert!(!content.contains("Secreto") && !content.contains("7391"));
    assert!(!content.contains("Confidencial") && !content.contains("informe"));
}
