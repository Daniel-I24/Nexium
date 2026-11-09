//! Validación de la interpretación. La salida del modelo es entrada no confiable:
//! solo se convierte en `Intent` si cada campo es válido.

use chrono::{NaiveDate, NaiveTime};
use serde::Deserialize;

use crate::{
    error::AiError,
    models::{FileQuery, Intent, ReminderRequest, SystemMetric, WeatherWhen},
};

const MAX_TERMS: usize = 8;
const MAX_TEXT: usize = 300;

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
    text.map(|value| value.trim().chars().take(MAX_TEXT).collect::<String>()).filter(|value| !value.is_empty())
}

/// Un nombre de archivo nunca puede contener una ruta.
pub fn valid_file_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 255 && !name.contains(['/', '\\', ':']) && !name.contains("..")
}

pub fn parse_interpretation(text: &str, fallback_question: &str) -> Result<Intent, AiError> {
    let raw: RawInterpretation = serde_json::from_str(text.trim()).map_err(|_| AiError::InvalidResponse)?;
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
            city: clean(raw.city),
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
            let content = clean(raw.reminder_content).ok_or(AiError::InvalidResponse)?;
            let date_text = clean(raw.reminder_date);
            let time_text = clean(raw.reminder_time);
            let date = date_text.as_deref().and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
            let time = time_text.as_deref().and_then(|t| NaiveTime::parse_from_str(t, "%H:%M").ok());
            // Una fecha que no se entiende se pregunta: nunca se adivina.
            let unreadable = (date_text.is_some() && date.is_none()) || (time_text.is_some() && time.is_none());
            // La aclaración se muestra tal cual: si el modelo no devuelve una pregunta, se usa la genérica.
            let model_doubt = clean(raw.reminder_ambiguity);
            let is_question = model_doubt.as_deref().is_some_and(|doubt| doubt.contains('?'));
            let ambiguity = if is_question { model_doubt } else { (unreadable || model_doubt.is_some()).then(|| "¿Puedes indicarme la fecha y la hora exactas?".to_owned()) };
            Intent::CreateReminder(ReminderRequest { content, date, time, ambiguity })
        }
        "list_reminders" => Intent::ListReminders,
        "analyze_screen" => Intent::AnalyzeScreen { question: clean(raw.screen_question).unwrap_or_else(|| fallback_question.to_owned()) },
        "clarify" => Intent::Clarify {
            question: clean(raw.clarification_question).ok_or(AiError::InvalidResponse)?,
            suggestions: raw.suggestions.into_iter().filter_map(|s| clean(Some(s))).take(3).collect(),
        },
        "unknown" => Intent::Unknown,
        _ => return Err(AiError::InvalidResponse),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse(text: &str) -> Result<Intent, AiError> {
        parse_interpretation(text, "pregunta")
    }

    #[test]
    fn extracts_a_complete_reminder() {
        let intent = parse(r#"{"intent":"create_reminder","reminder_content":"entregar el proyecto","reminder_date":"2026-11-12","reminder_time":"08:00"}"#).unwrap();
        let Intent::CreateReminder(reminder) = intent else { panic!() };
        assert_eq!(reminder.content, "entregar el proyecto");
        assert_eq!(reminder.date, NaiveDate::from_ymd_opt(2026, 11, 12));
        assert_eq!(reminder.time, NaiveTime::from_hms_opt(8, 0, 0));
        assert_eq!(reminder.ambiguity, None);
    }

    #[test]
    fn an_unreadable_date_becomes_a_question() {
        let intent = parse(r#"{"intent":"create_reminder","reminder_content":"x","reminder_date":"el jueves"}"#).unwrap();
        let Intent::CreateReminder(reminder) = intent else { panic!() };
        assert_eq!(reminder.date, None);
        assert!(reminder.ambiguity.is_some());
    }

    #[test]
    fn a_doubt_that_is_not_a_question_becomes_one() {
        let intent = parse(r#"{"intent":"create_reminder","reminder_content":"estudiar","reminder_ambiguity":"pronto"}"#).unwrap();
        let Intent::CreateReminder(reminder) = intent else { panic!() };
        assert_eq!(reminder.ambiguity.as_deref(), Some("¿Puedes indicarme la fecha y la hora exactas?"));
    }

    #[test]
    fn rejects_unknown_intents_and_invalid_json() {
        assert_eq!(parse(r#"{"intent":"run_shell_command"}"#), Err(AiError::InvalidResponse));
        assert_eq!(parse("no es json"), Err(AiError::InvalidResponse));
        assert_eq!(parse(r#"{"intent":"system","metric":"delete_all"}"#), Err(AiError::InvalidResponse));
    }

    #[test]
    fn a_file_name_cannot_carry_a_path() {
        assert_eq!(parse(r#"{"intent":"search_files","exact_file_name":"..\\..\\Windows\\system32"}"#), Err(AiError::InvalidResponse));
        let intent = parse(r#"{"intent":"search_files","exact_file_name":"C:\\a.pdf","search_terms":["informe"]}"#).unwrap();
        assert_eq!(intent, Intent::SearchFiles(FileQuery { exact_name: None, terms: vec!["informe".into()] }));
    }

    #[test]
    fn limits_the_number_of_terms() {
        let terms: Vec<String> = (0..20).map(|i| format!("t{i}")).collect();
        let text = json!({ "intent": "search_files", "search_terms": terms }).to_string();
        let Intent::SearchFiles(query) = parse(&text).unwrap() else { panic!() };
        assert_eq!(query.terms.len(), MAX_TERMS);
    }
}
