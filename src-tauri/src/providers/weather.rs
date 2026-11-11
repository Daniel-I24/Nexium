//! Clima real de Open-Meteo. Sin conexión se informa: nunca se muestran datos inventados ni antiguos.

use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;

const GEOCODING_URL: &str = "https://geocoding-api.open-meteo.com/v1";
const FORECAST_URL: &str = "https://api.open-meteo.com/v1";
const TIMEOUT: Duration = Duration::from_secs(10);

pub const ATTRIBUTION_TEXT: &str = "Datos meteorológicos de Open-Meteo (CC BY 4.0)";
pub const ATTRIBUTION_URL: &str = "https://open-meteo.com/";

#[derive(Debug, PartialEq)]
pub enum WeatherError {
    CityNotFound,
    Unreachable,
    Invalid,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DayForecast {
    pub condition: String,
    pub min_c: f64,
    pub max_c: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WeatherReport {
    pub place: String,
    pub temperature_c: f64,
    pub condition: String,
    pub today: DayForecast,
    pub tomorrow: DayForecast,
}

#[async_trait]
pub trait WeatherProvider: Send + Sync {
    async fn report(&self, city: &str) -> Result<WeatherReport, WeatherError>;
}

pub struct OpenMeteo {
    http: reqwest::Client,
    geocoding_url: String,
    forecast_url: String,
}

#[derive(Deserialize)]
struct GeocodingResponse {
    #[serde(default)]
    results: Vec<Place>,
}

#[derive(Deserialize)]
struct Place {
    name: String,
    latitude: f64,
    longitude: f64,
    #[serde(default)]
    country: String,
}

#[derive(Deserialize)]
struct ForecastResponse {
    current: Current,
    daily: Daily,
}

#[derive(Deserialize)]
struct Current {
    temperature_2m: f64,
    weather_code: u32,
}

#[derive(Deserialize)]
struct Daily {
    weather_code: Vec<u32>,
    temperature_2m_max: Vec<f64>,
    temperature_2m_min: Vec<f64>,
}

/// Códigos WMO que usa Open-Meteo.
pub fn condition_name(code: u32) -> &'static str {
    match code {
        0 => "despejado",
        1 => "mayormente despejado",
        2 => "parcialmente nublado",
        3 => "nublado",
        45 | 48 => "niebla",
        51 | 53 | 55 | 56 | 57 => "llovizna",
        61 | 63 | 65 | 66 | 67 => "lluvia",
        71 | 73 | 75 | 77 => "nieve",
        80..=82 => "chubascos",
        85 | 86 => "chubascos de nieve",
        95 | 96 | 99 => "tormenta",
        _ => "condición desconocida",
    }
}

impl Default for OpenMeteo {
    fn default() -> Self {
        Self::with_urls(GEOCODING_URL, FORECAST_URL)
    }
}

impl OpenMeteo {
    pub fn with_urls(geocoding_url: &str, forecast_url: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            geocoding_url: geocoding_url.to_owned(),
            forecast_url: forecast_url.to_owned(),
        }
    }

    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        url: String,
        query: &[(&str, String)],
    ) -> Result<T, WeatherError> {
        let response = self
            .http
            .get(url)
            .query(query)
            .timeout(TIMEOUT)
            .send()
            .await
            .map_err(|_| WeatherError::Unreachable)?;
        if !response.status().is_success() {
            return Err(WeatherError::Unreachable);
        }
        response.json().await.map_err(|_| WeatherError::Invalid)
    }
}

fn day(daily: &Daily, index: usize) -> Result<DayForecast, WeatherError> {
    Ok(DayForecast {
        condition: condition_name(*daily.weather_code.get(index).ok_or(WeatherError::Invalid)?)
            .to_owned(),
        min_c: *daily
            .temperature_2m_min
            .get(index)
            .ok_or(WeatherError::Invalid)?,
        max_c: *daily
            .temperature_2m_max
            .get(index)
            .ok_or(WeatherError::Invalid)?,
    })
}

#[async_trait]
impl WeatherProvider for OpenMeteo {
    async fn report(&self, city: &str) -> Result<WeatherReport, WeatherError> {
        let places: GeocodingResponse = self
            .get(
                format!("{}/search", self.geocoding_url),
                &[
                    ("name", city.to_owned()),
                    ("count", "1".into()),
                    ("language", "es".into()),
                    ("format", "json".into()),
                ],
            )
            .await?;
        let place = places
            .results
            .into_iter()
            .next()
            .ok_or(WeatherError::CityNotFound)?;
        let forecast: ForecastResponse = self
            .get(
                format!("{}/forecast", self.forecast_url),
                &[
                    ("latitude", place.latitude.to_string()),
                    ("longitude", place.longitude.to_string()),
                    ("current", "temperature_2m,weather_code".into()),
                    (
                        "daily",
                        "weather_code,temperature_2m_max,temperature_2m_min".into(),
                    ),
                    ("timezone", "auto".into()),
                    ("forecast_days", "2".into()),
                ],
            )
            .await?;
        Ok(WeatherReport {
            place: if place.country.is_empty() {
                place.name
            } else {
                format!("{}, {}", place.name, place.country)
            },
            temperature_c: forecast.current.temperature_2m,
            condition: condition_name(forecast.current.weather_code).to_owned(),
            today: day(&forecast.daily, 0)?,
            tomorrow: day(&forecast.daily, 1)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::{
        matchers::{path, query_param},
        Mock, MockServer, ResponseTemplate,
    };

    async fn server_with_data() -> MockServer {
        let server = MockServer::start().await;
        Mock::given(path("/search"))
            .and(query_param("name", "Bogotá"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "results": [{ "name": "Bogotá", "latitude": 4.6, "longitude": -74.1, "country": "Colombia" }] })))
            .mount(&server)
            .await;
        Mock::given(path("/search"))
            .and(query_param("name", "Nolugar"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .mount(&server)
            .await;
        Mock::given(path("/forecast"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "current": { "temperature_2m": 15.5, "weather_code": 2 },
                "daily": { "weather_code": [3, 61], "temperature_2m_max": [19.0, 18.0], "temperature_2m_min": [9.0, 8.0] }
            })))
            .mount(&server)
            .await;
        server
    }

    fn client(server: &MockServer) -> OpenMeteo {
        OpenMeteo::with_urls(&server.uri(), &server.uri())
    }

    #[tokio::test]
    async fn separates_current_conditions_from_the_forecast() {
        let server = server_with_data().await;
        let report = client(&server).report("Bogotá").await.unwrap();
        assert_eq!(report.place, "Bogotá, Colombia");
        assert_eq!(report.temperature_c, 15.5);
        assert_eq!(report.condition, "parcialmente nublado");
        assert_eq!(report.today.max_c, 19.0);
        assert_eq!(report.tomorrow.condition, "lluvia");
    }

    #[tokio::test]
    async fn an_unknown_city_is_reported() {
        let server = server_with_data().await;
        assert_eq!(
            client(&server).report("Nolugar").await.unwrap_err(),
            WeatherError::CityNotFound
        );
    }

    #[tokio::test]
    async fn no_connection_never_invents_data() {
        let client = OpenMeteo::with_urls("http://127.0.0.1:9", "http://127.0.0.1:9");
        assert_eq!(
            client.report("Bogotá").await.unwrap_err(),
            WeatherError::Unreachable
        );
    }

    #[tokio::test]
    async fn a_server_error_is_reported() {
        let server = MockServer::start().await;
        Mock::given(path("/search"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        assert_eq!(
            client(&server).report("Bogotá").await.unwrap_err(),
            WeatherError::Unreachable
        );
    }
}
