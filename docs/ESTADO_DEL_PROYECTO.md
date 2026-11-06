# Nexium: estado del proyecto

Última actualización: 5 de noviembre de 2026.

**Etapa actual:** 3 y 4 en curso. La aplicación completa aún no se ha ejecutado en una ventana real.

| Etapa | Estado |
|---|---|
| 1. Análisis | Completada. Revisada al cambiar a Gemini |
| 2. Diseño visual | Aprobada, con ajustes incorporados |
| 3. Configuración | Compila y pasa pruebas en Windows. CI en curso, con un fallo en Linux por diagnosticar. Prueba de Whisper parcial |
| 4. Módulos | Código y pruebas automáticas escritos. Sin verificación de extremo a extremo |
| 5. Integración | Sin empezar |
| 6. Multiplataforma | CI configurada; sin resultado favorable todavía |
| 7. Entrega | README y plan de pruebas manuales escritos |

## Implementado

Código escrito, con pruebas automáticas, en este orden de módulos: interfaz, núcleo de comandos, IA (Gemini), sistema, archivos, clima y hora, recordatorios, historial, voz, pantalla.

## Probado

**Automático, en este Windows:**

- `cargo test --lib`: 92 pruebas correctas.
- `cargo test --test contract`: 7 pruebas correctas. Los JSON de Rust coinciden con los que usa la interfaz.
- `npm test`: 26 pruebas correctas.
- `npm run lint` y `tsc --noEmit`: sin errores.

Las pruebas de Gemini y Open-Meteo usan servidores simulados, así que no demuestran que los servicios reales respondan como se espera.

**Visual:** interfaz abierta en Edge sin ventana visible y fotografiada con datos de ejemplo, en 1100×720, 720×520 y 1920×1080, en ambos temas. Se corrigieron el ancho en pantallas grandes y el espaciado de la coma en la lectura. Tras esas correcciones no se ha vuelto a fotografiar.

**Voz:** Whisper transcribió audio humano real. Resultados parciales en `docs/PRUEBA_VOZ.md`.

## Sin probar

- La aplicación en una ventana real de Tauri.
- Comandos reales desde la interfaz, micrófono, bandeja, notificaciones y almacén de claves.
- Captura de pantalla real.
- **Gemini real: pendiente de la clave.** Falta probar las frases en español, incluidas las fechas ambiguas, y comparar modelos con capturas sin datos sensibles. Por eso la elección de modelos aún no es un resultado medido. Herramienta lista: `src-tauri/examples/gemini_live.rs`.
- Open-Meteo real.
- Cualquier cosa en macOS o Linux más allá de lo que compruebe la CI. Lista en `docs/PLAN_PRUEBAS_MANUALES.md`.

## Pruebas reales con Gemini

Sin resultados todavía. Se registrarán aquí.

## Errores detectados

| Error | Estado |
|---|---|
| `whisper-rs-sys` no compila sin `libclang` | Corregido: LLVM en el perfil de usuario y `LIBCLANG_PATH` |
| MSBuild no compila dentro del directorio temporal | Evitado: nunca se compila en Temp |
| TypeScript 7 incompatible con `typescript-eslint` | Corregido: se fija la 6.0.3 |
| Latencia de Whisper en frases cortas | **Abierto:** ver `docs/PRUEBA_VOZ.md` |
| CI de Linux: fallan las pruebas de Rust (código 101) | **Abierto:** causa desconocida. Añadido diagnóstico a la CI |
| La medición optimizada de Whisper se detuvo por falta de memoria en el equipo | **Abierto:** pendiente de relanzar con memoria libre |

## Limitaciones pendientes

- El estado Respondiendo es breve: la respuesta de Gemini no llega en streaming en esta versión.
- Los commits de los módulos del backend se verificaron con el árbol completo. Los commits intermedios no se compilaron por separado.

## Decisiones pendientes del propietario

- Certificados de firma antes de una distribución pública. Por ahora, no se compran.
- Condiciones de la capa gratuita de Gemini para una distribución pública: uso de datos, 18 años o más, uso profesional, y solo servicios de pago en el EEE, Reino Unido y Suiza.
- Plan de pago de Open-Meteo si hay uso comercial.
