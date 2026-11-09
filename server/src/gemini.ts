import contract from "./ai-contract.json";
import { ApiError, type ErrorCode } from "./errors";
import type { AnalyzeInput, InterpretInput } from "./validation";

const RETRY_DELAY_MS = 1000;

export interface ModelAnswer {
  output: string;
  tokens: number;
}

interface Upstream {
  body: unknown;
  timeoutMs: number;
}

interface InteractionResponse {
  steps?: { type?: string; content?: { type?: string; text?: string }[] }[];
  usage?: { total_tokens?: number };
}

export function interpretRequest({ text, now }: InterpretInput): Upstream {
  const { model, timeoutMs, maxOutputTokens, thinkingLevel, systemInstruction, responseSchema } = contract.interpret;
  return {
    timeoutMs,
    body: {
      model,
      store: false,
      system_instruction: systemInstruction.replace("{now}", now),
      input: text,
      generation_config: { max_output_tokens: maxOutputTokens, thinking_level: thinkingLevel },
      response_format: { type: "text", mime_type: "application/json", schema: responseSchema },
    },
  };
}

export function analyzeRequest({ question, image }: AnalyzeInput): Upstream {
  const { model, timeoutMs, maxOutputTokens, systemInstruction } = contract.analyze;
  return {
    timeoutMs,
    body: {
      model,
      store: false,
      system_instruction: systemInstruction,
      input: [
        { type: "text", text: question },
        { type: "image", data: image, mime_type: "image/jpeg" },
      ],
      generation_config: { max_output_tokens: maxOutputTokens },
    },
  };
}

async function errorCode(response: Response): Promise<ErrorCode> {
  if (response.status !== 429) return "unavailable";
  const envelope = await response.json<{ error?: { code?: unknown; status?: unknown; message?: unknown } }>().catch(() => undefined);
  const code = String(envelope?.error?.code ?? envelope?.error?.status ?? "").toLowerCase();
  const daily = code === "quota_exceeded" || (code === "resource_exhausted" && String(envelope?.error?.message).toLowerCase().includes("per day"));
  return daily ? "service_quota" : "rate_limited";
}

async function send(upstream: Upstream, key: string): Promise<ModelAnswer> {
  let response: Response;
  try {
    response = await fetch(`${contract.baseUrl}/interactions`, {
      method: "POST",
      headers: { "Content-Type": "application/json", "x-goog-api-key": key, "Api-Revision": contract.apiRevision },
      body: JSON.stringify(upstream.body),
      signal: AbortSignal.timeout(upstream.timeoutMs),
    });
  } catch (error) {
    throw new ApiError(error instanceof Error && error.name === "TimeoutError" ? "timeout" : "unavailable");
  }
  if (!response.ok) throw Object.assign(new ApiError(await errorCode(response)), { upstreamStatus: response.status });
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

/** Un único reintento y solo ante un corte: un 429 nunca se repite. */
export async function callGemini(upstream: Upstream, key: string): Promise<ModelAnswer> {
  try {
    return await send(upstream, key);
  } catch (error) {
    const retryable = error instanceof ApiError && error.code === "unavailable" && "upstreamStatus" in error && Number(error.upstreamStatus) >= 500;
    if (!retryable) throw error;
    await new Promise((resolve) => setTimeout(resolve, RETRY_DELAY_MS));
    return send(upstream, key);
  }
}
