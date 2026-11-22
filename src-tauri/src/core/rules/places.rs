//! Nombres de lugar para el clima: cuándo una frase nombra una ciudad y cómo se conserva lo escrito.

use super::normalize;

/// Palabras que señalan un sitio sin nombrarlo: "en la ciudad en la que me encuentro", "aquí", "donde estoy".
const NOT_A_PLACE: &[&str] = &[
    "aca",
    "ahi",
    "alla",
    "aqui",
    "donde",
    "encuentro",
    "esta",
    "estoy",
    "este",
    "lugar",
    "me",
    "mi",
    "mis",
    "pais",
    "que",
    "region",
    "sitio",
    "ubicacion",
    "vivo",
    "zona",
    // El momento o el propio clima tampoco son lugares: «clima de hoy», «pronóstico de lluvia».
    "ahora",
    "clima",
    "dia",
    "hoy",
    "lluvia",
    "manana",
    "noche",
    "pronostico",
    "semana",
    "tarde",
    "temperatura",
    "tiempo",
];

/// Palabras que introducen el lugar: «clima de Bogotá», «en Medellín», «para Cali».
const BEFORE_PLACE: &[&str] = &["de", "en", "para"];
/// Palabras de la solicitud a partir de las cuales se busca el lugar.
const WEATHER_WORDS: &[&str] = &[
    "clima",
    "tiempo",
    "temperatura",
    "llover",
    "lloviendo",
    "llueve",
    "pronostico",
];
/// Lo que puede ir tras la ciudad sin formar parte de ella: «clima en Cali hoy, por favor».
const AFTER_PLACE: &[&str] = &["hoy", "manana", "ahora", "por", "favor", "porfa"];

/// Si un texto ya normalizado puede ser el nombre de un lugar. "Bogotá" o "El Cairo" sí; "la ciudad en la que
/// me encuentro" no: entonces la solicitud no nombra ninguna ciudad.
pub fn is_place_name(normalized: &str) -> bool {
    let words: Vec<&str> = normalized
        .split(' ')
        .filter(|word| !word.is_empty())
        .collect();
    // «Ciudad» solo forma parte de un nombre seguida de «de»: «Ciudad de México», pero no «mi ciudad».
    let generic_city = words
        .iter()
        .enumerate()
        .any(|(i, word)| *word == "ciudad" && !matches!(words.get(i + 1), Some(&("de" | "del"))));
    !words.is_empty()
        && words.len() <= 8
        && !generic_city
        && !words.iter().any(|word| NOT_A_PLACE.contains(word))
}

/// La ciudad que nombra una solicitud de clima ya preparada, o ninguna si no nombra un lugar. Se toma el
/// primer «de», «en» o «para» tras la palabra del clima cuyo resto sea un nombre de lugar: en «clima de hoy en
/// Cali» se descarta «hoy en cali» y queda «cali»; en «clima en San José de las Lajas» queda el nombre entero.
pub(super) fn weather_city(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split(' ').filter(|word| !word.is_empty()).collect();
    let start = words
        .iter()
        .position(|word| WEATHER_WORDS.contains(word))
        .unwrap_or(0);
    (start..words.len())
        .filter(|&i| BEFORE_PLACE.contains(&words[i]))
        .find_map(|i| {
            let mut place = &words[i + 1..];
            while let Some((last, rest)) = place.split_last() {
                if !AFTER_PLACE.contains(last) {
                    break;
                }
                place = rest;
            }
            let place = place.join(" ");
            is_place_name(&place).then_some(place)
        })
}

/// El nombre tal como lo escribió la persona ("Bogotá"), sin cambiar sus mayúsculas: Open-Meteo devuelve
/// después el nombre oficial, que es el que se muestra en la respuesta.
pub(super) fn original_city(original: &str, normalized_city: &str) -> String {
    let words: Vec<&str> = original
        .split(|c: char| !(c.is_alphanumeric() || c == '-'))
        .filter(|word| !word.is_empty())
        .collect();
    let parts: Vec<&str> = normalized_city.split(' ').collect();
    // Las palabras seguidas que forman la ciudad, la última vez que aparecen: «la» de «la temperatura en La Paz»
    // no cuenta.
    words
        .windows(parts.len())
        .rev()
        .find(|window| {
            window
                .iter()
                .zip(&parts)
                .all(|(word, part)| normalize(word) == *part)
        })
        .map_or_else(|| normalized_city.to_owned(), |window| window.join(" "))
}
