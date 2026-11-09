# Servidor de IA de Nexium

Worker de Cloudflare que se interpone entre la aplicación y Google Gemini. Es lo único que conoce la clave de Gemini: el repositorio y los instaladores son públicos, y cualquiera podría extraer una clave incluida en ellos.

## Operaciones

Ambas son `POST`, exigen `Content-Type`, `Content-Length` y la cabecera `X-Nexium-Install` con un UUID v4 anónimo. Devuelven `{ "output": "…", "tokens": 42 }`.

| Ruta | `Content-Type` | Cuerpo | Límites |
|---|---|---|---|
| `/v1/interpret` | `application/json` | `{ "text": "…", "now": "2026-11-09T10:30:00-05:00" }` | Texto de hasta 500 caracteres. Cuerpo de hasta 4 KiB |
| `/v1/analyze` | `application/x-nexium-capture` | Una línea `{ "question": "…", "imageType": "image/jpeg" }`, un salto de línea y el JPEG en base64 | Imagen de hasta 512 KB |

Un campo adicional se rechaza: el cliente no puede elegir modelo, instrucciones ni URL. El modelo y las instrucciones están en `src/ai-contract.json`.

De la imagen se comprueban el tamaño, el tipo declarado, que todo sea base64 estándar con su relleno y la cabecera del archivo. El alfabeto también garantiza que sus bytes, que se insertan tal cual, no puedan salirse del JSON que recibe Google. El Worker no la decodifica ni la convierte en cadena de texto, para no acercarse al límite de 10 ms de CPU. Solo acepta JPEG: la aplicación convierte la captura antes de enviarla.

## Regiones

Los [términos de Google](https://aistudio.google.com/docs/terms) solo permiten servicios de pago en aplicaciones ofrecidas a usuarios del Espacio Económico Europeo, Reino Unido y Suiza, y Nexium usa la capa gratuita. Por eso el Worker responde `region_unavailable` a esas regiones, incluidas las regiones ultraperiféricas de la UE y las Åland, y `country_unknown` cuando Cloudflare no puede determinar el país (códigos `XX` y `T1`, este último para Tor). No se llama a Google ni se consume cupo.

El país sale de `request.cf.country`, que rellena Cloudflare y el cliente no puede modificar. Una VPN sí cambia el país que ve Cloudflare. El país no se registra.

## Errores

`{ "error": { "code": "…", "message": "…en español…" } }`

| Código | Estado | Significado |
|---|---|---|
| `invalid_request`, `invalid_install` | 400 | Formato no válido |
| `payload_too_large` | 413 | Texto o imagen demasiado grande |
| `region_unavailable`, `country_unknown` | 451 | Región sin servicio o país desconocido (ver [Regiones](#regiones)) |
| `rate_limited` | 429 | Demasiadas solicitudes por minuto. Incluye `Retry-After` |
| `daily_limit` | 429 | Límite diario de la instalación o de la red |
| `service_quota` | 429 | Cupo global o cuota gratuita de Gemini agotados |
| `no_output` | 502 | El modelo no devolvió texto |
| `unavailable` | 503 | Google o el contador no están disponibles |
| `timeout` | 504 | Google tardó demasiado |

Los detalles de Google nunca llegan al cliente. Un 429 no se reintenta; un error 5xx de Google se reintenta una sola vez.

## Límites de uso

Se configuran en `vars.LIMITS` de `wrangler.jsonc`, por operación:

- **`install`:** por instalación.
- **`ip`:** por dirección IP, o por red `/64` en IPv6. Es más amplio, porque un aula entera puede compartir una IP.
- **`global`:** cupo de todo el servicio.

El cupo global debe quedar por debajo de la cuota real más baja de tu proyecto, con margen. Google solo la muestra en AI Studio y no se ha podido consultar, así que las cifras actuales (500 interpretaciones y 50 análisis al día) son un valor prudente, no un dato de Google. Para cambiarlas no hay que tocar el código: edita `vars.LIMITS` y ejecuta `npx wrangler deploy`. Si Google agota antes su cuota, la aplicación recibe el mismo mensaje de cuota.

El identificador de instalación no se puede verificar, porque el cliente es público: alguien podría inventar identificadores. Por eso existen los límites por IP y el cupo global.

## Privacidad

- No se registra el texto, la imagen, la IP ni el identificador de instalación. Los registros solo contienen operación, código de resultado, estado, duración y tokens.
- La IP y el identificador solo sirven para localizar un contador. Cada contador guarda cuatro números y se borra solo cuando pasa de día.
- Gemini recibe `store: false`. En la capa gratuita, Google puede usar lo que recibe para mejorar sus productos.

## Desarrollo

```powershell
npm ci
npm run typecheck
npm test
```

Para probarlo en local, copia `.dev.vars.example` como `.dev.vars`, escribe la clave ahí y ejecuta `npm run dev`. `.dev.vars` está excluido de Git.

Las pruebas se ejecutan en `workerd`, el motor real de Workers, con Durable Objects reales. Solo se simulan las respuestas de Google.

## Despliegue

Desde esta carpeta, con una cuenta gratuita de Cloudflare:

```powershell
npx wrangler login
npx wrangler secret put GEMINI_API_KEY
npx wrangler deploy
```

`secret put` pide la clave en la terminal, y Cloudflare la guarda cifrada. No la escribas en ningún archivo del repositorio.

`deploy` muestra la URL del servicio. Esa URL no es secreta: se pasa a la compilación de la aplicación en la variable `NEXIUM_AI_SERVER_URL`.

## Límites del plan gratuito

- 100 000 solicitudes al día en Workers y 100 000 en Durable Objects. Cada solicitud usa hasta 3 contadores.
- 10 ms de CPU por solicitud. La espera a Google no cuenta.
