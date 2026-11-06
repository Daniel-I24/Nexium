//! Una sola captura bajo demanda, siempre en memoria: nunca se escribe en disco.

use std::io::Cursor;

use async_trait::async_trait;
use image::{imageops::FilterType, DynamicImage};

const MAX_SIDE: u32 = 1568;
const JPEG_QUALITY: u8 = 80;

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

/// Reduce la imagen y la codifica en JPEG: menos tokens y menos detalle innecesario.
pub fn encode_for_analysis(image: DynamicImage) -> Result<Vec<u8>, CaptureError> {
    let resized = if image.width().max(image.height()) > MAX_SIDE { image.resize(MAX_SIDE, MAX_SIDE, FilterType::Triangle) } else { image };
    let mut buffer = Cursor::new(Vec::new());
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buffer, JPEG_QUALITY);
    resized.to_rgb8().write_with_encoder(encoder).map_err(|_| CaptureError::Failed("No se pudo preparar la captura.".into()))?;
    Ok(buffer.into_inner())
}

fn permission_message() -> String {
    if cfg!(target_os = "macos") {
        "macOS no permite capturar la pantalla. Autoriza a Nexium en Ajustes del Sistema, Privacidad y seguridad, Grabación de pantalla.".into()
    } else {
        "El sistema no permitió capturar la pantalla.".into()
    }
}

fn capture_with_xcap() -> Result<DynamicImage, CaptureError> {
    let monitors = xcap::Monitor::all().map_err(|_| CaptureError::PermissionDenied(permission_message()))?;
    let monitor = monitors
        .iter()
        .find(|monitor| monitor.is_primary().unwrap_or(false))
        .or_else(|| monitors.first())
        .ok_or_else(|| CaptureError::Failed("No se encontró ninguna pantalla.".into()))?;
    let captured = monitor.capture_image().map_err(|_| CaptureError::PermissionDenied(permission_message()))?;
    let (width, height) = captured.dimensions();
    let buffer = image::RgbaImage::from_raw(width, height, captured.into_raw()).ok_or_else(|| CaptureError::Failed("La captura está dañada.".into()))?;
    Ok(DynamicImage::ImageRgba8(buffer))
}

#[cfg(target_os = "linux")]
fn is_wayland() -> bool {
    std::env::var("XDG_SESSION_TYPE").is_ok_and(|session| session == "wayland") || std::env::var_os("WAYLAND_DISPLAY").is_some()
}

/// En Wayland solo el portal del escritorio puede capturar, y es él quien pide permiso.
#[cfg(target_os = "linux")]
async fn capture_with_portal() -> Result<DynamicImage, CaptureError> {
    let denied = || CaptureError::PermissionDenied("El escritorio no permitió capturar la pantalla.".into());
    let response = ashpd::desktop::screenshot::Screenshot::request().interactive(false).send().await.map_err(|_| denied())?.response().map_err(|_| denied())?;
    let path = url::Url::parse(response.uri().as_str()).ok().and_then(|uri| uri.to_file_path().ok()).ok_or_else(|| CaptureError::Failed("El portal devolvió una ruta no válida.".into()))?;
    // El portal siempre escribe un archivo temporal: se lee y se borra de inmediato.
    let bytes = tokio::fs::read(&path).await.map_err(|_| CaptureError::Failed("No se pudo leer la captura.".into()));
    let _ = tokio::fs::remove_file(&path).await;
    image::load_from_memory(&bytes?).map_err(|_| CaptureError::Failed("La captura está dañada.".into()))
}

#[async_trait]
impl ScreenCaptureProvider for NativeScreenCapture {
    async fn capture_once(&self) -> Result<Vec<u8>, CaptureError> {
        #[cfg(target_os = "linux")]
        if is_wayland() {
            let image = capture_with_portal().await?;
            return tokio::task::spawn_blocking(move || encode_for_analysis(image)).await.map_err(|_| CaptureError::Failed("La captura se interrumpió.".into()))?;
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
    fn small_images_keep_their_size() {
        let bytes = encode_for_analysis(DynamicImage::new_rgba8(800, 600)).unwrap();
        assert_eq!(image::load_from_memory(&bytes).unwrap().width(), 800);
    }
}
