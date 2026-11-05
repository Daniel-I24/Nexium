# PROMPT MAESTRO — DESARROLLO DE UN ASISTENTE INTELIGENTE DE ESCRITORIO MULTIPLATAFORMA

## 1. TU ROL Y RESPONSABILIDAD

Actúa como un equipo profesional de ingeniería de software especializado en:

- Arquitectura de software.
- Desarrollo de aplicaciones de escritorio multiplataforma.
- React y TypeScript.
- Tailwind CSS y diseño de interfaces.
- Rust y desarrollo de integraciones nativas.
- Inteligencia artificial y modelos multimodales.
- Bases de datos.
- Ciberseguridad.
- Pruebas y control de calidad.

Tu responsabilidad es planificar, diseñar, programar, integrar, probar y documentar una aplicación de escritorio real.

No quiero que te limites a producir código. Debes tomar decisiones técnicas justificadas, prevenir errores, mantener una estructura organizada y verificar que cada funcionalidad realmente funcione.

El objetivo es construir un producto de calidad en el menor tiempo razonable, evitando complejidad innecesaria.

Debes trabajar como un ingeniero responsable del resultado final.

---

# 2. CONTEXTO DEL PROYECTO

## Problema

Los asistentes de escritorio actuales suelen ser genéricos y superficiales. Algunos se limitan a abrir aplicaciones o ejecutar comandos básicos, mientras que otros dependen de integraciones cerradas para proporcionar información del propio equipo.

No existe una experiencia suficientemente integrada que combine:

- Interacción mediante comandos de voz naturales.
- Consultas reales del estado del sistema operativo.
- Búsqueda y localización de archivos.
- Inteligencia artificial para comprender solicitudes.
- Análisis visual de la pantalla bajo demanda.
- Recordatorios y consultas de información externa.
- Historial de interacciones.

El proyecto busca ofrecer una solución sencilla, inteligente y accesible que integre estas capacidades en una única aplicación de escritorio.

## Objetivo general

Desarrollar un asistente inteligente de escritorio multiplataforma capaz de interpretar instrucciones en lenguaje natural, ejecutar consultas reales del sistema operativo, localizar archivos, gestionar recordatorios, consultar información externa y analizar capturas de pantalla mediante inteligencia artificial.

La aplicación debe ser moderna, segura, rápida, intuitiva y fácil de utilizar.

## Alcance

La aplicación estará diseñada para:

- Windows.
- macOS.
- Linux.

No desarrollar versiones móviles ni aplicaciones web independientes en esta primera versión.

El objetivo es construir una única aplicación de escritorio que pueda compilarse y distribuirse para los tres sistemas operativos.

---

# 3. STACK TECNOLÓGICO

Selecciona las herramientas que proporcionen el mejor equilibrio entre calidad, estabilidad, rendimiento y velocidad de desarrollo.

Stack tecnológico propuesto:

### Aplicación de escritorio
- Tauri 2.

### Frontend
- React.
- TypeScript.
- Tailwind CSS.

### Backend nativo
- Rust.

### Base de datos
- SQLite.

### Inteligencia artificial
- API de un modelo multimodal compatible con procesamiento de texto e imágenes.
- Arquitectura preparada para cambiar de proveedor sin modificar toda la aplicación.

### Voz
- Reconocimiento de voz mediante un servicio compatible con los sistemas operativos objetivo.
- Seleccionar una solución estable y documentar sus dependencias.

### APIs externas
- API meteorológica.
- Servicio de inteligencia artificial.
- Servicios adicionales únicamente cuando sean necesarios.

### Pruebas
- Vitest para pruebas del frontend.
- Herramientas de pruebas adecuadas para Rust.
- Pruebas de integración entre los componentes.

### Control de versiones
- Git y GitHub.

Puedes modificar alguna tecnología si identificas una alternativa considerablemente mejor.

Sin embargo, Tailwind CSS es un requisito obligatorio y no puede sustituirse por otro framework de estilos.

Antes de instalar dependencias, verifica compatibilidad, versiones estables y requisitos de cada tecnología.

No utilices versiones experimentales sin una justificación clara.

---

# 4. FUNCIONALIDADES OBLIGATORIAS

## MÓDULO 1. ASISTENTE INTELIGENTE E INTERACCIÓN POR VOZ

Esta es la funcionalidad central de la aplicación.

El usuario debe poder activar el asistente mediante un botón visible.

Flujo:

1. El usuario activa el micrófono.
2. La aplicación captura su solicitud.
3. El sistema convierte la voz en texto.
4. La inteligencia artificial interpreta la intención cuando sea necesario.
5. El sistema identifica la operación correspondiente.
6. Ejecuta la operación.
7. Presenta el resultado en la interfaz.

Ejemplos:

- ¿Qué hora es?
- ¿Cómo está el clima hoy?
- ¿Cuánta memoria RAM estoy utilizando?
- ¿Estoy conectado a Internet?
- Busca mi documento de bases de datos.
- Recuérdame entregar el proyecto mañana.
- Analiza lo que aparece en mi pantalla.

### Requisitos

- Reconocer solicitudes en lenguaje natural.
- Diferenciar entre intenciones.
- Manejar solicitudes ambiguas.
- Responder en español.
- Informar cuando no comprenda una solicitud.
- Indicar cuándo está escuchando, procesando y respondiendo.
- Evitar mantener el micrófono activo permanentemente.
- Permitir cancelar una operación.

No utilizar inteligencia artificial para todas las operaciones.

Por ejemplo, consultar la hora o la utilización de RAM no necesita una petición a un modelo de lenguaje.

La IA debe utilizarse donde realmente aporte valor.

## MÓDULO 2. INTELIGENCIA ARTIFICIAL

La inteligencia artificial es un requisito obligatorio y debe tener una participación central en la experiencia.

No quiero una integración superficial que simplemente envíe preguntas a un chatbot.

Debe utilizarse para:

### A. Interpretación de solicitudes

Comprender instrucciones naturales y clasificarlas según su intención.

Ejemplo:

"Necesito encontrar el informe que hice para la clase de programación".

La IA debe identificar que se trata de una búsqueda de archivos y extraer los términos relevantes.

### B. Extracción de información

Identificar datos relevantes dentro de las solicitudes.

Ejemplo:

"Recuérdame entregar el proyecto el próximo jueves a las ocho de la mañana".

Debe identificar:

- Tipo de operación: recordatorio.
- Contenido: entregar el proyecto.
- Fecha: próximo jueves.
- Hora: 08:00.

Si la fecha es ambigua, debe solicitar aclaración.

### C. Análisis visual

Utilizar un modelo con capacidades de visión para interpretar capturas de pantalla.

Debe poder explicar errores visibles, identificar elementos de una interfaz y responder preguntas relacionadas con el contenido de la pantalla.

### D. Arquitectura de IA

Implementar una capa independiente para la inteligencia artificial.

Debe permitir:

- Cambiar de proveedor.
- Configurar modelos.
- Controlar errores.
- Gestionar tiempos de espera.
- Validar respuestas.
- Evitar solicitudes innecesarias.
- Controlar costos de utilización.

La IA no debe tener acceso ilimitado al equipo.

Debe recibir información estrictamente necesaria para resolver cada solicitud.

Las operaciones del sistema deben ejecutarse mediante funciones controladas y validadas por la aplicación.

No permitir que el modelo ejecute código arbitrario ni comandos del sistema.

### Seguridad de las API

No incluir claves API directamente en el código.

Utilizar almacenamiento seguro de credenciales cuando esté disponible.

Si la aplicación se distribuye, evaluar un servicio intermediario para proteger las claves del proveedor.

Documentar claramente cómo se configura la inteligencia artificial y qué servicios requieren conexión a Internet.

## MÓDULO 3. CONSULTAS REALES DEL SISTEMA OPERATIVO

El asistente debe consultar información real del equipo.

Información obligatoria:

- Porcentaje de utilización de CPU.
- Memoria RAM utilizada.
- Memoria RAM disponible.
- Estado de batería.
- Porcentaje de batería.
- Estado de conexión a Internet.
- Estado de conexión Wi-Fi.
- Nombre de la red conectada, cuando el sistema lo permita.
- Estado del modo oscuro, cuando exista una API compatible.

### Requisitos multiplataforma

No asumir que Windows, macOS y Linux proporcionan la misma información mediante las mismas interfaces.

Crear una capa de abstracción para las consultas del sistema.

Implementar adaptadores específicos cuando sea necesario.

Si una función no está disponible en determinado sistema operativo:

- Informar que no está disponible.
- No inventar valores.
- No mostrar información simulada.
- No provocar errores en el resto de la aplicación.

Priorizar el uso de APIs oficiales y mecanismos seguros del sistema operativo.

## MÓDULO 4. BÚSQUEDA Y ANÁLISIS DE ARCHIVOS

El asistente debe buscar archivos dentro de las ubicaciones autorizadas por el usuario.

Ubicaciones iniciales:

- Documentos.
- Escritorio.

Debe permitir solicitudes como:

- Busca mi informe de programación.
- ¿Existe el archivo proyecto.pdf?
- Encuentra los documentos relacionados con bases de datos.

### Funcionamiento

1. Identificar la solicitud.
2. Determinar los términos de búsqueda.
3. Consultar las ubicaciones autorizadas.
4. Verificar las coincidencias.
5. Presentar los resultados.

Cada resultado debe mostrar:

- Nombre del archivo.
- Extensión.
- Ubicación.
- Ruta absoluta.
- Estado de existencia.

Si hay varias coincidencias, mostrarlas individualmente.

La IA puede ayudar a interpretar la búsqueda, pero la existencia y ubicación deben verificarse mediante operaciones reales del sistema.

### Seguridad

- Solicitar autorización para acceder a las carpetas.
- No buscar indiscriminadamente en todo el disco.
- No modificar archivos.
- No eliminar archivos.
- No mover archivos.
- No leer contenido privado sin autorización.
- Evitar búsquedas que puedan bloquear la aplicación.

## MÓDULO 5. ANÁLISIS DE PANTALLA

Permitir que el usuario solicite un análisis puntual de su pantalla.

Ejemplos:

- ¿Qué aparece en mi pantalla?
- Explícame este error.
- ¿Qué significa este mensaje?
- Ayúdame a entender esta interfaz.

### Funcionamiento

1. El usuario solicita el análisis.
2. La aplicación informa que realizará una captura.
3. Se obtiene una única imagen.
4. Se envía al modelo multimodal.
5. La IA analiza el contenido.
6. Se presenta una respuesta contextualizada.

### Restricciones

- No realizar capturas permanentes.
- No transmitir la pantalla continuamente.
- No capturar sin autorización.
- No almacenar imágenes permanentemente.
- Evitar enviar información innecesaria.
- Manejar errores de permisos y conexión.

La función debe ser compatible con los mecanismos de captura disponibles en cada sistema operativo.

## MÓDULO 6. CLIMA Y HORA

### Hora

- Consultar la hora local del equipo.
- Mostrar fecha y hora.
- Responder mediante voz o texto.

### Clima

- Consultar una API meteorológica.
- Mostrar temperatura.
- Mostrar condición climática.
- Mostrar ubicación.
- Diferenciar condiciones actuales y pronósticos.

Si no existe conexión, informar que no fue posible consultar el servicio.

No generar información meteorológica ficticia.

## MÓDULO 7. RECORDATORIOS

Permitir crear recordatorios mediante lenguaje natural.

Ejemplo:

"Recuérdame entregar el trabajo el viernes a las nueve de la mañana".

### Requisitos

- Extraer contenido.
- Identificar fecha.
- Identificar hora.
- Solicitar aclaración cuando exista ambigüedad.
- Guardar en SQLite.
- Mostrar recordatorios pendientes.
- Permitir editar y eliminar.
- Generar notificaciones del sistema operativo.
- Mantener la información después de cerrar la aplicación.

Verificar que las notificaciones funcionen correctamente en los sistemas operativos compatibles.

No prometer notificaciones cuando la aplicación o el sistema no permitan garantizar su ejecución.

## MÓDULO 8. HISTORIAL

Crear un panel donde el usuario pueda consultar sus interacciones anteriores.

Registrar:

- Solicitud.
- Fecha y hora.
- Categoría.
- Estado.
- Respuesta.

Permitir:

- Consultar historial.
- Eliminar registros.
- Visualizar operaciones fallidas.
- Diferenciar operaciones completadas y pendientes.

No guardar claves API, contraseñas ni capturas de pantalla.

---

# 5. DISEÑO Y EXPERIENCIA DE USUARIO — REQUISITO CRÍTICO

La interfaz debe construirse exclusivamente utilizando:

- React.
- TypeScript.
- Tailwind CSS.

No sustituir Tailwind CSS por Bootstrap, Material UI, componentes visuales prefabricados o frameworks de diseño alternativos.

Se permiten bibliotecas pequeñas y justificadas para iconografía, animaciones o componentes auxiliares.

## Identidad visual

Quiero una aplicación con personalidad propia.

No quiero el típico diseño de inteligencia artificial que se repite en miles de proyectos.

Evitar:

- Degradados morados y azules como recurso principal.
- Fondos negros con luces de neón.
- Tarjetas repetitivas.
- Bordes redondeados exagerados.
- Sombras excesivas.
- Iconos genéricos sin coherencia.
- Elementos decorativos sin función.
- Interfaces que parezcan plantillas generadas automáticamente.
- Animaciones innecesarias.
- Diseños sobrecargados.

## Dirección artística

Diseñar una identidad visual original, moderna, elegante y tecnológica.

Priorizar:

- Composición visual diferenciada.
- Jerarquía tipográfica.
- Espaciado consistente.
- Paleta de colores limitada y bien seleccionada.
- Contraste adecuado.
- Elementos visuales propios.
- Microinteracciones.
- Transiciones sutiles.
- Iconografía coherente.
- Excelente legibilidad.

No elegir colores aleatoriamente.

Primero presentar propuestas de identidad visual y justificar la selección de la paleta.

La aplicación debe transmitir confianza, inteligencia y sencillez.

## Interfaz principal

Debe incluir:

- Identidad y nombre de la aplicación.
- Área principal de interacción.
- Control de voz.
- Indicador de procesamiento.
- Respuestas del asistente.
- Acceso a funcionalidades.
- Historial.
- Configuración.

El diseño debe evitar saturar al usuario con información.

Cada pantalla debe tener una finalidad concreta.

## Responsive y escalabilidad visual

Aunque sea una aplicación de escritorio, debe adaptarse a distintos tamaños de ventana y resoluciones.

Evitar interfaces rígidas que solo funcionen en una resolución específica.

---

# 6. ARQUITECTURA DEL SOFTWARE

Separar claramente las responsabilidades del frontend y del backend nativo.

## Frontend

Responsable de:

- Interfaz.
- Componentes.
- Navegación.
- Estados visuales.
- Interacción.
- Validaciones de presentación.
- Comunicación con el backend.

## Backend nativo

Responsable de:

- Consultas del sistema operativo.
- Acceso controlado a archivos.
- Captura de pantalla.
- Persistencia local.
- Notificaciones.
- Operaciones sensibles.
- Integración con servicios externos cuando corresponda.

## Comunicación

Utilizar los mecanismos seguros de comunicación de Tauri.

Definir contratos claros entre frontend y backend.

No permitir que el frontend ejecute operaciones nativas arbitrarias.

Validar todas las solicitudes recibidas.

---

# 7. PROGRAMACIÓN ORIENTADA A OBJETOS Y BUENAS PRÁCTICAS

El proyecto debe aplicar correctamente:

- Abstracción.
- Herencia.
- Polimorfismo.
- Encapsulamiento.
- Composición.
- SOLID.
- DRY.
- Separación de responsabilidades.
- Bajo acoplamiento.
- Alta cohesión.

La programación orientada a objetos debe tener una aplicación justificada.

Ejemplos de abstracciones:

- SystemProvider.
- FileSearchProvider.
- AIProvider.
- SpeechProvider.
- NotificationProvider.
- CommandHandler.

Los proveedores concretos pueden tener implementaciones diferentes según el sistema operativo o servicio utilizado.

No implementar herencia artificial solamente para cumplir un requisito.

Priorizar composición cuando sea más adecuada.

Evitar duplicación de lógica.

No crear patrones innecesarios.

---

# 8. ESTRUCTURA Y ORGANIZACIÓN DEL PROYECTO

Mantener una estructura clara y profesional.

Propuesta inicial:

```text
assistant-desktop/
│
├── src/
│   ├── app/
│   ├── components/
│   ├── features/
│   │   ├── assistant/
│   │   ├── voice/
│   │   ├── system/
│   │   ├── files/
│   │   ├── screen-analysis/
│   │   ├── reminders/
│   │   ├── weather/
│   │   └── history/
│   ├── hooks/
│   ├── services/
│   ├── types/
│   ├── utils/
│   └── styles/
│
├── src-tauri/
│   ├── src/
│   │   ├── commands/
│   │   ├── services/
│   │   ├── providers/
│   │   ├── database/
│   │   ├── models/
│   │   └── platform/
│   │       ├── windows/
│   │       ├── macos/
│   │       └── linux/
│   ├── capabilities/
│   └── Cargo.toml
│
├── tests/
├── public/
├── .env.example
├── .gitignore
├── package.json
├── README.md
└── tailwind.config.js (si la versión seleccionada lo requiere).
```

Puedes adaptar esta estructura según las necesidades reales de Tauri y la versión de Tailwind CSS.

No crear carpetas vacías.

No colocar toda la lógica en App.tsx ni en archivos monolíticos.

Cada módulo debe tener responsabilidades claras.

---

# 9. ESTÁNDARES DE CODIFICACIÓN

Reglas obligatorias:

- Código y nombres técnicos en inglés.
- Interfaz de usuario completamente en español.
- Componentes React en PascalCase.
- Variables y funciones en camelCase para TypeScript.
- Tipos e interfaces con nombres descriptivos.
- Convenciones idiomáticas de Rust.
- Imports organizados.
- Tipado estricto.
- Evitar el uso innecesario de any.
- Funciones pequeñas y especializadas.
- Evitar código duplicado.
- Evitar archivos excesivamente extensos.
- Evitar dependencias circulares.
- Mantener consistencia en todo el proyecto.
- Comentarios solamente cuando aporten claridad.

No sacrificar calidad por escribir menos código.

---

# 10. SEGURIDAD

La aplicación debe aplicar seguridad desde su diseño.

Requisitos:

- Validación de entradas.
- Protección de credenciales.
- Almacenamiento seguro de claves.
- Permisos mínimos necesarios.
- Restricciones de acceso a archivos.
- Validación de rutas.
- Protección de la comunicación IPC.
- Configuración estricta de capacidades de Tauri.
- No permitir ejecución arbitraria de comandos.
- No exponer funciones nativas innecesarias.
- No almacenar información sensible.
- No enviar información privada sin autorización.
- Manejo seguro de errores.

No incluir claves API en el repositorio.

Si se requiere un servicio externo para proteger credenciales, justificarlo y mantenerlo lo más sencillo posible.

---

# 11. RENDIMIENTO

Priorizar:

- Inicio rápido.
- Bajo consumo de RAM.
- Bajo consumo de CPU.
- Interfaz fluida.
- Operaciones en segundo plano.
- Consultas eficientes.
- Manejo adecuado de memoria.
- Cierre correcto de procesos.
- Tiempos de espera en servicios externos.
- Evitar renders innecesarios.
- Evitar consultas repetitivas.

No bloquear la interfaz durante el procesamiento de voz, consultas, búsquedas o análisis de imágenes.

No utilizar recursos constantemente cuando no sean necesarios.

---

# 12. COMPATIBILIDAD MULTIPLATAFORMA

Este es un requisito obligatorio.

La aplicación debe diseñarse para Windows, macOS y Linux desde el principio.

No desarrollar primero una solución exclusiva de Windows y posteriormente intentar adaptarla.

Utilizar interfaces comunes y adaptadores específicos por sistema operativo.

Crear una matriz de compatibilidad que identifique:

- Funcionalidades disponibles.
- Funcionalidades con restricciones.
- Dependencias adicionales.
- Permisos requeridos.
- Limitaciones conocidas.

Verificar especialmente:

- Acceso a archivos.
- Consultas de CPU y RAM.
- Batería.
- Información Wi-Fi.
- Modo oscuro.
- Captura de pantalla.
- Notificaciones.
- Reconocimiento de voz.

No afirmar compatibilidad total de una funcionalidad si no se ha comprobado en el sistema correspondiente.

Configurar compilaciones independientes para cada plataforma.

---

# 13. PRUEBAS Y CONTROL DE CALIDAD

No quiero descubrir errores importantes al final del proyecto.

Cada módulo debe probarse antes de integrarse.

Implementar:

- Pruebas unitarias.
- Pruebas de integración.
- Validación de entradas.
- Pruebas de errores.
- Pruebas de persistencia.
- Pruebas de comunicación entre frontend y backend.
- Verificación de compilación.

Probar casos normales y casos límite.

No utilizar información simulada en la aplicación final.

Los mocks solamente deben utilizarse en pruebas.

Mantener un registro de:

- Funcionalidades implementadas.
- Funcionalidades probadas.
- Errores detectados.
- Errores corregidos.
- Limitaciones pendientes.

No declarar una funcionalidad terminada hasta haber comprobado su funcionamiento.

---

# 14. DOCUMENTACIÓN

Crear documentación profesional que incluya:

- Descripción del proyecto.
- Objetivo.
- Tecnologías.
- Arquitectura.
- Estructura.
- Instalación.
- Requisitos.
- Configuración.
- Variables de entorno.
- Ejecución.
- Pruebas.
- Compilación.
- Distribución.
- Limitaciones conocidas.

Las instrucciones deben ser claras y reproducibles.

---

# 15. METODOLOGÍA DE DESARROLLO

Trabajaremos por etapas.

No desarrollar todo de una sola vez.

## ETAPA 1. ANÁLISIS Y PLANIFICACIÓN

Antes de escribir código:

1. Analiza todos los requisitos.
2. Identifica dependencias.
3. Comprueba la viabilidad multiplataforma.
4. Selecciona las herramientas definitivas.
5. Define la arquitectura.
6. Diseña la estructura de carpetas.
7. Identifica riesgos técnicos.
8. Define cómo se probará cada funcionalidad.

Presenta las decisiones y sus justificaciones.

No comiences a generar archivos todavía.

## ETAPA 2. DISEÑO VISUAL

Antes de desarrollar toda la interfaz:

1. Define la identidad visual.
2. Propón una paleta de colores.
3. Selecciona tipografías.
4. Define el sistema de espaciado.
5. Define botones, controles y componentes.
6. Diseña la pantalla principal.
7. Establece las reglas de Tailwind CSS.
8. Verifica coherencia entre los componentes.

Primero debemos aprobar una dirección visual.

## ETAPA 3. CONFIGURACIÓN

- Preparar el entorno.
- Instalar dependencias.
- Configurar Tauri.
- Configurar React.
- Configurar TypeScript.
- Configurar Tailwind CSS.
- Preparar SQLite.
- Crear la estructura.
- Verificar que la aplicación compile.

## ETAPA 4. DESARROLLO MODULAR

Implementar progresivamente:

1. Interfaz principal.
2. Núcleo de procesamiento de comandos.
3. Integración de inteligencia artificial.
4. Consultas del sistema.
5. Búsqueda de archivos.
6. Clima y hora.
7. Recordatorios.
8. Reconocimiento de voz.
9. Análisis de pantalla.
10. Historial.

Cada módulo debe integrarse y probarse antes de continuar.

El orden puede modificarse si existe una dependencia técnica que lo justifique.

## ETAPA 5. INTEGRACIÓN

Verificar:

- Comunicación frontend/backend.
- Funcionamiento de la IA.
- Persistencia.
- Permisos.
- Notificaciones.
- Integración de módulos.
- Manejo de errores.

## ETAPA 6. VALIDACIÓN MULTIPLATAFORMA

Verificar compilación y funcionamiento en Windows, macOS y Linux.

Si no tienes acceso a un sistema operativo, no afirmes haber probado su funcionamiento real.

Documenta las pruebas pendientes.

## ETAPA 7. ENTREGA

La aplicación debe estar:

- Funcional.
- Compilable.
- Organizada.
- Documentada.
- Probada.
- Segura.
- Lista para distribuirse.

---

# 16. REGLAS DE TRABAJO CONMIGO

Estas instrucciones deben respetarse durante toda la conversación.

1. No avances sin tener claridad sobre la etapa actual.
2. No cambies tecnologías sin justificarlo.
3. No agregues funcionalidades innecesarias.
4. No generes grandes cantidades de código sin verificar su integración.
5. No asumas que un comando se ejecutó correctamente.
6. No inventes resultados de pruebas.
7. No ocultes errores.
8. No utilices datos simulados en producción.
9. No repitas código.
10. No modifiques archivos ajenos al alcance sin justificación.
11. No rompas funcionalidades que ya funcionan.
12. No entregues archivos incompletos.
13. Indica siempre la ruta exacta de cada archivo.
14. Indica los comandos exactos que debo ejecutar.
15. Explica brevemente qué se está implementando.
16. Si aparece un error, identifica primero su causa antes de proponer modificaciones.
17. Evita soluciones temporales que generen problemas posteriores.
18. Mantén actualizado el estado del proyecto.
19. Prioriza las soluciones sencillas y comprobables.
20. No me hagas repetir información que ya está definida en este documento.

Cuando una decisión sea necesaria, propón la mejor alternativa y explica brevemente por qué.

No conviertas cada decisión técnica en una pregunta para mí.

---

# 17. CRITERIOS DE ACEPTACIÓN FINAL

El proyecto estará terminado cuando cumpla con los siguientes criterios:

### Funcionalidad
- Reconocimiento de comandos de voz.
- Interpretación de lenguaje natural mediante IA.
- Consultas reales del sistema.
- Búsqueda de archivos.
- Análisis de pantalla mediante visión artificial.
- Consultas de clima y hora.
- Creación de recordatorios.
- Notificaciones.
- Historial.

### Ingeniería
- React y TypeScript.
- Tailwind CSS obligatorio.
- Tauri 2.
- Rust.
- SQLite.
- Arquitectura modular.
- POO correctamente aplicada.
- Código en inglés.
- Interfaz en español.
- Buenas prácticas.
- Seguridad.
- Rendimiento.

### Diseño
- Identidad visual propia.
- Diseño profesional.
- Interfaz intuitiva.
- Consistencia.
- Adaptabilidad a diferentes resoluciones.
- Sin apariencia de plantilla genérica.

### Compatibilidad
- Windows.
- macOS.
- Linux.

### Calidad
- Pruebas.
- Manejo de errores.
- Documentación.
- Compilación.
- Instalación reproducible.

---

# 18. INSTRUCCIÓN INICIAL

Por ahora NO escribas código.

Quiero que comiences realizando un análisis técnico integral del proyecto.

Tu primera respuesta debe contener:

1. Tu comprensión del problema y del producto.
2. La arquitectura tecnológica definitiva que propones.
3. La justificación de cada tecnología.
4. La estructura de carpetas propuesta.
5. Cómo implementarás la inteligencia artificial.
6. Cómo garantizarás el acceso multiplataforma al sistema.
7. Qué APIs y servicios externos necesitaremos.
8. Los riesgos técnicos y las limitaciones que debemos conocer.
9. Un plan de desarrollo por etapas.
10. Una estimación de complejidad de cada módulo.

Identifica cualquier contradicción o requisito técnicamente inviable antes de comenzar.

No agregues funcionalidades por iniciativa propia.

No conviertas el proyecto en una plataforma empresarial.

**OBJETIVO FINAL:**

Construir un asistente inteligente de escritorio multiplataforma, con identidad visual única, inteligencia artificial funcional, capacidades reales de interacción con el sistema operativo y una arquitectura profesional.

Quiero entregar un producto pequeño en alcance, pero grande en calidad.

La prioridad es terminar correctamente, no desarrollar más de lo necesario.
