//! Capa independiente de IA. Nada fuera de este módulo conoce el formato de un proveedor.

pub mod gemini;
pub mod schema;

use async_trait::async_trait;
use chrono::{DateTime, Local};

use crate::{error::AiError, models::Intent};

pub struct InterpretRequest {
    pub text: String,
    pub now: DateTime<Local>,
    pub model: String,
}

pub struct VisionRequest {
    pub image_jpeg: Vec<u8>,
    pub question: String,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiAnswer<T> {
    pub value: T,
    pub tokens: u64,
}

#[async_trait]
pub trait AiProvider: Send + Sync {
    fn id(&self) -> &str;
    fn display_name(&self) -> &str;
    /// Qué ocurre con los datos enviados. Se muestra antes de cada captura y en Ajustes.
    fn data_notice(&self) -> &str;
    fn key_url(&self) -> &str;
    /// Cuenta de la clave en el almacén seguro.
    fn key_account(&self) -> &str;
    /// Variable de `.env` que se admite solo en desarrollo.
    fn dev_key_variable(&self) -> &str;
    /// Mensaje en español para un error de cuota de este proveedor.
    fn quota_message(&self) -> String;
    async fn interpret(&self, request: InterpretRequest) -> Result<AiAnswer<Intent>, AiError>;
    async fn analyze_image(&self, request: VisionRequest) -> Result<AiAnswer<String>, AiError>;
}
