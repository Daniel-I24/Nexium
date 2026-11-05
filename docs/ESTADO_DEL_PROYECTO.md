# Nexium: estado del proyecto

Última actualización: 5 de noviembre de 2026.

**Etapa actual:** 2, diseño visual. A la espera de la aprobación del propietario.

| Etapa | Estado |
|---|---|
| 1. Análisis y planificación | Completada: `docs/ANALISIS_TECNICO.md` |
| 2. Diseño visual | Propuesta lista: `docs/DISENO_VISUAL.md`. Pendiente de aprobación |
| 3 a 7 | Sin empezar |

## Implementado

Nada de la aplicación. Solo existen documentación y archivos de control (`.gitignore`, `.env.example`).

## Probado

Solo el entorno y las dependencias nativas en una prueba desechable, fuera del repositorio. Ver "Resultado de la prueba de viabilidad" en `docs/ANALISIS_TECNICO.md`.

## Errores detectados

- Corregido: `whisper-rs-sys` no compilaba sin `libclang`. Se resolvió con LLVM 23.1.5 en el perfil de usuario y `LIBCLANG_PATH`. Ver el riesgo R1 del análisis.

## Limitaciones pendientes

- No hay ninguna verificación en macOS ni en Linux.
- Whisper compila, pero aún no ha transcrito audio.
- Las pruebas de IA en vivo requieren una clave de API del propietario.
