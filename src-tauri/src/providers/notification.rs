//! Notificaciones del sistema. Los recordatorios, en Windows, usan el escenario de recordatorio: el aviso
//! aparece desplegado y se queda en pantalla hasta que la persona lo descarta, en lugar de pasar al centro de
//! notificaciones a los pocos segundos.

use tauri::{AppHandle, Runtime};
use tauri_plugin_notification::NotificationExt;

pub trait NotificationProvider: Send + Sync {
    fn notify(&self, title: &str, body: &str) -> Result<(), String>;
    /// Un aviso que debe verse aunque la persona no esté mirando: se queda hasta descartarlo, si el sistema lo
    /// permite. Donde no hay esa opción, es una notificación normal.
    fn notify_reminder(&self, title: &str, body: &str) -> Result<(), String>;
}

pub struct TauriNotifications<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> TauriNotifications<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: Runtime> NotificationProvider for TauriNotifications<R> {
    fn notify(&self, title: &str, body: &str) -> Result<(), String> {
        self.app
            .notification()
            .builder()
            .title(title)
            .body(body)
            .show()
            .map_err(|error| error.to_string())
    }

    #[cfg(windows)]
    fn notify_reminder(&self, title: &str, body: &str) -> Result<(), String> {
        windows_reminder::show(&self.app, title, body).or_else(|_| self.notify(title, body))
    }

    #[cfg(not(windows))]
    fn notify_reminder(&self, title: &str, body: &str) -> Result<(), String> {
        self.notify(title, body)
    }
}

#[cfg(windows)]
mod windows_reminder {
    use super::{AppHandle, Runtime};
    use tauri::Manager;
    use tauri_winrt_notification::{Scenario, Toast};

    /// Acción del botón del aviso.
    const OPEN: &str = "abrir";

    /// El mismo identificador que usa Tauri: el de la app instalada o, al ejecutarla sin instalar (desde
    /// `target\debug` o `target\release`), el de PowerShell, porque Windows no conoce todavía a Nexium.
    pub fn app_id<R: Runtime>(app: &AppHandle<R>) -> String {
        let installed = tauri::utils::platform::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.display().to_string()))
            .is_some_and(|dir| {
                !(dir.ends_with(r"\target\debug") || dir.ends_with(r"\target\release"))
            });
        if installed {
            app.config().identifier.clone()
        } else {
            Toast::POWERSHELL_APP_ID.to_owned()
        }
    }

    /// Windows solo respeta el escenario de recordatorio si el aviso tiene al menos un botón.
    pub fn show<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str) -> Result<(), String> {
        let handle = app.clone();
        Toast::new(&app_id(app))
            .title(title)
            .text1(body)
            .scenario(Scenario::Reminder)
            .add_button("Abrir Nexium", OPEN)
            .on_activated(move |action| {
                if action.as_deref() == Some(OPEN) || action.is_none() {
                    if let Some(window) = handle.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                    }
                }
                Ok(())
            })
            .show()
            .map_err(|error| error.to_string())
    }
}
