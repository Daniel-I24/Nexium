//! Adaptadores por sistema operativo. Cada uno compila solo en su plataforma.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
mod window_capture;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::wifi_status;
#[cfg(target_os = "macos")]
pub use macos::wifi_status;
#[cfg(target_os = "windows")]
pub use windows::wifi_status;

pub use window_capture::WindowAwareCapture;
