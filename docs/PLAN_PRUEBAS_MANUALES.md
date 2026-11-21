# Nexium: pruebas manuales pendientes

Estas pruebas requieren un equipo real o un dato del propietario. Marca el resultado y, si algo falla, anota el sistema y el mensaje.

## En todos los sistemas

Instalador: descárgalo de la última ejecución de [CI](https://github.com/Daniel-I24/Nexium/actions), en "Artifacts".

| # | Prueba | Resultado esperado |
|---|---|---|
| 1 | Instalar y abrir | Ventana con el símbolo de Nexium, sin amarillo |
| 2 | Cambiar el sistema a modo oscuro y claro | La app cambia de tema |
| 3 | Escribir "¿Qué hora es?" | Hora correcta, con "sin IA" como origen |
| 4 | "¿Cuánta RAM estoy utilizando?" | Coincide con el monitor del sistema |
| 5 | "¿Cuánta batería tengo?" | Porcentaje real, o "No disponible" en un equipo sin batería |
| 6 | "¿Estoy conectado a Internet?" con y sin red | Refleja el estado real |
| 7 | "¿A qué red Wi-Fi estoy conectado?" | Nombre real, o "No disponible" con el motivo |
| 8 | "¿Existe el archivo proyecto.pdf?" | Pide autorización de carpetas antes de buscar |
| 9 | En Ajustes, descargar el modelo de voz | Progreso visible y el modelo queda disponible |
| 10 | Pulsar Hablar | La barra pasa a amarillo; el sistema puede pedir permiso de micrófono |
| 11 | Hablar y pulsar Terminar | La frase se transcribe y se ejecuta |
| 12 | Pulsar Cancelar mientras escucha | La barra sale del amarillo y no se transcribe nada |
| 13 | Abrir Ajustes en una instalación nueva | No pide ninguna clave. Muestra el estado del servicio de IA y el aviso de privacidad |
| 14 | "Recuérdame probar Nexium hoy a las HH:MM", dos minutos más tarde | Pide confirmar, y llega la notificación a esa hora |
| 15 | Cerrar la ventana con el recordatorio pendiente | La app sigue en la bandeja y el aviso llega |
| 16 | Salir desde la bandeja y volver a abrir tras la hora | El recordatorio sigue guardado y aparece como vencido |
| 17 | Analizar pantalla con una ventana sin datos personales | Aviso nombrando a Google Gemini antes de capturar; responde sobre esa ventana |
| 18 | Revisar el Historial | Aparecen las solicitudes con su estado escrito y con icono, y se pueden filtrar y eliminar |

## Prueba manual de lo corregido (21 de noviembre de 2026, app instalada)

Desinstala la versión anterior e instala la nueva con el `.exe` de `src-tauri\target\release\bundle\nsis` que genera `compilar-distribucion.cmd`. Anota **Correcto**, **Falla** (con lo que viste) o **No probado**.

### H1. Voz: cuatro frases y sus tiempos

Pulsa Hablar, di cada frase y pulsa Terminar. Después abre `%APPDATA%\io.github.daniel-i24.nexium\rendimiento.log` y copia la última línea de cada una.

| Frase | Transcripción que muestra Nexium | Transcripción (ms) | Total (ms) | Resultado |
|---|---|---|---|---|
| «¿Cuánta batería tengo?» | | | | |
| «¿Cuánta RAM estoy usando?» | | | | |
| «¿Qué hora es?» | | | | |
| «¿A qué red wifi estoy conectado?» | | | | |

Esperado: cada transcripción por debajo de 2 000 ms y cada total por debajo de 3 000 ms; las cuatro resueltas «sin IA».

### H2. Resumen de un documento

Escribe «Resume mi documento de prueba» (con `documento-prueba.pdf` en una carpeta autorizada).

| Comprobación | Esperado | Resultado |
|---|---|---|
| Candidatos | Solo documentos cuyo nombre coincide; ningún `.txt` que solo mencione «prueba» por dentro | |
| Preselección | Si hay uno claro, viene marcado | |
| Resumen | De 2 a 3 frases | |
| Ideas clave | De 5 a 8, cada una con su nombre en negrita y una explicación breve | |
| Datos importantes | Fechas, cifras o definiciones del documento, si las tiene | |

### H3. Historial con una sola entrada por solicitud

1. En Ajustes, quita las carpetas autorizadas. Escribe «Resume mi documento de prueba», permite Documentos y Escritorio y confirma el resumen.
2. Borra la ciudad para el clima, escribe «Dame el clima en la ciudad en la que me encuentro» y da una ciudad.
3. Escribe otra solicitud que pida permiso y pulsa Cancelar.

| Comprobación | Esperado | Resultado |
|---|---|---|
| Historial, paso 1 | Una sola entrada, «Resume mi documento de prueba», Completada | |
| Historial, paso 2 | Una sola entrada, la solicitud original, Completada; ninguna «Falta la ciudad» pendiente | |
| Historial, paso 3 | Una sola entrada, Cancelada | |
| Conversación | La respuesta final sustituye a la pregunta pendiente: la cinta no repite la solicitud | |

### H4. La notificación abre su recordatorio

Crea «Recuérdame tomar agua dentro de dos minutos» y deja Nexium en la bandeja.

| Comprobación | Esperado | Resultado |
|---|---|---|
| Título | «💧 Recordatorio de Nexium» | |
| Abrir Nexium | La ventana se abre en Recordatorios, con ese recordatorio a la vista, con el foco y un destello amarillo breve | |
| Otro emoji | Repite con «Recuérdame estudiar para el parcial…»: «📚 Recordatorio de Nexium» | |

## Prueba manual de lo corregido (15 de noviembre de 2026, app instalada)

Instala con el `.exe` de `src-tauri\target\release\bundle\nsis` que genera `compilar-distribucion.cmd`. Para cada caso anota **Correcto**, **Falla** (con lo que viste) o **No probado**.

**Equipo:** resolución ____ · escala ____ % · tamaño de texto ____ % · tema ____

### G1. Marco de la app al reducir la ventana

Reduce la ventana al mínimo y ve agrandándola, en Asistente, Recordatorios, Historial y Ajustes. Repite con el tamaño de texto de Windows al 150 %.

| Comprobación | Esperado | Resultado |
|---|---|---|
| La ventana | Nunca aparece una barra de desplazamiento de la ventana entera: solo se desplaza el contenido de cada pantalla | |
| Barra inferior | Hablar, el campo de texto y Analizar pantalla se ven enteros en cualquier tamaño. En ventanas estrechas, Hablar y Analizar pantalla muestran solo su icono (al pasar el ratón dicen su nombre) | |
| Navegación | Las cuatro secciones siempre visibles a la izquierda; en estrecho, solo los iconos | |
| Guardar ajustes | Siempre visible al pie de Ajustes, fuera de lo que se desplaza. Al guardar, el botón dice «Ajustes guardados» hasta el siguiente cambio | |

### G2. Animaciones

Con los efectos de animación de Windows activados (Accesibilidad > Efectos visuales):

| Comprobación | Esperado | Resultado |
|---|---|---|
| Cambio de sección | La sección anterior se desvanece y la nueva entra subiendo un poco, sin un instante en blanco | |
| Respuesta nueva | La anterior pasa a la cinta (línea que sube desde abajo) y la nueva aparece justo después | |
| Barra | Al pulsar Hablar, el amarillo se enciende con un fundido; el contenido de cada estado aparece sin saltos | |
| Voz | Mientras hablas, barras simétricas sobre una línea central que se mueven de forma fluida con tu voz | |
| Diálogos | Borrar todo: el fondo se oscurece mientras el panel entra; al cancelar, los dos salen antes de cerrarse | |
| Listas | Al eliminar un recordatorio o una entrada del historial, la fila se aparta y se desvanece | |

Desactiva los efectos de animación, reinicia Nexium y repite: todo debe cambiar al instante, sin movimiento.

### G3. Voz y tiempos

1. Pulsa Hablar y di «¿Cuánta batería tengo?». Repite con «¿Cuánta RAM estoy usando?», «¿A qué red wifi estoy conectado?» y «Abre los recordatorios».
2. Abre `%APPDATA%\io.github.daniel-i24.nexium\rendimiento.log` y anota la última línea de cada orden.

| Comprobación | Esperado | Resultado |
|---|---|---|
| Micrófono | En Ajustes, con Automático, «Nexium usará …» nombra tu micrófono real, nunca Stereo Mix ni Mezcla estéreo | |
| Transcripción | «RAM» y «Wi-Fi» se escriben bien | |
| Origen | «lectura directa del equipo, sin IA» en las tres primeras; la cuarta abre Recordatorios | |
| Tiempo total tras soltar Terminar | Menos de 3 s (la línea de `rendimiento.log` dice el total y cada etapa) | ____ s |
| Primera orden tras abrir Nexium | «carga del modelo» cerca de 0 ms si hablaste más de un segundo | ____ ms |

### G4. Permisos de carpetas y resumen de un documento

Copia `C:\Nexium\bench-audio\documento-prueba.pdf` a tu carpeta Documentos con el nombre `Guía de bases de datos.pdf`. En Ajustes, quita todas las carpetas autorizadas.

1. Escribe «Resume mi documento de bases de datos».
2. Pulsa **Permitir Documentos y Escritorio**.
3. Pulsa **Resumir «Guía de bases de datos.pdf»**.
4. Escribe «Analiza este archivo», pulsa **Elegir archivo** y elige el mismo PDF.

| Comprobación | Esperado | Resultado |
|---|---|---|
| Permiso | Aparece en la conversación con «Permitir Documentos y Escritorio», «Elegir otra carpeta» y «Cancelar» | |
| Continuación | Al permitir, la búsqueda sigue sola, sin repetir la solicitud | |
| Confirmación | Nombra el archivo y dice que su texto irá a Google Gemini, como mucho 30.000 caracteres | |
| Resumen | Una frase de resumen e «Ideas clave» numeradas, de tú y sin Markdown | |
| Historial | La entrada se llama «Resumen de «Guía de bases de datos.pdf»» y no contiene el texto del documento | |
| Selector | «Elegir archivo» abre el selector de Windows, que solo muestra .txt, .md, .csv, .pdf y .docx | |
| Ajustes | Añadir carpeta abre el selector de carpetas; Quitar la retira al momento | |
| Un PDF escaneado, si tienes uno | Nexium dice que el PDF parece escaneado y no envía nada | |

### G5. Clima sin ciudad y con ciudad

1. En Ajustes, borra la ciudad para el clima y guarda.
2. Escribe «Dame el clima en la ciudad en la que me encuentro».
3. Escribe una ciudad en minúsculas y sin tildes (por ejemplo, «medellin»), deja marcado «Guardarla como ciudad para el clima» y pulsa Consultar el clima.
4. Vuelve a escribir la solicitud del paso 2.

| Comprobación | Esperado | Resultado |
|---|---|---|
| Paso 2 | Nexium pregunta «¿De qué ciudad quieres el clima?»; no busca una ciudad llamada «la ciudad en la que me encuentro» | |
| Paso 3 | Muestra el clima con el nombre oficial («Medellín, Colombia») | |
| Ajustes | La ciudad para el clima es «Medellín» | |
| Paso 4 | Responde con Medellín sin preguntar | |
| Ciudad inexistente | Escribir «Nolugarx» en Ajustes y guardar da «No encontré la ciudad…» | |

### G6. Borrar todo

Con varias respuestas en la conversación del Asistente, ve a Historial y pulsa Borrar todo.

| Comprobación | Esperado | Resultado |
|---|---|---|
| Diálogo | Dice que también se vacía la conversación del Asistente | |
| Resultado | Al volver a Asistente, no queda ninguna respuesta ni línea en la cinta | |

### G7. Notificaciones

Crea un recordatorio para dentro de dos minutos y deja Nexium en la bandeja.

| Comprobación | Esperado | Resultado |
|---|---|---|
| Aviso | Aparece en pantalla, desplegado, firmado por Nexium, con el botón «Abrir Nexium», y no desaparece solo | |
| Botón | «Abrir Nexium» muestra la ventana | |
| Si solo va al centro de notificaciones | Revisa lo que indica la pantalla de Recordatorios (banners de Nexium y No molestar) y anota qué ajuste estaba activo | |

## Prueba manual final (Windows, app instalada)

Instala con el `.exe` de `src-tauri\target\release\bundle\nsis` generado por `compilar-distribucion.cmd`. Anota para cada caso **Correcto**, **Falla** (con lo que viste) o **No probado**, y el equipo: resolución, escala de pantalla y tamaño de texto de Windows.

**Equipo:** resolución ____ · escala ____ % · tamaño de texto ____ % · tema ____

### F1. Análisis de pantalla con `captura-prueba.jpg`

1. Abre `C:\Nexium\bench-audio\captura-prueba.jpg` maximizada en el visor de fotos, sin otras ventanas delante.
2. En Nexium, pulsa Analizar pantalla. Antes de capturar, comprueba el aviso: una frase que nombra a Google Gemini, y "Más información" con el aviso completo. Pulsa Capturar.

| Comprobación | Esperado | Resultado |
|---|---|---|
| Nexium se oculta durante la captura y vuelve a mostrarse | Sí, sin quedarse oculto | |
| Formato | Un resumen en negrita, "Causa probable" y "Qué hacer" con pasos numerados. Sin asteriscos, almohadillas ni viñetas de Markdown | |
| Contenido | Habla de lo que muestra la imagen (el error y su código), sin pasos inventados | |
| Trato | De tú ("Comprueba…", "Abre…"), nunca de usted | |
| Origen | "Origen: una captura analizada con IA (Google Gemini). Nexium ya descartó la imagen." | |
| Historial | La solicitud aparece como Completada, con icono | |

### F2. Notificación de un recordatorio

1. Escribe "Recuérdame probar Nexium hoy a las HH:MM", con una hora dos minutos más tarde, y guárdalo.
2. Espera con Nexium abierto (o en la bandeja).

| Comprobación | Esperado | Resultado |
|---|---|---|
| Notificación | Llega a la hora, con el título "Recordatorio de Nexium" y el texto del recordatorio | |
| Nombre de la app | La notificación la firma **Nexium**, no PowerShell ni otra app | |
| Recordatorios | El recordatorio pasa a Vencido: icono, palabra en rojo y línea lateral | |

### F3. Aviso de la bandeja la primera vez

Con una instalación nueva (o tras borrar `%APPDATA%\io.github.daniel-i24.nexium`):

| Comprobación | Esperado | Resultado |
|---|---|---|
| Cerrar la ventana con la X por primera vez | La ventana se oculta y llega la notificación "Nexium sigue activo", que explica que sigue en la bandeja y que se sale con clic derecho en su icono > Salir | |
| Abrir desde la bandeja y cerrar otra vez | Se oculta sin ningún aviso | |
| Cerrar Nexium y volver a abrirlo; cerrar la ventana | Sin aviso: no se repite tras reiniciar | |
| Clic derecho en el icono > Salir | Nexium se cierra del todo (no queda en el Administrador de tareas) | |

### F4. Redimensionado en todas las pantallas

En Asistente (con una respuesta y con la conversación vacía), Recordatorios, Historial y Ajustes:

| Comprobación | Esperado | Resultado |
|---|---|---|
| Al abrir | Si 1100 × 720 no cabe, la ventana se abre más pequeña y centrada | |
| Ventana mínima (arrastra un borde hasta que no encoja más) | Nada cortado ni superpuesto. Sin barra de desplazamiento horizontal. Hablar, el campo de texto y Analizar pantalla a la vista | |
| Tamaño mediano y maximizada | El contenido se centra con un ancho máximo; la barra de señal ocupa todo el ancho | |
| Ajustes, en cada tamaño | "Guardar ajustes" siempre visible abajo, y "Ajustes guardados." al lado al pulsarlo | |
| Escala de pantalla 150 % y 200 % (Configuración > Pantalla) | Igual que al 100 %, con el texto más grande | |
| Tamaño de texto 150 % (Accesibilidad > Tamaño del texto) | Si la app lo sigue, nada se corta; con la ventana mínima puede desplazarse la ventana entera | |
| Tema claro y oscuro del sistema | La app cambia de tema; el texto se lee bien en los dos | |

### F5. Voz con el micrófono elegido en Ajustes

1. En Ajustes > Voz, elige el micrófono que vas a usar y pulsa Guardar ajustes.
2. Comprueba que "Nexium usará: …" nombra ese micrófono.
3. Pulsa Hablar, di "¿Qué hora es?" y pulsa Terminar.

| Comprobación | Esperado | Resultado |
|---|---|---|
| Escuchando | La barra se vuelve amarilla con un fundido; el trazo se mueve con tu voz | |
| Resultado | La frase se transcribe y responde la hora | |
| Cancelar mientras escuchas | La barra vuelve a En espera y no se transcribe nada | |
| Micrófono desconectado | Ajustes avisa de que el elegido no está conectado y cuál usará | |
| Un fallo de voz | Aparece en la conversación y en Historial como Fallida, con la causa | |

### F6. Confirmaciones de borrado

| Comprobación | Esperado | Resultado |
|---|---|---|
| Historial > Borrar todo | Diálogo "¿Borrar todo el historial?" que dice cuántas entradas se borrarán y que no se puede deshacer | |
| Con el filtro Fallidas activo > Borrar todo | Avisa de que también se borra lo que no muestra el filtro | |
| Teclado en el diálogo | El foco empieza en Cancelar; Tab recorre solo los botones del diálogo; Escape lo cierra sin borrar; al cerrar, el foco vuelve a Borrar todo | |
| Recordatorios > Eliminar | Diálogo que nombra el recordatorio y su fecha; Cancelar no borra, Eliminar sí | |

### F7. Animaciones con y sin Reducir movimiento

Primero con los efectos de animación de Windows activados (Accesibilidad > Efectos visuales > Efectos de animación):

| Comprobación | Esperado | Resultado |
|---|---|---|
| Cambiar de pantalla | El contenido aparece con un fundido breve y sube un poco | |
| Respuesta nueva | Aparece con el mismo movimiento breve | |
| Barra: Hablar, Terminar, Procesando, error | Cada cambio de estado se funde; la señal amarilla se enciende y se apaga con un fundido; en Procesando, una línea se desliza | |
| Diálogo de confirmación | Aparece con un fundido y una leve escala; el fondo se oscurece | |
| Ningún movimiento estorba | Nada rebota ni retrasa el uso | |

Después, desactiva Efectos de animación y reinicia Nexium:

| Comprobación | Esperado | Resultado |
|---|---|---|
| Lo mismo que arriba | Todo cambia de golpe, sin fundidos ni desplazamientos; los estados se siguen viendo y leyendo igual | |
| Procesando | La línea queda quieta; el texto indica el paso | |
| Escuchando | El trazo del micrófono sigue mostrando el nivel | |

## Solo en Windows

| # | Prueba | Resultado esperado |
|---|---|---|
| W1 | Instalador sin firmar | SmartScreen muestra un aviso: "Más información" y "Ejecutar de todas formas" |
| W2 | Desactivar "Permitir que las aplicaciones de escritorio accedan a tu ubicación" y preguntar por la red Wi-Fi | "No disponible", con el motivo. No debe fallar |
| W3 | Notificación con la app instalada | Aparece con el nombre Nexium, no PowerShell |

## Solo en macOS

| # | Prueba | Resultado esperado |
|---|---|---|
| M1 | Abrir el `.dmg` sin firmar | Gatekeeper bloquea la apertura normal; Ajustes del Sistema permite abrirla |
| M2 | Pulsar Hablar | Pide permiso de micrófono con el texto de Nexium |
| M3 | Analizar pantalla | Pide permiso de Grabación de pantalla. Sin firma, el permiso puede no conservarse |
| M4 | Preguntar por la red Wi-Fi | Nombre real, o "No disponible" si macOS lo oculta |

## Solo en Linux

| # | Prueba | Resultado esperado |
|---|---|---|
| L1 | Analizar pantalla en Wayland | Diálogo del escritorio; la captura pasa por el portal |
| L2 | Analizar pantalla en X11 | Captura directa |
| L3 | Sin NetworkManager, preguntar por el Wi-Fi | "No disponible", con el motivo |
| L4 | Bandeja del sistema en GNOME | Puede requerir una extensión. Si no hay bandeja, cerrar la ventana cierra la app |
