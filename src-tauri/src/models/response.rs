//! Respuestas que el núcleo entrega a la interfaz, entradas del historial y etapas de una solicitud.

use serde::Serialize;

use super::{Category, DocumentCandidate, DocumentTask, InteractionStatus, Section, WeatherWhen};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DataRow {
    pub label: String,
    pub value: String,
    pub mono: bool,
    pub unavailable: bool,
}

impl DataRow {
    pub fn value(label: &str, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            mono: true,
            unavailable: false,
        }
    }

    pub fn text(label: &str, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            mono: false,
            unavailable: false,
        }
    }

    pub fn unavailable(label: &str) -> Self {
        Self {
            label: label.into(),
            value: "No disponible".into(),
            mono: false,
            unavailable: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileResult {
    pub name: String,
    pub extension: String,
    pub location: String,
    pub path: String,
    pub exists: bool,
    /// Se encontró por lo que contiene, no por su nombre.
    pub matched_in_content: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReminderOption {
    pub label: String,
    pub due_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedRequest {
    pub label: String,
    pub request: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Attribution {
    pub text: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Reminder {
    pub id: i64,
    pub content: String,
    pub due_at: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ResponseBody {
    Reading {
        value: String,
        unit: String,
        context: String,
        percent: Option<f64>,
        rows: Vec<DataRow>,
        attribution: Option<Attribution>,
    },
    Rows {
        rows: Vec<DataRow>,
        note: Option<String>,
        attribution: Option<Attribution>,
    },
    Text {
        text: String,
    },
    ScreenAnalysis {
        summary: String,
        cause: Option<String>,
        steps: Vec<String>,
    },
    Files {
        summary: String,
        files: Vec<FileResult>,
        note: Option<String>,
    },
    ReminderDraft {
        content: String,
        due_at: Option<String>,
        question: Option<String>,
        options: Vec<ReminderOption>,
    },
    ReminderSaved {
        reminder: Reminder,
    },
    Clarification {
        question: String,
        suggestions: Vec<SuggestedRequest>,
    },
    FolderConsent {
        folders: Vec<String>,
        pending_request: String,
    },
    CaptureRequest {
        question: String,
    },
    /// Pide confirmación antes de enviar el contenido de un documento a la IA. Sin candidatos, ofrece elegir uno.
    DocumentConsent {
        task: DocumentTask,
        provider: String,
        candidates: Vec<DocumentCandidate>,
        /// Caracteres que se envían como máximo.
        max_characters: usize,
        /// Por qué no hay candidatos, si no los hay.
        note: Option<String>,
        /// El candidato que se marca de entrada, solo si la coincidencia es clara.
        preselected: Option<String>,
    },
    /// Resumen o análisis de un documento, ya validado y en texto plano.
    DocumentAnalysis {
        file_name: String,
        task: DocumentTask,
        summary: String,
        points: Vec<String>,
        details: Vec<String>,
        /// Solo se envió el principio del documento.
        truncated: bool,
    },
    /// La solicitud de clima no nombra ciudad y no hay una en Ajustes: se pregunta, y se ofrece guardarla.
    WeatherCityQuestion {
        when: WeatherWhen,
    },
    /// Abre una sección de la app. La interfaz cambia de pantalla al recibirla.
    Navigate {
        section: Section,
    },
    Error {
        code: String,
        message: String,
        hint: Option<String>,
    },
}

/// Análisis de una captura, ya validado y en texto plano.
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenAnalysis {
    pub summary: String,
    pub cause: Option<String>,
    pub steps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AssistantResponse {
    pub id: i64,
    pub request: String,
    pub created_at: String,
    pub category: Category,
    pub status: InteractionStatus,
    pub origin_label: String,
    pub summary: String,
    pub body: ResponseBody,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: i64,
    pub request: String,
    pub created_at: String,
    pub category: Category,
    pub status: InteractionStatus,
    pub response: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Stage {
    Idle,
    Listening,
    Transcribing,
    Interpreting,
    Executing,
    Capturing,
    Analyzing,
    /// Leyendo un documento en el equipo.
    Reading,
    /// La IA resume o analiza un documento.
    Summarizing,
    Responding,
}
