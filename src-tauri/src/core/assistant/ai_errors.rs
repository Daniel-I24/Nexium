//! Comprobaciones previas a la IA y el mensaje de cada error. Ninguno se reintenta ni cambia de plan.

use chrono::{DateTime, Local};

use super::Assistant;
use crate::{core::handlers::Outcome, error::AiError, models::Category};

impl Assistant {
    /// Comprueba lo que no cuesta nada antes de gastar cuota.
    pub(crate) fn check_ai_available(&self) -> Result<(), AiError> {
        if !self.ai.is_configured() {
            return Err(AiError::NotConfigured);
        }
        Ok(())
    }

    /// Mensaje en español para cada error de IA. Ninguno se reintenta ni cambia de plan automáticamente.
    pub fn describe_ai_error(&self, error: &AiError) -> (String, Option<String>) {
        let local_hint = "Las consultas de hora, sistema y red siguen funcionando.".to_owned();
        match error {
            AiError::NotConfigured => ("Esta versión de Nexium no incluye el servicio de IA.".into(), Some(local_hint)),
            AiError::QuotaExhausted(resets_in) => (self.ai.quota_message(), Some(format!("{} {local_hint}", reset_sentence(*resets_in, Local::now())))),
            AiError::RateLimited => ("El servicio de IA recibió demasiadas solicitudes en poco tiempo.".into(), Some(format!("Espera un minuto y vuelve a intentarlo. {local_hint}"))),
            AiError::DailyLimit(resets_in) => ("Se alcanzó el límite diario de uso de IA de este equipo o esta red.".into(), Some(format!("{} {local_hint}", reset_sentence(*resets_in, Local::now())))),
            AiError::RegionUnavailable => ("El servicio de IA de Nexium no está disponible en tu región.".into(), Some(format!("No se ofrece en el Espacio Económico Europeo, Reino Unido ni Suiza. {local_hint}"))),
            AiError::RegionUnknown => ("No se pudo determinar tu país, y el servicio de IA de Nexium no se ofrece sin conocerlo.".into(), Some(format!("Si usas Tor o una conexión que oculte tu ubicación, prueba con otra conexión. {local_hint}"))),
            AiError::Unavailable => (
                "El servidor de IA no responde en este momento.".into(),
                Some(format!("El fallo está en el servicio (Google o el servidor de Nexium), no en tu equipo ni en tu conexión. Vuelve a intentarlo en unos minutos. {local_hint}")),
            ),
            AiError::Network => ("Sin conexión con el servidor de IA de Nexium.".into(), Some(format!("Comprueba tu conexión a Internet. {local_hint}"))),
            AiError::Timeout => ("La IA tardó demasiado en responder y Nexium dejó de esperar.".into(), Some(format!("Suele ser una saturación pasajera del servicio. Vuelve a intentarlo en un momento. {local_hint}"))),
            AiError::InvalidResponse => ("No pude interpretar la respuesta del modelo.".into(), Some("Prueba a escribir la solicitud de otra forma.".into())),
            AiError::Rejected(reason) => (format!("El servicio de IA rechazó la solicitud. {reason}"), None),
            AiError::Cancelled => ("Operación cancelada.".into(), None),
        }
    }

    pub(crate) fn ai_failure(&self, category: Category, error: &AiError) -> Outcome {
        let (message, hint) = self.describe_ai_error(error);
        Outcome::failed(category, error.code(), message, hint)
    }
}

/// Cuándo vuelve a haber IA, en la hora del equipo: el servidor indica los segundos que faltan.
pub(crate) fn reset_sentence(resets_in: Option<u64>, now: DateTime<Local>) -> String {
    let Some(at) = resets_in
        .and_then(|seconds| i64::try_from(seconds).ok())
        .and_then(|seconds| now.checked_add_signed(chrono::Duration::seconds(seconds)))
    else {
        return "Vuelve a intentarlo mañana.".into();
    };
    let day = match (at.date_naive() - now.date_naive()).num_days() {
        0 => "hoy".to_owned(),
        1 => "mañana".to_owned(),
        _ => at.format("el %d/%m").to_string(),
    };
    format!(
        "Se restablece {day} a las {}, hora de tu equipo.",
        at.format("%H:%M")
    )
}
