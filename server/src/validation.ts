import { ApiError } from "./errors";

export const MAX_TEXT_CHARS = 500;
export const MAX_IMAGE_BYTES = 1_000_000;
const MAX_IMAGE_BASE64 = Math.ceil(MAX_IMAGE_BYTES / 3) * 4;
export const MAX_INTERPRET_BODY = 4_096;
export const MAX_ANALYZE_BODY = MAX_IMAGE_BASE64 + 4_096;

const INSTALL_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const LOCAL_TIME = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):\d{2}([+-]\d{2}:\d{2})$/;
const BASE64 = /^[A-Za-z0-9+/]+={0,2}$/;
const WEEKDAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
/** Los tres primeros bytes de todo JPEG (FF D8 FF), vistos en base64. */
const JPEG_PREFIX = "/9j/";

export interface InterpretInput {
  text: string;
  now: string;
}

export interface AnalyzeInput {
  question: string;
  image: string;
}

export function installId(header: string | null): string {
  if (!header || !INSTALL_ID.test(header)) throw new ApiError("invalid_install");
  return header;
}

/** Lee el cuerpo con un tope: nada se analiza sin saber antes cuánto ocupa. */
export async function readObject(request: Request, maxBytes: number): Promise<Record<string, unknown>> {
  if (!request.headers.get("Content-Type")?.toLowerCase().startsWith("application/json")) throw new ApiError("invalid_request");
  const declared = Number(request.headers.get("Content-Length"));
  if (!Number.isInteger(declared) || declared <= 0) throw new ApiError("invalid_request");
  if (declared > maxBytes) throw new ApiError("payload_too_large");
  const raw = await request.text();
  if (raw.length > maxBytes) throw new ApiError("payload_too_large");
  let body: unknown;
  try {
    body = JSON.parse(raw);
  } catch {
    throw new ApiError("invalid_request");
  }
  if (typeof body !== "object" || body === null || Array.isArray(body)) throw new ApiError("invalid_request");
  return body as Record<string, unknown>;
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

function jpegBase64(value: unknown): string {
  if (typeof value !== "string") throw new ApiError("invalid_request");
  if (value.length > MAX_IMAGE_BASE64) throw new ApiError("payload_too_large");
  if (value.length % 4 !== 0 || !value.startsWith(JPEG_PREFIX) || !BASE64.test(value)) throw new ApiError("invalid_request");
  return value;
}

export function interpretInput(body: Record<string, unknown>): InterpretInput {
  onlyFields(body, "text", "now");
  return { text: text(body.text), now: describeNow(body.now) };
}

export function analyzeInput(body: Record<string, unknown>): AnalyzeInput {
  onlyFields(body, "question", "image");
  return { question: text(body.question), image: jpegBase64(body.image) };
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
