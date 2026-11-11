import contract from "./ai-contract.json";
import { ApiError, type ErrorCode } from "./errors";
import type { AnalyzeInput, InterpretInput } from "./validation";

const RETRY_DELAY_MS = 1000;
/** Un reintento con menos tiempo que esto fallaría casi seguro: mejor responder ya. */
const MIN_RETRY_MS = 3000;

export interface ModelAnswer {
  output: string;
  tokens: number;
}

export interface Upstream {
  body: string | Blob;
  /** Espera máxima de cada intento. */
  timeoutMs: number;
  /** Espera máxima sumando el reintento: la aplicación sigue mostrando que procesa. */
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
    timeoutMs,
    totalTimeoutMs,
    body: JSON.stringify({
      model,
      store: false,
      system_instruction: systemInstruction.replace("{now}", now),
      input: text,
      generation_config: { max_output_tokens: maxOutputTokens, thinking_level: thinkingLevel },
      response_format: { type: "text", mime_type: "application/json", schema: responseSchema },
    }),
  };
}

export function analyzeRequest({ question, image }: AnalyzeInput): Upstream {
  const { model, timeoutMs, totalTimeoutMs, maxOutputTokens, systemInstruction, responseSchema } = contract.analyze;
  // La imagen se empalma como bytes. La marca es aleatoria para que la pregunta no pueda coincidir con ella.
  const marker = crypto.randomUUID();
  const [before, after] = JSON.stringify({
    model,
    store: false,
    system_instruction: systemInstruction,
    input: [
      { type: "text", text: question },
      { type: "image", data: marker, mime_type: "image/jpeg" },
    ],
    generation_config: { max_output_tokens: maxOutputTokens },
    response_format: { type: "text", mime_type: "application/json", schema: responseSchema },
  }).split(marker) as [string, string];
  return { timeoutMs, totalTimeoutMs, body: new Blob([before, image, after]) };
}

async function errorCode(response: Response): Promise<ErrorCode> {
  if (response.status !== 429) return "unavailable";
  const envelope = await response.json<{ error?: { code?: unknown; status?: unknown; message?: unknown } }>().catch(() => undefined);
  const code = String(envelope?.error?.code ?? envelope?.error?.status ?? "").toLowerCase();
  const daily = code === "quota_exceeded" || (code === "resource_exhausted" && String(envelope?.error?.message).toLowerCase().includes("per day"));
  return daily ? "service_quota" : "rate_limited";
}

async function send(upstream: Upstream, key: string, timeoutMs: number): Promise<ModelAnswer> {
  let response: Response;
  try {
    response = await fetch(`${contract.baseUrl}/interactions`, {
      method: "POST",
      headers: { "Content-Type": "application/json", "x-goog-api-key": key, "Api-Revision": contract.apiRevision },
      body: upstream.body,
      signal: AbortSignal.timeout(timeoutMs),
    });
  } catch (error) {
    throw transient(error instanceof Error && error.name === "TimeoutError" ? "timeout" : "unavailable");
  }
  if (response.status >= 500) throw transient("unavailable");
  if (!response.ok) throw new ApiError(await errorCode(response));
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
  return { output, tokens: answer.usage?.total_tokens ?? 0 };
}

/** Un único reintento, solo ante un fallo pasajero y dentro del tiempo total: un 429 nunca se repite. */
export async function callGemini(upstream: Upstream, key: string): Promise<ModelAnswer> {
  const deadline = Date.now() + upstream.totalTimeoutMs;
  try {
    return await send(upstream, key, upstream.timeoutMs);
  } catch (error) {
    const remaining = deadline - Date.now() - RETRY_DELAY_MS;
    if (!isTransient(error) || remaining < MIN_RETRY_MS) throw error;
    await new Promise((resolve) => setTimeout(resolve, RETRY_DELAY_MS));
    return send(upstream, key, Math.min(upstream.timeoutMs, remaining));
  }
}
