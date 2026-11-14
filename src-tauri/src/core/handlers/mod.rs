//! Un `CommandHandler` por operación. El enrutador solo conoce esta interfaz.

mod conversation;
mod files;
mod reminders;
mod system;
mod time;
mod weather;

use async_trait::async_trait;
use chrono::{DateTime, Local};
use tokio_util::sync::CancellationToken;

pub use conversation::{ClarifyHandler, ScreenHandler};
pub use files::FilesHandler;
pub use reminders::{draft_reminder, ReminderIntentHandler};
pub use system::SystemHandler;
pub use time::TimeHandler;
pub use weather::WeatherHandler;

use crate::models::{Category, Intent, InteractionStatus, ResponseBody};

pub const ORIGIN_LOCAL: &str = "lectura directa del equipo, sin IA";

#[derive(Debug, PartialEq)]
pub struct Outcome {
    pub category: Category,
    pub status: InteractionStatus,
    pub summary: String,
    pub body: ResponseBody,
    /// Origen de los datos, si no es el equipo. La IA interpreta, pero los datos no salen de ella.
    pub source: Option<String>,
}

impl Outcome {
    pub fn done(category: Category, summary: impl Into<String>, body: ResponseBody) -> Self {
        Self {
            category,
            status: InteractionStatus::Completed,
            summary: summary.into(),
            body,
            source: None,
        }
    }

    pub fn failed(
        category: Category,
        code: &str,
        message: impl Into<String>,
        hint: Option<String>,
    ) -> Self {
        let message = message.into();
        Self {
            category,
            status: InteractionStatus::Failed,
            summary: message.clone(),
            body: ResponseBody::Error {
                code: code.into(),
                message,
                hint,
            },
            source: None,
        }
    }

    pub fn pending(category: Category, summary: impl Into<String>, body: ResponseBody) -> Self {
        Self {
            category,
            status: InteractionStatus::Pending,
            summary: summary.into(),
            body,
            source: None,
        }
    }
}

pub struct HandlerContext {
    pub request: String,
    pub now: DateTime<Local>,
    pub cancel: CancellationToken,
}

#[async_trait]
pub trait CommandHandler: Send + Sync {
    /// Atiende la intención si le corresponde. Si no, devuelve `None` y el asistente prueba el siguiente.
    async fn handle(&self, intent: &Intent, context: &HandlerContext) -> Option<Outcome>;
}

#[cfg(test)]
pub(crate) mod test_support {
    use chrono::{DateTime, Local, TimeZone};

    pub fn now() -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 11, 5, 10, 0, 0).unwrap()
    }
}
