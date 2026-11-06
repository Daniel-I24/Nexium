use tauri::{AppHandle, Runtime};
use tauri_plugin_notification::NotificationExt;

pub trait NotificationProvider: Send + Sync {
    fn notify(&self, title: &str, body: &str) -> Result<(), String>;
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
        self.app.notification().builder().title(title).body(body).show().map_err(|error| error.to_string())
    }
}
