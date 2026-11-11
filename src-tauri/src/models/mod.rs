//! Contratos con la interfaz. `src/types/api.ts` refleja estos tipos.

use chrono::{DateTime, Local, NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};

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
        Self { label: label.into(), value: value.into(), mono: true, unavailable: false }
    }

    pub fn text(label: &str, value: impl Into<String>) -> Self {
        Self { label: label.into(), value: value.into(), mono: false, unavailable: false }
    }

    pub fn unavailable(label: &str) -> Self {
        Self { label: label.into(), value: "No disponible".into(), mono: false, unavailable: true }
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
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
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
    Responding,
}

/// Lectura del sistema: un valor real o la causa de que no exista. No hay tercera opción.
#[derive(Debug, Clone, PartialEq)]
pub enum Reading<T> {
    Value(T),
    Unavailable(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MemoryInfo {
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BatteryInfo {
    pub percent: f64,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WifiInfo {
    pub connected: bool,
    pub ssid: Reading<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemMetric {
    Cpu,
    Ram,
    Battery,
    Internet,
    Wifi,
    DarkMode,
    Overview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Toda operación que Nexium puede ejecutar. Ni reglas ni IA pueden pedir otra cosa.
#[derive(Debug, Clone, PartialEq)]
pub enum Intent {
    Time,
    System(SystemMetric),
    Weather { city: Option<String>, when: WeatherWhen },
    SearchFiles(FileQuery),
    CreateReminder(ReminderRequest),
    ListReminders,
    AnalyzeScreen { question: String },
    Clarify { question: String, suggestions: Vec<String> },
    Unknown,
}

pub fn now_local() -> DateTime<Local> {
    Local::now()
}
