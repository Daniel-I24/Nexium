# Rendimiento de Nexium

Mediciones reales, sin estimaciones. Cada cifra indica cómo se obtuvo.

## Equipo y método

- Windows 11 Home, 12 núcleos lógicos, 15,3 GB de RAM, conectado a la corriente.
- Ejecutable de distribución (`npm run tauri build`, perfil `release`), lanzado directamente desde `src-tauri/target/release/nexium.exe`, sin instalar.
- Script de medición con PowerShell. Suma `nexium.exe` y sus procesos hijos de WebView2; los procesos de WebView2 de otras aplicaciones no se cuentan.
- **Arranque:** desde que se lanza el proceso hasta que la ventana existe y WebView2 tiene al menos tres procesos. No mide el primer pintado de la interfaz.
- **Memoria:** tras 20 s en reposo. La memoria privada es la que el sistema reserva solo para Nexium. El conjunto de trabajo cuenta varias veces las bibliotecas compartidas.
- **CPU en reposo:** tiempo de CPU consumido durante 60 s sin interacción, con la ventana abierta en la pantalla del asistente.

## Línea base: 11 de noviembre de 2026, antes de la Fase B

Commit `6748435`.

| Medida | Resultado |
|---|---|
| Instalador NSIS (`.exe`) | 3,48 MiB |
| Instalador MSI | 4,67 MiB |
| Ejecutable `nexium.exe` | 11,1 MiB |
| Arranque en frío (primera ejecución) | 3,98 s |
| Arranque con el sistema ya en caché (4 ejecuciones) | 527 a 573 ms |
| Memoria privada en reposo, total | 178 MB, de los que `nexium.exe` usa 6 MB y WebView2 el resto (6 procesos) |
| Conjunto de trabajo en reposo, total | 396 MB |
| CPU en reposo | 0,44 % de un núcleo (0,037 % del total) |
| Frontend (`dist/`) | 537 KB: JS 255 KB (79 KB comprimido), CSS 19 KB, fuentes 262 KB |
| Fuentes en formato `.woff` antiguo, duplicadas en `.woff2` | 87 KB del total anterior |

**Memoria con Whisper cargado: no medida en la app.** El modelo no está descargado en este equipo y cargarlo requiere hablar al micrófono, que forma parte de las pruebas manuales. La referencia disponible es la del banco de pruebas de voz (`docs/PRUEBA_VOZ.md`): 332 MB de pico con `small`, fuera de la app.

## Después de la Fase B: 13 de noviembre de 2026

Commit `b320f79`. Misma máquina y mismo método, conectada a la corriente. Cuatro series de 5 arranques cada una.

| Medida | Línea base (una serie) | Después (cuatro series) | Conclusión |
|---|---|---|---|
| Instalador NSIS | 3,48 MiB | 3,40 MiB | Mejora: sin las fuentes `.woff` |
| Instalador MSI | 4,67 MiB | 4,60 MiB | Mejora: sin las fuentes `.woff` |
| Frontend (`dist/`) | 537 KB | 450 KB | Mejora: sin las fuentes `.woff` |
| Arranque en frío (primera ejecución de cada serie) | 3,98 s | 1,01 a 3,17 s | No concluyente: depende de la caché de Windows |
| Arranque en caliente | 527 a 573 ms | 585 a 768 ms | No concluyente: la línea base es una sola serie |
| Memoria privada en reposo | 178 MB | 179 a 182 MB | Sin cambio apreciable |
| CPU en reposo | 0,44 % de un núcleo | 0,44 a 0,68 % de un núcleo | Sin cambio apreciable: misma variación que entre series |

- La Fase B no buscaba cambiar el arranque, la memoria ni la CPU: sus cambios fueron de estructura, de validación y de tamaño. Solo el tamaño mejoró de forma medible.
- El arranque en caliente parece unos 60 ms más lento, pero la línea base se tomó con una sola serie, otro día y sin repetir. Para afirmarlo habría que volver a compilar `6748435` y medir las dos versiones seguidas.
- En una serie, el conjunto de trabajo bajó a 93 MB: Windows recortó la memoria de la ventana en reposo. La memoria privada no cambió.

## Velocidad de las solicitudes: 14 de noviembre de 2026

Objetivos: una orden local por voz en menos de 3 s en total, una orden escrita local en menos de 300 ms y la interpretación con IA con una mediana inferior a 3 s.

### Cómo se mide

- **En la app:** cada solicitud deja una línea en `rendimiento.log`, en la carpeta de datos de Nexium (en Windows, `%APPDATA%\io.github.daniel-i24.nexium\rendimiento.log`), con el tiempo de cada etapa: fin de grabación, carga del modelo, transcripción, interpretación y respuesta. Solo guarda duraciones, si se resolvió en local o con IA y la categoría; nunca el texto pedido ni la respuesta. Se recorta solo al pasar de 256 KB. Ejemplo:

  ```
  2026-11-14 16:44:02 voz · local · system | fin de grabación 9 ms · carga del modelo 0 ms · transcripción 1068 ms · interpretación 0 ms · respuesta 251 ms · total 1331 ms
  ```

- **Voz, en el banco de pruebas:** `src-tauri/examples/voice_pipeline_bench.rs` recorre la misma cadena que la app (modelo precargado, transcripción con Whisper `small`, intérprete local y respuesta) con 30 clips de órdenes: 15 frases dichas a dos velocidades por la voz española de Windows (Microsoft Helena), a 16 kHz. Es habla sintética y limpia: con una voz y un micrófono reales los tiempos y los aciertos pueden cambiar, y eso queda para la prueba manual.
- **IA:** `src-tauri/examples/ai_live.rs frases` contra el servidor desplegado, con 25 frases que necesitan IA y 7 s de pausa entre ellas.
- **Equipo:** el de la línea base (AMD Ryzen 5 7535HS, 16 GB), con batería y sin conectar a la corriente; Whisper con 6 hilos. Los tiempos cambian en otras CPU.

### Antes y después

| Medida | Antes | Después |
|---|---|---|
| Orden local por voz: transcripción y respuesta, mediana | 910 ms | 1141 ms |
| Orden local por voz: máximo | 3293 ms (la primera pregunta de CPU) | 1433 ms |
| Carga del modelo al terminar de hablar | 572 ms, en la primera orden tras abrir o liberar el modelo | 0 ms: se carga mientras hablas (455 a 527 ms) |
| Espera fija al terminar cada solicitud | 150 ms | 0 ms |
| Órdenes de los 30 clips resueltas sin IA | 21 | 24 |
| Orden escrita local: mediana | 0,3 ms | 0,7 ms |
| Orden escrita local: máximo (uso de CPU) | 2452 ms la primera vez, 266 ms después | 253 ms |
| Interpretación con IA: mediana (25 frases) | 1218 ms | Sin cambios en el servidor |

**Total de una orden local por voz.** Al soltar Terminar quedan el fin de grabación (unos 10 ms), la transcripción y la respuesta: unos 1,1 a 1,4 s en este equipo, por debajo de los 3 s. Los «unos 12 s» de la prueba manual no se reprodujeron con el modelo cargado; las causas que sumaban tiempo después de hablar eran la carga del modelo (ahora en paralelo a la grabación), la primera lectura de CPU (ahora preparada al abrir Nexium) y la espera fija de 150 ms. `rendimiento.log` dirá en la prueba manual qué etapa tarda en cada equipo.

**Por qué la mediana de la voz sube algo.** El contexto previo de Whisper (`VOCABULARY_PROMPT`: «RAM, CPU, batería, Wi-Fi, recordatorio, clima, pantalla, documento.») cuesta unos 0,1 s por transcripción y hace que «RAM» y «Wi-Fi» se escriban bien: con él se resuelven en local 3 órdenes más de 30. Una frase completa como contexto costaba medio segundo, así que se dejó la lista corta. Las dos mediciones se hicieron con batería, y la segunda con la carga más baja (del 98 % al 19 %): parte de la diferencia es del equipo.

**Cambios que explican el resultado.**

- **Precarga del modelo** al pulsar Hablar, en segundo plano. Si la grabación termina antes de que acabe de cargarse, la transcripción espera solo lo que falte.
- **Whisper sin reintentos de temperatura** (`temperature_inc = 0`), sin contexto entre solicitudes y en un solo segmento: con frases cortas, repetir la decodificación puede multiplicar la espera y no mejora la orden.
- **Lector de CPU persistente**, en su propio hilo y preparado al abrir Nexium: crearlo cuesta de 1,2 a 2,4 s en Windows.
- **Intérprete local más tolerante:** normaliza tildes, signos y mayúsculas, unifica las formas de escribir Wi-Fi, corrige errores leves en palabras largas de las órdenes («vateria», «procesadr»), reconoce variantes frecuentes («cuánta batería tengo», «estoy conectado», «qué horas son», «Recuerda mi…») y abre secciones sin IA («Abre los recordatorios»).

**IA.** La mediana ya estaba en 1,2 s (de 1,0 a 2,7 s, las 25 frases correctas), por debajo de los 3 s. Mientras la IA trabaja, la barra dice qué hace: «Interpretando tu solicitud…» o «Analizando la imagen…».

## La voz en la app instalada: 21 de noviembre de 2026

La prueba manual dio unos 13 s de transcripción por orden en la app instalada (`rendimiento.log`: «transcripción 13186 ms»), frente a 1 s en las mediciones del 14 de noviembre.

**Causa.** Las mediciones anteriores usaban la compilación de desarrollo; la app instalada es la de distribución. `whisper-rs-sys` compila whisper.cpp con CMake y fija la configuración Release. En el perfil `release`, el crate `cmake` sustituye con MSVC las banderas de esa configuración por las de `cc` y descarta a propósito las de optimización: whisper.cpp quedaba en `/Od`, sin optimizar (en el proyecto de Visual Studio, `Optimization` vacío en Release). En desarrollo la configuración elegida es otra y conservaba `/O2`. El micrófono, la conversión a 16 kHz y la mezcla a un canal eran correctos.

**Corrección.** La característica `force-debug` de `whisper-rs-sys` hace que la distribución se compile como el desarrollo: `/O2` en Windows y `-O2` en Linux y macOS. La CI comprueba en Windows, tras compilar los instaladores, que whisper.cpp lleva `/O2`. Además:

- El paso a 16 kHz promedia las muestras que cubre (filtro paso bajo), en lugar de interpolar entre dos.
- Antes de transcribir se recorta el silencio del principio y del final, con 0,3 s de margen, y se sube una grabación baja hasta un pico útil (ganancia máxima 8, sin saturar).
- Whisper usa un hilo por núcleo físico, hasta 8 (en este equipo, 6, igual que antes).
- Se mantienen el español forzado, la decodificación voraz (sin búsqueda en haz), sin reintentos de temperatura, `audio_ctx` proporcional a la duración y el contexto de vocabulario.
- Interpretar con IA nunca espera más de 10 s: el servidor corta a los 9 s con reintento incluido (antes, hasta 22 s; en la prueba manual, 17 s) y la app a los 10 s, con un error claro.

**Mediciones con el ejecutable de distribución**, mismos 30 clips de órdenes, este equipo con batería:

| Medida | Antes (whisper.cpp en `/Od`) | Después |
|---|---|---|
| Orden local por voz: mediana | 12 920 ms | 1 164 ms |
| Orden local por voz: máximo | 16 177 ms | 1 747 ms |
| Órdenes resueltas sin IA (de 30) | 26 | 26 |

`tests/voice_real_audio.rs` transcribe una grabación real a 48 kHz y en estéreo por la misma cadena y exige, en release, menos de 2 s; `cargo test --release --test voice_real_audio -- --ignored` con `NEXIUM_WHISPER_MODEL`.

**Modo de energía.** Todas las mediciones de este equipo se hicieron con batería. La diferencia de 13 s a 1 s es la optimización del compilador, no el modo de energía: los mismos clips, con la misma batería, pasan de un tiempo a otro solo al cambiar de compilación.

## La voz con grabaciones reales: 21 de noviembre de 2026

Cuatro órdenes dichas por el propietario con su micrófono (Microphone Array, Realtek; 48 kHz, 2 canales, F32), grabadas con el mismo flujo de la app (`examples/voice_record.rs`) y borradas después de medir. Llegaban limpias: de 52 a 55 dB entre voz y ruido, picos de 0,23 a 0,66 y la voz entre 0,8 y 2,3 s dentro de los 5 s grabados. Equipo conectado a la corriente (plan «Turbo»).

**`force-debug`, comprobado.** En `whisper-rs-sys` 0.15.0 (la que usa `whisper-rs` 0.16.0), `build.rs` solo hace dos cosas con esa característica: define `CMAKE_BUILD_TYPE=RelWithDebInfo` y añade `-DWHISPER_DEBUG`. Con Visual Studio, `CMAKE_BUILD_TYPE` no decide la configuración: se compila `--config Release`, y lo que cambia es que el crate `cmake` ya no pisa las banderas de Release. El registro de MSBuild muestra `/O2 /Ob2 /D NDEBUG` en cada archivo. No es una compilación de depuración. `WHISPER_DEBUG` solo añade mensajes de registro (unas 35 líneas en cuatro órdenes), que Nexium descarta con `install_logging_hooks`. En Linux y macOS, `RelWithDebInfo` es `-O2 -g`.

**Verificación.** La CI comprueba con `cargo tree` que `whisper-rs-sys` se compila con `force-debug`; no lee el sistema de archivos, que depende de la caché y de la organización de carpetas de Cargo. La optimización real se verifica a mano, antes de cada versión, con el ejecutable de distribución:

- `NEXIUM_WHISPER_MODEL=<modelo> cargo test --release --test voice_real_audio -- --ignored`: transcribe una grabación real a 48 kHz y en estéreo y falla si tarda 2 s o más (sin optimizar, tarda más de 10 s).
- O `cargo run --release --example voice_params_bench -- <modelo> <carpeta de clips>`: debe dar alrededor de 1 s por orden corta.

| Ejecutable de distribución | Transcripción por orden, mediana | Máximo | Órdenes sin IA |
|---|---|---|---|
| Sin `force-debug` (whisper.cpp en `/Od`, sin recorte) | 18 752 ms | 19 604 ms | 4 de 4 |
| Con `force-debug` y la cadena actual | 1 074 ms | 1 166 ms | 4 de 4 |

**Cada parámetro, con las mismas grabaciones** (`examples/voice_params_bench.rs`, mejor de dos repeticiones; la configuración de la app es la primera fila):

| Variante | Mediana | Conclusión |
|---|---|---|
| Configuración de la app: español, voraz, sin reintentos, `audio_ctx`, 6 hilos, recorte y contexto de vocabulario | 986 ms | La elegida |
| Sin recortar el silencio (5 s de audio) | 1 420 ms | El recorte ahorra unos 0,4 s |
| Sin contexto de vocabulario | 869 ms | 0,1 s más rápido; con tu voz acierta igual, pero en los clips sintéticos resolvía 3 órdenes menos sin IA. Se mantiene |
| Ventana completa de 30 s (sin `audio_ctx`) | 6 567 ms | `audio_ctx` es imprescindible |
| Reintentos de temperatura (0,2) | 1 077 ms | Sin ventaja: siguen desactivados |
| 4 hilos | 1 277 ms | |
| 8 hilos | 1 003 ms | Igual que 6 |
| 12 hilos (todos los lógicos) | 1 349 ms | Peor: los hilos lógicos compiten |
| Idioma automático | 6 940 ms | Detectar el idioma cuesta 6 s: español forzado |

**Una corrección más.** «¿A qué red wifi estoy conectado?» se transcribió como «Aquí RedWiFi estoy conectado» y la regla lo tomaba como Internet. Wi-Fi pegado a la palabra anterior ahora se separa antes de aplicar las reglas.

**Objetivos en este equipo, con esta voz:** transcripción de una orden corta por debajo de 2 s (1,07 s de mediana y 1,17 s como máximo) y orden local completa por debajo de 3 s (transcripción más respuesta, como mucho 1,19 s).

## Medición final: 21 de noviembre de 2026

Ejecutable de distribución sin instalar, mismo método que la línea base, con batería.

| Medida | Línea base (11 nov.) | 15 nov. | 21 nov. |
|---|---|---|---|
| Instalador NSIS | 3,48 MiB | 4,55 MiB | 4,54 MiB |
| Instalador MSI | 4,67 MiB | 6,28 MiB | 6,20 MiB |
| Ejecutable `nexium.exe` | 11,1 MiB | 16,9 MiB | 15,5 MiB |
| Arranque | 527 a 573 ms | 579 a 895 ms | 688 a 806 ms (2 326 ms el primero, en frío) |
| Memoria privada en reposo | 178 MB | 198 MB | 176 MB |
| CPU en reposo | 0,44 % de un núcleo | 0,49 a 0,65 % | 0,70 % |
| Orden local por voz, mediana (30 clips) | | 12 920 ms en la app instalada | 1 164 ms |

El tamaño y el arranque siguen en lo esperado; la memoria vuelve a la línea base. La medida que cambia es la voz: ver la sección anterior.

## Al terminar las correcciones: 15 de noviembre de 2026

Commit `805b042`, ejecutable de distribución sin instalar, mismo método que la línea base (cinco arranques, memoria tras 20 s en reposo, CPU durante 60 s). **El equipo estaba con batería**, no conectado a la corriente como en la línea base: los tiempos pueden ser algo peores por eso.

| Medida | Línea base (11 nov.) | Fase B (13 nov.) | Ahora | Conclusión |
|---|---|---|---|---|
| Instalador NSIS | 3,48 MiB | 3,40 MiB | 4,55 MiB | Crece: lectura de PDF y `.docx`, selector del sistema y `panic = "unwind"` |
| Instalador MSI | 4,67 MiB | 4,60 MiB | 6,28 MiB | Ídem |
| Ejecutable `nexium.exe` | 11,1 MiB | | 16,9 MiB | Ídem |
| Frontend (`dist/`) | 537 KB | 450 KB | 505 KB | Crece algo: componentes de documentos, clima y movimiento |
| Arranque (cinco arranques seguidos) | 527 a 573 ms | 585 a 768 ms | 579 a 895 ms | Sin cambio claro: variación de una serie con batería |
| Memoria privada en reposo, total | 178 MB | 179 a 182 MB | 198 MB | Unos 16 MB más, casi todo en WebView2; `nexium.exe` pasa de 6 a 12 MB por el lector de CPU que se prepara al abrir |
| CPU en reposo | 0,44 % de un núcleo | 0,44 a 0,68 % | 0,49 a 0,65 % | Sin cambio apreciable |

El aumento de tamaño es el precio de leer documentos en el equipo en lugar de enviarlos enteros. La velocidad de cada solicitud, que era el objetivo, está en «Velocidad de las solicitudes», arriba.

## Comprobaciones del ejecutable de distribución

Búsqueda de texto en `nexium.exe`:

| Texto | Apariciones |
|---|---|
| `remote-debugging`, `9222`, `remote-allow-origins` | 0: la depuración remota no está incluida |
| URL del servidor de Nexium | 1 |
| URL de la API de Gemini | 0 |
| Prefijo de las claves de Google (`AIza`) | 0 |

## Auditoría de dependencias

11 de noviembre de 2026.

| Herramienta | Alcance | Resultado |
|---|---|---|
| `npm audit --omit=dev` | Dependencias de la app | 0 vulnerabilidades |
| `npm audit` en `server/` | Worker | 0 vulnerabilidades |
| `npm audit` | Incluye las herramientas de desarrollo | 1 moderada, ver abajo |
| `cargo audit` 0.23.0 | 618 crates de `Cargo.lock` | 0 vulnerabilidades, 5 avisos |

**Vulnerabilidad moderada de desarrollo, no corregida:** `minimatch` 7.0.0 a 11.0.0 puede consumir mucha CPU con ciertos patrones de búsqueda de archivos ([GHSA-g3hv-75wg-m249](https://github.com/advisories/GHSA-g3hv-75wg-m249)). Llega a través de ESLint y `typescript-eslint`, que solo se usan para analizar el código. No forma parte de la app ni del Worker, y solo recibe los patrones del propio proyecto. La corrección que propone `npm audit fix --force` degrada ESLint a la versión 9, un cambio incompatible, así que no se aplica. Se revisará cuando ESLint publique una versión corregida.

**Avisos de `cargo audit`:** todos llegan a través de Tauri y solo se compilan en Linux. No hay versión corregida que Nexium pueda elegir sin que Tauri la adopte.

| Crate | Aviso | Origen |
|---|---|---|
| `glib` 0.18.5 | Implementación insegura en `VariantStrIter` ([RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429)). El código de Nexium no lo usa directamente; Tauri no se ha comprobado | GTK 0.18, para el icono de bandeja de Tauri |
| `proc-macro-error` 1.0.4 | Sin mantenimiento ([RUSTSEC-2024-0370](https://rustsec.org/advisories/RUSTSEC-2024-0370)). Solo actúa al compilar | Macros de `glib` |
| `zbus_names`, `zvariant`, `zvariant_derive` | Sin mantenimiento (RUSTSEC-2026-0370 a 0372) | `zbus`, para las notificaciones de Tauri en Linux |
