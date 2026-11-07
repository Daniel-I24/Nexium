# Nexium: análisis técnico

Fecha: 5 de noviembre de 2026, revisado el mismo día al cambiar el proveedor de IA a Gemini por decisión del propietario. Responde a los 10 puntos de la sección 18 de `PROMPT_MAESTRO.md`.

Las versiones se consultaron hoy en npm y en el índice de crates.io. "Verificado" significa ejecutado en este equipo con Windows 11. Nada se ha comprobado aún en macOS ni en Linux.

## 1. Comprensión del problema y del producto

Nexium es un asistente de escritorio para Windows, macOS y Linux. Recibe solicitudes por voz o texto en español, decide qué operación corresponde, la ejecuta con funciones controladas y presenta un resultado real.

Tres ideas guían el diseño:

- **Los hechos salen del equipo, no del modelo.** La RAM, la existencia de un archivo o la hora nunca los responde la IA.
- **La IA interpreta y analiza, pero no actúa.** Devuelve datos estructurados que la aplicación valida, y solo la aplicación ejecuta operaciones.
- **El alcance es pequeño.** Son ocho módulos, sin extras. Lo que no se pueda garantizar en un sistema se informa como no disponible.

## 2. Arquitectura tecnológica definitiva

```text
React + TypeScript + Tailwind      Solo presentación y estado de la interfaz
        │  invoke / eventos, con contratos tipados
        ▼
Comandos Tauri (Rust)              Validan toda entrada; no hay comando genérico
        ▼
CommandRouter ──► reglas locales ──► (si no bastan) AiProvider.interpret()
        │                                 ▼
        │                           Intent validado
        ▼
CommandHandler (uno por operación)
   ├─ SystemProvider        → adaptadores Windows / macOS / Linux
   ├─ FileSearchProvider    → solo carpetas autorizadas
   ├─ WeatherProvider       → Open-Meteo
   ├─ ReminderService       → SQLite + planificador + NotificationProvider
   ├─ ScreenCaptureProvider → xcap / portal de Wayland
   └─ AiProvider.analyze_image()
SpeechProvider → micrófono (cpal) + Whisper local
HistoryService → SQLite
```

### Versiones

| Componente | Versión | Nota |
|---|---|---|
| Node.js / npm | 24.21 / 11.21 | Instalados. Cumplen el requisito de Vite (`>=22.12`) |
| Rust / Cargo | 1.99.1 | Instalados con rustup. Tauri exige 1.90 o superior |
| Tauri (crate / CLI) | 2.12.5 / 2.12.4 | `cargo search` muestra 3.0.0-alpha, que **se descarta** por experimental |
| `tauri-plugin-notification` | 2.8.0 | |
| `tauri-plugin-single-instance` | 2.5.5 | Evita dos instancias que dupliquen avisos |
| React | 19.3.1 | |
| TypeScript | 6.0.3 | La 7.0.2 se descarta: `typescript-eslint` 8.72 exige una versión menor de 6.1 |
| Vite / plugin de React | 8.3.6 / 6.2.1 | El plugin exige Vite 8 |
| Tailwind / plugin de Vite | 4.3.4 | Configuración en CSS |
| Vitest | 5.1.0 | Admite Vite 8 |
| `rusqlite` | 0.41 con `bundled` | |
| `keyring` | 4.2 | |
| `sysinfo` | 0.40 | |
| `starship-battery` | 0.12 | Bifurcación mantenida de `battery` |
| `xcap` | 0.9 | |
| `ashpd` | 0.14 | Solo Linux, portal de captura en Wayland |
| `cpal` | 0.18 | |
| `whisper-rs` | 0.16 | |
| `reqwest` | 0.13 | |

## 3. Justificación de cada tecnología

| Decisión | Por qué | Alternativa descartada |
|---|---|---|
| **Tauri 2** | Binario pequeño, poca RAM y permisos por capacidades | Electron: más consumo |
| **React, TypeScript, Tailwind 4** | Requisito del proyecto. Tailwind 4 permite un tema único en CSS | |
| **Rust** | Acceso nativo, y las claves nunca llegan a la interfaz | Lógica en TypeScript: expondría la clave a la vista web |
| **`rusqlite` con `bundled`** | La misma versión de SQLite en los tres sistemas, y solo el backend accede a los datos | `tauri-plugin-sql`: abre SQL hacia la interfaz |
| **`keyring`** | Usa Credential Manager, Keychain y Secret Service | Archivo cifrado: habría que gestionar otra clave |
| **Whisper local para voz** | Mismo motor en los tres sistemas, sin coste por uso, sin segunda clave y sin enviar la voz fuera | Ver abajo |
| **`cpal` para el micrófono** | Captura nativa e igual en todos los sistemas | `getUserMedia`: depende de cada vista web |
| **Gemini como proveedor inicial** | Decisión del propietario: capa gratuita, texto y visión en una sola API, salidas estructuradas | Cualquier otro se puede añadir tras `AiProvider` |
| **Open-Meteo** | Sin clave, con geocodificación y pronóstico | OpenWeather: exige clave |
| **Sin SDK de IA en Rust** | Google no ofrece SDK para Rust, así que se usa HTTP directo con `reqwest` | |

### Por qué no se usa la Web Speech API

- **Windows:** hay una [regresión en WebView2 Runtime 153](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5724) por la que falla con error `network`, mientras que en el Edge normal funciona. Este equipo tiene la 156. No se ha ejecutado una prueba propia con esta versión.
- **macOS y Linux:** usan WebKit, y las fuentes consultadas no confirman su disponibilidad.
- **En todos los casos:** depende de un servicio externo fuera de nuestro control.

Una función central no puede depender de eso.

### Decisiones sobre la voz

- Modelo por defecto: `ggml-small-q5_1.bin` (190 MB, licencia MIT), con `ggml-base-q5_1.bin` (60 MB) como opción para equipos modestos. El español necesita algo más que el modelo `tiny`. La calidad y la latencia se medirán en la Etapa 4 con este hardware.
- Se descarga una vez, con permiso expreso y barra de progreso, y se verifica su hash. No se incluye en el instalador.
- **Compilación:** exige CMake y `libclang`, ver el riesgo R1.

## 4. Estructura de carpetas

```text
C:\Nexium\Nexium
├── src/
│   ├── app/              App, navegación, proveedores
│   ├── components/       Button, DataRow, Readout, Scale…
│   ├── features/
│   │   ├── assistant/    Cinta, lectura, respuestas
│   │   ├── voice/        Barra de señal y estados
│   │   ├── system/  files/  screen-analysis/  weather/
│   │   ├── reminders/  history/  settings/
│   ├── services/         Cliente tipado de los comandos Tauri
│   ├── types/
│   └── styles/theme.css  Tokens de Tailwind
├── src-tauri/
│   ├── src/
│   │   ├── commands/     Fina capa IPC: valida y delega
│   │   ├── core/         CommandRouter, reglas, Intent, handlers
│   │   ├── providers/    ai/  speech/  weather/  capture/  files/  notification/
│   │   ├── services/     reminders, history, settings, secrets, usage
│   │   ├── database/     Conexión y migraciones
│   │   ├── models/
│   │   └── platform/     windows/  macos/  linux/  (SystemProvider)
│   ├── capabilities/     Permisos mínimos
│   └── Cargo.toml
├── tests/                Integración del frontend
├── docs/
└── .github/workflows/    Compilación en los tres sistemas
```

Las pruebas de Rust van junto a su módulo y en `src-tauri/tests/`. Se añade `core/` y `settings/` a la propuesta del documento porque el enrutador no es un proveedor y Ajustes es una pantalla obligatoria. No se crean carpetas vacías: cada una aparece con su módulo.

## 5. Implementación de la inteligencia artificial

### Cuándo se usa

| Solicitud | Resolución | IA |
|---|---|---|
| Hora, RAM, CPU, batería, Wi-Fi, Internet, modo oscuro | Reglas locales en español, con tolerancia a variaciones | No |
| Clima con ciudad clara o configurada | Reglas locales | No |
| Lenguaje libre o reglas sin coincidencia clara | `interpret()` | Sí |
| Recordatorios | `interpret()` para extraer contenido, fecha y hora | Sí |
| Búsqueda en lenguaje natural | `interpret()` para extraer términos y sinónimos | Sí |
| Análisis de pantalla | `analyze_image()` | Sí, con imagen |

Sin conexión o sin clave, lo resuelto por reglas sigue funcionando y lo demás lo informa con claridad.

### Abstracción

```rust
#[async_trait]
trait AiProvider: Send + Sync {
    fn display_name(&self) -> &str;        // "Google Gemini": aparece en el aviso de captura
    fn data_notice(&self) -> &str;         // Aviso de privacidad propio del proveedor
    async fn interpret(&self, request: InterpretRequest) -> Result<Intent, AiError>;
    async fn analyze_image(&self, request: VisionRequest) -> Result<VisionAnswer, AiError>;
}
```

`GeminiProvider` es la primera implementación y nada fuera de `providers/ai/gemini.rs` conoce el formato de Gemini. `AiError` es neutro (`QuotaExhausted`, `RateLimited`, `InvalidKey`…). Cambiar de proveedor equivale a añadir otra implementación y seleccionarla en la configuración. El proveedor activo y sus modelos se guardan en la configuración de SQLite y se editan en Ajustes, sin tocar código.

### Gemini: API y modelos

Fuentes oficiales consultadas hoy: [modelos](https://aistudio.google.com/docs/models), [precios](https://aistudio.google.com/docs/pricing), [límites](https://aistudio.google.com/docs/rate-limits), [términos](https://aistudio.google.com/docs/terms), [Interactions API](https://aistudio.google.com/docs/interactions) y [errores](https://aistudio.google.com/docs/api-errors). Las direcciones antiguas de `ai.google.dev` redirigen ahí.

**API:** `POST /v1beta/interactions`, con encabezado `Api-Revision: 2026-05-20`. La documentación presenta `generateContent` como heredado, aunque sigue funcionando.

| Tarea | Modelo por defecto | Por qué |
|---|---|---|
| Interpretar | `gemini-3.5-flash-lite` | Estable, en capa gratuita, el más rápido y económico de la familia 3.5. Clasificar y extraer no necesita más. Pensamiento bajo |
| Analizar pantalla | `gemini-3.6-flash` | Estable, en capa gratuita y multimodal, para leer texto pequeño y errores en una interfaz |

**Por qué dos modelos:** una consulta de intención debe ser rápida y gastar poco, y una captura debe leerse con precisión. Son dos ajustes independientes.

**Prueba real:** los dos modelos acertaron cuatro capturas sintéticas. Se mantienen ambos porque `gemini-3.6-flash` explicó mejor los errores; la muestra es pequeña. Resultados en `docs/ESTADO_DEL_PROYECTO.md`. Se descartan `gemini-3.8-flash` (marcado "New", no estable) y `gemini-2.5-flash` (deprecado: desde el 1 de diciembre de 2026 las peticiones se redirigen a otros modelos).

**Límites de la capa gratuita:** Google no publica las cifras por modelo en su documentación: solo se ven en AI Studio, para cada proyecto. La cuota diaria se reinicia a medianoche, hora del Pacífico. Por eso Nexium no incluye cifras fijas y mantiene su propio límite diario configurable.

### Privacidad en la capa gratuita de Gemini

**Google sí puede usar estos datos.** Según sus [términos](https://aistudio.google.com/docs/terms), en los servicios no pagados:

- Google usa lo enviado y las respuestas generadas para "proveer, mejorar y desarrollar" sus productos y tecnologías de aprendizaje automático.
- Personas pueden leer, anotar y procesar las entradas y salidas, tras desvincularlas de la cuenta, la clave y el proyecto.
- Google pide expresamente **no enviar información sensible, confidencial o personal**.

En los servicios de pago, Google declara que no usa el contenido para mejorar productos.

**Esto choca con el análisis de pantalla,** porque una captura puede contener cualquier cosa. Medidas de Nexium:

1. El aviso previo a cada captura nombra a Google Gemini, explica este uso y pide no capturar información personal. Sin pulsar Capturar no se toma ninguna imagen.
2. Las solicitudes de texto envían solo lo necesario. Nunca se envían rutas ni nombres de archivos encontrados, ni el historial.
3. Se envía `store: false`, porque por defecto la API guarda cada interacción en el servidor (1 día en la capa gratuita). Esto evita ese almacenamiento, pero **no** cambia el uso descrito arriba.
4. El aviso general aparece en Ajustes, junto a la clave, y en el README.

**Otras condiciones de los términos que afectan a la distribución:**

- Es necesario tener 18 años o más, y no se puede ofrecer un cliente de la API dirigido a menores.
- Los términos describen el servicio como destinado a desarrolladores con fines profesionales o comerciales, no para uso de consumidores.
- En el Espacio Económico Europeo, Reino Unido y Suiza, una aplicación ofrecida a usuarios solo puede usar servicios de pago.

Antes de una distribución pública hay que revisar estas condiciones. Queda como decisión pendiente del propietario.

### Controles obligatorios

- **Salida estructurada:** `response_format` con un esquema JSON. Después, Rust valida con tipos propios: una intención desconocida o una fecha inválida no se ejecutan.
- **Lista cerrada:** el modelo solo puede elegir entre intenciones de un `enum`. No hay herramientas ni acceso a comandos o archivos.
- **Datos mínimos:** se envía el texto de la solicitud y la fecha y zona horaria actuales.
- **Fechas:** el modelo devuelve la fecha resuelta y si existe ambigüedad. Rust comprueba que sea válida y futura. Si hay ambigüedad o falta la hora, se pide aclaración. El usuario confirma antes de guardar.
- **Tiempos de espera y cancelación:** 20 s para interpretar y 60 s para visión, cancelables desde la interfaz.
- **Cuota agotada:** `quota_exceeded` produce "Se agotó la cuota diaria gratuita de Google Gemini. Se renueva a medianoche, hora del Pacífico." `rate_limit_exceeded` y `too_many_requests` piden esperar un minuto. **No hay reintentos automáticos ante 429, ni cambio a un plan de pago.** Solo un 503 o 504 admite un único reintento.
- **Sin gasto innecesario:** hora, CPU, RAM, batería, red y modo oscuro nunca llaman a la IA. Una frase que las reglas no entienden sí la usa.
- **Costos:** límite diario de solicitudes configurable, contador de tokens en SQLite, `max_output_tokens` acotado y captura reducida a 1568 px como máximo en el lado largo.

### Claves

- **Aplicación distribuida:** la clave se escribe en Ajustes, se guarda en el almacén seguro del sistema y nunca vuelve a la interfaz. Si Secret Service no existe en un Linux, la clave solo se conserva durante la sesión; nunca se guarda en claro.
- **Desarrollo:** únicamente las compilaciones de desarrollo y las pruebas en vivo leen `GEMINI_API_KEY` desde `.env`, que Git ignora. El código de lectura no existe en la compilación de distribución.
- **Registros:** la clave nunca aparece en registros ni en mensajes de error.

**Servicio intermediario: evaluado y descartado para esta versión.** Protegería una clave compartida, pero exige servidor, cuentas, límites y un coste continuo: justo la plataforma que el documento pide no construir. Cada usuario usa su propia clave. `AiProvider` deja abierta la opción de añadirlo después.

## 6. Acceso multiplataforma al sistema

`SystemProvider` es la interfaz común. Cada lectura devuelve un valor que distingue explícitamente entre dato real, no disponible y error:

```rust
enum Reading<T> { Value(T), Unavailable { reason: UnavailableReason } }
```

Así, un valor inventado no puede pasar el tipado, y un fallo no afecta al resto.

### Matriz de compatibilidad (prevista)

Salvo lo marcado en la leyenda, esta matriz es **lo que se espera según la documentación**, no un resultado probado.

| Función | Windows | macOS | Linux | Permisos o dependencias |
|---|---|---|---|---|
| CPU y RAM | `sysinfo` | `sysinfo` | `sysinfo` | Ninguno |
| Batería | `starship-battery` | Igual | Igual | Equipos sin batería: "No disponible" |
| Internet | Prueba de conexión real | Igual | Igual | Necesita red |
| Estado Wi-Fi | WLAN API | `networksetup` e `ipconfig` | `nmcli`, de NetworkManager | Herramientas del sistema con argumentos fijos. Linux sin NetworkManager: "No disponible" |
| Nombre de red (SSID) | Con restricción | Con restricción | Con NetworkManager | Windows 11 24H2: ubicación para apps de escritorio. macOS: Location Services |
| Modo oscuro | API de ventana de Tauri | Igual | Igual | Depende del escritorio en Linux |
| Archivos | Rutas conocidas | Rutas conocidas | Rutas XDG | Consentimiento en la app |
| Captura | `xcap` | `xcap` | X11: `xcap`. Wayland: portal | macOS: Grabación de pantalla. Wayland: diálogo del sistema |
| Micrófono | `cpal` | `cpal` | `cpal` con ALSA | macOS: `NSMicrophoneUsageDescription`. Linux: `libasound2-dev` |
| Notificaciones | Plugin de Tauri | Plugin | Plugin | Windows: solo con identidad propia si está instalada |
| Voz | Whisper | Whisper (Metal) | Whisper | Al compilar: CMake, C++ y `libclang` |
| Clave segura | Credential Manager | Keychain | Secret Service | Linux: necesita un servicio de llaves |

**Comprobado hasta ahora:** ver "Resultado de la prueba de viabilidad" en la sección 8. En este Windows, `netsh wlan` devuelve el SSID con la ubicación permitida. Faltan por verificar los casos con la ubicación desactivada, y todo macOS y Linux.

**Dependencias de compilación en Linux (Ubuntu):** `libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev`, más `libasound2-dev cmake pkg-config libclang-dev libxcb1-dev libxrandr-dev libdbus-1-dev libpipewire-0.3-dev libwayland-dev libegl-dev`.

**Requisito mínimo:** Windows 10 1803 o posterior con WebView2, macOS 10.15 o posterior, y WebKitGTK 4.1 en Linux. `xcap` puede elevar el mínimo de macOS, lo que se fijará en CI.

## 7. APIs y servicios externos

| Servicio | Para qué | Clave | Coste | Datos que salen del equipo |
|---|---|---|---|---|
| **API de Gemini (Google)** | Interpretar y analizar imágenes | Sí, del usuario | Capa gratuita; Google puede usar los datos (sección 5) | Texto de la solicitud, fecha y zona. La captura, solo tras el aviso |
| **Open-Meteo** | Clima y geocodificación | No | Gratis solo sin fines comerciales | Ciudad y coordenadas |
| **Hugging Face** | Descarga única del modelo de voz | No | Gratis | Una petición de descarga |

Whisper funciona sin Internet una vez descargado.

**Ubicación del clima:** usa la ciudad indicada en la solicitud o la configurada en Ajustes. No se añade geolocalización por IP.

**Licencia de Open-Meteo:** sus [condiciones](https://open-meteo.com/en/terms) limitan el uso gratuito a fines no comerciales, con menos de 10 000 llamadas al día, y piden atribución CC BY 4.0. **Un uso comercial requiere un plan de pago.** La atribución se muestra en cada respuesta de clima y en Ajustes.

## 8. Riesgos técnicos y limitaciones

### Contradicciones y requisitos inviables

| # | Problema | Decisión |
|---|---|---|
| C1 | El documento pide no generar archivos ni código en la Etapa 1 | Queda sustituido por la instrucción del propietario. Solo se crean documentación y archivos de control |
| C2 | Se exige la herencia, pero Rust no tiene herencia de clases y el mismo documento prohíbe la artificial | En Rust se usan traits, supertraits y métodos por defecto para abstracción y polimorfismo; structs con campos privados para encapsular; composición para el resto. Se documenta como decisión. No se fuerza herencia de clases |
| C3 | "Análisis de archivos" en el título, pero el funcionamiento solo describe búsqueda y prohíbe leer contenido sin permiso | La versión 1 busca por nombre y metadatos. No lee contenido. "Relacionados con bases de datos" se resuelve con términos en el nombre, y esa limitación se indica al usuario |
| C4 | El módulo 6 dice "responder mediante voz o texto" y los demás hablan solo de interfaz | Texto en la interfaz cumple el "o". No se añade síntesis de voz. La entrada de texto sí se incluye, porque es la alternativa cuando no hay micrófono |
| C5 | Registrar la respuesta en el historial frente a no almacenar información sensible | Se guarda el texto de la respuesta, pero nunca imágenes, claves ni audio. El análisis de una pantalla puede describir algo privado: se avisará y se podrá eliminar cada registro |
| C6 | Verificar el funcionamiento en tres sistemas con un solo equipo Windows | **Inviable de forma directa.** CI verifica compilación y pruebas. Las pruebas manuales quedan documentadas como pendientes |
| C7 | "Lista para distribuirse" sin certificados de firma | Se generan instaladores sin firmar. Sin firma, Windows mostrará SmartScreen y macOS bloqueará la apertura normal. **Decisión del propietario: no se compran certificados por ahora.** Queda como decisión pendiente antes de una distribución pública |
| C8 | Notificaciones fiables frente a "no prometer" | Ver R4 |
| C9 | `.env.example` obligatorio, pero la app no usa `.env` | Se incluye solo para pruebas en vivo durante el desarrollo |

### Riesgos

| # | Riesgo | Prob. | Impacto | Plan |
|---|---|---|---|---|
| R1 | **Compilar `whisper-rs`:** exige CMake y `libclang`. Resuelto en este Windows; falta comprobarlo en macOS y Linux | Media | Alto | Ver "Resultado de la prueba de viabilidad". CI lo comprueba en la Etapa 3, antes de escribir el módulo |
| R2 | Wayland: `xcap` marca soporte limitado | Alta | Medio | Adaptador propio con el portal `ashpd`. Pendiente de prueba manual |
| R3 | SSID en macOS: Apple exige Location Services y los resultados varían entre versiones, sobre todo sin firma | Alta | Bajo | Se muestra "No disponible" con el motivo |
| R4 | Los avisos solo se pueden emitir si Nexium está en ejecución: el plugin no documenta programación en escritorio | Alta | Alto | Planificador propio en Rust. Al cerrar la ventana, la app sigue en la bandeja del sistema, con opción "Salir". Al abrir, muestra los vencidos. La interfaz dice claramente que con la app cerrada no hay avisos |
| R5 | Windows muestra la notificación como PowerShell en desarrollo | Segura | Bajo | [Comportamiento documentado](https://tauri.app/plugin/notification/). Se verifica con el instalador |
| R6 | macOS: los permisos de captura y micrófono pueden no conservarse sin firma | Media | Medio | Documentado. Prueba manual |
| R7 | Inyección de instrucciones desde el texto de una captura | Media | Medio | La visión no tiene herramientas ni puede ejecutar nada, y su resultado solo se muestra como texto |
| R8 | Whisper lento en equipos sin GPU | Media | Medio | Modelo menor, proceso en segundo plano y cancelación. Se mide en la Etapa 4 |
| R9 | Linux sin Secret Service | Media | Bajo | Clave solo durante la sesión |
| R10 | Datos en claro: SQLite no está cifrado | Segura | Medio | Solo solicitudes y respuestas, con eliminación disponible. Documentado |
| R11 | Búsqueda lenta en carpetas grandes | Media | Bajo | Proceso en segundo plano, límite de profundidad y resultados, cancelable. No sigue enlaces simbólicos |
| R12 | Cuota gratuita de Gemini agotada, o sus cifras cambian sin aviso | Media | Medio | Las operaciones locales no consumen cuota. Mensaje claro y sin reintentos. Límite diario propio |
| R13 | Datos personales en una captura enviada a la capa gratuita | Media | Alto | Aviso obligatorio antes de cada captura (sección 5). El riesgo no se elimina: depende de lo que muestre la pantalla |

### Resultado de la prueba de viabilidad

Prueba desechable ejecutada hoy en este equipo con Windows 11, fuera del repositorio y ya eliminada.

**Lo que compiló y se ejecutó:**

| Crate | Resultado real |
|---|---|
| `xcap` | Enumeró 1 monitor. No se hizo ninguna captura |
| `sysinfo` | Leyó la memoria total |
| `rusqlite` | SQLite 3.54.1 incluido |
| `cpal` | Encontró un micrófono por defecto. No se grabó audio |
| `starship-battery` | Detectó 1 batería |
| `whisper-rs` | Compiló y enlazó. **Aún no ha transcrito audio** |
| `keyring` | Solo compiló. Falta probar guardar y leer una clave |

**Lo que falló y cómo se resolvió, en orden:**

1. Sin `libclang`, `whisper-rs-sys` no compila. Build Tools no lo incluye.
2. `WHISPER_DONT_GENERATE_BINDINGS=1` no sirve en Windows: los enlaces pregenerados no coinciden con esta plataforma (`_IO_FILE`).
3. **Solución verificada:** `libclang` del paquete oficial de LLVM 23.1.5, extraído sin privilegios de administrador en `%USERPROFILE%\.nexium-tools\llvm-23.1.5` (102 MB), con `LIBCLANG_PATH` apuntando a su carpeta `bin`.
4. MSBuild no puede compilar dentro del directorio temporal de Windows (`MSB8029`, `FTK1011`). El proyecto real está en `C:\Nexium\Nexium`, así que no le afecta, pero `CARGO_TARGET_DIR` nunca debe apuntar a Temp.

**Consecuencia:** la voz sigue siendo viable, con dos requisitos de compilación en Windows: CMake (incluido en Build Tools, aunque no está en el `PATH`) y `LIBCLANG_PATH`. Se documentarán en el README, y CI los instalará en los tres sistemas. La primera compilación de estas dependencias tardó casi 5 minutos.

**Prueba real con audio:** ver `docs/PRUEBA_VOZ.md`, realizada en la Etapa 3 a petición del propietario.

## 9. Plan de desarrollo por etapas

| Etapa | Contenido | Criterio de salida |
|---|---|---|
| 1. Análisis | Este documento | Hecho |
| 2. Diseño | `docs/DISENO_VISUAL.md` y lienzo | Aprobación del propietario |
| 3. Configuración | Tauri, React, Tailwind, SQLite y CI. **Prueba de Whisper con audio real antes del resto de módulos** | Compilación, CI en verde y `docs/PRUEBA_VOZ.md` |
| 4. Módulos | Ver orden abajo | Pruebas y compilación por módulo |
| 5. Integración | Flujos completos, permisos, errores | Lista de comprobación en Windows |
| 6. Multiplataforma | Compilación y pruebas en GitHub Actions | CI en verde; pruebas manuales documentadas |
| 7. Entrega | README, instaladores y limitaciones | Instalación reproducible |

**Orden de la Etapa 4:**

1. Interfaz principal
2. Núcleo de comandos
3. Consultas del sistema
4. Inteligencia artificial
5. Búsqueda de archivos
6. Clima y hora
7. Recordatorios
8. Historial
9. Voz
10. Análisis de pantalla

**Cambios respecto al orden del documento, con su motivo:** las consultas del sistema van antes que la IA, porque permiten probar el núcleo sin coste ni clave. El historial sube antes de la voz porque registra toda operación del núcleo y es más barato integrarlo pronto.

**Cómo se prueba cada módulo:**

| Módulo | Pruebas automáticas | Prueba real |
|---|---|---|
| Interfaz | Vitest y Testing Library: estados y accesibilidad | Abrirla en pantalla a distintos tamaños y corregir la composición antes de seguir con los módulos |
| Núcleo | Tabla de frases en español a intención; casos límite | |
| IA | Respuestas simuladas solo en pruebas: válida, inválida, tiempo agotado, cuota agotada, límite por minuto, clave inválida | Con clave: frases reales en español, incluidas fechas ambiguas, y capturas sin datos sensibles |
| Sistema | Contratos y simulaciones de "no disponible" | Comparar con el Administrador de tareas |
| Archivos | Carpetas temporales, rutas fuera de lo autorizado, recorrido `..` y enlaces simbólicos | Búsqueda en carpetas reales |
| Clima | Análisis de respuestas guardadas, errores y sin conexión | Consulta real |
| Recordatorios | CRUD, persistencia al reabrir, fechas límite y planificador con reloj simulado | Notificación real |
| Historial | CRUD, filtros, que no haya secretos | |
| Voz | Audio a 16 kHz, estado y cancelación | Frases reales con un micrófono |
| Captura | Reducción de imagen, que no se escriba en disco, permiso denegado | Captura y análisis real |

## 10. Complejidad estimada

| Módulo | Complejidad | Motivo principal |
|---|---|---|
| Interfaz principal | Media | Identidad propia, estados y adaptación |
| Núcleo de comandos | Media | Reglas fiables en español |
| Inteligencia artificial | Media-alta | Esquemas, validación, costes, errores |
| Consultas del sistema | Media-alta | Wi-Fi diferente en cada sistema |
| Búsqueda de archivos | Media | Seguridad de rutas, sin bloqueos |
| Clima y hora | Baja | API sencilla |
| Recordatorios | Media-alta | Fechas, planificador, bandeja y notificaciones |
| Historial | Baja | CRUD con filtros |
| Voz | Alta | Compilación nativa, audio y modelo |
| Análisis de pantalla | Alta | Permisos, Wayland y privacidad |
| Distribución multiplataforma | Alta | CI y sistemas no disponibles para prueba directa |
