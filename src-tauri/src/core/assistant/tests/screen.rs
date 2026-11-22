//! Análisis de pantalla: permiso, una sola captura y errores.

use super::*;

#[tokio::test]
async fn a_screen_request_only_asks_for_permission() {
    let h = harness(None, true, false);
    let response = ask(&h.assistant, "Explícame este error").await;
    assert!(matches!(response.body, ResponseBody::CaptureRequest { .. }));
    assert_eq!(h.capture_calls.load(Ordering::SeqCst), 0);
    assert_eq!(h.ai_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn analysis_captures_once_and_does_not_keep_the_image() {
    let h = harness(None, true, false);
    let response = h
        .assistant
        .analyze_screen(
            "Explícame este error",
            None,
            CancellationToken::new(),
            silent(),
        )
        .await;
    assert_eq!(h.capture_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        response.body,
        ResponseBody::ScreenAnalysis {
            summary: "Un error de ruta".into(),
            cause: Some("Falta un archivo.".into()),
            steps: vec!["Comprueba la carpeta.".into()]
        }
    );
    assert!(response.origin_label.contains("descartó la imagen"));
    let entries = h.assistant.history.list(None).unwrap();
    assert_eq!(entries[0].response, "Un error de ruta");
}

#[tokio::test]
async fn an_unreadable_analysis_shows_a_clear_message() {
    let h = harness(None, true, false);
    let response = h
        .assistant
        .analyze_screen(
            "respuesta ilegible",
            None,
            CancellationToken::new(),
            silent(),
        )
        .await;
    let ResponseBody::Error {
        code,
        message,
        hint,
    } = response.body
    else {
        panic!()
    };
    assert_eq!(code, "invalidResponse");
    assert!(message.contains("formato que Nexium no puede mostrar"));
    assert!(hint.unwrap().contains("ya se descartó"));
}

#[tokio::test]
async fn without_the_service_no_capture_is_taken() {
    let h = harness(None, false, false);
    h.assistant
        .analyze_screen("x", None, CancellationToken::new(), silent())
        .await;
    assert_eq!(h.capture_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_capture_permission_error_is_explained() {
    let h = harness(None, true, true);
    let response = h
        .assistant
        .analyze_screen("x", None, CancellationToken::new(), silent())
        .await;
    assert!(
        matches!(response.body, ResponseBody::Error { ref message, .. } if message == "sin permiso")
    );
    assert_eq!(h.ai_calls.load(Ordering::SeqCst), 0);
}
