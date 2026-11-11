//! Adaptador del servidor de Nexium: la única vía de IA en los instaladores.
//! El servidor decide modelo e instrucciones; aquí solo viaja lo que escribió el usuario.

use std::time::Duration;

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::{header::CONTENT_TYPE, RequestBuilder, Response};
use serde::Deserialize;
use serde_json::json;

use super::{schema, AiAnswer, AiProvider, InterpretRequest, VisionRequest};
use crate::{
    error::AiError,
    models::{Intent, ScreenAnalysis},
};

/// Mayores que los del servidor, que puede reintentar una vez: quien corta primero es él.
const INTERPRET_TIMEOUT: Duration = Duration::from_secs(45);
const VISION_TIMEOUT: Duration = Duration::from_secs(130);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, Local, TimeZone};
    use serde_json::Value;
    use wiremock::{
        matchers::{body_json, body_string_contains, header, method, path},
        Mock, MockServer, ResponseTemplate,
    };

    const INSTALL: &str = "3f2b8a10-5c4e-4d7a-9b1c-0a1b2c3d4e5f";

    fn provider(server: &MockServer) -> NexiumServer {
        NexiumServer::new(Some(&server.uri()), INSTALL.into())
    }

    fn request() -> InterpretRequest {
        InterpretRequest {
            text: "Busca mi informe".into(),
            now: Local::now(),
        }
    }

    fn vision() -> VisionRequest {
        VisionRequest {
            image_jpeg: vec![1, 2, 3],
            question: "¿Qué es?".into(),
        }
    }

    async fn replying(status: u16, body: Value) -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(status).set_body_json(body))
            .expect(1)
            .mount(&server)
            .await;
        server
    }

    async fn error_for(status: u16, code: &str) -> AiError {
        let server = replying(
            status,
            json!({ "error": { "code": code, "message": "Mensaje del servidor." } }),
        )
        .await;
        provider(&server).interpret(request()).await.unwrap_err()
    }

    #[tokio::test]
    async fn sends_only_the_text_the_time_and_the_anonymous_id() {
        let server = MockServer::start().await;
        let now = FixedOffset::west_opt(5 * 3600)
            .unwrap()
            .with_ymd_and_hms(2026, 11, 9, 10, 30, 0)
            .unwrap()
            .with_timezone(&Local);
        let expected = now.format("%Y-%m-%dT%H:%M:%S%:z").to_string();
        Mock::given(method("POST"))
            .and(path("/v1/interpret"))
            .and(header(INSTALL_HEADER, INSTALL))
            .and(body_json(json!({ "text": "Busca mi informe", "now": expected })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "output": r#"{"intent":"search_files","search_terms":["informe"]}"#, "tokens": 42 })))
            .expect(1)
            .mount(&server)
            .await;
        let answer = provider(&server)
            .interpret(InterpretRequest {
                text: "Busca mi informe".into(),
                now,
            })
            .await
            .unwrap();
        assert_eq!(answer.tokens, 42);
        assert!(matches!(answer.value, Intent::SearchFiles(_)));
    }

    #[tokio::test]
    async fn sends_the_capture_as_base64() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/analyze"))
            .and(header(CONTENT_TYPE.as_str(), CAPTURE_CONTENT_TYPE))
            .and(body_string_contains("\"question\":\"¿Qué es?\""))
            .and(body_string_contains("\"imageType\":\"image/jpeg\""))
            .and(body_string_contains("}\nAQID"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "output": r#"{"summary":"Un error de ruta.","cause":null,"steps":[]}"#, "tokens": 9 })))
            .expect(1)
            .mount(&server)
            .await;
        assert_eq!(
            provider(&server)
                .analyze_image(vision())
                .await
                .unwrap()
                .value
                .summary,
            "Un error de ruta."
        );
    }

    #[tokio::test]
    async fn a_free_text_analysis_is_rejected() {
        let server = replying(
            200,
            json!({ "output": "### Error\n**Falta** un archivo", "tokens": 9 }),
        )
        .await;
        assert_eq!(
            provider(&server).analyze_image(vision()).await.unwrap_err(),
            AiError::InvalidResponse
        );
    }

    #[tokio::test]
    async fn each_server_error_has_its_meaning_and_is_not_retried() {
        assert_eq!(
            error_for(429, "service_quota").await,
            AiError::QuotaExhausted
        );
        assert_eq!(error_for(429, "daily_limit").await, AiError::DailyLimit);
        assert_eq!(error_for(429, "rate_limited").await, AiError::RateLimited);
        assert_eq!(
            error_for(451, "region_unavailable").await,
            AiError::RegionUnavailable
        );
        assert_eq!(
            error_for(451, "country_unknown").await,
            AiError::RegionUnknown
        );
        assert_eq!(error_for(504, "timeout").await, AiError::Timeout);
        assert_eq!(error_for(503, "unavailable").await, AiError::Unavailable);
    }

    #[tokio::test]
    async fn a_rejection_shows_the_message_of_the_server() {
        assert_eq!(
            error_for(413, "payload_too_large").await,
            AiError::Rejected("Mensaje del servidor.".into())
        );
    }

    #[tokio::test]
    async fn a_platform_error_page_is_an_outage() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(429).set_body_string("<html>error 1027</html>"))
            .mount(&server)
            .await;
        assert_eq!(
            provider(&server).interpret(request()).await.unwrap_err(),
            AiError::Unavailable
        );
    }

    #[tokio::test]
    async fn an_invalid_answer_is_rejected() {
        let server = replying(200, json!({ "output": r#"{"intent":"format_disk"}"# })).await;
        assert_eq!(
            provider(&server).interpret(request()).await.unwrap_err(),
            AiError::InvalidResponse
        );
        let server = replying(200, json!({ "otra": "cosa" })).await;
        assert_eq!(
            provider(&server).analyze_image(vision()).await.unwrap_err(),
            AiError::InvalidResponse
        );
    }

    #[tokio::test]
    async fn an_unreachable_server_is_a_network_error() {
        let provider = NexiumServer::new(Some("http://127.0.0.1:9"), INSTALL.into());
        assert_eq!(
            provider.interpret(request()).await.unwrap_err(),
            AiError::Network
        );
    }

    #[tokio::test]
    async fn without_a_server_nothing_is_sent() {
        let provider = NexiumServer::new(None, INSTALL.into());
        assert!(!provider.is_configured());
        assert_eq!(
            provider.interpret(request()).await.unwrap_err(),
            AiError::NotConfigured
        );
    }

    #[test]
    fn a_line_break_in_the_question_stays_inside_the_first_line() {
        let body = String::from_utf8(capture_body("una\ndos", &[1, 2, 3])).unwrap();
        let (line, image) = body.split_once('\n').unwrap();
        assert_eq!(image, "AQID");
        assert_eq!(
            serde_json::from_str::<Value>(line).unwrap()["question"],
            "una\ndos"
        );
    }

    #[test]
    fn png_and_jpeg_captures_leave_as_standard_base64_of_a_jpeg() {
        let source = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(640, 400, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x ^ y) % 256) as u8])
        }));
        for format in [image::ImageFormat::Png, image::ImageFormat::Jpeg] {
            let mut file = std::io::Cursor::new(Vec::new());
            source.write_to(&mut file, format).unwrap();
            let loaded = image::load_from_memory(file.get_ref()).unwrap();
            let jpeg = crate::providers::capture::encode_for_analysis(loaded).unwrap();
            let body = String::from_utf8(capture_body("¿Qué es?", &jpeg)).unwrap();
            let (_, encoded) = body.split_once('\n').unwrap();
            assert_eq!(STANDARD.decode(encoded).unwrap(), jpeg, "{format:?}");
            assert_eq!(&jpeg[..3], &[0xFF, 0xD8, 0xFF], "{format:?}");
            assert!((0xC0..=0xFE).contains(&jpeg[3]), "{format:?}");
        }
    }

    #[test]
    fn only_https_addresses_are_used() {
        assert_eq!(
            usable_url(" https://nexium.example/ ").as_deref(),
            Some("https://nexium.example")
        );
        for url in [
            "http://nexium.example",
            "ftp://nexium.example",
            "https://",
            "",
        ] {
            assert_eq!(usable_url(url), None, "{url}");
        }
    }

    #[test]
    fn the_notice_names_the_server_and_the_use_by_google() {
        let provider = NexiumServer::new(None, INSTALL.into());
        assert!(provider.data_notice().contains("servidor de Nexium"));
        assert!(provider.data_notice().contains("Google puede usar"));
    }
}
