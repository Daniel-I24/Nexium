//! Tamaño de la ventana principal según la pantalla en la que se abre.
//!
//! `tauri.conf.json` fija el tamaño preferido (1100 × 720) y el mínimo (640 × 400), ambos en píxeles lógicos.
//! En una pantalla pequeña o con mucha escala, el preferido puede no caber en el área de trabajo: entonces se
//! reduce para dejar un margen, sin bajar nunca del mínimo.

use tauri::{LogicalSize, Runtime, WebviewWindow};

pub const PREFERRED: (f64, f64) = (1100.0, 720.0);
/// Mínimo justificado en docs/DISENO_VISUAL.md, sección 6.
pub const MINIMUM: (f64, f64) = (640.0, 400.0);
/// Parte del área de trabajo que puede ocupar la ventana al abrirse: deja ver que hay escritorio detrás.
const SHARE: f64 = 0.9;

/// Tamaño lógico con el que abrir la ventana en un área de trabajo de `work` píxeles lógicos.
pub fn fit(work: (f64, f64)) -> (f64, f64) {
    let side = |preferred: f64, available: f64, minimum: f64| {
        (available * SHARE).min(preferred).max(minimum).round()
    };
    (
        side(PREFERRED.0, work.0, MINIMUM.0),
        side(PREFERRED.1, work.1, MINIMUM.1),
    )
}

/// Ajusta y centra la ventana si el tamaño preferido no cabe. Si no se conoce la pantalla, no cambia nada.
pub fn fit_to_screen<R: Runtime>(window: &WebviewWindow<R>) {
    let Ok(Some(monitor)) = window.current_monitor() else {
        return;
    };
    let scale = monitor.scale_factor();
    let area = monitor.work_area().size.to_logical::<f64>(scale);
    let (width, height) = fit((area.width, area.height));
    if (width, height) != PREFERRED {
        let _ = window.set_size(LogicalSize::new(width, height));
        let _ = window.center();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_large_screen_keeps_the_preferred_size() {
        assert_eq!(fit((1920.0, 1032.0)), PREFERRED);
    }

    #[test]
    fn a_small_or_scaled_screen_shrinks_the_window_with_a_margin() {
        // 1280 × 720 al 150 %: 853 × 432 lógicos, ya sin la barra de tareas.
        assert_eq!(fit((853.0, 432.0)), (768.0, 400.0));
        // 1366 × 768 al 125 %: 1093 × 576.
        assert_eq!(fit((1093.0, 576.0)), (984.0, 518.0));
    }

    #[test]
    fn never_goes_below_the_minimum() {
        assert_eq!(fit((500.0, 300.0)), MINIMUM);
    }
}
