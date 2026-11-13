import { env, exports } from "cloudflare:workers";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { Env, OperationLimits } from "../src/env";
import { callGemini } from "../src/gemini";
import { handleRequest } from "../src/index";
import { REAL_JPEG, REAL_PNG } from "./captures";
import { CAPTURE_CONTENT_TYPE, MAX_IMAGE_BYTES, MAX_TEXT_CHARS } from "../src/validation";

const INSTALL = "3f2b8a10-5c4e-4d7a-9b1c-0a1b2c3d4e5f";
const NOW = "2026-11-09T10:30:00-05:00";
const JPEG = "/9j/4AAQ";
const WIDE = { perMinute: 1000, perDay: 1000 };
const WIDE_LIMITS: OperationLimits = { install: WIDE, ip: WIDE, global: WIDE };

let namespaceId = 0;

/** Cada prueba usa contadores propios, del Durable Object real. */
function testEnv(overrides: Partial<OperationLimits> = {}): Env {
  const prefix = `prueba-${++namespaceId}-${Math.random()}`;
  const real = env as unknown as Env;
  const limits = { ...WIDE_LIMITS, ...overrides };
  return {
    ...real,
    LIMITS: { interpret: limits, analyze: limits },
    USAGE_COUNTER: { getByName: (name: string) => real.USAGE_COUNTER.getByName(`${prefix}:${name}`) } as unknown as Env["USAGE_COUNTER"],
  };
}

function post(path: string, body: unknown, headers: Record<string, string> = {}, country: string | null = "CO"): Request {
  const raw = typeof body === "string" ? body : JSON.stringify(body);
  return new Request(`https://nexium.test${path}`, {
    method: "POST",
    headers: { "Content-Type": "application/json", "Content-Length": String(new TextEncoder().encode(raw).length), "X-Nexium-Install": INSTALL, "CF-Connecting-IP": "203.0.113.7", ...headers },
    body: raw,
    ...(country === null ? {} : { cf: { country } as unknown as IncomingRequestCfProperties }),
  });
}

function capture(image: string = JPEG, question: unknown = "¿Qué es?", imageType: unknown = "image/jpeg"): Request {
  return post("/v1/analyze", `${JSON.stringify({ question, imageType })}\n${image}`, { "Content-Type": CAPTURE_CONTENT_TYPE });
}

const interpret = (headers?: Record<string, string>, text = "Busca mi informe") => post("/v1/interpret", { text, now: NOW }, headers);

function geminiReplies(...replies: (() => Response)[]) {
  const spy = vi.spyOn(globalThis, "fetch");
  for (const reply of replies) spy.mockImplementationOnce(() => Promise.resolve(reply()));
  return spy;
}

const success = (text = '{"intent":"unknown"}') => () =>
  Response.json({ usage: { total_tokens: 42 }, steps: [{ type: "thought", content: [{ type: "text", text: "no" }] }, { type: "model_output", content: [{ type: "text", text }] }] });
const failure = (status: number, code: string, message = "detalle interno de Google") => () => Response.json({ error: { code, message } }, { status });

async function errorOf(response: Response): Promise<string> {
  return (await response.json<{ error: { code: string } }>()).error.code;
}

afterEach(() => vi.restoreAllMocks());

describe("operaciones", () => {
  it("el Worker desplegable responde por su entrada real", async () => {
    const response = await (exports as unknown as { default: Fetcher }).default.fetch("https://nexium.test/no-existe");
    expect(response.status).toBe(404);
  });

  it("no expone más que las dos operaciones, solo por POST", async () => {
    const spy = vi.spyOn(globalThis, "fetch");
    for (const path of ["/", "/health", "/v1/models", "/v1/interactions"]) {
      expect((await handleRequest(post(path, {}), testEnv())).status).toBe(404);
    }
    expect((await handleRequest(new Request("https://nexium.test/v1/interpret"), testEnv())).status).toBe(405);
    expect(spy).not.toHaveBeenCalled();
  });

  it("interpreta con el modelo y las instrucciones del servidor", async () => {
    const spy = geminiReplies(success());
    const response = await handleRequest(interpret(), testEnv());
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({ output: '{"intent":"unknown"}', tokens: 42 });
    const [url, init] = spy.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("https://generativelanguage.googleapis.com/v1beta/interactions");
    expect(new Headers(init.headers).get("x-goog-api-key")).toBe("clave-de-prueba");
    const sent = JSON.parse(init.body as string);
    expect(sent.model).toBe("gemini-3.5-flash-lite");
    expect(sent.store).toBe(false);
    expect(sent.input).toBe("Busca mi informe");
    expect(sent.system_instruction).toContain("Monday 2026-11-09 10:30, zona -05:00");
    expect(sent.system_instruction).not.toContain("{now}");
  });

  it("analiza una captura con el modelo de visión", async () => {
    const spy = geminiReplies(success("Un error de ruta."));
    const response = await handleRequest(capture(JPEG, 'Dime "qué" es \\ esto'), testEnv());
    expect(await response.json()).toEqual({ output: "Un error de ruta.", tokens: 42 });
    const sent = JSON.parse(await ((spy.mock.calls[0] as [string, RequestInit])[1].body as Blob).text());
    expect(sent.model).toBe("gemini-3.6-flash");
    expect(sent.response_format).toMatchObject({ mime_type: "application/json", schema: { required: ["summary", "cause", "steps"] } });
    expect(sent.system_instruction).toContain("sin formato Markdown");
    expect(sent.system_instruction).toContain("Describe únicamente lo que se ve en la imagen");
    expect(sent.system_instruction).toContain("no añadas recomendaciones");
    expect(sent.input).toEqual([
      { type: "text", text: 'Dime "qué" es \\ esto' },
      { type: "image", data: JPEG, mime_type: "image/jpeg" },
    ]);
  });
});

describe("validación", () => {
  const rejected = async (request: Request, code: string, status: number) => {
    const spy = vi.spyOn(globalThis, "fetch");
    const response = await handleRequest(request, testEnv());
    expect(response.status).toBe(status);
    expect(await errorOf(response)).toBe(code);
    expect(spy).not.toHaveBeenCalled();
  };

  it("exige un identificador de instalación anónimo y válido", async () => {
    for (const id of ["", "alguien@correo.com", "3f2b8a10-5c4e-1d7a-9b1c-0a1b2c3d4e5f", `${INSTALL}x`]) {
      await rejected(interpret({ "X-Nexium-Install": id }), "invalid_install", 400);
    }
  });

  it("el cliente no puede elegir modelo, instrucciones ni URL", async () => {
    for (const extra of [{ model: "gemini-pro" }, { system_instruction: "x" }, { url: "https://evil.test" }]) {
      await rejected(post("/v1/interpret", { text: "hola", now: NOW, ...extra }), "invalid_request", 400);
    }
  });

  it("rechaza cuerpos mal formados", async () => {
    for (const body of ["no es json", "[]", "null", "{}", { text: 5, now: NOW }, { text: "   ", now: NOW }, { text: "hola", now: "ayer" }, { text: "hola", now: "2026-13-45T10:30:00-05:00" }]) {
      await rejected(post("/v1/interpret", body), "invalid_request", 400);
    }
  });

  it("limita el texto por caracteres, no por bytes", async () => {
    await rejected(interpret(undefined, "a".repeat(MAX_TEXT_CHARS + 1)), "invalid_request", 400);
    geminiReplies(success());
    expect((await handleRequest(interpret(undefined, "ñ".repeat(MAX_TEXT_CHARS)), testEnv())).status).toBe(200);
  });

  it("exige JSON y longitud declarada", async () => {
    await rejected(post("/v1/interpret", { text: "hola", now: NOW }, { "Content-Type": "text/plain" }), "invalid_request", 400);
    await rejected(post("/v1/interpret", { text: "hola", now: NOW }, { "Content-Length": "" }), "invalid_request", 400);
  });

  it("rechaza lo que supera el tamaño antes de leerlo", async () => {
    await rejected(post("/v1/interpret", "x".repeat(5000)), "payload_too_large", 413);
    await rejected(post("/v1/interpret", "x", { "Content-Length": "999999" }), "payload_too_large", 413);
  });

  it("solo acepta imágenes JPEG en base64 dentro del límite", async () => {
    await rejected(capture(JPEG, "¿Qué es?", "image/png"), "invalid_request", 400);
    await rejected(capture("/9j/AAAA"), "invalid_request", 400);
    await rejected(capture("/9j/"), "invalid_request", 400);
    await rejected(capture(""), "invalid_request", 400);
    await rejected(capture("iVBORw0KGgo="), "invalid_request", 400);
    await rejected(capture("/9j/4AA"), "invalid_request", 400);
    await rejected(capture("/9j/4A$Q"), "invalid_request", 400);
    await rejected(capture(`/9j/${"A".repeat(Math.ceil(MAX_IMAGE_BYTES / 3) * 4)}`), "payload_too_large", 413);
  });

  it("exige el formato de captura: una línea con los campos y después la imagen", async () => {
    const raw = (body: string, type = CAPTURE_CONTENT_TYPE) => post("/v1/analyze", body, { "Content-Type": type });
    await rejected(raw(JSON.stringify({ question: "¿Qué es?", imageType: "image/jpeg", image: JPEG })), "invalid_request", 400);
    await rejected(raw(`no es json\n${JPEG}`), "invalid_request", 400);
    await rejected(raw(`\n${JPEG}`), "invalid_request", 400);
    await rejected(raw(`${JSON.stringify({ question: "¿Qué es?" })}\n${JPEG}`), "invalid_request", 400);
    await rejected(raw(`${JSON.stringify({ question: "x", imageType: "image/jpeg", model: "gemini-pro" })}\n${JPEG}`), "invalid_request", 400);
    await rejected(raw(`${JSON.stringify({ question: "x", imageType: "image/jpeg" })}\n${JPEG}`, "application/json"), "invalid_request", 400);
    await rejected(raw(`${"x".repeat(5000)}\n${JPEG}`), "invalid_request", 400);
  });

  it("la imagen no puede salirse de su campo en el JSON enviado a Google", async () => {
    await rejected(capture('/9j/4AAQ","model":"otro"AA'.padEnd(28, "A")), "invalid_request", 400);
    await rejected(capture("/9j/4AAQ\\AAA"), "invalid_request", 400);
  });

  it("la pregunta no puede coincidir con la marca interna de la imagen", async () => {
    const spy = geminiReplies(success("ok"));
    await handleRequest(capture(JPEG, "00000000-0000-4000-8000-000000000000"), testEnv());
    expect(JSON.parse(await ((spy.mock.calls[0] as [string, RequestInit])[1].body as Blob).text()).input[1].data).toBe(JPEG);
  });

  it("acepta una captura JPEG real con cualquiera de los tres rellenos de base64", async () => {
    const bytes = Uint8Array.from(atob(REAL_JPEG), (char) => char.charCodeAt(0));
    for (const extra of [0, 1, 2]) {
      const padded = new Uint8Array([...bytes, ...new Array<number>(extra).fill(0)]);
      const image = btoa(String.fromCharCode(...padded));
      const spy = geminiReplies(success("ok"));
      expect((await handleRequest(capture(image), testEnv())).status).toBe(200);
      expect(JSON.parse(await ((spy.mock.calls.at(-1) as [string, RequestInit])[1].body as Blob).text()).input[1].data).toBe(image);
      spy.mockRestore();
    }
  });

  it("rechaza una captura PNG real aunque se declare JPEG", async () => {
    await rejected(capture(REAL_PNG), "invalid_request", 400);
    await rejected(capture(REAL_PNG, "¿Qué es?", "image/png"), "invalid_request", 400);
  });

  it("valida todo el alfabeto de base64, no solo el principio", async () => {
    const lastGroupStart = REAL_JPEG.length - 4;
    const withBadByte = (position: number, char: string) => `${REAL_JPEG.slice(0, position)}${char}${REAL_JPEG.slice(position + 1)}`;
    for (const char of ["$", " ", "-", "_", "\"", "\\", "ñ", "\r"]) {
      await rejected(capture(withBadByte(REAL_JPEG.length >> 1, char)), "invalid_request", 400);
      await rejected(capture(withBadByte(lastGroupStart, char)), "invalid_request", 400);
    }
    await rejected(capture(`${JPEG}$$$$`), "invalid_request", 400);
  });

  it("valida el relleno de base64", async () => {
    for (const image of [`${JPEG}A===`, `${JPEG}====`, `${JPEG}=AAA`, `${JPEG}A=AA`, `/9j/4A==${JPEG}`, `${JPEG}AAA=AAAA`]) {
      await rejected(capture(image), "invalid_request", 400);
    }
  });

  it("rechaza un relleno con bits sobrantes", async () => {
    await rejected(capture(`${JPEG}AB==`), "invalid_request", 400);
    await rejected(capture(`${JPEG}AAB=`), "invalid_request", 400);
    geminiReplies(success("ok"));
    expect((await handleRequest(capture(`${JPEG}AA==`), testEnv())).status).toBe(200);
  });

  it("no llama a Google si falta el secreto", async () => {
    const spy = vi.spyOn(globalThis, "fetch");
    const noKey: Env = { ...testEnv() };
    delete noKey.GEMINI_API_KEY;
    const response = await handleRequest(interpret(), noKey);
    expect(response.status).toBe(503);
    expect(spy).not.toHaveBeenCalled();
  });
});

describe("regiones", () => {
  const EEA_UK_CH = "AT BE BG HR CY CZ DK EE FI FR DE GR HU IE IT LV LT LU MT NL PL PT RO SK SI ES SE IS LI NO GB CH".split(" ");

  const refusal = async (country: string | null, code: string, message: string) => {
    const spy = vi.spyOn(globalThis, "fetch");
    const response = await handleRequest(post("/v1/interpret", { text: "hola", now: NOW }, {}, country), testEnv());
    expect(response.status).toBe(451);
    expect(await response.clone().text()).toContain(message);
    expect(await errorOf(response)).toBe(code);
    expect(spy).not.toHaveBeenCalled();
  };
  const blocked = (country: string) => refusal(country, "region_unavailable", "no está disponible en tu región");

  it.each(EEA_UK_CH)("bloquea %s", blocked);
  it.each(["GF", "GP", "MF", "MQ", "RE", "YT", "AX"])("bloquea el territorio de la UE %s", blocked);
  it.each(["XX", "T1", null])("explica que no se pudo determinar el país: %s", (country) => refusal(country, "country_unknown", "No se pudo determinar tu país"));

  it.each(["CO", "MX", "US", "AR", "JP"])("permite %s", async (country) => {
    geminiReplies(success());
    expect((await handleRequest(post("/v1/interpret", { text: "hola", now: NOW }, {}, country), testEnv())).status).toBe(200);
  });

  it("no se puede evitar con una cabecera", async () => {
    await handleRequest(post("/v1/interpret", { text: "hola", now: NOW }, { "CF-IPCountry": "CO" }, "DE"), testEnv()).then(async (response) => expect(await errorOf(response)).toBe("region_unavailable"));
  });

  it("no consume ningún cupo", async () => {
    const limited = testEnv({ global: { perMinute: 1, perDay: 1 } });
    await handleRequest(post("/v1/interpret", { text: "hola", now: NOW }, {}, "DE"), limited);
    geminiReplies(success());
    expect((await handleRequest(interpret(), limited)).status).toBe(200);
  });

  it("no registra el país", async () => {
    const log = vi.spyOn(console, "log");
    await handleRequest(post("/v1/interpret", { text: "hola", now: NOW }, {}, "DE"), testEnv());
    expect(log.mock.calls.flat().join(" ")).not.toContain("DE");
  });
});

describe("límites de uso", () => {
  // Los contadores usan el reloj real: una prueba no debe cruzar el cambio de minuto.
  beforeEach(async () => {
    const intoMinute = Date.now() % 60_000;
    if (intoMinute > 50_000) await new Promise((resolve) => setTimeout(resolve, 60_100 - intoMinute));
  });

  const otherInstall = (n: number) => ({ "X-Nexium-Install": `3f2b8a10-5c4e-4d7a-9b1c-0a1b2c3d4e${String(n).padStart(2, "0")}` });

  it("por minuto e instalación, con aviso de espera", async () => {
    const limited = testEnv({ install: { perMinute: 1, perDay: 10 } });
    geminiReplies(success());
    expect((await handleRequest(interpret(), limited)).status).toBe(200);
    const spy = vi.spyOn(globalThis, "fetch").mockClear();
    const response = await handleRequest(interpret(), limited);
    expect(response.status).toBe(429);
    expect(Number(response.headers.get("Retry-After"))).toBeGreaterThan(0);
    expect(await errorOf(response)).toBe("rate_limited");
    expect(spy).not.toHaveBeenCalled();
  });

  it("por día e instalación", async () => {
    const limited = testEnv({ install: { perMinute: 10, perDay: 1 } });
    geminiReplies(success());
    await handleRequest(interpret(), limited);
    expect(await errorOf(await handleRequest(interpret(), limited))).toBe("daily_limit");
  });

  it("inventar identificadores no evita el límite por IP", async () => {
    const limited = testEnv({ ip: { perMinute: 1, perDay: 10 } });
    geminiReplies(success());
    expect((await handleRequest(interpret(otherInstall(1)), limited)).status).toBe(200);
    expect(await errorOf(await handleRequest(interpret(otherInstall(2)), limited))).toBe("rate_limited");
  });

  it("toda una red IPv6 /64 comparte límite", async () => {
    const limited = testEnv({ ip: { perMinute: 1, perDay: 10 } });
    geminiReplies(success());
    await handleRequest(interpret({ ...otherInstall(1), "CF-Connecting-IP": "2001:db8:1:2:aaaa::1" }), limited);
    expect(await errorOf(await handleRequest(interpret({ ...otherInstall(2), "CF-Connecting-IP": "2001:db8:1:2:bbbb::2" }), limited))).toBe("rate_limited");
  });

  it("el cupo global agotado da el mensaje de cuota", async () => {
    const limited = testEnv({ global: { perMinute: 10, perDay: 1 } });
    geminiReplies(success());
    await handleRequest(interpret(otherInstall(1)), limited);
    const response = await handleRequest(interpret(otherInstall(2)), limited);
    expect(await response.clone().text()).toContain("cuota diaria gratuita");
    expect(await errorOf(response)).toBe("service_quota");
  });

  it("quien supera su límite no consume el cupo común", async () => {
    const limited = testEnv({ install: { perMinute: 1, perDay: 10 }, global: { perMinute: 10, perDay: 2 } });
    geminiReplies(success(), success());
    await handleRequest(interpret(), limited);
    await handleRequest(interpret(), limited);
    await handleRequest(interpret(), limited);
    expect((await handleRequest(interpret(otherInstall(3)), limited)).status).toBe(200);
  });

  it("interpretar y analizar tienen cupos separados", async () => {
    const limited = testEnv({ install: { perMinute: 1, perDay: 10 } });
    geminiReplies(success(), success());
    expect((await handleRequest(interpret(), limited)).status).toBe(200);
    expect((await handleRequest(capture(), limited)).status).toBe(200);
  });

  it("si el contador falla, no se llama a Google", async () => {
    const spy = vi.spyOn(globalThis, "fetch");
    const broken: Env = { ...testEnv(), USAGE_COUNTER: { getByName: () => { throw new Error("sin cuota de Durable Objects"); } } as unknown as Env["USAGE_COUNTER"] };
    expect((await handleRequest(interpret(), broken)).status).toBe(503);
    expect(spy).not.toHaveBeenCalled();
  });
});

describe("errores de Google", () => {
  const cases: [string, () => Response, string, number, number][] = [
    ["cuota diaria", failure(429, "quota_exceeded"), "service_quota", 429, 1],
    ["cuota heredada por día", failure(429, "RESOURCE_EXHAUSTED", "limit per day"), "service_quota", 429, 1],
    ["límite por minuto", failure(429, "rate_limit_exceeded"), "rate_limited", 429, 1],
    ["clave rechazada", failure(401, "authentication", "API key clave-de-prueba invalid"), "unavailable", 503, 1],
    ["región no admitida", failure(400, "failed_precondition"), "unavailable", 503, 1],
  ];

  it.each(cases)("%s", async (_name, reply, code, status, calls) => {
    const spy = geminiReplies(reply, reply);
    const response = await handleRequest(interpret(), testEnv());
    const text = await response.clone().text();
    expect(response.status).toBe(status);
    expect(await errorOf(response)).toBe(code);
    expect(spy).toHaveBeenCalledTimes(calls);
    expect(text).not.toContain("clave-de-prueba");
    expect(text).not.toContain("detalle interno");
  });

  it("en el análisis, un 503 pasa al modelo de respaldo con la imagen completa", async () => {
    const log = vi.spyOn(console, "log");
    const spy = geminiReplies(failure(503, "service_unavailable"), success("ok"));
    const response = await handleRequest(capture(), testEnv());
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({ output: "ok", tokens: 42 });
    const sent = await Promise.all(spy.mock.calls.map(([, init]) => ((init as RequestInit).body as Blob).text().then((text) => JSON.parse(text))));
    expect(sent.map((body) => body.model)).toEqual(["gemini-3.6-flash", "gemini-3.5-flash-lite"]);
    expect(sent[1].input[1].data).toBe(JPEG);
    expect(JSON.parse(log.mock.calls.flat().join(" "))).toMatchObject({ operation: "analyze", code: "ok", model: "gemini-3.5-flash-lite" });
  });

  it("en el análisis, si el respaldo también da 503, no hay un tercer intento", async () => {
    const log = vi.spyOn(console, "log");
    const spy = geminiReplies(failure(503, "service_unavailable"), failure(503, "service_unavailable"), success("ok"));
    const response = await handleRequest(capture(), testEnv());
    expect(response.status).toBe(503);
    expect(await errorOf(response)).toBe("unavailable");
    expect(spy).toHaveBeenCalledTimes(2);
    expect(JSON.parse(log.mock.calls.flat().join(" "))).toMatchObject({ upstream: { status: 503, model: "gemini-3.5-flash-lite" } });
  });

  it("en el análisis, otros fallos no se repiten con el mismo modelo", async () => {
    for (const reply of [failure(500, "internal"), failure(429, "rate_limit_exceeded")]) {
      const spy = geminiReplies(reply, success("ok"));
      expect((await handleRequest(capture(), testEnv())).status).not.toBe(200);
      expect(spy).toHaveBeenCalledTimes(1);
      spy.mockRestore();
    }
    const spy = vi.spyOn(globalThis, "fetch").mockRejectedValue(new DOMException("agotado", "TimeoutError"));
    expect(await errorOf(await handleRequest(capture(), testEnv()))).toBe("timeout");
    expect(spy).toHaveBeenCalledTimes(1);
  });

  it("al interpretar, el éxito registra el modelo y un corte se reintenta con el mismo", async () => {
    const log = vi.spyOn(console, "log");
    const spy = geminiReplies(failure(503, "service_unavailable"), success());
    expect((await handleRequest(interpret(), testEnv())).status).toBe(200);
    const models = await Promise.all(spy.mock.calls.map(([, init]) => JSON.parse((init as RequestInit).body as string).model));
    expect(models).toEqual(["gemini-3.5-flash-lite", "gemini-3.5-flash-lite"]);
    expect(JSON.parse(log.mock.calls.flat().join(" "))).toMatchObject({ code: "ok", model: "gemini-3.5-flash-lite" });
  });

  it("un corte se reintenta una sola vez", async () => {
    const reply = failure(503, "service_unavailable");
    const spy = geminiReplies(reply, reply, reply);
    expect(await errorOf(await handleRequest(interpret(), testEnv()))).toBe("unavailable");
    expect(spy).toHaveBeenCalledTimes(2);
  });

  it("el reintento puede salir bien", async () => {
    geminiReplies(failure(500, "internal"), success());
    expect((await handleRequest(interpret(), testEnv())).status).toBe(200);
  });

  it("un tiempo agotado se reintenta una vez y después se informa aparte", async () => {
    const spy = vi.spyOn(globalThis, "fetch").mockRejectedValue(new DOMException("agotado", "TimeoutError"));
    const response = await handleRequest(interpret(), testEnv());
    expect(response.status).toBe(504);
    expect(await errorOf(response)).toBe("timeout");
    expect(spy).toHaveBeenCalledTimes(2);
  });

  it("tras un tiempo agotado, el reintento puede salir bien", async () => {
    const spy = vi.spyOn(globalThis, "fetch").mockRejectedValueOnce(new DOMException("agotado", "TimeoutError")).mockResolvedValueOnce(success()());
    expect((await handleRequest(interpret(), testEnv())).status).toBe(200);
    expect(spy).toHaveBeenCalledTimes(2);
  });

  it("un fallo de red se reintenta una vez y no filtra detalles", async () => {
    const spy = vi.spyOn(globalThis, "fetch").mockRejectedValue(new Error("clave-de-prueba en el mensaje"));
    const response = await handleRequest(interpret(), testEnv());
    expect(await response.clone().text()).not.toContain("clave-de-prueba");
    expect(await errorOf(response)).toBe("unavailable");
    expect(spy).toHaveBeenCalledTimes(2);
  });

  it("no reintenta si ya no queda tiempo suficiente", async () => {
    const spy = vi.spyOn(globalThis, "fetch").mockImplementation(async () => {
      await new Promise((resolve) => setTimeout(resolve, 200));
      throw new DOMException("agotado", "TimeoutError");
    });
    await expect(callGemini({ model: "m", body: () => "{}", timeoutMs: 100, totalTimeoutMs: 3500 }, "clave-de-prueba")).rejects.toMatchObject({ code: "timeout" });
    expect(spy).toHaveBeenCalledTimes(1);
  });

  it("el reintento usa como mucho el tiempo que queda", async () => {
    const signals: AbortSignal[] = [];
    vi.spyOn(globalThis, "fetch").mockImplementation(async (_url, init) => {
      signals.push(init?.signal as AbortSignal);
      if (signals.length === 1) throw new DOMException("agotado", "TimeoutError");
      return success()();
    });
    await callGemini({ model: "m", body: () => "{}", timeoutMs: 60_000, totalTimeoutMs: 10_000 }, "clave-de-prueba");
    expect(signals).toHaveLength(2);
  });

  it("una respuesta sin texto o ilegible no se da por buena", async () => {
    geminiReplies(() => Response.json({ steps: [] }));
    expect(await errorOf(await handleRequest(interpret(), testEnv()))).toBe("no_output");
    geminiReplies(() => new Response("<html>"));
    expect(await errorOf(await handleRequest(interpret(), testEnv()))).toBe("unavailable");
  });
});

describe("registros", () => {
  it("un error de Google registra su estado y su código, nunca su mensaje", async () => {
    const log = vi.spyOn(console, "log");
    geminiReplies(failure(400, "invalid_argument", "detalle interno de Google con datos"));
    await handleRequest(interpret(), testEnv());
    const written = log.mock.calls.flat().join(" ");
    expect(JSON.parse(written)).toMatchObject({ code: "unavailable", upstream: { status: 400, code: "invalid_argument" } });
    expect(written).not.toContain("detalle interno");
  });

  it("solo contienen métricas, nunca datos del usuario", async () => {
    const log = vi.spyOn(console, "log");
    geminiReplies(success("respuesta privada"));
    await handleRequest(capture(JPEG, "pregunta privada"), testEnv());
    const written = log.mock.calls.flat().join(" ");
    expect(JSON.parse(written)).toMatchObject({ operation: "analyze", code: "ok", status: 200, tokens: 42 });
    for (const secret of ["pregunta privada", "respuesta privada", JPEG, INSTALL, "203.0.113.7", "clave-de-prueba"]) {
      expect(written).not.toContain(secret);
    }
  });
});
