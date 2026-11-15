//! Reglas locales en español: lo que se resuelve aquí no gasta cuota de IA.

use std::sync::LazyLock;

use regex::Regex;

mod dates;
mod places;
mod text;

pub use dates::ambiguous_date_question;
pub use places::is_place_name;
pub use text::{normalize, prepare};

use crate::{
    models::{DocumentTask, FileQuery, Intent, Section, SystemMetric, WeatherWhen},
    providers::ai::schema::valid_file_name,
};
use places::original_city;
#[cfg(test)]
use text::edit_distance;

const STOP_WORDS: &[&str] = &[
    "a",
    "al",
    "algo",
    "analiza",
    "analizame",
    "analizar",
    "archivo",
    "archivos",
    "busca",
    "buscame",
    "buscar",
    "clase",
    "como",
    "con",
    "dame",
    "de",
    "del",
    "dice",
    "documento",
    "documentos",
    "el",
    "en",
    "encontrar",
    "encuentra",
    "esa",
    "ese",
    "esta",
    "este",
    "existe",
    "explicame",
    "fichero",
    "hay",
    "haz",
    "hazme",
    "hice",
    "la",
    "las",
    "lo",
    "los",
    "mi",
    "mis",
    "necesito",
    "para",
    "por",
    "que",
    "quiero",
    "relacionados",
    "resume",
    "resumeme",
    "resumen",
    "resumir",
    "se",
    "sobre",
    "su",
    "tengo",
    "trata",
    "un",
    "una",
    "y",
];

pub(super) fn regex(pattern: &str) -> Regex {
    Regex::new(pattern).expect("expresión regular válida")
}

static TIME: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"^(me dices |dime |sabes )?(que hora|a que hora estamos|la hora|que dia es|en que fecha|que fecha)|\b(que hora es|que horas son|dime la hora|hora actual|que dia es hoy|fecha de hoy|fecha actual)\b",
    )
});
static CPU: LazyLock<Regex> = LazyLock::new(|| regex(r"\b(cpu|procesador)\b"));
static RAM: LazyLock<Regex> = LazyLock::new(|| regex(r"\b(ram|memoria)\b"));
static BATTERY: LazyLock<Regex> =
    LazyLock::new(|| regex(r"\b(bateria|pila|carga del equipo|cuanta carga)\b"));
static WIFI: LazyLock<Regex> = LazyLock::new(|| regex(r"\b(wifi|red inalambrica|a que red)\b"));
/// "Recuérdame" y lo que suele salir al transcribirlo ("Recuerda mi").
static REMINDER_START: LazyLock<Regex> = LazyLock::new(|| {
    regex(r"^(recuerdame|recuerda me|recuerda mi|recordame|recuerdeme|recordarme|acuerdame)\b")
});
/// "Resume mi documento de bases de datos", "Analiza este archivo".
static DOCUMENT_VERB: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"^(resume|resumeme|resumir|(haz|hazme|dame) un resumen|un resumen|analiza|analizame|analizar|que dice|de que trata|explicame)\b",
    )
});
static DOCUMENT_NOUN: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"\b(documento|documentos|archivo|archivos|informe|pdf|docx|trabajo|apuntes|notas|presentacion|texto)\b",
    )
});
/// "Abre los recordatorios", "ve a Ajustes": cambiar de sección no necesita IA.
static OPEN_SECTION: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"^(abre|abrir|abreme|ve al?|ir al?|vamos al?|llevame al?|pasa al?|cambia al?|entra al?|entrar al?)\b.*\b(asistente|recordatorios|historial|ajustes|configuracion|preferencias)\b",
    )
});
fn document_intent(original: &str, text: &str) -> Intent {
    let task = if text.starts_with("analiz") || text.starts_with("explicame") {
        DocumentTask::Analyze
    } else {
        DocumentTask::Summarize
    };
    let query = match file_name_in(original) {
        Some(name) => FileQuery {
            exact_name: Some(name),
            terms: Vec::new(),
        },
        // Sin términos útiles ("Analiza este archivo"), la interfaz ofrece el selector del sistema.
        None => FileQuery {
            exact_name: None,
            terms: local_search_terms(original)
                .into_iter()
                .filter(|term| !DOCUMENT_NOUN.is_match(term))
                .collect(),
        },
    };
    Intent::Document { task, query }
}

fn section_in(text: &str) -> Option<Section> {
    let captures = OPEN_SECTION.captures(text)?;
    Some(match captures.get(2)?.as_str() {
        "asistente" => Section::Assistant,
        "recordatorios" => Section::Reminders,
        "historial" => Section::History,
        _ => Section::Settings,
    })
}
static INTERNET: LazyLock<Regex> = LazyLock::new(|| {
    regex(r"\b(internet|conexion a la red|estoy conectado|tengo conexion|hay conexion)\b")
});
static DARK_MODE: LazyLock<Regex> = LazyLock::new(|| regex(r"\b(modo|tema) oscuro\b"));
static OVERVIEW: LazyLock<Regex> = LazyLock::new(|| {
    regex(r"\b(estado del (sistema|equipo)|como esta (mi|el) (equipo|computador|ordenador|pc))\b")
});
static WEATHER: LazyLock<Regex> =
    LazyLock::new(|| regex(r"\b(clima|tiempo hace|temperatura|llover|lloviendo|pronostico)\b"));
static WEATHER_CITY: LazyLock<Regex> =
    LazyLock::new(|| regex(r"\ben ([a-z][a-z ]{1,40}?)(?: hoy| manana| ahora)?$"));
static SCREEN: LazyLock<Regex> = LazyLock::new(|| {
    regex(r"\b(mi pantalla|la pantalla|este error|este mensaje|esta interfaz|esta ventana)\b")
});
static FILE_NAME: LazyLock<Regex> =
    LazyLock::new(|| regex(r"(?i)[\p{L}\p{N}_ -]*[\p{L}\p{N}_-]\.[a-z0-9]{1,5}\b"));
static FILE_REQUEST: LazyLock<Regex> =
    LazyLock::new(|| regex(r"\b(existe|busca|encuentra|buscar|encontrar|archivo)\b"));
static SEARCH: LazyLock<Regex> =
    LazyLock::new(|| regex(r"\b(busca|buscame|encuentra|encontrar|localiza)\b"));
static LIST_REMINDERS: LazyLock<Regex> =
    LazyLock::new(|| regex(r"\b(que|cuales|mis|ver) recordatorios\b|\bque tengo pendiente\b"));

fn weather_intent(original: &str, text: &str) -> Intent {
    let when = if text.contains("manana") {
        WeatherWhen::Tomorrow
    } else if text.contains("hoy") {
        WeatherWhen::Today
    } else {
        WeatherWhen::Now
    };
    let city = WEATHER_CITY
        .captures(text)
        .and_then(|c| c.get(1))
        .filter(|m| is_place_name(m.as_str()))
        .map(|m| original_city(original, m.as_str()));
    Intent::Weather { city, when }
}

/// Términos útiles de una búsqueda sin IA: se usa también si la IA no está disponible.
pub fn local_search_terms(text: &str) -> Vec<String> {
    normalize(text)
        .split(' ')
        .filter(|word| word.len() > 1 && !STOP_WORDS.contains(word))
        .map(str::to_owned)
        .collect()
}

fn file_name_in(original: &str) -> Option<String> {
    let found = FILE_NAME.find(original)?.as_str();
    // El patrón admite espacios; el nombre empieza en la última palabra.
    let name = found.split_whitespace().last()?.to_owned();
    valid_file_name(&name).then_some(name)
}

pub fn is_search_request(text: &str) -> bool {
    SEARCH.is_match(&normalize(text))
}

/// Devuelve una intención solo si la frase no deja lugar a dudas.
pub fn match_local(original: &str) -> Option<Intent> {
    let text = prepare(original);
    if REMINDER_START.is_match(&text)
        || text.contains("ponme un recordatorio")
        || text.contains("crea un recordatorio")
    {
        return ambiguous_date_question(original);
    }
    if let Some(section) = section_in(&text) {
        return Some(Intent::OpenSection(section));
    }
    if DOCUMENT_VERB.is_match(&text) && DOCUMENT_NOUN.is_match(&text) {
        return Some(document_intent(original, &text));
    }
    if LIST_REMINDERS.is_match(&text) {
        return Some(Intent::ListReminders);
    }
    if SCREEN.is_match(&text) {
        return Some(Intent::AnalyzeScreen {
            question: original.trim().to_owned(),
        });
    }
    if FILE_REQUEST.is_match(&text) {
        if let Some(name) = file_name_in(original) {
            return Some(Intent::SearchFiles(FileQuery {
                exact_name: Some(name),
                terms: Vec::new(),
            }));
        }
    }
    if SEARCH.is_match(&text) {
        // Las búsquedas libres se benefician de la IA.
        return None;
    }
    if DARK_MODE.is_match(&text) {
        return Some(Intent::System(SystemMetric::DarkMode));
    }
    if WEATHER.is_match(&text) {
        return Some(weather_intent(original, &text));
    }
    if TIME.is_match(&text) {
        return Some(Intent::Time);
    }
    if WIFI.is_match(&text) {
        return Some(Intent::System(SystemMetric::Wifi));
    }
    if RAM.is_match(&text) {
        return Some(Intent::System(SystemMetric::Ram));
    }
    if CPU.is_match(&text) {
        return Some(Intent::System(SystemMetric::Cpu));
    }
    if BATTERY.is_match(&text) {
        return Some(Intent::System(SystemMetric::Battery));
    }
    if INTERNET.is_match(&text) {
        return Some(Intent::System(SystemMetric::Internet));
    }
    if OVERVIEW.is_match(&text) {
        return Some(Intent::System(SystemMetric::Overview));
    }
    None
}

#[cfg(test)]
mod tests;
