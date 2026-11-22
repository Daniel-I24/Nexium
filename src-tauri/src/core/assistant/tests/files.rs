//! Archivos y documentos: permisos, ofertas y lo que nunca se guarda.

use super::*;

#[tokio::test]
async fn a_document_needs_folders_and_then_is_offered_without_sending_anything() {
    let h = harness(None, true, false);
    let response = ask(&h.assistant, "Resume mi informe de programación").await;
    assert!(matches!(response.body, ResponseBody::FolderConsent { .. }));
    authorize(&h);
    let response = ask(&h.assistant, "Resume mi informe de programación").await;
    let ResponseBody::DocumentConsent {
        candidates,
        provider,
        task,
        ..
    } = response.body
    else {
        panic!("{:?}", response.body)
    };
    assert_eq!(task, crate::models::DocumentTask::Summarize);
    assert_eq!(provider, "Proveedor de prueba");
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].name, "informe programacion");
    // Ofrecer un archivo no lo lee ni lo envía.
    assert_eq!(h.ai_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_document_summary_never_stores_its_content() {
    let h = harness(None, true, false);
    let file = h._folder.path().join("acta.txt");
    std::fs::write(
        &file,
        "Contenido privado del acta de la reunión de presupuesto.",
    )
    .unwrap();
    let response = h
        .assistant
        .analyze_document(
            &file,
            crate::models::DocumentTask::Summarize,
            None,
            CancellationToken::new(),
            silent(),
        )
        .await;
    assert_eq!(response.request, "Resumen de «acta.txt»");
    let ResponseBody::DocumentAnalysis {
        summary, points, ..
    } = &response.body
    else {
        panic!("{:?}", response.body)
    };
    assert_eq!(summary, "Trata de bases de datos.");
    assert_eq!(points.len(), 1);
    let history = h.assistant.history.list(None).unwrap();
    assert_eq!(history[0].request, "Resumen de «acta.txt»");
    assert!(!history[0].response.contains("Contenido privado"));
}

#[tokio::test]
async fn an_unreadable_document_is_explained_without_calling_the_ai() {
    let h = harness(None, true, false);
    let file = h._folder.path().join("foto.png");
    std::fs::write(&file, "x").unwrap();
    let response = h
        .assistant
        .analyze_document(
            &file,
            crate::models::DocumentTask::Analyze,
            None,
            CancellationToken::new(),
            silent(),
        )
        .await;
    assert_eq!(response.status, InteractionStatus::Failed);
    assert!(response.summary.contains("No sé leer archivos .png"));
    assert_eq!(h.ai_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn file_search_asks_for_authorization_first() {
    let h = harness(None, true, false);
    let response = ask(&h.assistant, "¿Existe el archivo proyecto.pdf?").await;
    assert!(matches!(response.body, ResponseBody::FolderConsent { .. }));
}

#[tokio::test]
async fn search_falls_back_to_local_terms_when_ai_is_unavailable() {
    let h = harness(None, false, false);
    authorize(&h);
    let response = ask(&h.assistant, "Busca mi informe de programación").await;
    let ResponseBody::Files { files, note, .. } = response.body else {
        panic!("{:?}", response.body)
    };
    assert_eq!(files.len(), 1);
    assert!(note.unwrap().contains("sin IA"));
    assert_eq!(response.origin_label, "reglas locales, sin IA");
}
