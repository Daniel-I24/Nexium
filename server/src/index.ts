import type { Env } from "./env";
import { ApiError, errorResponse, errorStatus } from "./errors";
import { analyzeRequest, callGemini, interpretRequest, type ModelAnswer } from "./gemini";
import { enforceLimits } from "./limiter";
import { assertRegionAllowed } from "./region";
import { installId, interpretInput, ipBucket, MAX_INTERPRET_BODY, readCapture, readObject } from "./validation";

export { UsageCounter } from "./limiter";

type Operation = "interpret" | "analyze";

const OPERATIONS: Record<string, Operation> = { "/v1/interpret": "interpret", "/v1/analyze": "analyze" };

async function run(operation: Operation, request: Request, env: Env): Promise<ModelAnswer> {
  assertRegionAllowed(request.cf?.country);
  const install = installId(request.headers.get("X-Nexium-Install"));
  const upstream = operation === "interpret" ? interpretRequest(interpretInput(await readObject(request, MAX_INTERPRET_BODY))) : analyzeRequest(await readCapture(request));
  if (!env.GEMINI_API_KEY) throw new ApiError("unavailable");
  await enforceLimits(env, env.LIMITS[operation], operation, install, ipBucket(request.headers.get("CF-Connecting-IP") ?? "unknown"));
  return callGemini(upstream, env.GEMINI_API_KEY);
}

/** Solo métricas: nunca el texto, la imagen, la dirección IP ni el identificador de instalación. */
function logMetric(operation: Operation | "unknown", code: string, status: number, startedAt: number, tokens: number): void {
  console.log(JSON.stringify({ operation, code, status, durationMs: Date.now() - startedAt, tokens }));
}

export async function handleRequest(request: Request, env: Env): Promise<Response> {
  const startedAt = Date.now();
  const operation = OPERATIONS[new URL(request.url).pathname];
  try {
    if (!operation) throw new ApiError("not_found");
    if (request.method !== "POST") throw new ApiError("method_not_allowed");
    const answer = await run(operation, request, env);
    logMetric(operation, "ok", 200, startedAt, answer.tokens);
    return Response.json(answer);
  } catch (error) {
    const known = error instanceof ApiError ? error : new ApiError("unavailable");
    logMetric(operation ?? "unknown", error instanceof ApiError ? known.code : "internal", errorStatus(known.code), startedAt, 0);
    return errorResponse(known);
  }
}

export default { fetch: handleRequest } satisfies ExportedHandler<Env>;
