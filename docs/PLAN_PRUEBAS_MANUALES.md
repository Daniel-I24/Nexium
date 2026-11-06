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
| 13 | Guardar la clave de Gemini en Ajustes | Indica "almacén seguro del sistema" y sigue tras reiniciar |
| 14 | "Recuérdame probar Nexium hoy a las HH:MM", dos minutos más tarde | Pide confirmar, y llega la notificación a esa hora |
| 15 | Cerrar la ventana con el recordatorio pendiente | La app sigue en la bandeja y el aviso llega |
| 16 | Salir desde la bandeja y volver a abrir tras la hora | El recordatorio sigue guardado y aparece como vencido |
| 17 | Analizar pantalla con una ventana sin datos personales | Aviso nombrando a Google Gemini antes de capturar; responde sobre esa ventana |
| 18 | Revisar el Historial | Aparecen las solicitudes, y se pueden filtrar y eliminar |

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
| L4 | Sin servicio de llaves, guardar la clave | Indica "solo durante esta sesión" |
| L5 | Bandeja del sistema en GNOME | Puede requerir una extensión. Si no hay bandeja, cerrar la ventana cierra la app |
