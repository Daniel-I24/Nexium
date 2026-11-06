//! Consultas reales del equipo detrás de una interfaz común. Lo que difiere por sistema está en `platform`.

use std::{net::SocketAddr, sync::Arc, time::Duration};

use async_trait::async_trait;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System, MINIMUM_CPU_UPDATE_INTERVAL};

use crate::{
    models::{BatteryInfo, MemoryInfo, Reading, WifiInfo},
    platform,
};

#[async_trait]
pub trait SystemProvider: Send + Sync {
    async fn cpu_percent(&self) -> Reading<f64>;
    async fn memory(&self) -> Reading<MemoryInfo>;
    async fn battery(&self) -> Reading<BatteryInfo>;
    async fn internet(&self) -> Reading<bool>;
    async fn wifi(&self) -> Reading<WifiInfo>;
    async fn dark_mode(&self) -> Reading<bool>;
}

pub type ThemeSource = Arc<dyn Fn() -> Reading<bool> + Send + Sync>;

pub struct NativeSystemProvider {
    theme: ThemeSource,
}

impl NativeSystemProvider {
    pub fn new(theme: ThemeSource) -> Self {
        Self { theme }
    }
}

fn read_battery() -> Reading<BatteryInfo> {
    let manager = match starship_battery::Manager::new() {
        Ok(manager) => manager,
        Err(_) => return Reading::Unavailable("El sistema no permite consultar la batería.".into()),
    };
    let first = manager.batteries().ok().and_then(|mut batteries| batteries.next());
    match first {
        Some(Ok(battery)) => {
            let percent = f64::from(battery.state_of_charge().value) * 100.0;
            let state = match battery.state() {
                starship_battery::State::Charging => "Cargando",
                starship_battery::State::Discharging => "En uso",
                starship_battery::State::Full => "Completa",
                starship_battery::State::Empty => "Vacía",
                _ => "Sin información",
            };
            Reading::Value(BatteryInfo { percent, state: state.into() })
        }
        Some(Err(_)) => Reading::Unavailable("El sistema no permite consultar la batería.".into()),
        None => Reading::Unavailable("Este equipo no tiene batería.".into()),
    }
}

#[async_trait]
impl SystemProvider for NativeSystemProvider {
    async fn cpu_percent(&self) -> Reading<f64> {
        let refresh = RefreshKind::nothing().with_cpu(CpuRefreshKind::nothing().with_usage());
        let Ok(mut system) = System::new_with_specifics(refresh) else {
            return Reading::Unavailable("El sistema no permite consultar la CPU.".into());
        };
        // El uso de CPU es una diferencia entre dos lecturas.
        tokio::time::sleep(MINIMUM_CPU_UPDATE_INTERVAL.max(Duration::from_millis(250))).await;
        system.refresh_cpu_usage();
        Reading::Value(f64::from(system.global_cpu_usage()))
    }

    async fn memory(&self) -> Reading<MemoryInfo> {
        let refresh = RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram());
        let Ok(system) = System::new_with_specifics(refresh) else {
            return Reading::Unavailable("El sistema no permite consultar la memoria.".into());
        };
        Reading::Value(MemoryInfo { used_bytes: system.used_memory(), available_bytes: system.available_memory(), total_bytes: system.total_memory() })
    }

    async fn battery(&self) -> Reading<BatteryInfo> {
        tokio::task::spawn_blocking(read_battery).await.unwrap_or_else(|_| Reading::Unavailable("No se pudo consultar la batería.".into()))
    }

    async fn internet(&self) -> Reading<bool> {
        // Conexión real a dos servidores públicos: no se envía ningún dato.
        const PROBES: [([u8; 4], u16); 2] = [([1, 1, 1, 1], 443), ([8, 8, 8, 8], 443)];
        for (ip, port) in PROBES {
            let address = SocketAddr::from((ip, port));
            if let Ok(Ok(_)) = tokio::time::timeout(Duration::from_secs(3), tokio::net::TcpStream::connect(address)).await {
                return Reading::Value(true);
            }
        }
        Reading::Value(false)
    }

    async fn wifi(&self) -> Reading<WifiInfo> {
        tokio::task::spawn_blocking(platform::wifi_status).await.unwrap_or_else(|_| Reading::Unavailable("No se pudo consultar el Wi-Fi.".into()))
    }

    async fn dark_mode(&self) -> Reading<bool> {
        (self.theme)()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider() -> NativeSystemProvider {
        NativeSystemProvider::new(Arc::new(|| Reading::Value(true)))
    }

    #[tokio::test]
    async fn memory_values_are_consistent() {
        let Reading::Value(memory) = provider().memory().await else { panic!("sin memoria") };
        assert!(memory.total_bytes > 0);
        assert!(memory.used_bytes <= memory.total_bytes);
        assert!(memory.available_bytes <= memory.total_bytes);
    }

    #[tokio::test]
    async fn cpu_is_a_percentage() {
        let Reading::Value(cpu) = provider().cpu_percent().await else { panic!("sin CPU") };
        assert!((0.0..=100.0).contains(&cpu));
    }

    #[tokio::test]
    async fn battery_never_invents_a_value() {
        // En un equipo sin batería debe ser Unavailable; con batería, un porcentaje válido.
        if let Reading::Value(battery) = provider().battery().await {
            assert!((0.0..=100.5).contains(&battery.percent));
        }
    }

    #[tokio::test]
    async fn wifi_query_does_not_panic_on_any_system() {
        let _ = provider().wifi().await;
    }
}
