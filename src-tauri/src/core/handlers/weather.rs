//! Clima real de Open-Meteo para la ciudad pedida o la de Ajustes.

use std::sync::Arc;

use async_trait::async_trait;

use super::{CommandHandler, HandlerContext, Outcome};
use crate::{
    core::format::{decimal, whole},
    models::{Attribution, Category, DataRow, Intent, ResponseBody, SuggestedRequest, WeatherWhen},
    providers::weather::{WeatherError, WeatherProvider, ATTRIBUTION_TEXT, ATTRIBUTION_URL},
    services::settings::SettingsService,
};

pub struct WeatherHandler {
    pub weather: Arc<dyn WeatherProvider>,
    pub settings: SettingsService,
}

fn attribution() -> Option<Attribution> {
    Some(Attribution {
        text: ATTRIBUTION_TEXT.into(),
        url: ATTRIBUTION_URL.into(),
    })
}

impl WeatherHandler {
    async fn answer(&self, city: Option<&String>, when: WeatherWhen) -> Outcome {
        let configured = self
            .settings
            .get()
            .map(|settings| settings.weather_city)
            .unwrap_or_default();
        let Some(city) = city
            .cloned()
            .or_else(|| (!configured.trim().is_empty()).then_some(configured))
        else {
            return Outcome::pending(
                Category::Weather,
                "Falta la ciudad",
                ResponseBody::Clarification { question: "¿De qué ciudad quieres el clima? Dímelo en tu solicitud, o configura una en Ajustes.".into(), suggestions: Vec::<SuggestedRequest>::new() },
            );
        };
        let report = match self.weather.report(&city).await {
            Ok(report) => report,
            Err(WeatherError::CityNotFound) => {
                return Outcome::failed(
                    Category::Weather,
                    "cityNotFound",
                    format!("No encontré la ciudad \u{201c}{city}\u{201d}."),
                    None,
                )
            }
            Err(_) => {
                return Outcome::failed(
                    Category::Weather,
                    "weatherUnavailable",
                    "No fue posible consultar el clima.",
                    Some("Comprueba tu conexión a Internet. No muestro datos antiguos.".into()),
                )
            }
        };
        let day_row = |label: &str, day: &crate::providers::weather::DayForecast| {
            DataRow::value(
                label,
                format!(
                    "{} °C a {} °C · {}",
                    whole(day.min_c),
                    whole(day.max_c),
                    day.condition
                ),
            )
        };
        let (mut outcome_value, mut unit, mut context, mut rows) = (
            decimal(report.temperature_c, 0),
            "°C".to_owned(),
            format!("{}, ahora en {}", report.condition, report.place),
            vec![day_row("Pronóstico de hoy", &report.today)],
        );
        match when {
            WeatherWhen::Now | WeatherWhen::Today => {}
            WeatherWhen::Tomorrow => {
                outcome_value = format!(
                    "{} a {}",
                    whole(report.tomorrow.min_c),
                    whole(report.tomorrow.max_c)
                );
                unit = "°C".into();
                context = format!(
                    "{}, pronóstico de mañana en {}",
                    report.tomorrow.condition, report.place
                );
                rows = vec![DataRow::value(
                    "Ahora",
                    format!("{} °C · {}", whole(report.temperature_c), report.condition),
                )];
            }
        }
        let summary = format!("{outcome_value} {unit} · {context}");
        Outcome {
            source: Some("Open-Meteo".into()),
            ..Outcome::done(
                Category::Weather,
                summary,
                ResponseBody::Reading {
                    value: outcome_value,
                    unit,
                    context,
                    percent: None,
                    rows,
                    attribution: attribution(),
                },
            )
        }
    }
}

#[async_trait]
impl CommandHandler for WeatherHandler {
    async fn handle(&self, intent: &Intent, _: &HandlerContext) -> Option<Outcome> {
        let Intent::Weather { city, when } = intent else {
            return None;
        };
        Some(self.answer(city.as_ref(), *when).await)
    }
}
