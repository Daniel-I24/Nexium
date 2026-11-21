//! Notificaciones del sistema. Los recordatorios, en Windows, usan el escenario de recordatorio: el aviso
//! aparece desplegado y se queda en pantalla hasta que la persona lo descarta, en lugar de pasar al centro de
//! notificaciones a los pocos segundos.

use tauri::{AppHandle, Emitter, Runtime};
use tauri_plugin_notification::NotificationExt;

pub trait NotificationProvider: Send + Sync {
    fn notify(&self, title: &str, body: &str) -> Result<(), String>;
    /// Un aviso que debe verse aunque la persona no esté mirando: se queda hasta descartarlo, si el sistema lo
    /// permite. Donde no hay esa opción, es una notificación normal. Al abrirlo, Nexium muestra ese recordatorio.
    fn notify_reminder(&self, title: &str, body: &str, reminder_id: i64) -> Result<(), String>;
}

/// Evento que abre la pantalla de Recordatorios y resalta el recordatorio indicado.
pub const OPEN_REMINDER_EVENT: &str = "reminders://open";

/// Emoji del título del aviso según lo que hay que hacer. Solo en las notificaciones: dentro de la app se usan
/// los iconos del sistema visual. Se elige en el equipo por palabras clave, sin IA.
pub fn reminder_emoji(content: &str) -> &'static str {
    const GROUPS: &[(&str, &[&str])] = &[
        (
            "💊",
            &[
                "medicina",
                "medicamento",
                "pastilla",
                "pildora",
                "dosis",
                "remedio",
                "jarabe",
                "inyeccion",
                "antibiotico",
                "vitamina",
            ],
        ),
        ("💧", &["agua", "beber", "hidrat"]),
        (
            "🗓️",
            &[
                "reunion",
                "cita",
                "junta",
                "entrevista",
                "llamada",
                "consulta",
                "videollamada",
            ],
        ),
        (
            "📚",
            &[
                "estudi", "tarea", "entrega", "examen", "parcial", "proyecto", "leer", "lectura",
                "clase", "informe", "trabajo",
            ],
        ),
        (
            "🛒",
            &["compra", "mercado", "supermercado", "tienda", "pedido"],
        ),
        (
            "🏃",
            &[
                "ejercicio",
                "correr",
                "gimnasio",
                "gym",
                "caminar",
                "entrenar",
                "deporte",
                "yoga",
                "nadar",
            ],
        ),
    ];
    let text = crate::core::rules::normalize(content);
    let words: Vec<&str> = text.split(' ').collect();
    GROUPS
        .iter()
        .find(|(_, keys)| {
            keys.iter()
                .any(|key| words.iter().any(|word| word.starts_with(key)))
        })
        .map_or("🔔", |(emoji, _)| emoji)
}

/// Título del aviso de un recordatorio, con su emoji.
pub fn reminder_title(content: &str) -> String {
    format!("{} Recordatorio de Nexium", reminder_emoji(content))
}

/// Muestra la ventana en Recordatorios con el recordatorio resaltado.
pub fn open_reminder<R: Runtime>(app: &AppHandle<R>, reminder_id: i64) {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    let _ = app.emit(OPEN_REMINDER_EVENT, reminder_id);
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
    fn notify_reminder(&self, title: &str, body: &str, reminder_id: i64) -> Result<(), String> {
        windows_reminder::show(&self.app, title, body, reminder_id)
            .or_else(|_| self.notify(title, body))
    }

    #[cfg(not(windows))]
    fn notify_reminder(&self, title: &str, body: &str, _reminder_id: i64) -> Result<(), String> {
        self.notify(title, body)
    }
}

#[cfg(windows)]
mod windows_reminder {
    use super::{open_reminder, AppHandle, Runtime};
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
    pub fn show<R: Runtime>(
        app: &AppHandle<R>,
        title: &str,
        body: &str,
        reminder_id: i64,
    ) -> Result<(), String> {
        let handle = app.clone();
        Toast::new(&app_id(app))
            .title(title)
            .text1(body)
            .scenario(Scenario::Reminder)
            .add_button("Abrir Nexium", OPEN)
            .on_activated(move |action| {
                // El botón o un clic en el aviso: Nexium se abre en ese recordatorio.
                if action.as_deref() == Some(OPEN) || action.is_none() {
                    open_reminder(&handle, reminder_id);
                }
                Ok(())
            })
            .show()
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_emoji_follows_what_the_reminder_is_about() {
        for (content, emoji) in [
            ("Tomar agua", "💧"),
            ("Tomarme la pastilla con un vaso de agua", "💊"),
            ("Medicamento de la tarde", "💊"),
            ("Estudiar para el parcial", "📚"),
            ("Entregar el proyecto final", "📚"),
            ("Reunión con el equipo", "🗓️"),
            ("Cita con el dentista", "🗓️"),
            ("Hacer la compra del mes", "🛒"),
            ("Ir al gimnasio", "🏃"),
            ("Pagar la luz", "🔔"),
        ] {
            assert_eq!(reminder_emoji(content), emoji, "{content}");
        }
        assert_eq!(reminder_title("Tomar agua"), "💧 Recordatorio de Nexium");
    }
}
