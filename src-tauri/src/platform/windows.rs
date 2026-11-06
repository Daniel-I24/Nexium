//! Wi-Fi mediante la API oficial WLAN.

use std::{ffi::c_void, ptr::null_mut};

use windows::Win32::{
    Foundation::{ERROR_SERVICE_NOT_ACTIVE, HANDLE, WIN32_ERROR},
    NetworkManagement::WiFi::{
        wlan_interface_state_connected, wlan_intf_opcode_current_connection, WlanCloseHandle, WlanEnumInterfaces, WlanFreeMemory, WlanOpenHandle,
        WlanQueryInterface, WLAN_CONNECTION_ATTRIBUTES, WLAN_INTERFACE_INFO_LIST,
    },
};

use crate::models::{Reading, WifiInfo};

const SSID_HIDDEN_NOTE: &str = "Windows oculta el nombre si no activas \u{201c}Permitir que las aplicaciones de escritorio accedan a tu ubicación\u{201d}.";

struct WlanHandle(HANDLE);

impl Drop for WlanHandle {
    fn drop(&mut self) {
        // SAFETY: el identificador viene de WlanOpenHandle y se cierra una sola vez.
        unsafe { WlanCloseHandle(self.0, None) };
    }
}

pub fn wifi_status() -> Reading<WifiInfo> {
    let mut version = 0u32;
    let mut handle = HANDLE::default();
    // SAFETY: los punteros apuntan a variables locales válidas durante la llamada.
    let opened = WIN32_ERROR(unsafe { WlanOpenHandle(2, None, &mut version, &mut handle) });
    if opened == ERROR_SERVICE_NOT_ACTIVE {
        return Reading::Unavailable("El servicio de Wi-Fi de Windows no está activo.".into());
    }
    if opened.is_err() {
        return Reading::Unavailable("Windows no permite consultar el Wi-Fi.".into());
    }
    let handle = WlanHandle(handle);

    let mut list: *mut WLAN_INTERFACE_INFO_LIST = null_mut();
    // SAFETY: `list` recibe memoria de la API, liberada abajo con WlanFreeMemory.
    if WIN32_ERROR(unsafe { WlanEnumInterfaces(handle.0, None, &mut list) }).is_err() || list.is_null() {
        return Reading::Unavailable("Windows no permite consultar el Wi-Fi.".into());
    }
    // SAFETY: la API devolvió una lista válida con `dwNumberOfItems` elementos consecutivos.
    let result = unsafe {
        let count = (*list).dwNumberOfItems as usize;
        let items = std::slice::from_raw_parts((*list).InterfaceInfo.as_ptr(), count);
        if items.is_empty() {
            Reading::Unavailable("Este equipo no tiene adaptador Wi-Fi.".into())
        } else if let Some(item) = items.iter().find(|item| item.isState == wlan_interface_state_connected) {
            Reading::Value(WifiInfo { connected: true, ssid: current_ssid(&handle, &item.InterfaceGuid) })
        } else {
            Reading::Value(WifiInfo { connected: false, ssid: Reading::Unavailable("Sin conexión Wi-Fi.".into()) })
        }
    };
    // SAFETY: `list` no se vuelve a usar después de liberarla.
    unsafe { WlanFreeMemory(list as *const c_void) };
    result
}

fn current_ssid(handle: &WlanHandle, guid: &windows::core::GUID) -> Reading<String> {
    let mut size = 0u32;
    let mut data: *mut c_void = null_mut();
    // SAFETY: `data` recibe memoria de la API, liberada abajo.
    let status = WIN32_ERROR(unsafe { WlanQueryInterface(handle.0, guid, wlan_intf_opcode_current_connection, None, &mut size, &mut data, None) });
    if status.is_err() || data.is_null() {
        return Reading::Unavailable(SSID_HIDDEN_NOTE.into());
    }
    // SAFETY: con este opcode, la API devuelve un WLAN_CONNECTION_ATTRIBUTES.
    let ssid = unsafe {
        let attributes = &*(data as *const WLAN_CONNECTION_ATTRIBUTES);
        let raw = attributes.wlanAssociationAttributes.dot11Ssid;
        let length = (raw.uSSIDLength as usize).min(raw.ucSSID.len());
        String::from_utf8_lossy(&raw.ucSSID[..length]).into_owned()
    };
    // SAFETY: `data` no se vuelve a usar después de liberarla.
    unsafe { WlanFreeMemory(data) };
    if ssid.is_empty() {
        Reading::Unavailable(SSID_HIDDEN_NOTE.into())
    } else {
        Reading::Value(ssid)
    }
}
