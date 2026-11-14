//! Consultas reales del equipo detrás de una interfaz común. Lo que difiere por sistema está en `platform`.

use std::{
    net::SocketAddr,
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant},
};

use async_trait::async_trait;
use sysinfo::{
    CpuRefreshKind, MemoryRefreshKind, RefreshKind, System, MINIMUM_CPU_UPDATE_INTERVAL,
};

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

/// Intervalo mínimo entre dos lecturas para que el uso de CPU signifique algo.
const CPU_WINDOW: Duration = Duration::from_millis(250);
/// Una lectura anterior más antigua que esto ya no describe el uso actual.
const CPU_STALE: Duration = Duration::from_secs(3);

type CpuReply = mpsc::Sender<Option<f64>>;

/// Lector de CPU que vive en su propio hilo y se conserva entre solicitudes. En Windows, crearlo cuesta de
/// 1,2 a 2,4 s medidos: se crea al abrir Nexium (`warm_up`) y después cada pregunta cuesta el intervalo de
/// 250 ms, o nada si hubo una lectura reciente. Al vivir en su hilo, nunca bloquea el de la interfaz.
#[derive(Default)]
struct CpuSampler {
    requests: Mutex<Option<mpsc::Sender<CpuReply>>>,
}

impl CpuSampler {
    fn sender(&self) -> mpsc::Sender<CpuReply> {
        let mut requests = self
            .requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        requests
            .get_or_insert_with(|| {
                let (sender, receiver) = mpsc::channel();
                std::thread::spawn(move || serve_cpu(&receiver));
                sender
            })
            .clone()
    }

    /// Uso global de CPU en el último intervalo. Bloquea como mucho `CPU_WINDOW`.
    fn read(&self) -> Option<f64> {
        let (reply, answer) = mpsc::channel();
        self.sender().send(reply).ok()?;
        answer.recv().ok()?
    }
}

fn serve_cpu(requests: &mpsc::Receiver<CpuReply>) {
    let refresh = RefreshKind::nothing().with_cpu(CpuRefreshKind::nothing().with_usage());
    let mut system = System::new_with_specifics(refresh).ok();
    let mut last = Instant::now();
    for reply in requests {
        let value = system.as_mut().map(|system| {
            // Con una lectura reciente (de 250 ms a 3 s), basta con otra: es el uso de ese intervalo.
            if !(CPU_WINDOW..=CPU_STALE).contains(&last.elapsed()) {
                system.refresh_cpu_usage();
                std::thread::sleep(MINIMUM_CPU_UPDATE_INTERVAL.max(CPU_WINDOW));
            }
            system.refresh_cpu_usage();
            last = Instant::now();
            f64::from(system.global_cpu_usage())
        });
        let _ = reply.send(value);
    }
}

pub struct NativeSystemProvider {
    theme: ThemeSource,
    cpu: Arc<CpuSampler>,
}

impl NativeSystemProvider {
    pub fn new(theme: ThemeSource) -> Self {
        Self {
            theme,
            cpu: Arc::default(),
        }
    }

    /// Prepara las lecturas lentas de crear en un hilo aparte. Se llama al abrir Nexium.
    pub fn warm_up(&self) {
        let cpu = self.cpu.clone();
        std::thread::spawn(move || cpu.read());
    }
}

fn read_battery() -> Reading<BatteryInfo> {
    let manager = match starship_battery::Manager::new() {
        Ok(manager) => manager,
        Err(_) => {
            return Reading::Unavailable("El sistema no permite consultar la batería.".into())
        }
    };
    let first = manager
        .batteries()
        .ok()
        .and_then(|mut batteries| batteries.next());
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
            Reading::Value(BatteryInfo {
                percent,
                state: state.into(),
            })
        }
        Some(Err(_)) => Reading::Unavailable("El sistema no permite consultar la batería.".into()),
        None => Reading::Unavailable("Este equipo no tiene batería.".into()),
    }
}

#[async_trait]
impl SystemProvider for NativeSystemProvider {
    async fn cpu_percent(&self) -> Reading<f64> {
        // El uso de CPU es una diferencia entre dos lecturas: se mide fuera del hilo de la interfaz.
        let cpu = self.cpu.clone();
        match tokio::task::spawn_blocking(move || cpu.read()).await {
            Ok(Some(percent)) => Reading::Value(percent),
            _ => Reading::Unavailable("El sistema no permite consultar la CPU.".into()),
        }
    }

    async fn memory(&self) -> Reading<MemoryInfo> {
        let refresh = RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram());
        let Ok(system) = System::new_with_specifics(refresh) else {
            return Reading::Unavailable("El sistema no permite consultar la memoria.".into());
        };
        Reading::Value(MemoryInfo {
            used_bytes: system.used_memory(),
            available_bytes: system.available_memory(),
            total_bytes: system.total_memory(),
        })
    }

    async fn battery(&self) -> Reading<BatteryInfo> {
        tokio::task::spawn_blocking(read_battery)
            .await
            .unwrap_or_else(|_| Reading::Unavailable("No se pudo consultar la batería.".into()))
    }

    async fn internet(&self) -> Reading<bool> {
        // Conexión real a dos servidores públicos: no se envía ningún dato.
        const PROBES: [([u8; 4], u16); 2] = [([1, 1, 1, 1], 443), ([8, 8, 8, 8], 443)];
        for (ip, port) in PROBES {
            let address = SocketAddr::from((ip, port));
            if let Ok(Ok(_)) = tokio::time::timeout(
                Duration::from_secs(3),
                tokio::net::TcpStream::connect(address),
            )
            .await
            {
                return Reading::Value(true);
            }
        }
        Reading::Value(false)
    }

    async fn wifi(&self) -> Reading<WifiInfo> {
        tokio::task::spawn_blocking(platform::wifi_status)
            .await
            .unwrap_or_else(|_| Reading::Unavailable("No se pudo consultar el Wi-Fi.".into()))
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
    async fn cpu_is_a_percentage_and_a_recent_reading_is_reused() {
        let provider = provider();
        let Reading::Value(first) = provider.cpu_percent().await else {
            return; // Sin acceso a la CPU en este sistema: no hay nada que comprobar.
        };
        assert!((0.0..=100.0).contains(&first));
        tokio::time::sleep(CPU_WINDOW).await;
        let started = Instant::now();
        let Reading::Value(second) = provider.cpu_percent().await else {
            panic!("la segunda lectura falló")
        };
        assert!((0.0..=100.0).contains(&second));
        // Con una lectura de hace 250 ms no hay que esperar otro intervalo.
        assert!(started.elapsed() < CPU_WINDOW, "{:?}", started.elapsed());
    }

    #[tokio::test]
    async fn memory_values_are_consistent() {
        let Reading::Value(memory) = provider().memory().await else {
            panic!("sin memoria")
        };
        assert!(memory.total_bytes > 0);
        assert!(memory.used_bytes <= memory.total_bytes);
        assert!(memory.available_bytes <= memory.total_bytes);
    }

    #[tokio::test]
    async fn cpu_is_a_percentage() {
        let Reading::Value(cpu) = provider().cpu_percent().await else {
            panic!("sin CPU")
        };
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
