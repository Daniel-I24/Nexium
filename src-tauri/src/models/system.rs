//! Lecturas del equipo: un valor real o la causa de que no exista.

/// Lectura del sistema: un valor real o la causa de que no exista. No hay tercera opción.
#[derive(Debug, Clone, PartialEq)]
pub enum Reading<T> {
    Value(T),
    Unavailable(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MemoryInfo {
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BatteryInfo {
    pub percent: f64,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WifiInfo {
    pub connected: bool,
    pub ssid: Reading<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemMetric {
    Cpu,
    Ram,
    Battery,
    Internet,
    Wifi,
    DarkMode,
    Overview,
}
