//! Validación de las respuestas del modelo. Son entrada no confiable:
//! solo se convierten en datos de Nexium si cada campo es válido.

use std::sync::LazyLock;

use chrono::{NaiveDate, NaiveTime};
use regex::Regex;
use serde::Deserialize;

use crate::{
    error::AiError,
    models::{
        DocumentSummary, FileQuery, Intent, ReminderRequest, ScreenAnalysis, SystemMetric,
        WeatherWhen,
    },
};

const MAX_TERMS: usize = 8;
const MAX_TEXT: usize = 300;
const MAX_CAUSE: usize = 600;
const MAX_STEPS: usize = 3;
/// Ideas clave y datos importantes de un documento, como mucho.
pub const MAX_POINTS: usize = 8;

/// Marcas de bloque al inicio de una línea: títulos, citas, viñetas y numeración.
static BLOCK_MARK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(?:#{1,6}\s+|>\s*|[-*+•]\s+|\d{1,2}[.)]\s+)").unwrap());
/// Enlaces: se conserva el texto y se descarta la dirección.
static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([^\]]*)\]\([^)]*\)").unwrap());
/// Negrita: dos asteriscos a cada lado.
static BOLD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\*\*([^*]+)\*\*").unwrap());
/// Cursiva: un asterisco pegado a cada extremo de una palabra. "*.log" o "2 * 3" no lo son.
static ITALIC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(^|[^\w*])\*([^*\s](?:[^*]*[^*\s])?)\*([^\w*]|$)").unwrap());

/// Quita las marcas de énfasis y enlaces de un fragmento que no es código.
fn without_emphasis(segment: &str) -> String {
    let segment = LINK.replace_all(segment, "${1}");
    let mut segment = BOLD.replace_all(&segment, "${1}").into_owned();
    // Dos cursivas seguidas comparten el separador: cada pasada resuelve una.
    for _ in 0..3 {
        let next = ITALIC.replace_all(&segment, "${1}${2}${3}").into_owned();
        if next == segment {
            break;
        }
        segment = next;
    }
    segment.replace("**", "")
}

/// La interfaz muestra texto plano: se quitan las marcas de Markdown, nunca el contenido.
/// Lo que va entre comillas invertidas (rutas, comandos, nombres de archivo) se conserva tal cual,
/// y los guiones bajos nunca se tocan, porque aparecen en nombres de archivo.
pub fn plain_text(text: &str) -> String {
    let lines: Vec<String> = text
        .lines()
        .map(|line| {
            let line = BLOCK_MARK.replace(line, "");
            let mut parts: Vec<String> = line.split('`').map(str::to_owned).collect();
            // Una comilla invertida sin cerrar no abre código: se une al texto anterior.
            if parts.len().is_multiple_of(2) {
                let unclosed = parts.pop().unwrap_or_default();
                if let Some(previous) = parts.last_mut() {
                    previous.push_str(&unclosed);
                }
            }
            parts
                .iter()
                .enumerate()
                .map(|(index, part)| {
                    if index % 2 == 1 {
                        part.clone()
                    } else {
                        without_emphasis(part)
                    }
                })
                .collect::<String>()
        })
        .filter(|line| !line.trim().is_empty())
        .collect();
    lines
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Recorta por palabras, sin cortar una a la mitad.
fn shorten(text: String, max: usize) -> String {
    if text.chars().count() <= max {
        return text;
    }
    let cut: String = text.chars().take(max).collect();
    let kept = cut.rsplit_once(' ').map_or(cut.as_str(), |(head, _)| head);
    format!("{}…", kept.trim_end_matches([',', ';', ':', ' ']))
}

static ENTITY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"&(#[xX][0-9a-fA-F]{1,6}|#[0-9]{1,7}|amp|lt|gt|quot|apos|nbsp);")
        .expect("expresión regular válida")
});

/// El modelo a veces escribe entidades HTML («qu&#xe9;») o caracteres de control. Se traducen o se quitan.
pub fn decode_entities(text: &str) -> String {
    let decoded = ENTITY.replace_all(text, |captures: &regex::Captures| {
        let entity = &captures[1];
        let character = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ if entity.starts_with("#x") || entity.starts_with("#X") => {
                u32::from_str_radix(&entity[2..], 16)
                    .ok()
                    .and_then(char::from_u32)
            }
            _ => entity[1..].parse().ok().and_then(char::from_u32),
        };
        character.map_or_else(String::new, String::from)
    });
    decoded
        .chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .collect()
}

/// Texto que la interfaz muestra tal cual: sin Markdown, sin entidades HTML y con longitud acotada.
fn prose(text: Option<String>, max: usize) -> Option<String> {
    text.map(|value| shorten(plain_text(&decode_entities(&value)), max))
        .filter(|value| !value.is_empty())
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawInterpretation {
    intent: String,
    metric: Option<String>,
    city: Option<String>,
    when: Option<String>,
    search_terms: Vec<String>,
    exact_file_name: Option<String>,
    reminder_content: Option<String>,
    reminder_date: Option<String>,
    reminder_time: Option<String>,
    reminder_ambiguity: Option<String>,
    screen_question: Option<String>,
    clarification_question: Option<String>,
    suggestions: Vec<String>,
}

fn clean(text: Option<String>) -> Option<String> {
    text.map(|value| value.trim().chars().take(MAX_TEXT).collect::<String>())
        .filter(|value| !value.is_empty())
}

/// Un nombre de archivo nunca puede contener una ruta.
pub fn valid_file_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.contains(['/', '\\', ':'])
        && !name.contains("..")
}

pub fn parse_interpretation(text: &str, fallback_question: &str) -> Result<Intent, AiError> {
    let raw: RawInterpretation =
        serde_json::from_str(text.trim()).map_err(|_| AiError::InvalidResponse)?;
    Ok(match raw.intent.as_str() {
        "time" => Intent::Time,
        "system" => Intent::System(match raw.metric.as_deref() {
            Some("cpu") => SystemMetric::Cpu,
            Some("ram") => SystemMetric::Ram,
            Some("battery") => SystemMetric::Battery,
            Some("internet") => SystemMetric::Internet,
            Some("wifi") => SystemMetric::Wifi,
            Some("dark_mode") => SystemMetric::DarkMode,
            Some("overview") | None => SystemMetric::Overview,
            Some(_) => return Err(AiError::InvalidResponse),
        }),
        "weather" => Intent::Weather {
            // "Mi ciudad" o "donde estoy" no son una ciudad: entonces se usa la de Ajustes o se pregunta.
            city: clean(raw.city).filter(|city| {
                crate::core::rules::is_place_name(&crate::core::rules::normalize(city))
            }),
            when: match raw.when.as_deref() {
                Some("tomorrow") => WeatherWhen::Tomorrow,
                Some("today") => WeatherWhen::Today,
                _ => WeatherWhen::Now,
            },
        },
        "search_files" => {
            let exact_name = clean(raw.exact_file_name).filter(|name| valid_file_name(name));
            let terms: Vec<String> = raw
                .search_terms
                .into_iter()
                .filter_map(|term| clean(Some(term)))
                .filter(|term| valid_file_name(term))
                .take(MAX_TERMS)
                .collect();
            if exact_name.is_none() && terms.is_empty() {
                return Err(AiError::InvalidResponse);
            }
            Intent::SearchFiles(FileQuery { exact_name, terms })
        }
        "create_reminder" => {
            let content = prose(raw.reminder_content, MAX_TEXT).ok_or(AiError::InvalidResponse)?;
            let date_text = clean(raw.reminder_date);
            let time_text = clean(raw.reminder_time);
            let date = date_text
                .as_deref()
                .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
            let time = time_text
                .as_deref()
                .and_then(|t| NaiveTime::parse_from_str(t, "%H:%M").ok());
            // Una fecha que no se entiende se pregunta: nunca se adivina.
            let unreadable =
                (date_text.is_some() && date.is_none()) || (time_text.is_some() && time.is_none());
            // La aclaración se muestra tal cual: si el modelo no devuelve una pregunta, se usa la genérica.
            let model_doubt = prose(raw.reminder_ambiguity, MAX_TEXT);
            let is_question = model_doubt
                .as_deref()
                .is_some_and(|doubt| doubt.contains('?'));
            let ambiguity = if is_question {
                model_doubt
            } else {
                (unreadable || model_doubt.is_some())
                    .then(|| "¿Puedes indicarme la fecha y la hora exactas?".to_owned())
            };
            Intent::CreateReminder(ReminderRequest {
                content,
                date,
                time,
                ambiguity,
            })
        }
        "list_reminders" => Intent::ListReminders,
        "analyze_screen" => Intent::AnalyzeScreen {
            question: prose(raw.screen_question, MAX_TEXT)
                .unwrap_or_else(|| fallback_question.to_owned()),
        },
        "clarify" => Intent::Clarify {
            question: prose(raw.clarification_question, MAX_TEXT)
                .ok_or(AiError::InvalidResponse)?,
            suggestions: raw
                .suggestions
                .into_iter()
                .filter_map(|s| prose(Some(s), MAX_TEXT))
                .take(3)
                .collect(),
        },
        "unknown" => Intent::Unknown,
        _ => return Err(AiError::InvalidResponse),
    })
}

#[derive(Deserialize)]
struct RawScreenAnalysis {
    summary: String,
    cause: Option<String>,
    steps: Vec<String>,
}

/// Resumen de una frase, causa probable y hasta tres pasos. Cualquier otra forma se rechaza.
pub fn parse_screen_analysis(text: &str) -> Result<ScreenAnalysis, AiError> {
    let value: serde_json::Value =
        serde_json::from_str(text.trim()).map_err(|_| AiError::InvalidResponse)?;
    // Los tres campos son obligatorios, como en el esquema: una causa ausente no equivale a "sin causa".
    if !["summary", "cause", "steps"]
        .iter()
        .all(|field| value.get(field).is_some())
    {
        return Err(AiError::InvalidResponse);
    }
    let raw: RawScreenAnalysis =
        serde_json::from_value(value).map_err(|_| AiError::InvalidResponse)?;
    if raw.steps.len() > MAX_STEPS {
        return Err(AiError::InvalidResponse);
    }
    Ok(ScreenAnalysis {
        summary: prose(Some(raw.summary), MAX_TEXT).ok_or(AiError::InvalidResponse)?,
        cause: prose(raw.cause, MAX_CAUSE),
        steps: raw
            .steps
            .into_iter()
            .filter_map(|step| prose(Some(step), MAX_TEXT))
            .collect(),
    })
}

#[derive(Deserialize)]
struct RawDocumentSummary {
    summary: String,
    points: Vec<String>,
    #[serde(default)]
    details: Vec<String>,
}

/// Resumen de dos o tres frases, hasta ocho ideas clave y hasta ocho datos importantes.
/// Cualquier otra forma se rechaza.
pub fn parse_document_summary(text: &str) -> Result<DocumentSummary, AiError> {
    let raw: RawDocumentSummary =
        serde_json::from_str(text.trim()).map_err(|_| AiError::InvalidResponse)?;
    if raw.points.len() > MAX_POINTS || raw.details.len() > MAX_POINTS {
        return Err(AiError::InvalidResponse);
    }
    Ok(DocumentSummary {
        summary: prose(Some(raw.summary), MAX_CAUSE).ok_or(AiError::InvalidResponse)?,
        points: raw
            .points
            .into_iter()
            .filter_map(|point| prose(Some(point), MAX_TEXT))
            .collect(),
        details: raw
            .details
            .into_iter()
            .filter_map(|detail| prose(Some(detail), MAX_TEXT))
            .collect(),
    })
}

#[cfg(test)]
#[path = "tests/schema.rs"]
mod tests;
