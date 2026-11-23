//! Instrucciones del procesador que necesita whisper.cpp tal como se compila (ver `.cargo/config.toml`).
//! Ejecutarlo sin ellas cierra Nexium con una instrucción ilegal, así que se comprueba antes de cargar el modelo.

/// Las que exige la compilación de ggml en x86-64. Deben coincidir con `[env]` de `.cargo/config.toml`.
pub const REQUIRED_X86_FEATURES: &[&str] = &["sse4.2", "avx", "avx2", "fma", "f16c", "bmi2"];

/// Las instrucciones que le faltan a este procesador. Vacío si puede usar la voz.
pub fn missing_cpu_features() -> Vec<&'static str> {
    #[cfg(target_arch = "x86_64")]
    {
        let present = |feature: &str| match feature {
            "sse4.2" => std::arch::is_x86_feature_detected!("sse4.2"),
            "avx" => std::arch::is_x86_feature_detected!("avx"),
            "avx2" => std::arch::is_x86_feature_detected!("avx2"),
            "fma" => std::arch::is_x86_feature_detected!("fma"),
            "f16c" => std::arch::is_x86_feature_detected!("f16c"),
            "bmi2" => std::arch::is_x86_feature_detected!("bmi2"),
            _ => false,
        };
        REQUIRED_X86_FEATURES
            .iter()
            .copied()
            .filter(|feature| !present(feature))
            .collect()
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        Vec::new()
    }
}

/// Ejecuta una parte del reconocimiento de voz conteniendo un pánico: un fallo de la voz nunca cierra Nexium.
pub fn contained<T>(
    work: impl FnOnce() -> Result<T, super::SpeechError>,
) -> Result<T, super::SpeechError> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)).unwrap_or_else(|_| {
        Err(super::SpeechError::Failed(
            "El reconocimiento de voz falló de forma inesperada. Vuelve a intentarlo; si se repite, escribe tu solicitud.".into(),
        ))
    })
}

#[cfg(test)]
#[path = "tests/cpu.rs"]
mod tests;
