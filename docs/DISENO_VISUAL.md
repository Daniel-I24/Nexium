# Nexium: diseño visual

**Estado:** dirección aprobada por el propietario el 5 de noviembre de 2026, con los ajustes ya incorporados en este documento. Revisado el 13 de noviembre de 2026: distribución fluida, movimiento, iconos, tipografía y accesibilidad (secciones 3 a 6).

**Vista previa:** [lienzo con la pantalla principal, los estados y el sistema visual](https://claude.ai/artifact/FUcCdeDGmBACfRBA61EpC1). El enlace es privado. Todos los datos que aparecen son de ejemplo y no llegarán a la aplicación. La maqueta carga las fuentes desde Google Fonts porque el lienzo no admite otra vía; la aplicación no lo hace (sección 3).

## 1. Identidad: "Instrumento"

Nexium se presenta como un instrumento de precisión, no como un chat. La referencia es el equipo de medida bien diseñado: superficies neutras, lecturas grandes, un solo color vivo y etiquetas exactas.

Tres ideas propias sostienen la identidad:

1. **La lectura.** La respuesta actual ocupa el área principal a gran tamaño, como la pantalla de un aparato. Un "11,4 GB" se lee de un vistazo, sin burbujas de chat.
2. **La cinta.** Las respuestas anteriores se comprimen en una sola línea con hora, solicitud y resultado. La sesión sigue visible y no compite con la respuesta actual.
3. **La señal.** El amarillo significa una sola cosa: Nexium te está oyendo o va a ver tu pantalla. Cuando el micrófono se abre, toda la barra inferior se vuelve amarilla, y al cerrarse el color desaparece. Así un requisito de privacidad se convierte en el rasgo más reconocible del producto.

Cada respuesta indica además su **origen** ("Lectura directa del equipo, sin IA" o "Interpretado con IA"). Es una línea de texto que da confianza y deja ver cuándo se usa IA.

### Símbolo

Una N de tres trazos rectos, con un nodo cuadrado en el cruce de la diagonal: el *nexo*. Es un SVG propio y también sirve de icono de la aplicación.

**En reposo no lleva amarillo:** el nodo es un cuadrado hueco con el color de la tinta. Dentro de la app, el nodo solo se rellena de `signal` mientras la barra está en Escuchando o Esperando permiso de captura. El icono del sistema nunca usa amarillo.

### Direcciones descartadas

| Dirección | Idea | Por qué no |
|---|---|---|
| Bitácora | Papel cálido, serifa y registro con márgenes | Elegante, pero poco tecnológica y muy parecida a la estética editorial que ya usan muchas herramientas de IA |
| Consola mineral | Solo tema oscuro, petróleo profundo | Demasiado cerca de la plantilla "IA sobre fondo oscuro" que el documento maestro pide evitar |

## 2. Paleta

El punto de partida es el neutro: gris aluminio con un matiz verde muy leve, que da carácter de material sin parecer un gris por defecto. El único color vivo es la señal.

| Token | Claro | Oscuro | Único uso |
|---|---|---|---|
| `base` | `#DFE2DD` | `#191D1E` | Fondo de ventana y barras |
| `panel` | `#EEF0EB` | `#23282A` | Área de lectura y campos |
| `ink` | `#14181A` | `#E6E9E3` | Texto y acción principal |
| `muted` | `#4A5254` | `#A4ACAA` | Texto secundario |
| `line` | `#757D7E` | `#737B7B` | Borde de controles |
| `hairline` | `#C5CAC4` | `#3A4143` | Separadores decorativos entre filas |
| `signal` | `#FFD000` | `#FFD000` | Micrófono abierto o captura de pantalla, nada más |
| `on-signal` | `#14181A` | `#14181A` | Texto e iconos sobre señal |
| `ok` | `#16623E` | `#62C795` | Completado, el archivo existe |
| `error` | `#A8152C` | `#FF8A96` | Fallo y acción destructiva |
| `query` | `#1B4A8F` | `#8FB8F2` | Aclaración o tarea pendiente |

**Contraste medido (WCAG), sobre `base` y `panel`:**

| Color | Claro | Oscuro |
|---|---|---|
| `ink` | 13,66 y 15,57 | 13,86 y 12,16 |
| `muted` | 6,12 y 6,97 | 7,33 y 6,43 |
| `ok` | 5,63 y 6,42 | 8,19 y 7,19 |
| `error` | 5,72 y 6,52 | 7,55 y 6,62 |
| `query` | 6,63 y 7,55 | 8,34 y 7,32 |
| `line` (controles, mínimo 3:1) | 3,22 y 3,67 | 3,92 y 3,44 |
| `on-signal` sobre `signal` | 12,15 | 12,15 |

Todo el texto supera 4,5:1. `tests/contrast.test.ts` lee los colores de `src/styles/theme.css` y comprueba en los dos temas el texto (4,5:1 sobre `base` y `panel`), los botones con fondo de color, y los bordes de control y el foco (3:1). `hairline` no cumple 3:1 y por eso solo separa filas: nunca es el único límite de un control.

**Reglas que se derivan de la medición:**

- En tema claro, `signal` apenas se distingue del fondo (1,13:1 sobre `base`). Siempre lleva borde de tinta.
- Ningún estado se comunica solo con color: siempre hay una palabra y, cuando hace falta, un icono.
- La aplicación sigue el tema del sistema. No se añade un selector de tema.

## 3. Tipografía

Las dos familias tienen licencia OFL y **viajan dentro de la aplicación** (`@fontsource-variable/instrument-sans` y `@fontsource/ibm-plex-mono`). No se cargan desde Google Fonts en tiempo de ejecución: la política de seguridad de contenido (`font-src 'self'`) lo impediría aunque alguien lo intentara, y una prueba automática comprueba que ningún archivo de `src/` ni `index.html` apunte a un servicio de fuentes.

- **Instrument Sans:** todo lo que se lee como lenguaje.
- **IBM Plex Mono:** todo lo que es un dato exacto, como cifras, horas y rutas. La diferencia de letra le dice al usuario qué viene directamente de una medición.

| Papel | Familia | Tamaño | Peso |
|---|---|---|---|
| Papel | Token | Tamaño con el texto al 100 % | Interlineado | Peso |
|---|---|---|---|---|
| Lectura | `text-reading` | 88 px, baja hasta 48 px según el ancho | 1 | 400 |
| Título de pantalla | `text-xl` | 32 px, baja hasta 26 px en la ventana mínima | 1,15 | 500 |
| Solicitud, párrafo de respuesta, título de sección | `text-lg` | 18 px | 1,4 | 400–600 |
| Interfaz | `text-md` | 15 px | 1,5 | 400–600 |
| Notas, origen, cinta y estados | `text-sm` | 13 px | 1,45 | 400–500 |
| Datos en línea y rutas | `text-xs` a `text-md`, Plex Mono | 12–15 px | 1,45–1,5 | 400–500 |

Cada token de tamaño lleva su interlineado (`--text-*--line-height`): más abierto en el texto pequeño y más cerrado en los títulos, así que los componentes no fijan `leading-*`. Hay tres niveles de jerarquía visibles en cualquier ventana: título de pantalla, título de sección o solicitud, y texto. Las cifras usan coma decimal (`es`), y los tamaños se definen en `rem` para respetar el tamaño de texto del sistema.

## 4. Espacio, forma y movimiento

- **Espaciado:** base de 4 px con la escala 4, 8, 12, 16, 24, 32, 48 y 64.
- **Radio:** 2 px como máximo.
- **Sombras:** ninguna. La jerarquía se construye con tono y líneas de 1 px. La única excepción son los menús flotantes, si llegan a hacer falta.
- **Controles:** mínimo de 44 px de alto. Foco visible con contorno de 2 px y separación de 2 px.
- **Trazo de voz:** responde al nivel real del micrófono. Nunca es una animación decorativa.

### Movimiento

Cada animación tiene una finalidad: indicar un cambio de estado, orientar (dónde apareció algo nuevo) o confirmar una acción. **Solo se animan `opacity` y `transform`**, que el navegador compone sin recalcular el diseño, así que es fluido también en equipos modestos. No hay rebotes, resortes ni efectos decorativos. `scripts/check-tailwind.mjs` rechaza `transition-colors`, `transition-all`, duraciones numéricas y curvas fuera de los tokens.

| Token | Valor | Uso |
|---|---|---|
| `duration-fast` | 120 ms | Cambios en el mismo sitio: estado de la barra, señal amarilla |
| `duration-base` | 160 ms | Contenido nuevo: respuestas, avisos, diálogos |
| `duration-slow` | 200 ms | Cambio de pantalla |
| `ease-instrument` | `cubic-bezier(0.2, 0, 0, 1)` | Cambios en el sitio y progreso |
| `ease-enter` | `cubic-bezier(0, 0, 0, 1)` | Entradas: llegan rápido y se asientan |

| Animación | Dónde | Qué hace | Para qué |
|---|---|---|---|
| `animate-section` | Cambio de pantalla | Aparece y sube 4 px en 200 ms | Orienta: cambió toda el área |
| `animate-enter` | Respuesta nueva, aviso, estado vacío | Aparece y sube 4 px en 160 ms | Señala dónde está lo nuevo |
| `animate-fade` | Contenido de cada estado de la barra, fondo del diálogo | Fundido de 120 ms | Indica el cambio de estado sin moverse |
| Capa `signal` | Barra al escuchar o pedir permiso de captura | La capa amarilla se enciende y apaga por opacidad | Un cambio de privacidad que no debe pasar inadvertido |
| `animate-dialog` | Diálogo de confirmación | Aparece desde el 98 % en 160 ms | Centra la atención en la decisión |
| `animate-sweep` | Procesando | Progreso indeterminado continuo | Confirma que Nexium sigue trabajando |
| Trazo de voz | Escuchando | Cada barra se escala en vertical (`scaleY`) | Muestra el nivel real del micrófono |

**Reducir movimiento.** Con `prefers-reduced-motion: reduce` (en Windows, Accesibilidad > Efectos visuales > Efectos de animación desactivado) se eliminan todas las animaciones y transiciones. Los estados cambian igual, pero de golpe; el progreso de Procesando queda quieto y el texto del estado indica lo que pasa. El trazo de voz sigue reflejando el nivel, porque es información, no adorno.

## 5. Componentes

| Componente | Comportamiento |
|---|---|
| **Barra de señal** | Zona inferior fija: botón de voz, campo de texto, Analizar pantalla y estado escrito |
| **Botón de voz** | 136 × 64 px. Cambia de "Hablar" a "Terminar" y, tras un error, a "Reintentar" |
| **Lectura** | Cifra grande, unidad y contexto. Para valores numéricos del sistema y del clima |
| **Escala** | Barra plana con marcas 0, 25, 50, 75 y 100 |
| **Filas de datos** | Etiqueta y valor separados por una línea fina. Sustituyen a las tarjetas |
| **Resultado de archivo** | Nombre, extensión, ubicación, ruta absoluta y estado de existencia |
| **Aclaración** | Recuadro con borde `query`, una pregunta y opciones como botones |
| **Línea de cinta** | Hora, solicitud y resultado resumido |
| **Fila de recordatorio e historial** | Misma estructura de fila, con estado escrito y acciones |
| **Botones** | Primario (tinta), secundario (línea), destructivo (error), señal |

### Estados de la barra de señal

Hay siete estados. Todos se definen en ambos temas y se escriben siempre con palabras.

| Estado | Barra | Texto de estado | Acción principal | Cancelar |
|---|---|---|---|---|
| En espera | `base`, línea superior `line` | `muted` | Hablar | No |
| Escuchando | `signal`, bordes `on-signal`, trazo del micrófono | `on-signal` | Terminar | Sí |
| Procesando | `base`, progreso en movimiento y paso concreto ("Transcribiendo") | `muted` | Inactiva | Sí |
| Respondiendo | `base`, línea de progreso llena, "Escribiendo la respuesta" | `ink` | Inactiva | Sí |
| Esperando permiso (captura) | `signal`, aviso con el destino de la imagen | `on-signal` | Capturar | Sí |
| Pendiente (aclaración) | `base`, línea superior `query`, pregunta y opciones | `query` | Opciones | Sí |
| Error | `base`, línea superior `error`, causa y siguiente paso | `error` | Reintentar | No |

**Aviso de captura.** El destino de la imagen se toma del proveedor configurado, nunca está escrito a mano en la interfaz. Con Gemini dice:

> Se tomará una sola imagen de tu pantalla y se enviará a Google Gemini para analizarla. En la capa gratuita, Google puede usar lo que envías para mejorar sus productos, y personas pueden revisarlo. No captures información personal ni confidencial. Nexium no guarda la imagen.

### Contraste medido de los estados

| Par de colores | Claro | Oscuro |
|---|---|---|
| Escuchando y captura: `on-signal` sobre `signal` (texto, trazo, borde) | 12,15 | 12,15 |
| Botón Terminar y Capturar: `signal` sobre `on-signal` | 12,15 | 12,15 |
| Barra `signal` frente a `base` | 1,13, por eso lleva borde | 11,55 |
| Procesando y Respondiendo: `ink` sobre `base` | 13,66 | 13,86 |
| Progreso `ink` sobre `hairline` | 10,74 | 8,49 |
| Error: `error` sobre `base` (texto y línea) | 5,72 | 7,55 |
| Botón Reintentar: `panel` sobre `ink` | 15,57 | 12,16 |
| Aclaración: `query` sobre `panel` | 7,55 | 7,32 |
| Aclaración sobre `base`: `query` | 6,63 | 8,34 |
| Opción: borde `line` sobre `panel` | 3,67 | 3,44 |

Todo el texto supera 4,5:1 y los límites de control superan 3:1 en ambos temas. La única excepción medida es `signal` sobre `base` claro, resuelta con el borde obligatorio.

**Iconos:** un único conjunto, Lucide (de trazo), a través de `src/components/Icon.tsx`, que es el único archivo que importa `lucide-react` (lo comprueba `check-tailwind.mjs`). Grosor 1,75 con `absoluteStrokeWidth`, igual en los dos tamaños: `sm` (16 px) junto a texto de interfaz y estados, y `md` (20 px) en los botones principales. Los tamaños van en `rem` (`size-4`, `size-5`) y crecen con el texto. Un icono acompaña siempre a una palabra y se oculta a los lectores de pantalla; solo lleva nombre propio cuando es lo único que transmite algo (el fallo en la cinta). Los elementos de navegación usan texto.

| Icono | Nombre | Uso |
|---|---|---|
| Micrófono | `mic` | Hablar |
| Escáner | `screen` | Analizar pantalla |
| Círculo con marca | `completed` | Completada, Existe, Disponible, Guardado, avisos correctos |
| Círculo con aspa | `failed` | Fallida, Ya no existe, errores |
| Reloj | `pending` | Pendiente |
| Círculo con exclamación | `overdue` | Vencido |
| Círculo tachado | `cancelled` | Cancelada |
| Campana | `reminder` | Estado vacío de Recordatorios |

**Estados con icono, palabra y color.** Ningún estado se distingue solo por el color (`StatusLabel`):

| Estado | Icono | Color | Además |
|---|---|---|---|
| Completada | `completed` | `ok` | |
| Fallida | `failed` | `error` | En la cinta, el icono se lee como "Fallida" |
| Pendiente | `pending` | `query` | |
| Vencido | `overdue` | `error` | Línea lateral `error` de 2 px en la fila |
| Cancelada | `cancelled` | `muted` | |

**Diálogo de confirmación** (`ConfirmDialog`). Antes de Borrar todo en Historial y de Eliminar un recordatorio. Usa el `<dialog>` modal del sistema: el resto de la ventana queda inerte, Escape cancela, el foco empieza en Cancelar para que Intro no borre por accidente y vuelve al botón que lo abrió. El título pregunta y la descripción nombra lo que se borra (el recordatorio con su fecha, o cuántas entradas; con un filtro activo, avisa de que se borra también lo que no se ve) y dice que no se puede deshacer. Fondo `panel`, borde `line`, fondo de ventana atenuado y botón destructivo en `error`.

**Avisos** (`Notice`). Resultado de una acción o fallo de una pantalla: icono, texto y color, `role="status"` o `role="alert"`, y entrada con `animate-enter`.

**Foco.** Contorno de 2 px con separación de 2 px, siempre con `ink` (no con el color del texto, que en un botón primario apenas se vería) y con `on-signal` sobre la barra amarilla. Cuando cambia el estado de la barra y el foco estaba en ella, pasa al control principal del estado nuevo (Terminar, Cancelar, Capturar, la primera opción, Reintentar o el campo de texto), en lugar de perderse.

## 6. Pantallas

Cada pantalla tiene una finalidad:

| Pantalla | Finalidad |
|---|---|
| **Asistente** | Pedir algo y leer la respuesta. Es la pantalla principal |
| **Recordatorios** | Ver, editar y eliminar recordatorios pendientes |
| **Historial** | Consultar, filtrar y eliminar interacciones |
| **Ajustes** | Estado y límite de la IA, carpetas autorizadas, modelo de voz y micrófono, ciudad del clima. Guardar va en un pie fijo, siempre visible, con su resultado al lado |

**Atribución.** Toda respuesta de clima muestra "Datos meteorológicos de Open-Meteo (CC BY 4.0)" con enlace a su sitio, y Ajustes incluye la misma atribución.

La pantalla vacía de Asistente muestra tres ejemplos de solicitudes reales que se pueden pulsar. Es la vía de descubrimiento de funciones y sustituye a una cuadrícula de módulos.

**Estados vacíos y de carga.** Cada lista dice que está cargando mientras llega, y nunca "vacío" antes de tiempo. Recordatorios sin elementos explica cómo crear uno y ofrece un ejemplo que se puede pulsar; Historial vacío lo dice según el filtro.

### Adaptación a cualquier pantalla

Nexium no se diseña para las medidas de un equipo concreto. La interfaz se adapta a tres cosas a la vez: el tamaño de la ventana, la escala de pantalla del sistema y el tamaño de texto que elija la persona.

- **Unidades relativas.** Tamaños de letra, espaciado, anchos máximos y puntos de corte en `rem`. Con el texto del sistema al 200 %, el punto de corte `wide` (56,25 rem) pasa de 900 a 1800 px: la composición estrecha se mantiene hasta que de verdad cabe la ancha.
- **Composición.** Navegación arriba (estrecha) o a la izquierda (ancha); a la derecha, una rejilla con el área de lectura (mínimo 10 rem, desplazamiento propio) y la barra de señal (como mucho media ventana, con desplazamiento interno). Si ni así cabe todo, se desplaza la ventana entera: ningún control queda cortado ni fuera de alcance.
- **Contenedores.** Contenido centrado con ancho máximo (`max-w-content`, 60 rem) y texto a `max-w-reading` (40 rem). Filas con `flex-wrap` y `gap`, y elementos con `min-w-0` y una base flexible (`basis-48`, `basis-60`) en lugar de anchos fijos. Rutas y palabras largas se parten.
- **Fecha y hora** en dos campos: un solo campo de fecha y hora no cabe en una ventana estrecha con el texto ampliado.
- **Desplazamiento.** Cada pantalla empieza arriba al cambiar de sección, y el área principal reserva espacio (`scroll-pb-32`) para que un campo nunca quede bajo el pie fijo de Ajustes al llegar a él con el teclado.

**Tamaño de la ventana.** Preferido de 1100 × 720 y mínimo de **640 × 400**, en píxeles lógicos (`tauri.conf.json`). El mínimo debe caber en el área de trabajo de las pantallas pequeñas habituales: 1280 × 720 al 150 % deja 853 × 432 lógicos con la barra de tareas, y 1366 × 768 al 125 %, 1093 × 576. Por debajo de 640 de ancho, la barra de señal tendría que apilar cada control en su propia fila; por debajo de 400 de alto, el área de lectura quedaría por debajo de 10 rem. Al abrirse, si el tamaño preferido no cabe, la ventana ocupa el 90 % del área de trabajo sin bajar del mínimo (`src-tauri/src/platform/window_size.rs`).

**Pruebas de diseño** (`npm run test:layout`, `tests/layout`). Playwright monta la app completa con un backend simulado y datos que fuerzan el diseño (nombres, rutas y textos largos). Recorre 20 vistas (las cuatro pantallas, los diálogos, Editar y los siete estados de la barra) en:

- 6 tamaños de ventana: mínima (640 × 400), 800 × 600, inicial (1100 × 720), 1366 × 768 y maximizada en 1080p y 1440p;
- 3 tamaños de texto del sistema: 100, 150 y 200 %, cambiando la letra predeterminada del navegador, como hace el sistema;
- 6 proyectos: escalas de pantalla de 100, 125, 150, 175 y 200 %, y los dos temas.

Fallan si hay desplazamiento horizontal, un control que no queda entero en la ventana tras desplazarse hasta él, uno cortado por un contenedor o tapado por otro elemento, o un texto que se sale de su caja. En Windows usan Edge, el mismo motor que WebView2; en Linux, Chromium. Con `NEXIUM_SHOTS=1` guardan una captura de cada combinación en `test-results/capturas`. Lo que no cubren (WebKit en macOS y Linux, la escala real del sistema con la ventana nativa) queda para la prueba manual.

## 7. Reglas de Tailwind CSS

Se usa Tailwind CSS 4 con configuración en CSS, sin `tailwind.config.js`.

1. **Una sola fuente de tokens:** `src/styles/theme.css` define el `@theme` con los tokens de esta guía. El tema oscuro solo reasigna las variables bajo `prefers-color-scheme: dark`, así que no se usa la variante `dark:` en los componentes.
2. **Solo nombres semánticos:** `bg-panel`, `text-ink`, `border-line`. Prohibidos los colores de la paleta por defecto (`bg-gray-200`).
3. **Sin valores arbitrarios** (`p-[13px]`). Si un valor falta, se justifica y se añade al tema.
4. **Radio y sombra restringidos:** el tema anula los valores por defecto para que `rounded-xl` y `shadow-lg` no estén disponibles.
5. **`@apply` casi nunca:** la reutilización se hace con componentes de React.
6. **Variantes por propiedades:** los componentes exponen `variant`, y un mapa tipado asocia cada variante con sus clases.
7. **Comprobación automática:** `scripts/check-tailwind.mjs` (parte de `npm run lint`) falla si aparecen colores de la paleta por defecto, valores arbitrarios, radios o sombras no permitidos, degradados, la variante `dark:`, transiciones de algo que no sea opacidad o transformación, duraciones o curvas fuera de los tokens, o iconos importados fuera de `Icon.tsx`. Lo que no se puede expresar con utilidades del tema (la rejilla de la columna, el alto máximo de la barra, el foco sobre la señal) se define como `@utility` en `theme.css`. Es la vía alternativa prevista: el script es más simple y no depende de la compatibilidad de un plugin.

## 8. Coherencia

- Un color, un significado: la señal no se reutiliza como acento decorativo.
- Una regla para los datos: si es una medición o una ruta, va en Plex Mono.
- Un patrón para listas: filas con líneas finas, en resultados, recordatorios e historial por igual.
- Todo estado se escribe con palabras, además de cualquier señal visual.
- Ningún elemento se añade solo por decorar: los puntos que pide evitar el documento maestro (degradados, neón, tarjetas repetidas, radios grandes, sombras) quedan descartados por las reglas de la sección 7.
