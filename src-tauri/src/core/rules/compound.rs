//! Varias solicitudes en una misma petición: «¿A qué red wifi estoy conectado? Y dame el clima».
//!
//! Se divide por signos («?», «,», «;», «.») y conectores («y», «también», «además») solo si cada parte es una
//! orden que las reglas locales reconocen. Si alguna parte no se reconoce, la petición va entera a la IA, que
//! devuelve sus acciones.

use std::sync::LazyLock;

use regex::Regex;

use super::{match_local, normalize, regex, REMINDER_START};
use crate::models::{Action, Intent};

/// Más de cuatro solicitudes a la vez ya no es una conversación: el resto se pide aparte.
pub const MAX_PARTS: usize = 4;

/// Signos y conectores que separan solicitudes. «e» también, ante palabras que empiezan por «i».
static SEPARATOR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\s*(?:[¿?¡!;]+|,|\.(?:\s|$)|\s(?:y|e)\s+(?:tambi[eé]n\s+|adem[aá]s\s+)?|\s(?:tambi[eé]n|adem[aá]s)\s+)\s*")
        .expect("expresión regular válida")
});
/// Conectores que quedan al principio de un fragmento separado por un signo: «… conectado? Y dame el clima».
static LEADING_CONNECTOR: LazyLock<Regex> =
    LazyLock::new(|| regex(r"^(?:y|e|tambien|ademas|y tambien|y ademas)\s+"));

/// Los fragmentos de una petición, sin signos ni conectores sueltos.
fn segments(text: &str) -> Vec<String> {
    SEPARATOR
        .split(text)
        .map(|segment| {
            let trimmed =
                segment.trim_matches(|c: char| c.is_whitespace() || ".,;:¿?¡!".contains(c));
            let normalized = normalize(trimmed);
            match LEADING_CONNECTOR.find(&normalized) {
                // El conector está al principio: se quitan tantas palabras como tenga.
                Some(connector) => {
                    let words = connector.as_str().split_whitespace().count();
                    trimmed
                        .split_whitespace()
                        .skip(words)
                        .collect::<Vec<_>>()
                        .join(" ")
                }
                None => trimmed.to_owned(),
            }
        })
        // Un conector suelto entre signos («… internet? Además, ¿qué hora es?») no es una solicitud.
        .filter(|segment| {
            !segment.is_empty()
                && !matches!(
                    normalize(segment).as_str(),
                    "y" | "e" | "tambien" | "ademas"
                )
        })
        .collect()
}

/// Cómo se reparte una petición entre las reglas locales.
#[derive(Debug, PartialEq)]
pub enum Compound {
    /// Una sola solicitud.
    Single,
    /// Varias, todas reconocibles sin IA. `extra`: cuántas más había, por encima de `MAX_PARTS`.
    Local { actions: Vec<Action>, extra: usize },
    /// Varias, y alguna necesita la IA: la petición va entera a la IA.
    NeedsAi,
}

pub fn split_compound(text: &str) -> Compound {
    let normalized = normalize(text);
    // Un recordatorio puede contener «y» en su contenido («comprar pan y leche»): lo divide la IA, si hace falta.
    if REMINDER_START.is_match(&normalized) {
        return Compound::Single;
    }
    let parts = segments(text);
    if parts.len() < 2 {
        return Compound::Single;
    }
    let recognized: Vec<Option<Intent>> = parts.iter().map(|part| match_local(part)).collect();
    if recognized.iter().all(Option::is_some) {
        let extra = parts.len().saturating_sub(MAX_PARTS);
        let actions = parts
            .into_iter()
            .zip(recognized)
            .take(MAX_PARTS)
            .filter_map(|(request, intent)| intent.map(|intent| Action { request, intent }))
            .collect();
        return Compound::Local { actions, extra };
    }
    // Una palabra suelta sin reconocer («gracias», «Tobago») no es otra solicitud.
    let unrecognized_request = parts
        .iter()
        .zip(&recognized)
        .any(|(part, intent)| intent.is_none() && part.split_whitespace().count() >= 2);
    if recognized.iter().any(Option::is_some) && unrecognized_request {
        Compound::NeedsAi
    } else {
        Compound::Single
    }
}
