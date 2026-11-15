//! Preparación del texto para las reglas: normalización y corrección de errores leves de transcripción.

use std::sync::LazyLock;

use regex::Regex;
use unicode_normalization::UnicodeNormalization;

use super::regex;

/// Minúsculas, sin acentos y sin signos: "¿Cuánta RAM?" pasa a "cuanta ram".
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text
        .nfd()
        .filter(|c| !('\u{300}'..='\u{36f}').contains(c))
        .flat_map(char::to_lowercase)
    {
        out.push(if c.is_alphanumeric() || c == '-' {
            c
        } else {
            ' '
        });
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Formas de escribir Wi-Fi que deja la transcripción. Se unifican antes de aplicar las reglas.
static WIFI_VARIANTS: LazyLock<Regex> =
    LazyLock::new(|| regex(r"\b(wi ?-?fi|guifi|uifi|wai ?fai|waifai)\b"));

/// Palabras de las órdenes que una transcripción puede escribir con un error leve ("vateria", "procesadr").
/// Solo palabras largas: en las cortas, una letra cambia el sentido ("hora" y "ahora").
const VOCABULARY: &[&str] = &[
    "bateria",
    "procesador",
    "memoria",
    "internet",
    "conectado",
    "conexion",
    "oscuro",
    "temperatura",
    "pronostico",
    "lloviendo",
    "recordatorio",
    "recordatorios",
    "recuerdame",
    "ajustes",
    "configuracion",
    "pantalla",
    "documento",
    "archivo",
];

pub(super) fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let next = (row[j + 1] + 1)
                .min(row[j] + 1)
                .min(previous + usize::from(ca != *cb));
            previous = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}

/// Corrige una palabra si está a un error de una del vocabulario (a dos, si es muy larga).
fn correct_word(word: &str) -> &str {
    let length = word.chars().count();
    if length < 5 || VOCABULARY.contains(&word) {
        return word;
    }
    let tolerance = if length >= 9 { 2 } else { 1 };
    VOCABULARY
        .iter()
        .filter(|candidate| candidate.len() >= 6)
        .map(|candidate| (edit_distance(word, candidate), *candidate))
        .filter(|(distance, _)| *distance <= tolerance)
        .min_by_key(|(distance, _)| *distance)
        .map_or(word, |(_, candidate)| candidate)
}

/// Texto listo para las reglas: normalizado, con las variantes unificadas y los errores leves corregidos.
pub fn prepare(text: &str) -> String {
    let text = WIFI_VARIANTS
        .replace_all(&normalize(text), "wifi")
        .into_owned();
    text.split(' ')
        .map(correct_word)
        .collect::<Vec<_>>()
        .join(" ")
}
