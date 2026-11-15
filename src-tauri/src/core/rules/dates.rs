//! Fechas numéricas que admiten dos lecturas ("5/6"): se preguntan sin IA.

use std::sync::LazyLock;

use regex::Regex;

use super::{normalize, regex};
use crate::{core::format::MONTHS, models::Intent};

static NUMERIC_DATE: LazyLock<Regex> = LazyLock::new(|| regex(r"\b\d+(?:[/-]\d+)+\b"));

/// Las dos lecturas de una fecha numérica, si realmente admite dos: "5/6" sí, "13/6" o "5/5" no.
fn date_readings(token: &str, before: &str) -> Option<(String, String)> {
    let separator = if token.contains('/') { '/' } else { '-' };
    let parts: Vec<&str> = token.split(separator).collect();
    let (first, second, year) = match parts[..] {
        [first, second] => (first, second, None),
        [first, second, year] if matches!(year.len(), 2 | 4) => (first, second, Some(year)),
        _ => return None,
    };
    // "las páginas 5-6" no es una fecha: sin año, el guión solo cuenta tras "el" o "del".
    if separator == '-'
        && year.is_none()
        && !matches!(normalize(before).split(' ').next_back(), Some("el" | "del"))
    {
        return None;
    }
    if first.len() > 2 || second.len() > 2 {
        return None;
    }
    let (first, second): (usize, usize) = (first.parse().ok()?, second.parse().ok()?);
    if first == second || !(1..=12).contains(&first) || !(1..=12).contains(&second) {
        return None;
    }
    let suffix = year.map_or_else(String::new, |year| {
        format!(" de {}{year}", if year.len() == 2 { "20" } else { "" })
    });
    let reading = |day: usize, month: usize| format!("{day} de {}{suffix}", MONTHS[month - 1]);
    Some((reading(first, second), reading(second, first)))
}

/// Pregunta por la primera fecha numérica ambigua, sin intervención de la IA.
/// Cada sugerencia es la misma solicitud con la fecha escrita sin ambigüedad.
pub fn ambiguous_date_question(original: &str) -> Option<Intent> {
    NUMERIC_DATE.find_iter(original).find_map(|found| {
        let (first, second) = date_readings(found.as_str(), &original[..found.start()])?;
        let with_date = |date: &str| {
            format!(
                "{}{date}{}",
                &original[..found.start()],
                &original[found.end()..]
            )
            .trim()
            .to_owned()
        };
        Some(Intent::Clarify {
            question: format!("¿Te refieres al {first} o al {second}?"),
            suggestions: vec![with_date(&first), with_date(&second)],
        })
    })
}
