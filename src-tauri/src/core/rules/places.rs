//! Nombres de lugar para el clima: cuándo una frase nombra una ciudad y cómo se conserva lo escrito.

use super::normalize;

/// Palabras que señalan un sitio sin nombrarlo: "en la ciudad en la que me encuentro", "aquí", "donde estoy".
const NOT_A_PLACE: &[&str] = &[
    "aca",
    "ahi",
    "alla",
    "aqui",
    "ciudad",
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
];

/// Si un texto ya normalizado puede ser el nombre de un lugar. "Bogotá" o "El Cairo" sí; "la ciudad en la que
/// me encuentro" no: entonces la solicitud no nombra ninguna ciudad.
pub fn is_place_name(normalized: &str) -> bool {
    let words: Vec<&str> = normalized
        .split(' ')
        .filter(|word| !word.is_empty())
        .collect();
    !words.is_empty() && words.len() <= 5 && !words.iter().any(|word| NOT_A_PLACE.contains(word))
}

/// El nombre tal como lo escribió la persona ("Bogotá"), sin cambiar sus mayúsculas: Open-Meteo devuelve
/// después el nombre oficial, que es el que se muestra en la respuesta.
pub(super) fn original_city(original: &str, normalized_city: &str) -> String {
    let words: Vec<&str> = original
        .split([' ', ',', '?', '¿', '.'])
        .filter(|word| {
            normalized_city
                .split(' ')
                .any(|part| normalize(word) == part)
        })
        .collect();
    if words.is_empty() {
        normalized_city.to_owned()
    } else {
        words.join(" ")
    }
}
