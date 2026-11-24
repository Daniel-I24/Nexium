use super::*;
use chrono::Local;
use wiremock::{
    matchers::{body_partial_json, header, method, path},
    Mock, MockServer, ResponseTemplate,
};

fn provider(server: &MockServer) -> GeminiProvider {
    GeminiProvider::with_base_url("clave-de-prueba".into(), &server.uri())
}

fn request() -> InterpretRequest {
    InterpretRequest {
        text: "Busca mi informe".into(),
        now: Local::now(),
    }
}

fn output(text: &str) -> Value {
    json!({
        "id": "x", "status": "completed", "object": "interaction",
        "usage": { "total_tokens": 42 },
        "steps": [{ "type": "model_output", "content": [{ "type": "text", "text": text }] }]
    })
}

async fn failing_server(status: u16, code: &str, expected_calls: u64) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(status)
                .set_body_json(json!({ "error": { "code": code, "message": "m" } })),
        )
        .expect(expected_calls)
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn interprets_and_never_stores_the_interaction() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/interactions"))
        .and(header("x-goog-api-key", "clave-de-prueba"))
        .and(body_partial_json(
            json!({ "store": false, "model": CONTRACT["interpret"]["model"] }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(output(
            r#"{"intent":"search_files","search_terms":["informe"]}"#,
        )))
        .expect(1)
        .mount(&server)
        .await;
    let answer = provider(&server).interpret(request()).await.unwrap();
    assert_eq!(answer.tokens, 42);
    assert!(matches!(
        answer.value[0].intent,
        crate::models::Intent::SearchFiles(_)
    ));
}

#[tokio::test]
async fn daily_quota_is_reported_and_not_retried() {
    let server = failing_server(429, "quota_exceeded", 1).await;
    assert_eq!(
        provider(&server).interpret(request()).await.unwrap_err(),
        AiError::QuotaExhausted(None)
    );
}

#[tokio::test]
async fn per_minute_limit_is_reported_and_not_retried() {
    let server = failing_server(429, "rate_limit_exceeded", 1).await;
    assert_eq!(
        provider(&server).interpret(request()).await.unwrap_err(),
        AiError::RateLimited
    );
}

#[tokio::test]
async fn an_outage_is_retried_only_once() {
    let server = failing_server(503, "service_unavailable", 2).await;
    assert_eq!(
        provider(&server).interpret(request()).await.unwrap_err(),
        AiError::Unavailable
    );
}

#[tokio::test]
async fn invalid_key_is_not_retried() {
    let server = failing_server(401, "authentication", 1).await;
    assert!(matches!(
        provider(&server).interpret(request()).await.unwrap_err(),
        AiError::Rejected(_)
    ));
}

#[tokio::test]
async fn unknown_model_is_reported() {
    let server = failing_server(404, "model_not_found", 1).await;
    assert!(matches!(
        provider(&server).interpret(request()).await.unwrap_err(),
        AiError::Rejected(_)
    ));
}

#[tokio::test]
async fn an_invalid_model_answer_is_rejected() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(output(r#"{"intent":"format_disk"}"#)),
        )
        .mount(&server)
        .await;
    assert_eq!(
        provider(&server).interpret(request()).await.unwrap_err(),
        AiError::InvalidResponse
    );
}

#[tokio::test]
async fn sends_the_image_inline_as_jpeg() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({ "store": false, "input": [{ "type": "text", "text": "¿Qué es?" }, { "type": "image", "data": "AQID", "mime_type": "image/jpeg" }] })))
        .respond_with(ResponseTemplate::new(200).set_body_json(output(r#"{"summary":"Un error de ruta.","cause":null,"steps":[]}"#)))
        .expect(1)
        .mount(&server)
        .await;
    let answer = provider(&server)
        .analyze_image(VisionRequest {
            image_jpeg: vec![1, 2, 3],
            question: "¿Qué es?".into(),
        })
        .await
        .unwrap();
    assert_eq!(answer.value.summary, "Un error de ruta.");
}

#[test]
fn the_shared_contract_can_be_used() {
    assert!(CONTRACT["interpret"]["systemInstruction"]
        .as_str()
        .unwrap()
        .contains("{now}"));
    assert_eq!(
        CONTRACT["interpret"]["responseSchema"]["properties"]["intent"]["type"],
        "string"
    );
    assert!(CONTRACT["analyze"]["model"].is_string());
}

#[test]
fn the_notice_names_google_and_the_data_use() {
    let provider = GeminiProvider::new("clave-de-prueba".into());
    assert!(provider.display_name().contains("Google"));
    assert!(provider.data_notice().contains("Google puede usar"));
}
