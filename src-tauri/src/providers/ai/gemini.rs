//! Adaptador directo de Gemini, solo para desarrollo: no existe en los instaladores.
//! Comparte modelos e instrucciones con el servidor de Nexium.

use std::{sync::LazyLock, time::Duration};

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Deserialize;
use serde_json::{json, Value};

use super::{schema, AiAnswer, AiProvider, InterpretRequest, VisionRequest};
use crate::{error::AiError, models::Intent};

pub const DEV_KEY_VARIABLE: &str = "GEMINI_API_KEY";

static CONTRACT: LazyLock<Value> = LazyLock::new(|| serde_json::from_str(include_str!("../../../../server/src/ai-contract.json")).expect("contrato de IA válido"));

/// Clave de `.env`. Nunca se muestra ni se guarda.
pub fn dev_key() -> Option<String> {
    let _ = dotenvy::from_filename("../.env");
    let _ = dotenvy::dotenv();
    std::env::var(DEV_KEY_VARIABLE).ok().filter(|value| !value.trim().is_empty())
}

pub struct GeminiProvider {
    http: reqwest::Client,
    base_url: String,
    key: String,
}

#[derive(Deserialize, Default)]
struct InteractionResponse {
    #[serde(default)]
    steps: Vec<Step>,
    #[serde(default)]
    usage: Usage,
}

#[derive(Deserialize, Default)]
struct Step {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    content: Vec<ContentPart>,
}

#[derive(Deserialize, Default)]
struct ContentPart {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    text: String,
}

#[derive(Deserialize, Default)]
struct Usage {
    #[serde(default)]
    total_tokens: u64,
}

#[derive(Deserialize, Default)]
struct ErrorEnvelope {
    #[serde(default)]
    error: ErrorDetail,
}

#[derive(Deserialize, Default)]
struct ErrorDetail {
    #[serde(default)]
    code: Value,
    #[serde(default)]
    status: String,
    #[serde(default)]
    message: String,
}

impl GeminiProvider {
    pub fn new(key: String) -> Self {
        Self::with_base_url(key, CONTRACT["baseUrl"].as_str().unwrap_or_default())
    }

    pub fn with_base_url(key: String, base_url: &str) -> Self {
        Self { http: reqwest::Client::new(), base_url: base_url.trim_end_matches('/').to_owned(), key }
    }

    async fn send(&self, body: Value, timeout: Duration) -> Result<InteractionResponse, AiError> {
        // Un único reintento y solo ante un corte del servicio: un 429 nunca se reintenta.
        match self.send_once(&body, timeout).await {
            Err(AiError::Unavailable) => {
                tokio::time::sleep(Duration::from_secs(1)).await;
                self.send_once(&body, timeout).await
            }
            other => other,
        }
    }

    async fn send_once(&self, body: &Value, timeout: Duration) -> Result<InteractionResponse, AiError> {
        let response = self
            .http
            .post(format!("{}/interactions", self.base_url))
            .header("x-goog-api-key", &self.key)
            .header("Api-Revision", CONTRACT["apiRevision"].as_str().unwrap_or_default())
            .timeout(timeout)
            .json(body)
            .send()
            .await
            .map_err(|error| if error.is_timeout() { AiError::Timeout } else { AiError::Network })?;
        let status = response.status();
        if status.is_success() {
            return response.json::<InteractionResponse>().await.map_err(|_| AiError::InvalidResponse);
        }
        let envelope: ErrorEnvelope = response.json().await.unwrap_or_default();
        Err(map_error(status.as_u16(), &envelope.error))
    }

    fn output_text(response: &InteractionResponse) -> Result<String, AiError> {
        let text: String = response
            .steps
            .iter()
            .filter(|step| step.kind == "model_output")
            .flat_map(|step| &step.content)
            .filter(|part| part.kind == "text")
            .map(|part| part.text.as_str())
            .collect();
        if text.trim().is_empty() {
            return Err(AiError::Rejected("El modelo no devolvió texto.".into()));
        }
        Ok(text)
    }
}

fn map_error(status: u16, error: &ErrorDetail) -> AiError {
    let code = error.code.as_str().unwrap_or(&error.status).to_ascii_lowercase();
    match (status, code.as_str()) {
        (429, "quota_exceeded") => AiError::QuotaExhausted,
        // RESOURCE_EXHAUSTED es el código de la API heredada: tampoco distingue minuto de día si el mensaje no lo dice.
        (429, "resource_exhausted") if error.message.to_ascii_lowercase().contains("per day") => AiError::QuotaExhausted,
        (429, _) => AiError::RateLimited,
        (401, _) => AiError::Rejected("Google rechazó la clave de .env.".into()),
        (403, _) => AiError::Rejected("Google denegó el acceso: el proyecto puede estar restringido o la clave puede estar bloqueada.".into()),
        (404, _) => AiError::Rejected("Google no reconoce el modelo del contrato de IA.".into()),
        (400, "failed_precondition") => AiError::Rejected("La API de Gemini no está disponible para esta cuenta o región.".into()),
        (500..=599, _) => AiError::Unavailable,
        _ => AiError::Rejected(format!("Código {status}.")),
    }
}

fn timeout_of(contract: &Value) -> Duration {
    Duration::from_millis(contract["timeoutMs"].as_u64().unwrap_or(20_000))
}

#[async_trait]
impl AiProvider for GeminiProvider {
    fn display_name(&self) -> &str {
        "Google Gemini"
    }

    fn data_notice(&self) -> &str {
        "En la capa gratuita, Google puede usar lo que envías para mejorar sus productos, y personas pueden revisarlo. No captures información personal ni confidencial."
    }

    fn quota_message(&self) -> String {
        "Se agotó la cuota diaria gratuita de Google Gemini. Se renueva a medianoche, hora del Pacífico.".into()
    }

    fn is_configured(&self) -> bool {
        true
    }

    async fn interpret(&self, request: InterpretRequest) -> Result<AiAnswer<Intent>, AiError> {
        let now = request.now.format("%A %Y-%m-%d %H:%M, zona %:z").to_string();
        let contract = &CONTRACT["interpret"];
        let body = json!({
            "model": contract["model"],
            "store": false,
            "system_instruction": contract["systemInstruction"].as_str().unwrap_or_default().replace("{now}", &now),
            "input": request.text,
            "generation_config": { "max_output_tokens": contract["maxOutputTokens"], "thinking_level": contract["thinkingLevel"] },
            "response_format": { "type": "text", "mime_type": "application/json", "schema": contract["responseSchema"] }
        });
        let response = self.send(body, timeout_of(contract)).await?;
        let value = schema::parse_interpretation(&Self::output_text(&response)?, &request.text)?;
        Ok(AiAnswer { value, tokens: response.usage.total_tokens })
    }

    async fn analyze_image(&self, request: VisionRequest) -> Result<AiAnswer<String>, AiError> {
        let contract = &CONTRACT["analyze"];
        let body = json!({
            "model": contract["model"],
            "store": false,
            "system_instruction": contract["systemInstruction"],
            "input": [
                { "type": "text", "text": request.question },
                { "type": "image", "data": STANDARD.encode(&request.image_jpeg), "mime_type": "image/jpeg" }
            ],
            "generation_config": { "max_output_tokens": contract["maxOutputTokens"] }
        });
        let response = self.send(body, timeout_of(contract)).await?;
        Ok(AiAnswer { value: Self::output_text(&response)?.trim().to_owned(), tokens: response.usage.total_tokens })
    }
}

#[cfg(test)]
mod tests {
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
        InterpretRequest { text: "Busca mi informe".into(), now: Local::now() }
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
            .respond_with(ResponseTemplate::new(status).set_body_json(json!({ "error": { "code": code, "message": "m" } })))
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
            .and(body_partial_json(json!({ "store": false, "model": CONTRACT["interpret"]["model"] })))
            .respond_with(ResponseTemplate::new(200).set_body_json(output(r#"{"intent":"search_files","search_terms":["informe"]}"#)))
            .expect(1)
            .mount(&server)
            .await;
        let answer = provider(&server).interpret(request()).await.unwrap();
        assert_eq!(answer.tokens, 42);
        assert!(matches!(answer.value, Intent::SearchFiles(_)));
    }

    #[tokio::test]
    async fn daily_quota_is_reported_and_not_retried() {
        let server = failing_server(429, "quota_exceeded", 1).await;
        assert_eq!(provider(&server).interpret(request()).await.unwrap_err(), AiError::QuotaExhausted);
    }

    #[tokio::test]
    async fn per_minute_limit_is_reported_and_not_retried() {
        let server = failing_server(429, "rate_limit_exceeded", 1).await;
        assert_eq!(provider(&server).interpret(request()).await.unwrap_err(), AiError::RateLimited);
    }

    #[tokio::test]
    async fn an_outage_is_retried_only_once() {
        let server = failing_server(503, "service_unavailable", 2).await;
        assert_eq!(provider(&server).interpret(request()).await.unwrap_err(), AiError::Unavailable);
    }

    #[tokio::test]
    async fn invalid_key_is_not_retried() {
        let server = failing_server(401, "authentication", 1).await;
        assert!(matches!(provider(&server).interpret(request()).await.unwrap_err(), AiError::Rejected(_)));
    }

    #[tokio::test]
    async fn unknown_model_is_reported() {
        let server = failing_server(404, "model_not_found", 1).await;
        assert!(matches!(provider(&server).interpret(request()).await.unwrap_err(), AiError::Rejected(_)));
    }

    #[tokio::test]
    async fn an_invalid_model_answer_is_rejected() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).respond_with(ResponseTemplate::new(200).set_body_json(output(r#"{"intent":"format_disk"}"#))).mount(&server).await;
        assert_eq!(provider(&server).interpret(request()).await.unwrap_err(), AiError::InvalidResponse);
    }

    #[tokio::test]
    async fn sends_the_image_inline_as_jpeg() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_partial_json(json!({ "store": false, "input": [{ "type": "text", "text": "¿Qué es?" }, { "type": "image", "data": "AQID", "mime_type": "image/jpeg" }] })))
            .respond_with(ResponseTemplate::new(200).set_body_json(output("Un error de ruta.")))
            .expect(1)
            .mount(&server)
            .await;
        let answer = provider(&server)
            .analyze_image(VisionRequest { image_jpeg: vec![1, 2, 3], question: "¿Qué es?".into() })
            .await
            .unwrap();
        assert_eq!(answer.value, "Un error de ruta.");
    }

    #[test]
    fn the_shared_contract_can_be_used() {
        assert!(CONTRACT["interpret"]["systemInstruction"].as_str().unwrap().contains("{now}"));
        assert_eq!(CONTRACT["interpret"]["responseSchema"]["properties"]["intent"]["type"], "string");
        assert!(CONTRACT["analyze"]["model"].is_string());
    }

    #[test]
    fn the_notice_names_google_and_the_data_use() {
        let provider = GeminiProvider::new("clave-de-prueba".into());
        assert!(provider.display_name().contains("Google"));
        assert!(provider.data_notice().contains("Google puede usar"));
    }
}
