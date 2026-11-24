//! Contratos con la interfaz. `src/types/api.ts` refleja estos tipos.

use chrono::{NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};

mod response;
mod system;

pub use response::*;
pub use system::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    Time,
    System,
    Files,
    Weather,
    Reminders,
    Screen,
    General,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Time => "time",
            Self::System => "system",
            Self::Files => "files",
            Self::Weather => "weather",
            Self::Reminders => "reminders",
            Self::Screen => "screen",
            Self::General => "general",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "time" => Self::Time,
            "system" => Self::System,
            "files" => Self::Files,
            "weather" => Self::Weather,
            "reminders" => Self::Reminders,
            "screen" => Self::Screen,
            _ => Self::General,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InteractionStatus {
    Completed,
    Pending,
    Failed,
    Cancelled,
}

impl InteractionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Pending => "pending",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "completed" => Self::Completed,
            "pending" => Self::Pending,
            "cancelled" => Self::Cancelled,
            _ => Self::Failed,
        }
    }
}

/// Qué se pide de un documento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DocumentTask {
    Summarize,
    Analyze,
}

impl DocumentTask {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Summarize => "summarize",
            Self::Analyze => "analyze",
        }
    }
}

/// Un archivo que se ofrece para resumir. La interfaz solo recibe un identificador, nunca la ruta para leerlo.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentCandidate {
    pub token: String,
    pub name: String,
    pub extension: String,
    pub location: String,
}

/// Resumen de un documento devuelto por la IA.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentSummary {
    pub summary: String,
    pub points: Vec<String>,
    /// Fechas, cifras, plazos o definiciones que aparecen en el documento.
    pub details: Vec<String>,
}

/// Secciones de la app que se pueden abrir con una orden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Section {
    Assistant,
    Reminders,
    History,
    Settings,
}

impl Section {
    pub fn label(self) -> &'static str {
        match self {
            Self::Assistant => "Asistente",
            Self::Reminders => "Recordatorios",
            Self::History => "Historial",
            Self::Settings => "Ajustes",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WeatherWhen {
    Now,
    Today,
    Tomorrow,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct FileQuery {
    pub exact_name: Option<String>,
    pub terms: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReminderRequest {
    pub content: String,
    pub date: Option<NaiveDate>,
    pub time: Option<NaiveTime>,
    pub ambiguity: Option<String>,
}

/// Una operación con el fragmento de la petición que la pidió. Una petición puede traer varias.
#[derive(Debug, Clone, PartialEq)]
pub struct Action {
    pub request: String,
    pub intent: Intent,
}

/// Toda operación que Nexium puede ejecutar. Ni reglas ni IA pueden pedir otra cosa.
#[derive(Debug, Clone, PartialEq)]
pub enum Intent {
    Time,
    System(SystemMetric),
    Weather {
        city: Option<String>,
        when: WeatherWhen,
    },
    SearchFiles(FileQuery),
    CreateReminder(ReminderRequest),
    ListReminders,
    OpenSection(Section),
    /// Resumir o analizar un documento. Sin nombre ni términos, se ofrece elegirlo en el selector del sistema.
    Document {
        task: DocumentTask,
        query: FileQuery,
    },
    AnalyzeScreen {
        question: String,
    },
    Clarify {
        question: String,
        suggestions: Vec<String>,
    },
    Unknown,
}
