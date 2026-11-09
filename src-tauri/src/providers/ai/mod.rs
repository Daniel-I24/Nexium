//! Capa independiente de IA. Nada fuera de este módulo conoce el formato de un proveedor.

#[cfg(debug_assertions)]
pub mod gemini;
pub mod schema;
pub mod server;

use std::sync::Mutex;

use async_trait::async_trait;
use chrono::{DateTime, Local};
use serde::Serialize;

use crate::{error::AiError, models::Intent};

pub struct InterpretRequest {
    pub text: String,
    pub now: DateTime<Local>,
}

pub struct VisionRequest {
    pub image_jpeg: Vec<u8>,
    pub question: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiAnswer<T> {
    pub value: T,
    pub tokens: u64,
}

#[async_trait]
pub trait AiProvider: Send + Sync {
    fn display_name(&self) -> &str;
    /// Qué ocurre con los datos enviados. Se muestra antes de cada captura y en Ajustes.
    fn data_notice(&self) -> &str;
    /// Mensaje en español para un error de cuota de este proveedor.
    fn quota_message(&self) -> String;
    fn is_configured(&self) -> bool;
    async fn interpret(&self, request: InterpretRequest) -> Result<AiAnswer<Intent>, AiError>;
    async fn analyze_image(&self, request: VisionRequest) -> Result<AiAnswer<String>, AiError>;
}

/// Estado del servicio según su último uso real: consultarlo aparte gastaría cuota.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AiServiceStatus {
    NotConfigured,
    NotUsedYet,
    Working,
    Limited,
    QuotaExhausted,
    RegionUnavailable,
    Unreachable,
}

#[derive(Default)]
pub struct AiStatusTracker(Mutex<Option<AiServiceStatus>>);

impl AiStatusTracker {
    pub fn record<T>(&self, result: &Result<T, AiError>) {
        let status = match result {
            Ok(_) => AiServiceStatus::Working,
            Err(AiError::RateLimited | AiError::DailyLimit) => AiServiceStatus::Limited,
            Err(AiError::QuotaExhausted) => AiServiceStatus::QuotaExhausted,
            Err(AiError::RegionUnavailable | AiError::RegionUnknown) => AiServiceStatus::RegionUnavailable,
            Err(AiError::Network | AiError::Timeout | AiError::Unavailable) => AiServiceStatus::Unreachable,
            // El resto no dice nada sobre el servicio: una cancelación o una solicitud rechazada, por ejemplo.
            Err(_) => return,
        };
        if let Ok(mut current) = self.0.lock() {
            *current = Some(status);
        }
    }

    pub fn current(&self, configured: bool) -> AiServiceStatus {
        if !configured {
            return AiServiceStatus::NotConfigured;
        }
        self.0.lock().ok().and_then(|current| *current).unwrap_or(AiServiceStatus::NotUsedYet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn after(result: Result<(), AiError>) -> AiServiceStatus {
        let tracker = AiStatusTracker::default();
        tracker.record(&result);
        tracker.current(true)
    }

    #[test]
    fn the_status_follows_the_last_real_use() {
        assert_eq!(after(Ok(())), AiServiceStatus::Working);
        assert_eq!(after(Err(AiError::RateLimited)), AiServiceStatus::Limited);
        assert_eq!(after(Err(AiError::DailyLimit)), AiServiceStatus::Limited);
        assert_eq!(after(Err(AiError::QuotaExhausted)), AiServiceStatus::QuotaExhausted);
        assert_eq!(after(Err(AiError::RegionUnavailable)), AiServiceStatus::RegionUnavailable);
        assert_eq!(after(Err(AiError::RegionUnknown)), AiServiceStatus::RegionUnavailable);
        assert_eq!(after(Err(AiError::Network)), AiServiceStatus::Unreachable);
        assert_eq!(after(Err(AiError::Cancelled)), AiServiceStatus::NotUsedYet);
    }

    #[test]
    fn an_unconfigured_service_says_so_whatever_happened_before() {
        let tracker = AiStatusTracker::default();
        tracker.record::<()>(&Ok(()));
        assert_eq!(tracker.current(false), AiServiceStatus::NotConfigured);
    }
}
