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
