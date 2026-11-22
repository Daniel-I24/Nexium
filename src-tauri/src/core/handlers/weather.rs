//! Clima real de Open-Meteo para la ciudad pedida o la de Ajustes.

use std::sync::Arc;

use async_trait::async_trait;

use super::{CommandHandler, HandlerContext, Outcome};
use crate::{
    core::format::{decimal, whole},
    models::{Attribution, Category, DataRow, Intent, ResponseBody, WeatherWhen},
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
                ResponseBody::WeatherCityQuestion { when },
            );
        };
        let report = match self.weather.report(&city).await {
            Ok(report) => report,
            Err(WeatherError::CityNotFound) => {
                // Nunca se cambia por la ciudad de Ajustes: se dice y se sugieren nombres parecidos.
                let similar = self.weather.similar_cities(&city).await;
                return Outcome::failed(
                    Category::Weather,
                    "cityNotFound",
                    format!("No encontré la ciudad «{city}»."),
                    Some(city_not_found_hint(&similar)),
                );
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

fn city_not_found_hint(similar: &[String]) -> String {
    match similar {
        [] => "Revisa cómo está escrita y vuelve a pedirlo.".into(),
        [only] => format!("¿Quisiste decir «{only}»? Vuelve a pedirlo con ese nombre."),
        [rest @ .., last] => format!(
            "Las más parecidas son {} y «{last}». Vuelve a pedirlo con el nombre correcto.",
            rest.iter()
                .map(|name| format!("«{name}»"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        database::Database,
        providers::weather::{DayForecast, WeatherReport},
    };

    /// Solo conoce Bogotá y Pasto; para lo demás sugiere Bogotá.
    struct KnownCities;

    #[async_trait]
    impl WeatherProvider for KnownCities {
        async fn report(&self, city: &str) -> Result<WeatherReport, WeatherError> {
            let day = DayForecast {
                condition: "nublado".into(),
                min_c: 10.0,
                max_c: 20.0,
            };
            match crate::core::rules::normalize(city).as_str() {
                place @ ("bogota" | "pasto") => Ok(WeatherReport {
                    place: place.into(),
                    temperature_c: 15.0,
                    condition: "nublado".into(),
                    today: day.clone(),
                    tomorrow: day,
                }),
                _ => Err(WeatherError::CityNotFound),
            }
        }
        async fn find_city(&self, city: &str) -> Result<String, WeatherError> {
            Ok(city.into())
        }
        async fn similar_cities(&self, _: &str) -> Vec<String> {
            vec!["Bogotá, Colombia".into()]
        }
    }

    fn handler() -> WeatherHandler {
        let settings = SettingsService::new(Database::in_memory().unwrap());
        let mut saved = settings.get().unwrap();
        saved.weather_city = "Pasto".into();
        settings.save(&saved).unwrap();
        WeatherHandler {
            weather: Arc::new(KnownCities),
            settings,
        }
    }

    #[tokio::test]
    async fn a_named_city_is_used_instead_of_the_saved_one() {
        let outcome = handler()
            .answer(Some(&"Bogota".into()), WeatherWhen::Now)
            .await;
        assert!(outcome.summary.contains("bogota"), "{}", outcome.summary);
        assert!(!outcome.summary.contains("pasto"));
    }

    #[tokio::test]
    async fn the_saved_city_is_used_only_without_a_named_one() {
        let outcome = handler().answer(None, WeatherWhen::Now).await;
        assert!(outcome.summary.contains("pasto"), "{}", outcome.summary);
    }

    #[tokio::test]
    async fn an_unknown_city_is_said_with_similar_names_and_never_replaced() {
        let outcome = handler()
            .answer(Some(&"Bogotx".into()), WeatherWhen::Now)
            .await;
        let ResponseBody::Error { message, hint, .. } = &outcome.body else {
            panic!("{:?}", outcome.body);
        };
        assert_eq!(message, "No encontré la ciudad «Bogotx».");
        assert_eq!(
            hint.as_deref(),
            Some("¿Quisiste decir «Bogotá, Colombia»? Vuelve a pedirlo con ese nombre.")
        );
        assert!(!outcome.summary.contains("pasto"));
    }

    #[test]
    fn the_hint_lists_the_closest_names() {
        assert_eq!(
            city_not_found_hint(&[]),
            "Revisa cómo está escrita y vuelve a pedirlo."
        );
        assert_eq!(
            city_not_found_hint(&["Cali, Colombia".into(), "Calima, Colombia".into(), "Cali, Perú".into()]),
            "Las más parecidas son «Cali, Colombia», «Calima, Colombia» y «Cali, Perú». Vuelve a pedirlo con el nombre correcto."
        );
    }
}
