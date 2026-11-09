# Nexium

Asistente inteligente de escritorio para Windows, macOS y Linux. Entiende solicitudes en español, por voz o por texto, y responde con datos reales de tu equipo: hora, CPU, memoria, batería, red, archivos, clima, recordatorios y análisis de pantalla bajo demanda.

Los hechos salen del equipo, no de un modelo. La IA interpreta lo que pides, pero solo la aplicación ejecuta operaciones, mediante funciones controladas.

> **Estado:** ver [`docs/ESTADO_DEL_PROYECTO.md`](docs/ESTADO_DEL_PROYECTO.md). Solo se ha ejecutado en Windows. En macOS y Linux se compila y se prueba en CI, pero faltan las [pruebas manuales](docs/PLAN_PRUEBAS_MANUALES.md).

## Privacidad: léelo antes de usar la IA

La IA viene integrada: no necesitas configurar ninguna clave. Tus solicitudes y capturas **pasan por el servidor de Nexium hacia Google Gemini**, en su capa gratuita. El servidor no guarda su contenido; solo registra métricas de uso y errores, y cuenta el uso con un identificador anónimo de instalación y tu dirección IP, que no se almacena. Detalles en [`server/README.md`](server/README.md).

Según los [términos de Google](https://aistudio.google.com/docs/terms), en los servicios no pagados:

- Google puede usar lo que envías y las respuestas para mejorar sus productos y tecnologías de aprendizaje automático.
- Personas pueden leer esos datos, tras desvincularlos de tu cuenta.
- Google pide **no enviar información sensible, confidencial o personal**.

Esto importa sobre todo en el **análisis de pantalla**: antes de cada captura, Nexium avisa de todo esto y nombra a Google Gemini. Sin pulsar Capturar, no se toma ninguna imagen. No analices una pantalla que muestre datos personales.

| Dato | ¿Sale del equipo? | Destino |
|---|---|---|
| Hora, CPU, RAM, batería, red, modo oscuro | No | |
| Voz | No: Whisper la transcribe localmente | |
| Nombres y rutas de archivos | No | |
| Texto de una solicitud no resuelta por reglas locales | Sí | Servidor de Nexium, y de ahí Google Gemini |
| Captura de pantalla, solo tras confirmar | Sí | Servidor de Nexium, y de ahí Google Gemini |
| Ciudad del clima | Sí | Open-Meteo |

Otras condiciones de esos términos: hay que tener 18 años o más, el servicio está pensado para desarrolladores con fines profesionales o comerciales, y en el Espacio Económico Europeo, Reino Unido y Suiza una aplicación ofrecida a usuarios solo puede usar servicios de pago.

## Funciones

- **Voz:** botón visible, micrófono solo mientras la barra está en amarillo, transcripción local con Whisper.
- **Sistema:** CPU, RAM, batería, Internet, Wi-Fi, nombre de la red y modo oscuro. Lo que un sistema no permite consultar aparece como "No disponible", nunca con un valor inventado.
- **Archivos:** búsqueda por nombre en Documentos y Escritorio, tras tu autorización. No abre, modifica, mueve ni elimina nada.
- **Pantalla:** una sola captura en memoria, comprimida y enviada al servicio de IA, y después descartada.
- **Clima y hora:** condiciones actuales y pronóstico de Open-Meteo; hora del equipo.
- **Recordatorios:** en lenguaje natural, con aclaración si la fecha es ambigua. Se guardan en SQLite.
- **Historial:** solicitudes, categoría, estado y respuesta, sin audio ni imágenes.

## Tecnologías

Tauri 2, React 19, TypeScript 6, Tailwind CSS 4, Rust, SQLite (`rusqlite`), Whisper (`whisper-rs`), `cpal`, `xcap`, `sysinfo`, Cloudflare Workers, API de Gemini y Open-Meteo. La justificación está en [`docs/ANALISIS_TECNICO.md`](docs/ANALISIS_TECNICO.md).

## Arquitectura

```text
React (solo presentación)
   │  comandos tipados de Tauri; ninguno genérico
   ▼
Rust: reglas locales ──► IA si hace falta ──► Intent validado ──► CommandHandler
        SystemProvider · FileSearchProvider · WeatherProvider · AiProvider
        SpeechProvider · ScreenCaptureProvider · NotificationProvider
   │  solo si hace falta IA
   ▼
Servidor de Nexium (Cloudflare Worker): valida, limita el uso y guarda la clave
   ▼
Google Gemini
```

La clave de Gemini solo existe como secreto de Cloudflare: nunca en el código, los instaladores, la CI ni tu equipo.

```text
src/            Interfaz: app, components, features, services, types, styles
src-tauri/src/  commands, core, providers, services, database, models, platform
server/         Worker intermediario de IA, con su README y sus pruebas
tests/          Pruebas de integración y contratos con Rust
docs/           Análisis, diseño, estado, prueba de voz y pruebas manuales
```

## Requisitos para compilar

- Node.js 22.12 o superior y Rust 1.90 o superior (con `rustup`).
- CMake y `libclang`, que necesita Whisper.
- Los [requisitos de Tauri 2](https://tauri.app/start/prerequisites/) de cada sistema.

**Windows:** Visual Studio Build Tools con "Desktop development with C++" incluye CMake. `libclang` se puede extraer del [paquete oficial de LLVM](https://github.com/llvm/llvm-project/releases) sin privilegios:

```powershell
# Tras dejar libclang.dll en %USERPROFILE%\.nexium-tools\llvm-<versión>\bin
. .\scripts\dev-env.ps1
```

No compiles dentro del directorio temporal de Windows: MSBuild falla allí.

**macOS:** Xcode Command Line Tools y `brew install cmake`.

**Linux (Ubuntu 24.04 o superior):** Ubuntu 22.04 no compila, porque `xcap` requiere encabezados de PipeWire más recientes que los suyos. Los paquetes generados en CI tampoco están pensados para distribuciones anteriores a 24.04.

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev \
  libayatana-appindicator3-dev librsvg2-dev libasound2-dev cmake pkg-config libclang-dev libxcb1-dev \
  libxrandr-dev libdbus-1-dev libpipewire-0.3-dev libwayland-dev libegl-dev libgbm-dev
```

## Ejecución

```bash
npm ci
npm run tauri dev
```

## Configuración

### IA

Quien usa Nexium no configura nada. **Ajustes** muestra el estado del servicio y un límite diario propio de solicitudes.

Las operaciones locales no consumen cuota. Si la cuota se agota, Nexium lo explica y no reintenta ni cambia de plan.

Modelos e instrucciones están en el servidor (`server/src/ai-contract.json`): `gemini-3.5-flash-lite` para interpretar y `gemini-3.6-flash` para analizar la pantalla.

### Variables de entorno

| Variable | Cuándo | Uso |
|---|---|---|
| `NEXIUM_AI_SERVER_URL` | Al compilar | Dirección del servidor de Nexium. Sin ella, la compilación no incluye IA. No es secreta |
| `GEMINI_API_KEY` | Solo en desarrollo, en `.env` | Sin `NEXIUM_AI_SERVER_URL`, `tauri dev` llama directamente a Gemini con esta clave. Ese código no existe en los instaladores |

### Voz

El modelo se descarga una sola vez desde **Ajustes**, con tu confirmación, y se verifica su hash. Se guarda en la carpeta de datos de la aplicación, dentro de `models/`. Se carga en memoria la primera vez que hablas y se libera tras un periodo sin uso, configurable en **Ajustes** (10 minutos por defecto). Detalles y mediciones: [`docs/PRUEBA_VOZ.md`](docs/PRUEBA_VOZ.md).

## Pruebas

```bash
npm run lint        # ESLint y reglas de Tailwind
npm test            # Vitest
cd src-tauri && cargo test
cd server && npm ci && npm test
```

Pruebas con servicios reales, que gastan cuota y nunca se ejecutan en CI:

```bash
cd src-tauri
cargo run --example ai_live -- frases
```

## Compilación y distribución

```bash
npm run tauri build
```

Cada envío a `main` compila instaladores en GitHub Actions (`.msi`, `.exe`, `.dmg`, `.deb`, `.AppImage`) y los deja en "Artifacts".

### Abrir un instalador sin firmar

Nexium es un proyecto universitario distribuido solo en GitHub, y sus instaladores no están firmados. El sistema avisará la primera vez. Descarga los instaladores únicamente de la página de [Releases](https://github.com/Daniel-I24/Nexium/releases) de este repositorio. No hace falta desactivar ninguna protección del sistema.

**Windows (SmartScreen):** al abrir el instalador aparece "Windows protegió su PC". Pulsa **Más información** y después **Ejecutar de todas formas**.

**macOS (Gatekeeper):** abre el `.dmg`, arrastra Nexium a Aplicaciones e inténtalo abrir. Cuando macOS lo bloquee, ve a **Ajustes del Sistema → Privacidad y seguridad**, baja hasta el aviso sobre Nexium y pulsa **Abrir igualmente**. Solo hay que hacerlo una vez.

## Limitaciones conocidas

- Los recordatorios solo avisan mientras Nexium está abierto o en la bandeja. Al salir del todo no hay avisos, y los vencidos aparecen al volver a abrirlo.
- La búsqueda de archivos es por nombre: no lee su contenido.
- El nombre de la red Wi-Fi depende de permisos de ubicación en Windows 11 24H2 y en macOS.
- Open-Meteo es gratuito solo para uso no comercial. **Un uso comercial requiere un plan de pago.**
- Las respuestas se muestran en texto. No hay voz sintetizada en esta versión.
- La base de datos local no está cifrada. Contiene solicitudes y respuestas, que puedes eliminar desde Historial.
- La IA necesita Internet y comparte una cuota gratuita limitada: en días de mucho uso puede agotarse hasta el día siguiente. Las consultas locales siguen funcionando.
- La IA no está disponible en el Espacio Económico Europeo, Reino Unido ni Suiza, porque los [términos de Google](https://aistudio.google.com/docs/terms) solo permiten allí servicios de pago. Tampoco cuando no se puede determinar el país, como en la red Tor. El servidor lo decide según el país que informa Cloudflare, y Nexium lo explica con un mensaje. Las consultas locales siguen funcionando.

## Atribución

Datos meteorológicos de [Open-Meteo](https://open-meteo.com/), con licencia [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Tipografías Instrument Sans e IBM Plex Mono, con licencia OFL. Reconocimiento de voz con [whisper.cpp](https://github.com/ggml-org/whisper.cpp), con licencia MIT.

## Licencia

[MIT](LICENSE).
