//! Wi-Fi con la herramienta del sistema `networksetup` y `ipconfig`, con argumentos fijos.
//! macOS exige Location Services para revelar el nombre de la red: si lo oculta, se informa.

use std::process::Command;

use crate::models::{Reading, WifiInfo};

const SSID_HIDDEN_NOTE: &str = "macOS oculta el nombre de la red si Nexium no tiene permiso de Localización.";

fn run(program: &str, arguments: &[&str]) -> Option<String> {
    let output = Command::new(program).args(arguments).output().ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

fn wifi_device(hardware_ports: &str) -> Option<String> {
    let mut lines = hardware_ports.lines();
    while let Some(line) = lines.next() {
        if line.contains("Wi-Fi") || line.contains("AirPort") {
            return lines.next()?.strip_prefix("Device: ").map(str::to_owned);
        }
    }
    None
}

fn ssid_from_summary(summary: &str) -> Option<String> {
    summary.lines().find_map(|line| line.trim().strip_prefix("SSID : ").map(str::to_owned)).filter(|name| name != "<redacted>")
}

pub fn wifi_status() -> Reading<WifiInfo> {
    let Some(ports) = run("/usr/sbin/networksetup", &["-listallhardwareports"]) else {
        return Reading::Unavailable("macOS no permite consultar el Wi-Fi.".into());
    };
    let Some(device) = wifi_device(&ports) else {
        return Reading::Unavailable("Este equipo no tiene adaptador Wi-Fi.".into());
    };
    let Some(summary) = run("/usr/sbin/ipconfig", &["getsummary", &device]) else {
        return Reading::Unavailable("macOS no permite consultar el Wi-Fi.".into());
    };
    let connected = summary.contains("LinkStatusActive : TRUE");
    if !connected {
        return Reading::Value(WifiInfo { connected: false, ssid: Reading::Unavailable("Sin conexión Wi-Fi.".into()) });
    }
    let ssid = ssid_from_summary(&summary).map_or_else(|| Reading::Unavailable(SSID_HIDDEN_NOTE.into()), Reading::Value);
    Reading::Value(WifiInfo { connected: true, ssid })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_wifi_device() {
        let ports = "Hardware Port: Ethernet\nDevice: en1\n\nHardware Port: Wi-Fi\nDevice: en0\n";
        assert_eq!(wifi_device(ports).as_deref(), Some("en0"));
    }

    #[test]
    fn a_redacted_name_is_not_a_name() {
        assert_eq!(ssid_from_summary("  SSID : <redacted>\n"), None);
        assert_eq!(ssid_from_summary("  SSID : Casa\n").as_deref(), Some("Casa"));
    }
}
