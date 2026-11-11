//! Validación de las respuestas del modelo. Son entrada no confiable:
//! solo se convierten en datos de Nexium si cada campo es válido.

use std::sync::LazyLock;

use chrono::{NaiveDate, NaiveTime};
use regex::Regex;
use serde::Deserialize;

use crate::{
    error::AiError,
    models::{FileQuery, Intent, ReminderRequest, ScreenAnalysis, SystemMetric, WeatherWhen},
};

const MAX_TERMS: usize = 8;
const MAX_TEXT: usize = 300;
const MAX_CAUSE: usize = 600;
const MAX_STEPS: usize = 3;

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

/// Texto que la interfaz muestra tal cual: sin Markdown y con longitud acotada.
fn prose(text: Option<String>, max: usize) -> Option<String> {
    text.map(|value| shorten(plain_text(&value), max))
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
        let Intent::CreateReminder(reminder) = intent else {
            panic!()
        };
        assert_eq!(reminder.content, "entregar el proyecto");
        assert_eq!(reminder.date, NaiveDate::from_ymd_opt(2026, 11, 12));
        assert_eq!(reminder.time, NaiveTime::from_hms_opt(8, 0, 0));
        assert_eq!(reminder.ambiguity, None);
    }

    #[test]
    fn an_unreadable_date_becomes_a_question() {
        let intent = parse(
            r#"{"intent":"create_reminder","reminder_content":"x","reminder_date":"el jueves"}"#,
        )
        .unwrap();
        let Intent::CreateReminder(reminder) = intent else {
            panic!()
        };
        assert_eq!(reminder.date, None);
        assert!(reminder.ambiguity.is_some());
    }

    #[test]
    fn a_doubt_that_is_not_a_question_becomes_one() {
        let intent = parse(r#"{"intent":"create_reminder","reminder_content":"estudiar","reminder_ambiguity":"pronto"}"#).unwrap();
        let Intent::CreateReminder(reminder) = intent else {
            panic!()
        };
        assert_eq!(
            reminder.ambiguity.as_deref(),
            Some("¿Puedes indicarme la fecha y la hora exactas?")
        );
    }

    #[test]
    fn markdown_never_reaches_the_interface() {
        let cases = [
            (
                "**Error 0x80070002**: falta `configuracion.ini`",
                "Error 0x80070002: falta configuracion.ini",
            ),
            ("Código de error: 0x80070002", "Código de error: 0x80070002"),
            (
                r"Abre `C:\Program Files\Inventario\configuracion.ini`",
                r"Abre C:\Program Files\Inventario\configuracion.ini",
            ),
            (
                "Busca `*.log` en `__pycache__`",
                "Busca *.log en __pycache__",
            ),
            (
                "El archivo __init__.py no existe",
                "El archivo __init__.py no existe",
            ),
            (
                "El archivo mi_informe_final.docx",
                "El archivo mi_informe_final.docx",
            ),
            (
                "Borra los archivos *.tmp y *.log",
                "Borra los archivos *.tmp y *.log",
            ),
            ("Calcula 2 * 3 * 4", "Calcula 2 * 3 * 4"),
            (
                "*Nota:* reinicia el *servicio*",
                "Nota: reinicia el servicio",
            ),
            ("### ¿Qué significa?", "¿Qué significa?"),
            (
                "1. Ejecuta `sfc /scannow` como administrador",
                "Ejecuta sfc /scannow como administrador",
            ),
            ("- Reinstala el programa", "Reinstala el programa"),
            ("Mira [la ayuda](https://ejemplo.test)", "Mira la ayuda"),
            (
                r"Ruta `C:\Users\[nombre]\(copia)`",
                r"Ruta C:\Users\[nombre]\(copia)",
            ),
            ("**Error sin cerrar", "Error sin cerrar"),
            ("Comilla `sin cerrar", "Comilla sin cerrar"),
            ("> cita\n\n**aviso**", "cita aviso"),
            ("Texto   con\nsaltos", "Texto con saltos"),
        ];
        for (input, expected) in cases {
            assert_eq!(plain_text(input), expected, "{input}");
        }
    }

    #[test]
    fn interpretation_texts_are_shown_without_markdown() {
        let intent = parse(r####"{"intent":"clarify","clarification_question":"**¿Qué** quieres abrir?","suggestions":["- Buscar un archivo"]}"####).unwrap();
        assert_eq!(
            intent,
            Intent::Clarify {
                question: "¿Qué quieres abrir?".into(),
                suggestions: vec!["Buscar un archivo".into()]
            }
        );
        let Intent::CreateReminder(reminder) = parse(r####"{"intent":"create_reminder","reminder_content":"**pagar** el arriendo","reminder_ambiguity":"### ¿A qué hora?"}"####).unwrap() else { panic!() };
        assert_eq!(reminder.content, "pagar el arriendo");
        assert_eq!(reminder.ambiguity.as_deref(), Some("¿A qué hora?"));
    }

    #[test]
    fn a_screen_analysis_has_a_summary_a_cause_and_up_to_three_steps() {
        let analysis = parse_screen_analysis(r####"{"summary":"**Falta** configuracion.ini.","cause":"El programa no encuentra su archivo.","steps":["1. Comprueba la carpeta.","- Reinstala el programa."]}"####).unwrap();
        assert_eq!(analysis.summary, "Falta configuracion.ini.");
        assert_eq!(
            analysis.cause.as_deref(),
            Some("El programa no encuentra su archivo.")
        );
        assert_eq!(
            analysis.steps,
            ["Comprueba la carpeta.", "Reinstala el programa."]
        );
    }

    #[test]
    fn a_screen_without_problems_has_no_cause_or_steps() {
        let analysis = parse_screen_analysis(
            r#"{"summary":"Un escritorio sin ventanas abiertas.","cause":null,"steps":[]}"#,
        )
        .unwrap();
        assert_eq!(analysis.cause, None);
        assert!(analysis.steps.is_empty());
    }

    #[test]
    fn an_invalid_screen_analysis_is_rejected() {
        for text in [
            "Un texto libre con **Markdown**",
            r#"{"summary":"","cause":null,"steps":[]}"#,
            r#"{"summary":"x","steps":[]}"#,
            r#"{"summary":"x","cause":null,"steps":["a","b","c","d"]}"#,
            r#"{"summary":"x","cause":null,"steps":"uno"}"#,
        ] {
            assert_eq!(
                parse_screen_analysis(text),
                Err(AiError::InvalidResponse),
                "{text}"
            );
        }
    }

    #[test]
    fn long_texts_are_cut_between_words() {
        let long = format!(
            r#"{{"summary":"{}","cause":null,"steps":[]}}"#,
            "palabra ".repeat(80)
        );
        let summary = parse_screen_analysis(&long).unwrap().summary;
        assert!(summary.chars().count() <= MAX_TEXT + 1);
        assert!(summary.ends_with("palabra…"));
    }

    #[test]
    fn rejects_unknown_intents_and_invalid_json() {
        assert_eq!(
            parse(r#"{"intent":"run_shell_command"}"#),
            Err(AiError::InvalidResponse)
        );
        assert_eq!(parse("no es json"), Err(AiError::InvalidResponse));
        assert_eq!(
            parse(r#"{"intent":"system","metric":"delete_all"}"#),
            Err(AiError::InvalidResponse)
        );
    }

    #[test]
    fn a_file_name_cannot_carry_a_path() {
        assert_eq!(
            parse(r#"{"intent":"search_files","exact_file_name":"..\\..\\Windows\\system32"}"#),
            Err(AiError::InvalidResponse)
        );
        let intent = parse(
            r#"{"intent":"search_files","exact_file_name":"C:\\a.pdf","search_terms":["informe"]}"#,
        )
        .unwrap();
        assert_eq!(
            intent,
            Intent::SearchFiles(FileQuery {
                exact_name: None,
                terms: vec!["informe".into()]
            })
        );
    }

    #[test]
    fn limits_the_number_of_terms() {
        let terms: Vec<String> = (0..20).map(|i| format!("t{i}")).collect();
        let text = json!({ "intent": "search_files", "search_terms": terms }).to_string();
        let Intent::SearchFiles(query) = parse(&text).unwrap() else {
            panic!()
        };
        assert_eq!(query.terms.len(), MAX_TERMS);
    }
}
