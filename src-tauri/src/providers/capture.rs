//! Una sola captura bajo demanda, siempre en memoria: nunca se escribe en disco.

use std::{future::Future, io::Cursor, time::Duration};

use async_trait::async_trait;
use image::{imageops::FilterType, DynamicImage};

const MAX_SIDE: u32 = 1568;
const REDUCED_SIDE: u32 = 1024;
const JPEG_QUALITIES: [u8; 4] = [80, 65, 50, 35];
/// Por debajo del máximo del servidor, y barato de validar con su límite de CPU.
pub const MAX_ENCODED_BYTES: usize = 500_000;
/// Cuánto se espera a que el sistema confirme que la ventana ya no se ve.
const HIDE_TIMEOUT: Duration = Duration::from_millis(1500);
const HIDE_POLL: Duration = Duration::from_millis(25);
/// Margen tras la confirmación: el compositor puede tardar un fotograma más en retirarla de la pantalla.
const HIDE_MARGIN: Duration = Duration::from_millis(250);

#[derive(Debug, PartialEq)]
pub enum CaptureError {
    PermissionDenied(String),
    Failed(String),
}

#[async_trait]
pub trait ScreenCaptureProvider: Send + Sync {
    async fn capture_once(&self) -> Result<Vec<u8>, CaptureError>;
}

pub struct NativeScreenCapture;

/// La ventana de Nexium vista desde la captura. Existe para poder probar el orden sin una ventana real.
pub trait ConcealableWindow {
    fn hide(&self) -> bool;
    fn is_visible(&self) -> Option<bool>;
    fn show(&self);
}

async fn hide_and_confirm(window: &impl ConcealableWindow) -> Result<(), CaptureError> {
    let not_hidden = || {
        CaptureError::Failed(
            "No se pudo ocultar Nexium antes de la captura. No se tomó ninguna imagen.".into(),
        )
    };
    if !window.hide() {
        return Err(not_hidden());
    }
    let started = tokio::time::Instant::now();
    while window.is_visible() != Some(false) {
        if started.elapsed() >= HIDE_TIMEOUT {
            return Err(not_hidden());
        }
        tokio::time::sleep(HIDE_POLL).await;
    }
    tokio::time::sleep(HIDE_MARGIN).await;
    Ok(())
}

/// Captura lo que hay detrás de Nexium: solo después de que el sistema confirme que su ventana está oculta.
/// La ventana vuelve a mostrarse siempre, también si algo falla.
pub async fn capture_behind<W, F>(window: &W, capture: F) -> Result<Vec<u8>, CaptureError>
where
    W: ConcealableWindow,
    F: Future<Output = Result<Vec<u8>, CaptureError>>,
{
    let result = match hide_and_confirm(window).await {
        Ok(()) => capture.await,
        Err(error) => Err(error),
    };
    window.show();
    result
}

fn jpeg(image: &DynamicImage, quality: u8) -> Result<Vec<u8>, CaptureError> {
    let mut buffer = Cursor::new(Vec::new());
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buffer, quality);
    image
        .write_with_encoder(encoder)
        .map_err(|_| CaptureError::Failed("No se pudo preparar la captura.".into()))?;
    Ok(buffer.into_inner())
}

/// Reduce la imagen y la codifica en JPEG: menos tokens y menos detalle innecesario.
/// Baja la calidad solo si hace falta, para conservar legible el texto pequeño.
pub fn encode_for_analysis(image: DynamicImage) -> Result<Vec<u8>, CaptureError> {
    let resized = if image.width().max(image.height()) > MAX_SIDE {
        image.resize(MAX_SIDE, MAX_SIDE, FilterType::Triangle)
    } else {
        image
    };
    let rgb = DynamicImage::ImageRgb8(resized.to_rgb8());
    for quality in JPEG_QUALITIES {
        let bytes = jpeg(&rgb, quality)?;
        if bytes.len() <= MAX_ENCODED_BYTES {
            return Ok(bytes);
        }
    }
    let bytes = jpeg(
        &rgb.resize(REDUCED_SIDE, REDUCED_SIDE, FilterType::Triangle),
        JPEG_QUALITIES[JPEG_QUALITIES.len() - 1],
    )?;
    if bytes.len() > MAX_ENCODED_BYTES {
        return Err(CaptureError::Failed(
            "La pantalla tiene demasiado detalle para enviarla.".into(),
        ));
    }
    Ok(bytes)
}

fn permission_message() -> String {
    if cfg!(target_os = "macos") {
        "macOS no permite capturar la pantalla. Autoriza a Nexium en Ajustes del Sistema, Privacidad y seguridad, Grabación de pantalla.".into()
    } else {
        "El sistema no permitió capturar la pantalla.".into()
    }
}

fn capture_with_xcap() -> Result<DynamicImage, CaptureError> {
    let monitors =
        xcap::Monitor::all().map_err(|_| CaptureError::PermissionDenied(permission_message()))?;
    let monitor = monitors
        .iter()
        .find(|monitor| monitor.is_primary().unwrap_or(false))
        .or_else(|| monitors.first())
        .ok_or_else(|| CaptureError::Failed("No se encontró ninguna pantalla.".into()))?;
    let captured = monitor
        .capture_image()
        .map_err(|_| CaptureError::PermissionDenied(permission_message()))?;
    let (width, height) = captured.dimensions();
    let buffer = image::RgbaImage::from_raw(width, height, captured.into_raw())
        .ok_or_else(|| CaptureError::Failed("La captura está dañada.".into()))?;
    Ok(DynamicImage::ImageRgba8(buffer))
}

#[cfg(target_os = "linux")]
fn is_wayland() -> bool {
    std::env::var("XDG_SESSION_TYPE").is_ok_and(|session| session == "wayland")
        || std::env::var_os("WAYLAND_DISPLAY").is_some()
}

/// En Wayland solo el portal del escritorio puede capturar, y es él quien pide permiso.
#[cfg(target_os = "linux")]
async fn capture_with_portal() -> Result<DynamicImage, CaptureError> {
    let denied =
        || CaptureError::PermissionDenied("El escritorio no permitió capturar la pantalla.".into());
    let response = ashpd::desktop::screenshot::Screenshot::request()
        .interactive(false)
        .send()
        .await
        .map_err(|_| denied())?
        .response()
        .map_err(|_| denied())?;
    let path = url::Url::parse(response.uri().as_str())
        .ok()
        .and_then(|uri| uri.to_file_path().ok())
        .ok_or_else(|| CaptureError::Failed("El portal devolvió una ruta no válida.".into()))?;
    // El portal siempre escribe un archivo temporal: se lee y se borra de inmediato.
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| CaptureError::Failed("No se pudo leer la captura.".into()));
    let _ = tokio::fs::remove_file(&path).await;
    image::load_from_memory(&bytes?)
        .map_err(|_| CaptureError::Failed("La captura está dañada.".into()))
}

#[async_trait]
impl ScreenCaptureProvider for NativeScreenCapture {
    async fn capture_once(&self) -> Result<Vec<u8>, CaptureError> {
        #[cfg(target_os = "linux")]
        if is_wayland() {
            let image = capture_with_portal().await?;
            return tokio::task::spawn_blocking(move || encode_for_analysis(image))
                .await
                .map_err(|_| CaptureError::Failed("La captura se interrumpió.".into()))?;
        }
        tokio::task::spawn_blocking(|| encode_for_analysis(capture_with_xcap()?))
            .await
            .map_err(|_| CaptureError::Failed("La captura se interrumpió.".into()))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_images_are_reduced_and_encoded_as_jpeg() {
        let image = DynamicImage::new_rgba8(3840, 2160);
        let bytes = encode_for_analysis(image).unwrap();
        assert_eq!(&bytes[..2], &[0xFF, 0xD8]);
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!(decoded.width(), MAX_SIDE);
        assert!(decoded.height() < 900);
    }

    #[test]
    fn detailed_images_are_compressed_under_the_limit() {
        let mut seed = 7u32;
        let noise = image::RgbImage::from_fn(1568, 900, |_, _| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            image::Rgb([(seed >> 24) as u8, (seed >> 16) as u8, (seed >> 8) as u8])
        });
        let bytes = encode_for_analysis(DynamicImage::ImageRgb8(noise)).unwrap();
        assert!(bytes.len() <= MAX_ENCODED_BYTES, "{}", bytes.len());
        assert_eq!(&bytes[..2], &[0xFF, 0xD8]);
    }

    use std::sync::Mutex;

    /// Ventana simulada que registra el orden de las llamadas.
    struct FakeWindow {
        log: Mutex<Vec<&'static str>>,
        hides: bool,
        visible_reads_before_hidden: Mutex<u32>,
    }

    impl FakeWindow {
        fn new(hides: bool, visible_reads_before_hidden: u32) -> Self {
            Self {
                log: Mutex::new(Vec::new()),
                hides,
                visible_reads_before_hidden: Mutex::new(visible_reads_before_hidden),
            }
        }

        fn log(&self) -> Vec<&'static str> {
            self.log.lock().unwrap().clone()
        }
    }

    impl ConcealableWindow for FakeWindow {
        fn hide(&self) -> bool {
            self.log.lock().unwrap().push("hide");
            true
        }

        fn is_visible(&self) -> Option<bool> {
            let mut pending = self.visible_reads_before_hidden.lock().unwrap();
            if !self.hides || *pending > 0 {
                *pending = pending.saturating_sub(1);
                return Some(true);
            }
            self.log.lock().unwrap().push("hidden");
            Some(false)
        }

        fn show(&self) {
            self.log.lock().unwrap().push("show");
        }
    }

    async fn capture(
        window: &FakeWindow,
        result: Result<Vec<u8>, CaptureError>,
    ) -> Result<Vec<u8>, CaptureError> {
        capture_behind(window, async {
            window.log.lock().unwrap().push("capture");
            result
        })
        .await
    }

    #[tokio::test(start_paused = true)]
    async fn captures_only_after_the_window_is_confirmed_hidden_and_shows_it_again() {
        let window = FakeWindow::new(true, 3);
        let started = tokio::time::Instant::now();
        assert_eq!(capture(&window, Ok(vec![1])).await.unwrap(), vec![1]);
        assert_eq!(window.log(), ["hide", "hidden", "capture", "show"]);
        assert!(started.elapsed() >= HIDE_MARGIN + HIDE_POLL * 3);
    }

    #[tokio::test(start_paused = true)]
    async fn a_window_that_does_not_hide_is_never_captured_and_comes_back() {
        let window = FakeWindow::new(false, 0);
        let error = capture(&window, Ok(vec![1])).await.unwrap_err();
        assert!(
            matches!(error, CaptureError::Failed(message) if message.contains("No se tomó ninguna imagen"))
        );
        assert_eq!(window.log(), ["hide", "show"]);
    }

    #[tokio::test(start_paused = true)]
    async fn the_window_comes_back_when_the_capture_fails() {
        let window = FakeWindow::new(true, 0);
        let error = capture(
            &window,
            Err(CaptureError::PermissionDenied("sin permiso".into())),
        )
        .await
        .unwrap_err();
        assert_eq!(error, CaptureError::PermissionDenied("sin permiso".into()));
        assert_eq!(window.log(), ["hide", "hidden", "capture", "show"]);
    }

    #[test]
    fn small_images_keep_their_size() {
        let bytes = encode_for_analysis(DynamicImage::new_rgba8(800, 600)).unwrap();
        assert_eq!(image::load_from_memory(&bytes).unwrap().width(), 800);
    }
}
