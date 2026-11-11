//! Wi-Fi mediante NetworkManager (`nmcli`), con argumentos fijos.

use std::process::Command;

use crate::models::{Reading, WifiInfo};

fn run_nmcli(arguments: &[&str]) -> Option<String> {
    let output = Command::new("nmcli").args(arguments).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn wifi_status() -> Reading<WifiInfo> {
    let Some(devices) = run_nmcli(&["-t", "-f", "TYPE,STATE,CONNECTION", "device"]) else {
        return Reading::Unavailable("Este sistema no usa NetworkManager.".into());
    };
    parse_devices(&devices)
}

fn parse_devices(output: &str) -> Reading<WifiInfo> {
    let wifi: Vec<Vec<&str>> = output
        .lines()
        .map(|line| line.splitn(3, ':').collect::<Vec<_>>())
        .filter(|fields| fields.first() == Some(&"wifi"))
        .collect();
    if wifi.is_empty() {
        return Reading::Unavailable("Este equipo no tiene adaptador Wi-Fi.".into());
    }
    match wifi
        .iter()
        .find(|fields| fields.get(1) == Some(&"connected"))
    {
        Some(fields) => {
            let name = fields
                .get(2)
                .copied()
                .unwrap_or_default()
                .replace("\\:", ":");
            let ssid = if name.is_empty() {
                Reading::Unavailable("El sistema no indica el nombre de la red.".into())
            } else {
                Reading::Value(name)
            };
            Reading::Value(WifiInfo {
                connected: true,
                ssid,
            })
        }
        None => Reading::Value(WifiInfo {
            connected: false,
            ssid: Reading::Unavailable("Sin conexión Wi-Fi.".into()),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_connected_network() {
        let Reading::Value(info) =
            parse_devices("ethernet:unavailable:\nwifi:connected:Casa\\:5G\n")
        else {
            panic!()
        };
        assert!(info.connected);
        assert_eq!(info.ssid, Reading::Value("Casa:5G".into()));
    }

    #[test]
    fn no_adapter_is_unavailable() {
        assert!(matches!(
            parse_devices("ethernet:connected:Cable\n"),
            Reading::Unavailable(_)
        ));
    }

    #[test]
    fn disconnected_adapter_has_no_name() {
        let Reading::Value(info) = parse_devices("wifi:disconnected:\n") else {
            panic!()
        };
        assert!(!info.connected);
    }
}
