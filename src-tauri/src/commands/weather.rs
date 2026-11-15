//! Ciudad para el clima: siempre se comprueba con Open-Meteo antes de guardarla.

use tauri::State;

use super::AppState;
use crate::{
    error::{AppError, AppResult},
    providers::weather::{WeatherError, WeatherProvider},
};

/// Nombre oficial de la ciudad escrita, o un error en español si no existe o no hay conexión.
pub async fn verified_city(weather: &dyn WeatherProvider, city: &str) -> AppResult<String> {
    let city = city.trim();
    if city.is_empty() || city.chars().count() > 100 {
        return Err(AppError::Validation(
            "Escribe el nombre de una ciudad.".into(),
        ));
    }
    match weather.find_city(city).await {
        Ok(name) => Ok(name),
        Err(WeatherError::CityNotFound) => Err(AppError::Validation(format!(
            "No encontré la ciudad «{city}». Escríbela de otra forma, por ejemplo con su país."
        ))),
        Err(_) => Err(AppError::Operation(
            "No pude comprobar la ciudad con Open-Meteo. Revisa tu conexión a Internet.".into(),
        )),
    }
}

/// Guarda la ciudad que se pidió en la conversación y devuelve su nombre oficial.
#[tauri::command]
pub async fn weather_city_set(state: State<'_, AppState>, city: String) -> AppResult<String> {
    let name = verified_city(state.weather.as_ref(), &city).await?;
    let mut settings = state.settings.get()?;
    settings.weather_city.clone_from(&name);
    state.settings.save(&settings)?;
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::weather::WeatherReport;
    use async_trait::async_trait;

    struct FakeWeather;

    #[async_trait]
    impl WeatherProvider for FakeWeather {
        async fn report(&self, _: &str) -> Result<WeatherReport, WeatherError> {
            Err(WeatherError::Unreachable)
        }
        async fn find_city(&self, city: &str) -> Result<String, WeatherError> {
            match crate::core::rules::normalize(city).as_str() {
                "medellin" => Ok("Medellín".into()),
                "sin red" => Err(WeatherError::Unreachable),
                _ => Err(WeatherError::CityNotFound),
            }
        }
    }

    #[tokio::test]
    async fn the_city_is_saved_with_its_official_name() {
        assert_eq!(
            verified_city(&FakeWeather, "  medellin ").await.unwrap(),
            "Medellín"
        );
    }

    #[tokio::test]
    async fn an_unknown_city_or_no_connection_is_explained() {
        let unknown = verified_city(&FakeWeather, "Ciudad Inventada")
            .await
            .unwrap_err()
            .to_string();
        assert!(unknown.contains("No encontré la ciudad «Ciudad Inventada»"));
        let offline = verified_city(&FakeWeather, "sin red")
            .await
            .unwrap_err()
            .to_string();
        assert!(offline.contains("Open-Meteo"));
        assert!(verified_city(&FakeWeather, "  ").await.is_err());
    }
}
