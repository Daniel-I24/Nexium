# Nexium: prueba de Whisper con audio real

**Estado:** medición con corpus terminada. Falta probar con tu voz y tu micrófono.

## Decisión

| Aspecto | Valor |
|---|---|
| Modelo por defecto | `ggml-small-q5_1.bin`, con `audio_ctx` ajustado a la duración del audio |
| Alternativa en Ajustes | `ggml-base-q5_1.bin`, para equipos modestos: tres veces más rápido, pero con más errores |
| Peso | 190 MB (`small`) y 60 MB (`base`). No van en el instalador |
| Cuándo se descarga | Una sola vez, al pulsar Descargar en Ajustes, sección Voz. Si no está, Hablar lo indica y se puede seguir escribiendo |
| Cuándo se carga | En la primera transcripción. Se libera tras un periodo sin uso, configurable en Ajustes (10 minutos por defecto, de 1 a 240) |
| Verificación | SHA-256 antes de aceptarlo. Un archivo incompleto o alterado se descarta |
| Origen | `huggingface.co/ggerganov/whisper.cpp` (licencia MIT) |
| Dónde se guarda | Carpeta de datos de la app, en `models/`. En Windows: `%APPDATA%\io.github.daniel-i24.nexium\models\` |
| Audio | Se transcribe en el equipo, no sale de él ni se guarda |

**Por qué `small`:** comete menos errores que `base` (15,70 % frente a 20,87 %), y 1,5 s de espera es aceptable para una orden. Sin `audio_ctx`, `small` tardaba 6,45 s en frases de 3 a 6 s, más que el propio audio.

## Resultado: frases cortas

- **Audio:** 40 grabaciones humanas de 3,1 a 6,0 s, 15 voces, 209 s en total, de [CIEMPIESS Test](https://huggingface.co/datasets/ciempiess/ciempiess_test) (español de México). Es habla espontánea, con su transcripción de referencia.
- **Equipo:** AMD Ryzen 5 7535HS, 16 GB, solo CPU, 6 hilos (los que usa la app en este equipo), 7 de noviembre de 2026.
- **Herramienta:** `src-tauri/examples/transcribe_bench.rs`.

| Configuración | WER (palabras erróneas) | Transcripción: media | Mediana | P95 | Máximo | Carga del modelo | Memoria pico |
|---|---|---|---|---|---|---|---|
| `small`, ventana completa | 15,70 % | 6,45 s | 6,33 s | 7,02 s | 8,06 s | 479 ms | 500 MB |
| **`small` con `audio_ctx`** | **15,70 %** | **1,53 s** | **1,52 s** | **1,73 s** | **1,99 s** | **342 ms** | **332 MB** |
| `base`, ventana completa | 20,39 % | 1,89 s | 1,87 s | 1,99 s | 2,09 s | 220 ms | 261 MB |
| `base` con `audio_ctx` | 20,87 % | 0,47 s | 0,47 s | 0,53 s | 0,60 s | 147 ms | 148 MB |

La carga va aparte porque solo ocurre en el primer uso, o tras liberar el modelo. La transcripción incluye crear el estado de Whisper, que la app hace en cada solicitud.

**En `small`, el mismo WER no significa los mismos textos:** con `audio_ctx` cambia el texto de 22 de 40 clips: 5 tienen más errores, 5 tienen menos y 12 tienen los mismos. En `base` cambian 29: 8 empeoran y 6 mejoran.

## Resultado: frases largas

Comprobación de que `audio_ctx` no daña las grabaciones largas, porque la app admite hasta 30 s. Son 30 grabaciones de 10 a 20 s, 10 voces, 381 s en total, de [Multilingual LibriSpeech](https://huggingface.co/datasets/facebook/multilingual_librispeech) (español). Es lectura en voz alta. Mismo equipo, `small` y 6 hilos.

| Configuración | WER | Transcripción: media | P95 | Memoria pico |
|---|---|---|---|---|
| Ventana completa | 9,32 % | 7,03 s | 7,51 s | 500 MB |
| Con `audio_ctx` | 9,88 % | 3,73 s | 6,68 s | 386 MB |

Pierde algo de precisión, y a cambio la espera baja casi a la mitad.

## Cómo leer estas cifras con cautela

- **No son órdenes a un asistente.** El WER de frases cortas está medido con conversación espontánea, y puede ser distinto con tu voz, tu micrófono y frases como "¿Qué hora es?". No hay ninguna prueba todavía con grabaciones de ese tipo.
- **La referencia infla el WER.** CIEMPIESS anota la pronunciación en algunas palabras (`etsistencia`, `sep`), que nunca coincidirán con el español escrito.
- **Compilación de desarrollo.** Whisper sí va optimizado (`/O2`, comprobado), por lo que la compilación final no debería cambiar mucho el tiempo, pero no se ha medido.
- **La carga es con el archivo en la caché del sistema.** La primera lectura tras encender el equipo puede tardar más.
- **Un solo equipo.** Los tiempos cambiarán en otras CPU. La memoria es del proceso de medición, no de la app completa.
- **Interrupción:** los 8 primeros clips de `small` con ventana completa se midieron antes de que el equipo se apagara, sin relación con la prueba, y los otros 32 tras reiniciar. La memoria libre nunca bajó de 5,4 GB.

## Pendiente

**Probar con tu voz y tu micrófono:** está en `docs/PLAN_PRUEBAS_MANUALES.md`, pruebas 9 a 12.

El audio de prueba no se incluye en el repositorio.
