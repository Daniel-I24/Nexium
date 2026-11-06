//! Reglas locales en español: lo que se resuelve aquí no gasta cuota de IA.

use std::sync::LazyLock;

use regex::Regex;
use unicode_normalization::UnicodeNormalization;

use crate::{
    models::{FileQuery, Intent, SystemMetric, WeatherWhen},
    providers::ai::schema::valid_file_name,
};

const STOP_WORDS: &[&str] = &[
    "a", "al", "algo", "archivo", "archivos", "busca", "buscame", "buscar", "clase", "como", "con", "de", "del", "documento", "documentos", "el", "en", "encontrar", "encuentra",
    "existe", "fichero", "hay", "hice", "la", "las", "lo", "los", "mi", "mis", "necesito", "para", "por", "que", "quiero", "relacionados", "se", "sobre", "su", "tengo", "un", "una", "y",
];

fn regex(pattern: &str) -> Regex {
    Regex::new(pattern).expect("expresión regular válida")
}

static TIME: LazyLock<Regex> = LazyLock::new(|| regex(r"^(me dices |dime |sabes )?(que hora|a que hora estamos|la hora|que dia es|en que fecha|que fecha)"));
static CPU: LazyLock<Regex> = LazyLock::new(|| regex(r"\b(cpu|procesador)\b"));
static RAM: LazyLock<Regex> = LazyLock::new(|| regex(r"\b(ram|memoria)\b"));
static BATTERY: LazyLock<Regex> = LazyLock::new(|| regex(r"\b(bateria|carga del equipo)\b"));
static WIFI: LazyLock<Regex> = LazyLock::new(|| regex(r"\b(wi ?-?fi|red inalambrica|a que red)\b"));
static INTERNET: LazyLock<Regex> = LazyLock::new(|| regex(r"\b(internet|conexion a la red|estoy conectado|tengo conexion|hay conexion)\b"));
static DARK_MODE: LazyLock<Regex> = LazyLock::new(|| regex(r"\b(modo|tema) oscuro\b"));
static OVERVIEW: LazyLock<Regex> = LazyLock::new(|| regex(r"\b(estado del (sistema|equipo)|como esta (mi|el) (equipo|computador|ordenador|pc))\b"));
static WEATHER: LazyLock<Regex> = LazyLock::new(|| regex(r"\b(clima|tiempo hace|temperatura|llover|lloviendo|pronostico)\b"));
static WEATHER_CITY: LazyLock<Regex> = LazyLock::new(|| regex(r"\ben ([a-z][a-z ]{1,40}?)(?: hoy| manana| ahora)?$"));
static SCREEN: LazyLock<Regex> = LazyLock::new(|| regex(r"\b(mi pantalla|la pantalla|este error|este mensaje|esta interfaz|esta ventana)\b"));
static FILE_NAME: LazyLock<Regex> = LazyLock::new(|| regex(r"(?i)[\p{L}\p{N}_ -]*[\p{L}\p{N}_-]\.[a-z0-9]{1,5}\b"));
static FILE_REQUEST: LazyLock<Regex> = LazyLock::new(|| regex(r"\b(existe|busca|encuentra|buscar|encontrar|archivo)\b"));
static SEARCH: LazyLock<Regex> = LazyLock::new(|| regex(r"\b(busca|buscame|encuentra|encontrar|localiza)\b"));
static LIST_REMINDERS: LazyLock<Regex> = LazyLock::new(|| regex(r"\b(que|cuales|mis|ver) recordatorios\b|\bque tengo pendiente\b"));

/// Minúsculas, sin acentos y sin signos: "¿Cuánta RAM?" pasa a "cuanta ram".
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.nfd().filter(|c| !('\u{300}'..='\u{36f}').contains(c)).flat_map(char::to_lowercase) {
        out.push(if c.is_alphanumeric() || c == '-' { c } else { ' ' });
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn title_case(text: &str) -> String {
    text.split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Conserva la grafía del usuario ("Bogotá") cuando la encuentra en la frase.
fn original_city(original: &str, normalized_city: &str) -> String {
    let words: Vec<&str> = original.split([' ', ',', '?', '¿', '.']).filter(|word| normalized_city.split(' ').any(|part| normalize(word) == part)).collect();
    title_case(&if words.is_empty() { normalized_city.to_owned() } else { words.join(" ") })
}

fn weather_intent(original: &str, text: &str) -> Intent {
    let when = if text.contains("manana") {
        WeatherWhen::Tomorrow
    } else if text.contains("hoy") {
        WeatherWhen::Today
    } else {
        WeatherWhen::Now
    };
    let city = WEATHER_CITY.captures(text).and_then(|c| c.get(1)).map(|m| original_city(original, m.as_str()));
    Intent::Weather { city, when }
}

/// Términos útiles de una búsqueda sin IA: se usa también si la IA no está disponible.
pub fn local_search_terms(text: &str) -> Vec<String> {
    normalize(text).split(' ').filter(|word| word.len() > 1 && !STOP_WORDS.contains(word)).map(str::to_owned).collect()
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
    let text = normalize(original);
    if text.starts_with("recuerdame") || text.starts_with("recordarme") || text.contains("ponme un recordatorio") || text.contains("crea un recordatorio") {
        return None;
    }
    if LIST_REMINDERS.is_match(&text) {
        return Some(Intent::ListReminders);
    }
    if SCREEN.is_match(&text) {
        return Some(Intent::AnalyzeScreen { question: original.trim().to_owned() });
    }
    if FILE_REQUEST.is_match(&text) {
        if let Some(name) = file_name_in(original) {
            return Some(Intent::SearchFiles(FileQuery { exact_name: Some(name), terms: Vec::new() }));
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
mod tests {
    use super::*;

    fn system(metric: SystemMetric) -> Option<Intent> {
        Some(Intent::System(metric))
    }

    #[test]
    fn recognizes_local_questions_without_ai() {
        let cases = [
            ("¿Qué hora es?", Some(Intent::Time)),
            ("Dime la hora", Some(Intent::Time)),
            ("¿Qué día es hoy?", Some(Intent::Time)),
            ("¿Cuánta memoria RAM estoy utilizando?", system(SystemMetric::Ram)),
            ("cuanta ram tengo libre", system(SystemMetric::Ram)),
            ("¿Cómo va el procesador?", system(SystemMetric::Cpu)),
            ("uso de CPU", system(SystemMetric::Cpu)),
            ("¿Cuánta batería me queda?", system(SystemMetric::Battery)),
            ("¿Estoy conectado a Internet?", system(SystemMetric::Internet)),
            ("¿A qué red Wi-Fi estoy conectado?", system(SystemMetric::Wifi)),
            ("¿Está activado el modo oscuro?", system(SystemMetric::DarkMode)),
            ("¿Cuál es el estado del sistema?", system(SystemMetric::Overview)),
        ];
        for (phrase, expected) in cases {
            assert_eq!(match_local(phrase), expected, "{phrase}");
        }
    }

    #[test]
    fn recognizes_weather_with_city_and_day() {
        assert_eq!(match_local("¿Cómo está el clima hoy?"), Some(Intent::Weather { city: None, when: WeatherWhen::Today }));
        assert_eq!(match_local("¿Va a llover mañana en Bogotá?"), Some(Intent::Weather { city: Some("Bogotá".into()), when: WeatherWhen::Tomorrow }));
    }

    #[test]
    fn recognizes_screen_requests() {
        for phrase in ["¿Qué aparece en mi pantalla?", "Explícame este error", "¿Qué significa este mensaje?", "Ayúdame a entender esta interfaz"] {
            assert!(matches!(match_local(phrase), Some(Intent::AnalyzeScreen { .. })), "{phrase}");
        }
    }

    #[test]
    fn exact_file_names_do_not_need_ai() {
        assert_eq!(match_local("¿Existe el archivo proyecto.pdf?"), Some(Intent::SearchFiles(FileQuery { exact_name: Some("proyecto.pdf".into()), terms: vec![] })));
    }

    #[test]
    fn leaves_ambiguous_phrases_to_the_ai() {
        for phrase in [
            "Recuérdame entregar el proyecto mañana",
            "Recuérdame revisar la memoria del informe el lunes",
            "Necesito encontrar el informe que hice para la clase de programación",
            "Busca mi documento de bases de datos",
            "cuéntame un chiste",
        ] {
            assert_eq!(match_local(phrase), None, "{phrase}");
        }
    }

    #[test]
    fn local_terms_drop_empty_words() {
        assert_eq!(local_search_terms("Busca mi informe de programación"), ["informe", "programacion"]);
    }

    #[test]
    fn normalize_removes_accents_and_signs() {
        assert_eq!(normalize("¿Cuánta   BATERÍA?"), "cuanta bateria");
    }
}
