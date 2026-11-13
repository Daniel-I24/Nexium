import contract from "./ai-contract.json";
import { ApiError, type ErrorCode } from "./errors";
import type { AnalyzeInput, InterpretInput } from "./validation";

const RETRY_DELAY_MS = 1000;
/** Un reintento con menos tiempo que esto fallaría casi seguro: mejor responder ya. */
const MIN_RETRY_MS = 3000;

export interface ModelAnswer {
  output: string;
  tokens: number;
  /** Modelo que respondió. Solo se registra: la aplicación no lo recibe. */
  model: string;
}

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

export function analyzeRequest({ question, image }: AnalyzeInput): Upstream {
  const { model, fallbackModel, timeoutMs, totalTimeoutMs, maxOutputTokens, systemInstruction, responseSchema } = contract.analyze;
  // La imagen se empalma como bytes. La marca es aleatoria para que la pregunta no pueda coincidir con ella.
  const marker = crypto.randomUUID();
  const body = (chosen: string) => {
    const [before, after] = JSON.stringify({
      model: chosen,
      store: false,
      system_instruction: systemInstruction,
      input: [
        { type: "text", text: question },
        { type: "image", data: marker, mime_type: "image/jpeg" },
      ],
      generation_config: { max_output_tokens: maxOutputTokens },
      response_format: { type: "text", mime_type: "application/json", schema: responseSchema },
    }).split(marker) as [string, string];
    return new Blob([before, image, after]);
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

function isGoogle503(error: unknown): boolean {
  return error instanceof ApiError && "upstream" in error && (error.upstream as UpstreamFailure).status === 503;
}

/**
 * Como mucho un segundo intento, dentro del tiempo total. Un 429 o un 4xx nunca se repiten.
 * Con modelo de respaldo, solo un 503 de Google lleva al segundo intento, y siempre con el otro modelo.
 * Sin él, un fallo pasajero se reintenta una vez con el mismo modelo.
 */
export async function callGemini(upstream: Upstream, key: string): Promise<ModelAnswer> {
  const deadline = Date.now() + upstream.totalTimeoutMs;
  try {
    return await send(upstream, upstream.model, key, upstream.timeoutMs);
  } catch (error) {
    const remaining = deadline - Date.now() - RETRY_DELAY_MS;
    if (remaining < MIN_RETRY_MS) throw error;
    if (upstream.fallbackModel) {
      if (!isGoogle503(error)) throw error;
      return send(upstream, upstream.fallbackModel, key, Math.min(upstream.timeoutMs, remaining));
    }
    if (!isTransient(error)) throw error;
    await new Promise((resolve) => setTimeout(resolve, RETRY_DELAY_MS));
    return send(upstream, upstream.model, key, Math.min(upstream.timeoutMs, remaining));
  }
}
