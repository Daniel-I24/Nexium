//! Adaptador del servidor de Nexium: la única vía de IA en los instaladores.
//! El servidor decide modelo e instrucciones; aquí solo viaja lo que escribió el usuario.

use std::time::Duration;

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::{header::CONTENT_TYPE, RequestBuilder, Response};
use serde::Deserialize;
use serde_json::json;

use super::{schema, AiAnswer, AiProvider, DocumentRequest, InterpretRequest, VisionRequest};
use crate::{
    error::AiError,
    models::{DocumentSummary, Intent, ScreenAnalysis},
};

/// Mayores que los del servidor, que puede reintentar una vez: quien corta primero es él.
/// Interpretar una orden no puede hacer esperar más: el servidor corta a los 9 s (con un segundo intento
/// solo si el primero falló pronto) y la app, a los 10 s, con un error claro.
pub const INTERPRET_TIMEOUT: Duration = Duration::from_secs(10);
const VISION_TIMEOUT: Duration = Duration::from_secs(130);
const DOCUMENT_TIMEOUT: Duration = Duration::from_secs(90);
const INSTALL_HEADER: &str = "X-Nexium-Install";
const CAPTURE_CONTENT_TYPE: &str = "application/x-nexium-capture";

pub struct NexiumServer {
    http: reqwest::Client,
    base_url: Option<String>,
    install_id: String,
}

#[derive(Deserialize)]
struct ModelReply {
    output: String,
    #[serde(default)]
    tokens: u64,
}

#[derive(Deserialize)]
struct ErrorEnvelope {
    error: ErrorDetail,
}

#[derive(Deserialize)]
struct ErrorDetail {
    code: String,
    message: String,
}

/// Solo HTTPS. El servidor local de desarrollo es la única excepción, y no existe en los instaladores.
fn usable_url(url: &str) -> Option<String> {
    let url = url.trim().trim_end_matches('/');
    let local = cfg!(debug_assertions)
        && (url.starts_with("http://127.0.0.1") || url.starts_with("http://localhost"));
    (url.len() > "https://".len() && (url.starts_with("https://") || local)).then(|| url.to_owned())
}

fn map_error(detail: ErrorDetail) -> AiError {
    match detail.code.as_str() {
        "service_quota" => AiError::QuotaExhausted,
        "daily_limit" => AiError::DailyLimit,
        "region_unavailable" => AiError::RegionUnavailable,
        "country_unknown" => AiError::RegionUnknown,
        "rate_limited" => AiError::RateLimited,
        "timeout" => AiError::Timeout,
        "unavailable" => AiError::Unavailable,
        _ => AiError::Rejected(detail.message),
    }
}

/// Una línea JSON y después la imagen: así el servidor no tiene que analizar 1 MB de JSON.
fn capture_body(question: &str, image_jpeg: &[u8]) -> Vec<u8> {
    let mut body = json!({ "question": question, "imageType": "image/jpeg" })
        .to_string()
        .into_bytes();
    body.push(b'\n');
    body.extend_from_slice(STANDARD.encode(image_jpeg).as_bytes());
    body
}

impl NexiumServer {
    pub fn new(base_url: Option<&str>, install_id: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.and_then(usable_url),
            install_id,
        }
    }

    fn request(&self, path: &str, timeout: Duration) -> Result<RequestBuilder, AiError> {
        let base_url = self.base_url.as_deref().ok_or(AiError::NotConfigured)?;
        Ok(self
            .http
            .post(format!("{base_url}{path}"))
            .header(INSTALL_HEADER, &self.install_id)
            .timeout(timeout))
    }

    async fn send(request: RequestBuilder) -> Result<ModelReply, AiError> {
        let response = request.send().await.map_err(|error| {
            if error.is_timeout() {
                AiError::Timeout
            } else {
                AiError::Network
            }
        })?;
        Self::read(response).await
    }

    async fn read(response: Response) -> Result<ModelReply, AiError> {
        if response.status().is_success() {
            return response.json().await.map_err(|_| AiError::InvalidResponse);
        }
        // Una página de error de la plataforma no trae nuestro formato.
        match response.json::<ErrorEnvelope>().await {
            Ok(envelope) => Err(map_error(envelope.error)),
            Err(_) => Err(AiError::Unavailable),
        }
    }
}

#[async_trait]
impl AiProvider for NexiumServer {
    fn display_name(&self) -> &str {
        "Google Gemini"
    }

    fn data_notice(&self) -> &str {
        "Tus solicitudes y capturas pasan por el servidor de Nexium hacia Google Gemini. El servidor no las guarda, pero en la capa gratuita Google puede usar lo que recibe para mejorar sus productos, y personas pueden revisarlo. No captures información personal ni confidencial."
    }

    fn quota_message(&self) -> String {
        "Se agotó la cuota diaria gratuita del servicio de IA de Nexium. Vuelve a intentarlo mañana.".into()
    }

    fn is_configured(&self) -> bool {
        self.base_url.is_some()
    }

    async fn interpret(&self, request: InterpretRequest) -> Result<AiAnswer<Intent>, AiError> {
        let now = request.now.format("%Y-%m-%dT%H:%M:%S%:z").to_string();
        let reply = Self::send(
            self.request("/v1/interpret", INTERPRET_TIMEOUT)?
                .json(&json!({ "text": request.text, "now": now })),
        )
        .await?;
        let value = schema::parse_interpretation(&reply.output, &request.text)?;
        Ok(AiAnswer {
            value,
            tokens: reply.tokens,
        })
    }

    async fn analyze_image(
        &self,
        request: VisionRequest,
    ) -> Result<AiAnswer<ScreenAnalysis>, AiError> {
        let body = capture_body(&request.question, &request.image_jpeg);
        let reply = Self::send(
            self.request("/v1/analyze", VISION_TIMEOUT)?
                .header(CONTENT_TYPE, CAPTURE_CONTENT_TYPE)
                .body(body),
        )
        .await?;
        Ok(AiAnswer {
            value: schema::parse_screen_analysis(&reply.output)?,
            tokens: reply.tokens,
        })
    }

    async fn analyze_document(
        &self,
        request: DocumentRequest,
    ) -> Result<AiAnswer<DocumentSummary>, AiError> {
        let reply = Self::send(
            self.request("/v1/document", DOCUMENT_TIMEOUT)?
                .json(&json!({
                    "task": request.task.as_str(),
                    "format": request.format,
                    "text": request.text,
                })),
        )
        .await?;
        Ok(AiAnswer {
            value: schema::parse_document_summary(&reply.output)?,
            tokens: reply.tokens,
        })
    }
}

#[cfg(test)]
#[path = "tests/server.rs"]
mod tests;
