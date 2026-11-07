//! Formato de datos para la interfaz en español.

use chrono::{DateTime, Datelike, Local, Timelike, Weekday};

pub const MONTHS: [&str; 12] = ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"];

pub fn weekday_name(day: Weekday) -> &'static str {
    match day {
        Weekday::Mon => "lunes",
        Weekday::Tue => "martes",
        Weekday::Wed => "miércoles",
        Weekday::Thu => "jueves",
        Weekday::Fri => "viernes",
        Weekday::Sat => "sábado",
        Weekday::Sun => "domingo",
    }
}

pub fn long_date(date: impl Datelike) -> String {
    format!("{} {} de {}", weekday_name(date.weekday()), date.day(), MONTHS[date.month0() as usize])
}

pub fn clock(now: &DateTime<Local>) -> String {
    format!("{:02}:{:02}", now.hour(), now.minute())
}

pub fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

pub fn gigabytes(bytes: u64) -> String {
    decimal(bytes as f64 / 1_073_741_824.0, 1)
}

pub fn whole(value: f64) -> String {
    format!("{value:.0}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn dates_are_in_spanish() {
        assert_eq!(long_date(NaiveDate::from_ymd_opt(2026, 11, 5).unwrap()), "jueves 5 de noviembre");
    }

    #[test]
    fn decimals_use_a_comma() {
        assert_eq!(decimal(11.44, 1), "11,4");
        assert_eq!(gigabytes(1_073_741_824 * 2), "2,0");
    }
}
