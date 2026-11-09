import { ApiError } from "./errors";

export const MAX_TEXT_CHARS = 500;
export const MAX_IMAGE_BYTES = 512_000;
const MAX_IMAGE_BASE64 = Math.ceil(MAX_IMAGE_BYTES / 3) * 4;
export const MAX_INTERPRET_BODY = 4_096;
export const MAX_ANALYZE_BODY = MAX_IMAGE_BASE64 + 4_096;

const INSTALL_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const LOCAL_TIME = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):\d{2}([+-]\d{2}:\d{2})$/;
const WEEKDAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const JPEG_TYPE = "image/jpeg";
export const CAPTURE_CONTENT_TYPE = "application/x-nexium-capture";
const NEWLINE = 0x0a;
const PADDING = 0x3d;
const BASE64_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
/** Valor de cada byte en base64, o -1 si no pertenece al alfabeto. */
const BASE64_VALUES = new Int8Array(256).fill(-1);
for (let value = 0; value < BASE64_ALPHABET.length; value++) BASE64_VALUES[BASE64_ALPHABET.charCodeAt(value)] = value;
/** Ocho caracteres de base64 son seis bytes: bastan para la cabecera y no obligan a decodificar el resto. */
const HEADER_CHARS = 8;

export interface InterpretInput {
  text: string;
  now: string;
}

export interface AnalyzeInput {
  question: string;
  /** JPEG en base64, como bytes: convertirlo en cadena costaría más CPU de la que permite el plan gratuito. */
  image: Uint8Array;
}

export function installId(header: string | null): string {
  if (!header || !INSTALL_ID.test(header)) throw new ApiError("invalid_install");
  return header;
}

/** Nada se lee sin saber antes cuánto ocupa. */
function checkEnvelope(request: Request, contentType: string, maxBytes: number): void {
  if (!request.headers.get("Content-Type")?.toLowerCase().startsWith(contentType)) throw new ApiError("invalid_request");
  const declared = Number(request.headers.get("Content-Length"));
  if (!Number.isInteger(declared) || declared <= 0) throw new ApiError("invalid_request");
  if (declared > maxBytes) throw new ApiError("payload_too_large");
}

function parseObject(raw: string): Record<string, unknown> {
  let body: unknown;
  try {
    body = JSON.parse(raw);
  } catch {
    throw new ApiError("invalid_request");
  }
  if (typeof body !== "object" || body === null || Array.isArray(body)) throw new ApiError("invalid_request");
  return body as Record<string, unknown>;
}

export async function readObject(request: Request, maxBytes: number): Promise<Record<string, unknown>> {
  checkEnvelope(request, "application/json", maxBytes);
  const raw = await request.text();
  if (raw.length > maxBytes) throw new ApiError("payload_too_large");
  return parseObject(raw);
}

/** Un campo desconocido se rechaza: el cliente no puede elegir modelo, instrucciones ni URL. */
function onlyFields(body: Record<string, unknown>, ...allowed: string[]): void {
  const keys = Object.keys(body);
  if (keys.length !== allowed.length || !allowed.every((field) => keys.includes(field))) throw new ApiError("invalid_request");
}

function text(value: unknown): string {
  if (typeof value !== "string") throw new ApiError("invalid_request");
  const trimmed = value.trim();
  // Se cuentan caracteres, como hace la aplicación, no unidades UTF-16.
  if (!trimmed || [...trimmed].length > MAX_TEXT_CHARS) throw new ApiError("invalid_request");
  return trimmed;
}

function describeNow(value: unknown): string {
  const match = typeof value === "string" ? LOCAL_TIME.exec(value) : null;
  if (!match || Number.isNaN(Date.parse(value as string))) throw new ApiError("invalid_request");
  const [, year, month, day, hour, minute, zone] = match as unknown as [string, string, string, string, string, string, string];
  const weekday = WEEKDAYS[new Date(Date.UTC(Number(year), Number(month) - 1, Number(day))).getUTCDay()];
  return `${weekday} ${year}-${month}-${day} ${hour}:${minute}, zona ${zone}`;
}

/** Base64 estándar y canónico: todo el alfabeto, relleno solo al final y sin bits sobrantes. No decodifica nada. */
function isBase64(data: Uint8Array): boolean {
  const { length } = data;
  if (length === 0 || length % 4 !== 0) return false;
  const padding = data[length - 1] !== PADDING ? 0 : data[length - 2] === PADDING ? 2 : 1;
  const end = length - padding;
  for (let index = 0; index < end; index++) {
    if ((BASE64_VALUES[data[index] as number] as number) < 0) return false;
  }
  const spareBits = padding === 0 ? 0 : padding === 2 ? 0x0f : 0x03;
  return ((BASE64_VALUES[data[end - 1] as number] as number) & spareBits) === 0;
}

/** Todo JPEG empieza por FF D8 FF, seguido del byte de un marcador (C0 a FE). Exige base64 ya validado. */
function hasJpegHeader(base64: Uint8Array): boolean {
  const head = atob(String.fromCharCode(...base64.subarray(0, HEADER_CHARS)));
  const marker = head.charCodeAt(3);
  return head.startsWith("\xff\xd8\xff") && marker >= 0xc0 && marker <= 0xfe;
}

/**
 * Comprueba tamaño, base64 y cabecera. La imagen nunca se decodifica: el límite de CPU es de 10 ms.
 * Validar todo el alfabeto también impide que estos bytes, que se insertan tal cual, se salgan del JSON que recibe Google.
 */
function checkJpegBase64(image: Uint8Array): void {
  if (image.length > MAX_IMAGE_BASE64) throw new ApiError("payload_too_large");
  if (!isBase64(image) || !hasJpegHeader(image)) throw new ApiError("invalid_request");
}

/** El cuerpo es una línea JSON con la pregunta y el tipo, y después la imagen en base64. */
export async function readCapture(request: Request): Promise<AnalyzeInput> {
  checkEnvelope(request, CAPTURE_CONTENT_TYPE, MAX_ANALYZE_BODY);
  const bytes = new Uint8Array(await request.arrayBuffer());
  if (bytes.length > MAX_ANALYZE_BODY) throw new ApiError("payload_too_large");
  const lineEnd = bytes.indexOf(NEWLINE);
  if (lineEnd <= 0 || lineEnd > MAX_INTERPRET_BODY) throw new ApiError("invalid_request");
  const fields = parseObject(new TextDecoder().decode(bytes.subarray(0, lineEnd)));
  onlyFields(fields, "question", "imageType");
  if (fields.imageType !== JPEG_TYPE) throw new ApiError("invalid_request");
  const image = bytes.subarray(lineEnd + 1);
  checkJpegBase64(image);
  return { question: text(fields.question), image };
}

export function interpretInput(body: Record<string, unknown>): InterpretInput {
  onlyFields(body, "text", "now");
  return { text: text(body.text), now: describeNow(body.now) };
}

/** En IPv6 una persona controla como mínimo un /64: limitar la dirección completa no serviría. */
export function ipBucket(ip: string): string {
  if (!ip.includes(":")) return ip;
  const [head = "", tail] = ip.toLowerCase().split("::");
  const headGroups = head ? head.split(":") : [];
  const tailGroups = tail ? tail.split(":") : [];
  const zeros = tail === undefined ? [] : Array<string>(Math.max(0, 8 - headGroups.length - tailGroups.length)).fill("0");
  return [...headGroups, ...zeros, ...tailGroups]
    .slice(0, 4)
    .map((group) => group.replace(/^0+(?=.)/, ""))
    .join(":");
}
