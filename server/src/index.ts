import type { Env } from "./env";
import { ApiError, errorResponse, errorStatus } from "./errors";
import { analyzeRequest, callGemini, documentRequest, interpretRequest, type ModelAnswer, type Upstream } from "./gemini";
import { enforceLimits } from "./limiter";
import { assertRegionAllowed } from "./region";
import { documentInput, installId, interpretInput, ipBucket, MAX_DOCUMENT_BODY, MAX_INTERPRET_BODY, readCapture, readObject } from "./validation";

export { UsageCounter } from "./limiter";

type Operation = "interpret" | "analyze" | "document";

const OPERATIONS: Record<string, Operation> = { "/v1/interpret": "interpret", "/v1/analyze": "analyze", "/v1/document": "document" };

/** Lee y valida el cuerpo de cada operación antes de gastar cupo. */
async function upstreamFor(operation: Operation, request: Request): Promise<Upstream> {
  if (operation === "interpret") return interpretRequest(interpretInput(await readObject(request, MAX_INTERPRET_BODY)));
  if (operation === "document") return documentRequest(documentInput(await readObject(request, MAX_DOCUMENT_BODY)));
  return analyzeRequest(await readCapture(request));
}

async function run(operation: Operation, request: Request, env: Env): Promise<ModelAnswer> {
  assertRegionAllowed(request.cf?.country);
  const install = installId(request.headers.get("X-Nexium-Install"));
  const upstream = await upstreamFor(operation, request);
  if (!env.GEMINI_API_KEY) throw new ApiError("unavailable");
  await enforceLimits(env, env.LIMITS[operation], operation, install, ipBucket(request.headers.get("CF-Connecting-IP") ?? "unknown"));
  return callGemini(upstream, env.GEMINI_API_KEY);
}

/** Solo métricas: nunca el texto, la imagen, la dirección IP ni el identificador de instalación. */
function logMetric(operation: Operation | "unknown", code: string, status: number, startedAt: number, tokens: number, extra: { model?: string; upstream?: unknown } = {}): void {
  console.log(JSON.stringify({ operation, code, status, durationMs: Date.now() - startedAt, tokens, ...extra }));
}

export async function handleRequest(request: Request, env: Env): Promise<Response> {
  const startedAt = Date.now();
  const operation = OPERATIONS[new URL(request.url).pathname];
  try {
    if (!operation) throw new ApiError("not_found");
    if (request.method !== "POST") throw new ApiError("method_not_allowed");
    const { output, tokens, model } = await run(operation, request, env);
    logMetric(operation, "ok", 200, startedAt, tokens, { model });
    return Response.json({ output, tokens });
  } catch (error) {
    const known = error instanceof ApiError ? error : new ApiError("unavailable");
    const upstream = error instanceof ApiError && "upstream" in error ? error.upstream : undefined;
    logMetric(operation ?? "unknown", error instanceof ApiError ? known.code : "internal", errorStatus(known.code), startedAt, 0, { upstream });
    return errorResponse(known);
  }
}

export default { fetch: handleRequest } satisfies ExportedHandler<Env>;
