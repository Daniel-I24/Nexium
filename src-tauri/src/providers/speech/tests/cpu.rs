use super::*;
use crate::providers::speech::SpeechError;

/// La compilación y la comprobación en ejecución deben pedir lo mismo: si una exige algo que la otra no
/// comprueba, un procesador sin esa instrucción volvería a cerrar Nexium.
#[test]
fn the_build_never_targets_the_build_machine_and_matches_the_runtime_check() {
    let config =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/.cargo/config.toml"))
            .unwrap();
    let value = |name: &str| {
        config
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{name} = ")))
            .map(|value| value.trim_matches('"').to_owned())
    };
    assert_eq!(value("GGML_NATIVE").as_deref(), Some("OFF"));
    assert_eq!(value("GGML_AVX512").as_deref(), Some("OFF"));
    for (flag, feature) in [
        ("GGML_SSE42", "sse4.2"),
        ("GGML_AVX", "avx"),
        ("GGML_AVX2", "avx2"),
        ("GGML_FMA", "fma"),
        ("GGML_F16C", "f16c"),
        ("GGML_BMI2", "bmi2"),
    ] {
        assert_eq!(value(flag).as_deref(), Some("ON"), "{flag}");
        assert!(REQUIRED_X86_FEATURES.contains(&feature), "{feature}");
    }
}

#[test]
fn this_machine_can_run_the_voice_build() {
    // La CI y el equipo de desarrollo tienen AVX2: si fallara aquí, la compilación no serviría ni en ellos.
    assert!(
        missing_cpu_features().is_empty(),
        "{:?}",
        missing_cpu_features()
    );
}

#[test]
fn a_panic_in_the_voice_code_becomes_a_clear_error() {
    let result: Result<(), SpeechError> = contained(|| panic!("fallo simulado"));
    let message = result.unwrap_err().to_string();
    assert!(message.contains("El reconocimiento de voz falló de forma inesperada"));
}

#[test]
fn an_unsupported_processor_is_explained() {
    let message = SpeechError::UnsupportedCpu(vec!["avx2", "fma"]).to_string();
    assert!(message.contains("AVX2"));
    assert!(message.contains("puedes escribir"));
}
