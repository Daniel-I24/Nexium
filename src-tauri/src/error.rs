use thiserror::Error;

/// Errores neutros de IA: el resto de la aplicación no conoce a ningún proveedor.
#[derive(Debug, Error, PartialEq)]
pub enum AiError {
    #[error("Esta versión no incluye el servicio de IA.")]
    NotConfigured,
    #[error("Se agotó la cuota diaria.")]
    QuotaExhausted,
    #[error("Demasiadas solicitudes en poco tiempo.")]
    RateLimited,
    #[error("Se alcanzó el límite diario configurado en Nexium.")]
    LocalLimitReached,
    #[error("Se alcanzó el límite diario de uso del servicio.")]
    DailyLimit,
    #[error("El proveedor rechazó la solicitud: {0}")]
    Rejected(String),
    #[error("El servicio de IA no está disponible.")]
    Unavailable,
    #[error("No hay conexión con el servicio de IA.")]
    Network,
    #[error("El servicio de IA tardó demasiado en responder.")]
    Timeout,
    #[error("La respuesta del modelo no es válida.")]
    InvalidResponse,
    #[error("Operación cancelada.")]
    Cancelled,
}

impl AiError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotConfigured => "notConfigured",
            Self::QuotaExhausted => "quotaExhausted",
            Self::RateLimited => "rateLimited",
            Self::LocalLimitReached => "localLimit",
            Self::DailyLimit => "dailyLimit",
            Self::Rejected(_) => "rejected",
            Self::Unavailable => "unavailable",
            Self::Network => "network",
            Self::Timeout => "timeout",
            Self::InvalidResponse => "invalidResponse",
            Self::Cancelled => "cancelled",
        }
    }
}

/// Errores que la interfaz recibe como texto en español, sin detalles internos.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Validation(String),
    #[error("No se pudo acceder a los datos locales: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("{0}")]
    Operation(String),
}

impl serde::Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
