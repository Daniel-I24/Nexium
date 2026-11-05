# Nexium: diseño visual

**Estado:** dirección aprobada por el propietario el 5 de noviembre de 2026, con los ajustes ya incorporados en este documento.

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

Todo el texto supera 4,5:1. `hairline` no cumple 3:1 y por eso solo separa filas: nunca es el único límite de un control.

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
| Lectura | Plex Mono | 88 px, baja hasta 48 px según el ancho | 400 |
| Respuesta destacada | Instrument Sans | 32 px | 500 |
| Solicitud y párrafo de respuesta | Instrument Sans | 18 px | 400–500 |
| Interfaz | Instrument Sans | 15 px | 400–600 |
| Notas, origen y cinta | Instrument Sans | 13 px | 400 |
| Datos en línea | Plex Mono | 12–15 px | 400–500 |

Las cifras usan coma decimal (`es`), y los tamaños se definen en `rem` para respetar el escalado del sistema.

## 4. Espacio, forma y movimiento

- **Espaciado:** base de 4 px con la escala 4, 8, 12, 16, 24, 32, 48 y 64.
- **Radio:** 2 px como máximo.
- **Sombras:** ninguna. La jerarquía se construye con tono y líneas de 1 px. La única excepción son los menús flotantes, si llegan a hacer falta.
- **Controles:** mínimo de 44 px de alto. Foco visible con contorno de 2 px y separación de 2 px.
- **Movimiento:** solo para comunicar un cambio de estado, entre 120 y 200 ms. Con `prefers-reduced-motion` se elimina.
- **Trazo de voz:** responde al nivel real del micrófono. Nunca es una animación decorativa.

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

**Iconos:** `lucide-react`, trazo 1,75 y dos tamaños (16 y 20 px), solo donde ayuden a reconocer un control. Los elementos de navegación usan texto.

## 6. Pantallas

Cada pantalla tiene una finalidad:

| Pantalla | Finalidad |
|---|---|
| **Asistente** | Pedir algo y leer la respuesta. Es la pantalla principal |
| **Recordatorios** | Ver, editar y eliminar recordatorios pendientes |
| **Historial** | Consultar, filtrar y eliminar interacciones |
| **Ajustes** | Clave y modelos de IA, carpetas autorizadas, modelo de voz, ciudad del clima |

**Atribución.** Toda respuesta de clima muestra "Datos meteorológicos de Open-Meteo (CC BY 4.0)" con enlace a su sitio, y Ajustes incluye la misma atribución.

La pantalla vacía de Asistente muestra tres ejemplos de solicitudes reales que se pueden pulsar. Es la vía de descubrimiento de funciones y sustituye a una cuadrícula de módulos.

**Adaptación al tamaño:** ventana inicial de 1100 × 720 y mínima de 720 × 520.

- Por debajo de 900 px, la navegación lateral pasa a ser una franja superior.
- La lectura reduce su tamaño hasta 48 px.
- Las filas y la barra de señal se reordenan sin desplazamiento horizontal.

## 7. Reglas de Tailwind CSS

Se usa Tailwind CSS 4 con configuración en CSS, sin `tailwind.config.js`.

1. **Una sola fuente de tokens:** `src/styles/theme.css` define el `@theme` con los tokens de esta guía. El tema oscuro solo reasigna las variables bajo `prefers-color-scheme: dark`, así que no se usa la variante `dark:` en los componentes.
2. **Solo nombres semánticos:** `bg-panel`, `text-ink`, `border-line`. Prohibidos los colores de la paleta por defecto (`bg-gray-200`).
3. **Sin valores arbitrarios** (`p-[13px]`). Si un valor falta, se justifica y se añade al tema.
4. **Radio y sombra restringidos:** el tema anula los valores por defecto para que `rounded-xl` y `shadow-lg` no estén disponibles.
5. **`@apply` casi nunca:** la reutilización se hace con componentes de React.
6. **Variantes por propiedades:** los componentes exponen `variant`, y un mapa tipado asocia cada variante con sus clases.
7. **Comprobación automática:** `scripts/check-tailwind.mjs` (parte de `npm run lint`) falla si aparecen colores de la paleta por defecto, valores arbitrarios, radios o sombras no permitidos, degradados o la variante `dark:`. Es la vía alternativa prevista: el script es más simple y no depende de la compatibilidad de un plugin.

## 8. Coherencia

- Un color, un significado: la señal no se reutiliza como acento decorativo.
- Una regla para los datos: si es una medición o una ruta, va en Plex Mono.
- Un patrón para listas: filas con líneas finas, en resultados, recordatorios e historial por igual.
- Todo estado se escribe con palabras, además de cualquier señal visual.
- Ningún elemento se añade solo por decorar: los puntos que pide evitar el documento maestro (degradados, neón, tarjetas repetidas, radios grandes, sombras) quedan descartados por las reglas de la sección 7.
