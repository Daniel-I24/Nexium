# Nexium: prueba de Whisper con audio real

**Estado:** medición parcial. Falta la parte más importante para el uso real: las frases cortas.

## Decisión provisional

| Aspecto | Valor |
|---|---|
| Modelo por defecto | `ggml-small-q5_1.bin` |
| Alternativa en Ajustes | `ggml-base-q5_1.bin`, para equipos modestos |
| Peso | 190 MB (`small`) y 60 MB (`base`). No van en el instalador |
| Cuándo se descarga | Una sola vez, al pulsar Descargar en Ajustes, sección Voz. Si no está, Hablar lo indica y se puede seguir escribiendo |
| Verificación | SHA-256 antes de aceptarlo. Un archivo incompleto o alterado se descarta |
| Origen | `huggingface.co/ggerganov/whisper.cpp` (licencia MIT) |
| Dónde se guarda | Carpeta de datos de la app, en `models/`. En Windows: `%APPDATA%\io.github.daniel-i24.nexium\models\` |
| Audio | Se transcribe en el equipo, no sale de él ni se guarda |

Es provisional porque se basa en clips largos. Se confirmará o cambiará con la medición pendiente.

## Resultado medido: frases largas

- **Audio:** 30 grabaciones humanas de 10 a 20 s, 10 voces, 381 s en total, de [Multilingual LibriSpeech](https://huggingface.co/datasets/facebook/multilingual_librispeech) (español). Es lectura literaria, con su transcripción de referencia.
- **Equipo:** AMD Ryzen 5 7535HS, 6 núcleos, 16 GB, solo CPU, 8 hilos.
- **Herramienta:** `src-tauri/examples/transcribe_bench.rs`.

| Modelo | WER (palabras erróneas) | Tiempo / duración del audio | Carga del modelo | Memoria pico |
|---|---|---|---|---|
| `small-q5_1` | **9,32 %** | 0,68 | 630 ms | 500 MB |
| `base-q5_1` | 16,05 % | 0,43 | 474 ms | 263 MB |

**Cómo leer estas cifras con cautela:**

- Se tomaron con Whisper optimizado, pero en una compilación de desarrollo, y con otras tareas en marcha en el equipo. Los tiempos pueden mejorar en la compilación final.
- La referencia del corpus no siempre coincide con la grafía ("á" antigua, números), lo que infla algo el WER.
- Es lectura en voz alta, no órdenes a un asistente.

## Problema detectado: latencia en frases cortas

Whisper procesa siempre una ventana de 30 s. Un clip de 10,5 s tardó 7,08 s con `small`, casi lo mismo que uno de 15 s (7,63 s). Por eso una orden de 3 s podría tardar varios segundos, demasiado para un asistente.

**Corrección candidata:** ajustar `audio_ctx` a la duración real. `transcribe_bench` ya admite la opción `auto`, pero el efecto en velocidad y precisión aún no está medido, y por eso la app todavía no lo usa.

## Pendiente

1. **Medir frases cortas:** los 40 clips reales de 2 a 6 s de [CIEMPIESS Test](https://huggingface.co/datasets/ciempiess/ciempiess_test) (español de México) ya están descargados, con ambos modelos y con y sin `auto`. La medición se detuvo porque el equipo se quedó sin memoria durante la compilación optimizada, sin llegar a producir resultados.
2. **Aplicar `audio_ctx` a la app** si la medición lo justifica.
3. **Probar con tu voz y tu micrófono:** ningún corpus sustituye las órdenes reales. Está en `docs/PLAN_PRUEBAS_MANUALES.md`, pruebas 9 a 12.

El audio de prueba no se incluye en el repositorio.
