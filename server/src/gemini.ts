import contract from "./ai-contract.json";
import { ApiError, type ErrorCode } from "./errors";
import type { AnalyzeInput, DocumentInput, InterpretInput } from "./validation";

const RETRY_DELAY_MS = 1000;
/** Un reintento con menos tiempo que esto fallaría casi seguro: mejor responder ya. */
const MIN_RETRY_MS = 3000;

export interface ModelAnswer {
  output: string;
  tokens: number;
  /** Modelo que respondió. Solo se registra: la aplicación no lo recibe. */
  model: string;
  /** Cada intento del análisis: modelo, resultado y duración. Solo se registra. */
  attempts?: Attempt[];
}

export interface Attempt {
  model: string;
  /** 200, el estado HTTP de Google, «timeout» o «network». */
  result: number | "timeout" | "network";
  ms: number;
}

/**
 * Orden de los intentos del análisis. El modelo principal de visión a veces está saturado (503) o tarda: entonces
 * se pasa al de respaldo, que se repite una vez tras una pausa, y se vuelve al principal si aún queda tiempo.
 */
const ANALYZE_PLAN = ["model", "fallback", "fallback", "model"] as const;

export interface Upstream {
  model: string;
  /** Solo en el análisis: se usa si Google devuelve 503 con el modelo principal, en lugar de repetir con el mismo. */
  fallbackModel?: string;
  /** El cuerpo depende del modelo, que va dentro. */
  body: (model: string) => string | Blob;
  /** Espera máxima de cada intento. */
  timeoutMs: number;
  /** Espera máxima sumando el segundo intento: la aplicación sigue mostrando que procesa. */
  totalTimeoutMs: number;
}

/** Un fallo pasajero: corte de Google (5xx), tiempo agotado o red caída. Un 4xx o un 429 nunca lo son. */
function transient(code: ErrorCode): ApiError {
  return Object.assign(new ApiError(code), { transient: true });
}

function isTransient(error: unknown): boolean {
  return error instanceof ApiError && "transient" in error;
}

interface InteractionResponse {
  steps?: { type?: string; content?: { type?: string; text?: string }[] }[];
  usage?: { total_tokens?: number };
}

export function interpretRequest({ text, now }: InterpretInput): Upstream {
  const { model, timeoutMs, totalTimeoutMs, maxOutputTokens, thinkingLevel, systemInstruction, responseSchema } = contract.interpret;
  return {
    model,
    timeoutMs,
    totalTimeoutMs,
    body: (chosen) =>
      JSON.stringify({
        model: chosen,
        store: false,
        system_instruction: systemInstruction.replace("{now}", now),
        input: text,
        generation_config: { max_output_tokens: maxOutputTokens, thinking_level: thinkingLevel },
        response_format: { type: "text", mime_type: "application/json", schema: responseSchema },
      }),
  };
}

/** El documento va como dato, con su formato delante; las instrucciones dependen de la tarea. */
export function documentRequest({ task, format, text }: DocumentInput): Upstream {
  const { model, timeoutMs, totalTimeoutMs, maxOutputTokens, thinkingLevel, systemInstruction, responseSchema } = contract.document;
  return {
    model,
    timeoutMs,
    totalTimeoutMs,
    body: (chosen) =>
      JSON.stringify({
        model: chosen,
        store: false,
        system_instruction: systemInstruction[task],
        input: `Formato: ${format}\n\n${text}`,
        generation_config: { max_output_tokens: maxOutputTokens, thinking_level: thinkingLevel },
        response_format: { type: "text", mime_type: "application/json", schema: responseSchema },
      }),
  };
}

export function analyzeRequest({ question, image }: AnalyzeInput): Upstream {
  const { model, fallbackModel, timeoutMs, totalTimeoutMs, maxOutputTokens, thinkingLevel, systemInstruction, responseSchema } = contract.analyze;
  // La imagen se empalma como bytes. La marca es aleatoria para que la pregunta no pueda coincidir con ella.
  const marker = crypto.randomUUID();
  // La imagen se prepara una sola vez y se reutiliza en cada intento: copiarla de nuevo gasta CPU, que en el
  // Worker está limitada.
  const imageBytes = new Blob([image]);
  const body = (chosen: string) => {
    const [before, after] = JSON.stringify({
      model: chosen,
      store: false,
      system_instruction: systemInstruction,
      input: [
        { type: "text", text: question },
        { type: "image", data: marker, mime_type: "image/jpeg" },
      ],
      generation_config: { max_output_tokens: maxOutputTokens, thinking_level: thinkingLevel },
      response_format: { type: "text", mime_type: "application/json", schema: responseSchema },
    }).split(marker) as [string, string];
    return new Blob([before, imageBytes, after]);
  };
  return { model, fallbackModel, timeoutMs, totalTimeoutMs, body };
}

interface GoogleError {
  error?: { code?: unknown; status?: unknown; message?: unknown };
}

/** Lo que se registra de un error de Google: el estado HTTP y su código. Nunca el mensaje, que puede repetir datos. */
export interface UpstreamFailure {
  status: number;
  code: string | undefined;
  model: string;
}

function identifier(value: unknown): string | undefined {
  const text = String(value ?? "").toLowerCase();
  return /^[a-z0-9_]{1,40}$/.test(text) ? text : undefined;
}

function errorCode(status: number, envelope: GoogleError | undefined): ErrorCode {
  if (status !== 429) return "unavailable";
  const code = String(envelope?.error?.code ?? envelope?.error?.status ?? "").toLowerCase();
  const daily = code === "quota_exceeded" || (code === "resource_exhausted" && String(envelope?.error?.message).toLowerCase().includes("per day"));
  return daily ? "service_quota" : "rate_limited";
}

async function send(upstream: Upstream, model: string, key: string, timeoutMs: number): Promise<ModelAnswer> {
  let response: Response;
  try {
    response = await fetch(`${contract.baseUrl}/interactions`, {
      method: "POST",
      headers: { "Content-Type": "application/json", "x-goog-api-key": key, "Api-Revision": contract.apiRevision },
      body: upstream.body(model),
      signal: AbortSignal.timeout(timeoutMs),
    });
  } catch (error) {
    throw transient(error instanceof Error && error.name === "TimeoutError" ? "timeout" : "unavailable");
  }
  if (!response.ok) {
    const envelope = await response.json<GoogleError>().catch(() => undefined);
    const failure: UpstreamFailure = { status: response.status, code: identifier(envelope?.error?.status) ?? identifier(envelope?.error?.code), model };
    const error = response.status >= 500 ? transient("unavailable") : new ApiError(errorCode(response.status, envelope));
    throw Object.assign(error, { upstream: failure });
  }
  const answer = await response.json<InteractionResponse>().catch(() => {
    throw new ApiError("unavailable");
  });
  const output = (answer.steps ?? [])
    .filter((step) => step.type === "model_output")
    .flatMap((step) => step.content ?? [])
    .filter((part) => part.type === "text")
    .map((part) => part.text ?? "")
    .join("");
  if (!output.trim()) throw new ApiError("no_output");
  return { output, tokens: answer.usage?.total_tokens ?? 0, model };
}

function attemptResult(error: unknown): Attempt["result"] {
  if (error instanceof ApiError && "upstream" in error) return (error.upstream as UpstreamFailure).status;
  return error instanceof ApiError && error.code === "timeout" ? "timeout" : "network";
}

/**
 * Análisis de pantalla: hasta cuatro intentos alternando modelos (ANALYZE_PLAN), dentro del tiempo total. Un 503,
 * otro error 5xx, un tiempo agotado o un fallo de red pasan al siguiente; un 429 o un 4xx se informan enseguida.
 */
async function callWithFallback(upstream: Upstream & { fallbackModel: string }, key: string): Promise<ModelAnswer> {
  const deadline = Date.now() + upstream.totalTimeoutMs;
  const attempts: Attempt[] = [];
  let lastError: unknown;
  for (const [index, which] of ANALYZE_PLAN.entries()) {
    if (index >= 2) await new Promise((resolve) => setTimeout(resolve, RETRY_DELAY_MS));
    const remaining = deadline - Date.now();
    if (index > 0 && remaining < MIN_RETRY_MS) break;
    const model = which === "model" ? upstream.model : upstream.fallbackModel;
    const started = Date.now();
    try {
      const answer = await send(upstream, model, key, Math.min(upstream.timeoutMs, remaining));
      attempts.push({ model, result: 200, ms: Date.now() - started });
      return { ...answer, attempts };
    } catch (error) {
      attempts.push({ model, result: attemptResult(error), ms: Date.now() - started });
      if (!isTransient(error)) throw Object.assign(error as object, { attempts });
      lastError = error;
    }
  }
  throw Object.assign(lastError as object, { attempts });
}

/**
 * Interpretar y resumir: como mucho un segundo intento con el mismo modelo, dentro del tiempo total. Un 429 o un
 * 4xx nunca se repiten. El análisis de pantalla, con modelo de respaldo, sigue su propio plan (callWithFallback).
 */
export async function callGemini(upstream: Upstream, key: string): Promise<ModelAnswer> {
  if (upstream.fallbackModel) return callWithFallback({ ...upstream, fallbackModel: upstream.fallbackModel }, key);
  const deadline = Date.now() + upstream.totalTimeoutMs;
  try {
    return await send(upstream, upstream.model, key, upstream.timeoutMs);
  } catch (error) {
    const remaining = deadline - Date.now() - RETRY_DELAY_MS;
    if (remaining < MIN_RETRY_MS || !isTransient(error)) throw error;
    await new Promise((resolve) => setTimeout(resolve, RETRY_DELAY_MS));
    return send(upstream, upstream.model, key, Math.min(upstream.timeoutMs, remaining));
  }
}
