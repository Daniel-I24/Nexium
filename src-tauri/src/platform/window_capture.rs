//! Captura con la ventana de Nexium oculta: el análisis debe tratar de lo que hay detrás.
//! La lógica de ocultar, confirmar y volver a mostrar está en `providers::capture`, con sus pruebas.

use async_trait::async_trait;
use tauri::{AppHandle, Manager, Runtime};

use crate::providers::capture::{capture_behind, CaptureError, ConcealableWindow, NativeScreenCapture, ScreenCaptureProvider};

/// Oculta Nexium durante la captura para que el análisis trate de lo que hay detrás.
pub struct WindowAwareCapture<R: Runtime> {
    app: AppHandle<R>,
    inner: NativeScreenCapture,
}

impl<R: Runtime> WindowAwareCapture<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self { app, inner: NativeScreenCapture }
    }
}

struct MainWindow<R: Runtime>(tauri::WebviewWindow<R>);

impl<R: Runtime> ConcealableWindow for MainWindow<R> {
    fn hide(&self) -> bool {
        self.0.hide().is_ok()
    }

    fn is_visible(&self) -> Option<bool> {
        self.0.is_visible().ok()
    }

    fn show(&self) {
        let _ = self.0.show();
        let _ = self.0.set_focus();
    }
}

#[async_trait]
impl<R: Runtime> ScreenCaptureProvider for WindowAwareCapture<R> {
    async fn capture_once(&self) -> Result<Vec<u8>, CaptureError> {
        match self.app.get_webview_window("main") {
            Some(window) => capture_behind(&MainWindow(window), self.inner.capture_once()).await,
            None => self.inner.capture_once().await,
        }
    }
}
