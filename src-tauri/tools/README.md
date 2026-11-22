# Herramientas de medición

Programas de apoyo al desarrollo. No forman parte de la app ni de los instaladores, y ninguno se ejecuta en la CI: los que usan servicios reales gastan cuota. Se ejecutan como ejemplos de Cargo, desde `src-tauri`.

| Herramienta | Para qué | Uso |
|---|---|---|
| `ai_live.rs` | Prueba el servicio de IA real con frases, capturas y documentos. Gasta cuota | `cargo run --example ai_live -- frases` |
| `weather_live.rs` | Consulta Open-Meteo con ciudades reales | `cargo run --example weather_live -- "Bogotá" "Medellín"` |
| `transcribe_bench.rs` | Precisión (WER), tiempo y memoria de Whisper con un corpus de audio | `cargo run --release --example transcribe_bench -- <modelo.bin> <carpeta> <hilos> <full\|auto> <resultados.jsonl>` |
| `voice_params_bench.rs` | Tiempo de transcripción de unas grabaciones variando la configuración de Whisper, para diagnosticar la voz | `cargo run --release --example voice_params_bench -- <modelo.bin> <carpeta> [opciones]` |
| `voice_record.rs` | Graba frases con el mismo flujo de la app (micrófono, mezcla a un canal y 16 kHz) para medirlas después | `cargo run --release --example voice_record -- <carpeta>` |

`wav.rs` es la lectura de WAV que comparten las dos mediciones de voz.

Las carpetas de audio llevan un `manifest.json` con `id`, `file` y `reference` (el texto esperado) de cada clip. Las grabaciones de voz se guardan fuera del repositorio y se borran al terminar de medir. Los resultados y el método están en `docs/PRUEBA_VOZ.md` y `docs/RENDIMIENTO.md`.
