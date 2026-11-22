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
async fn a_city_is_found_by_its_official_name() {
    let server = server_with_data().await;
    assert_eq!(client(&server).find_city("Bogotá").await.unwrap(), "Bogotá");
    assert_eq!(
        client(&server).find_city("Nolugar").await.unwrap_err(),
        WeatherError::CityNotFound
    );
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
async fn an_unknown_city_gets_similar_names() {
    let server = server_with_data().await;
    Mock::given(path("/search"))
        .and(query_param("name", "Bogot"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "results": [
            { "name": "Bogotá", "latitude": 4.6, "longitude": -74.1, "country": "Colombia" },
            { "name": "Bogotol", "latitude": 56.2, "longitude": 89.5, "country": "Rusia" },
            { "name": "Bogotol", "latitude": 56.3, "longitude": 89.6, "country": "Rusia" }
        ] })),
        )
        .mount(&server)
        .await;
    assert_eq!(
        client(&server).similar_cities("Bogotx").await,
        ["Bogotá, Colombia", "Bogotol, Rusia"]
    );
    assert!(client(&server).similar_cities("Nolugar").await.is_empty());
}

#[test]
fn suggestions_search_by_shorter_beginnings() {
    assert_eq!(shorter_prefixes("Bogotaa"), ["Bogota", "Bogo"]);
    assert_eq!(
        shorter_prefixes("Medellinn"),
        ["Medellin", "Medell", "Mede"]
    );
    assert!(shorter_prefixes("Rio").is_empty());
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
