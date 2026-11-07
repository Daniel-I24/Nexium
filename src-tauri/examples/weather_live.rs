//! Prueba real contra Open-Meteo. Uso: cargo run --example weather_live -- "<ciudad>" ...
use nexium_lib::providers::weather::{OpenMeteo, WeatherProvider};

#[tokio::main]
async fn main() {
    let provider = OpenMeteo::default();
    for city in std::env::args().skip(1) {
        println!("{city}: {:?}", provider.report(&city).await);
    }
}
