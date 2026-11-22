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
    /// Nombre oficial de una ciudad ("medellin" pasa a "Medellín"), o `CityNotFound` si no existe.
    async fn find_city(&self, city: &str) -> Result<String, WeatherError>;
    /// Ciudades de nombre parecido para una que no existe ("Bogotaa"), como mucho tres. Vacío si no hay o si
    /// falla la conexión: solo sirve para sugerir.
    async fn similar_cities(&self, _city: &str) -> Vec<String> {
        Vec::new()
    }
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

impl OpenMeteo {
    async fn search(&self, name: &str, count: usize) -> Result<Vec<Place>, WeatherError> {
        let places: GeocodingResponse = self
            .get(
                format!("{}/search", self.geocoding_url),
                &[
                    ("name", name.to_owned()),
                    ("count", count.to_string()),
                    ("language", "es".into()),
                    ("format", "json".into()),
                ],
            )
            .await?;
        Ok(places.results)
    }

    async fn geocode(&self, city: &str) -> Result<Place, WeatherError> {
        self.search(city, 1)
            .await?
            .into_iter()
            .next()
            .ok_or(WeatherError::CityNotFound)
    }
}

/// Comienzos cada vez más cortos de un nombre, de al menos tres letras: Open-Meteo busca por el comienzo del
/// nombre, así que «Bogotaa» encuentra Bogotá al buscar «Bogota».
pub fn shorter_prefixes(city: &str) -> Vec<String> {
    let chars: Vec<char> = city.trim().chars().collect();
    let mut prefixes: Vec<String> = [chars.len().saturating_sub(1), chars.len() * 2 / 3, 4]
        .into_iter()
        .filter(|&length| length >= 3 && length < chars.len())
        .map(|length| chars[..length].iter().collect::<String>().trim().to_owned())
        .collect();
    prefixes.dedup();
    prefixes
}

fn place_name(place: Place) -> String {
    if place.country.is_empty() {
        place.name
    } else {
        format!("{}, {}", place.name, place.country)
    }
}

#[async_trait]
impl WeatherProvider for OpenMeteo {
    async fn find_city(&self, city: &str) -> Result<String, WeatherError> {
        Ok(self.geocode(city).await?.name)
    }

    async fn similar_cities(&self, city: &str) -> Vec<String> {
        for prefix in shorter_prefixes(city) {
            let Ok(places) = self.search(&prefix, 3).await else {
                return Vec::new();
            };
            if !places.is_empty() {
                let mut names: Vec<String> = Vec::new();
                for name in places.into_iter().map(place_name) {
                    if !names.contains(&name) {
                        names.push(name);
                    }
                }
                return names;
            }
        }
        Vec::new()
    }

    async fn report(&self, city: &str) -> Result<WeatherReport, WeatherError> {
        let place = self.geocode(city).await?;
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
            place: place_name(place),
            temperature_c: forecast.current.temperature_2m,
            condition: condition_name(forecast.current.weather_code).to_owned(),
            today: day(&forecast.daily, 0)?,
            tomorrow: day(&forecast.daily, 1)?,
        })
    }
}

#[cfg(test)]
#[path = "tests/weather.rs"]
mod tests;
