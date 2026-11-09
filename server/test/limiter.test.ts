import { env } from "cloudflare:workers";
import { evictDurableObject, runDurableObjectAlarm, runInDurableObject } from "cloudflare:test";
import { describe, expect, it, vi } from "vitest";

import type { Env } from "../src/env";
import { ipBucket } from "../src/validation";

const DAY = 86_400_000;
// En el futuro: con una fecha pasada, la alarma de borrado se dispararía durante la prueba.
const START = (Math.floor(Date.now() / DAY) + 10) * DAY + 3_600_000;
const namespace = (env as unknown as Env).USAGE_COUNTER;
const counter = () => namespace.getByName(`limitador-${Math.random()}`);

describe("UsageCounter", () => {
  it("cuenta por minuto y se recupera en el minuto siguiente", async () => {
    const stub = counter();
    const limits = { perMinute: 2, perDay: 10 };
    expect(await stub.consume(limits, START)).toEqual({ allowed: true });
    expect(await stub.consume(limits, START + 1)).toEqual({ allowed: true });
    expect(await stub.consume(limits, START + 2)).toEqual({ allowed: false, window: "minute", retryAfterSeconds: 60 });
    expect((await stub.consume(limits, START + 60_000)).allowed).toBe(true);
  });

  it("cuenta por día y se renueva al día siguiente", async () => {
    const stub = counter();
    const limits = { perMinute: 10, perDay: 1 };
    await stub.consume(limits, START);
    expect(await stub.consume(limits, START + 1)).toMatchObject({ allowed: false, window: "day" });
    expect((await stub.consume(limits, START + DAY)).allowed).toBe(true);
  });

  it("los rechazos no se acumulan", async () => {
    const stub = counter();
    const limits = { perMinute: 1, perDay: 2 };
    await stub.consume(limits, START);
    for (let i = 1; i < 5; i++) await stub.consume(limits, START + i);
    expect((await stub.consume(limits, START + 60_000)).allowed).toBe(true);
  });

  it("sobrevive a que el objeto se expulse de memoria", async () => {
    const stub = counter();
    const limits = { perMinute: 1, perDay: 10 };
    await stub.consume(limits, START);
    await evictDurableObject(stub);
    expect((await stub.consume(limits, START + 1)).allowed).toBe(false);
  });

  it("solo guarda números y se programa su borrado", async () => {
    const stub = counter();
    const now = Date.now();
    await stub.consume({ perMinute: 5, perDay: 5 }, now);
    await runInDurableObject(stub, async (_instance, state) => {
      const stored = [...(await state.storage.list()).values()];
      expect(stored).toHaveLength(1);
      expect(Object.values(stored[0] as object).every((value) => typeof value === "number")).toBe(true);
      expect(await state.storage.getAlarm()).toBe((Math.floor(now / DAY) + 1) * DAY + 60_000);
    });
  });

  it("la alarma borra un contador caducado", async () => {
    const stub = counter();
    await stub.consume({ perMinute: 5, perDay: 5 }, Date.now() - 2 * DAY);
    // Su hora ya pasó: puede dispararse sola antes de esta llamada.
    await runDurableObjectAlarm(stub);
    await vi.waitFor(() => runInDurableObject(stub, async (_instance, state) => expect((await state.storage.list()).size).toBe(0)));
  });

  it("la alarma no borra un contador de hoy", async () => {
    const stub = counter();
    await stub.consume({ perMinute: 5, perDay: 5 }, Date.now());
    expect(await runDurableObjectAlarm(stub)).toBe(true);
    await runInDurableObject(stub, async (_instance, state) => {
      expect((await state.storage.list()).size).toBe(1);
      expect(await state.storage.getAlarm()).not.toBeNull();
    });
  });
});

describe("ipBucket", () => {
  it.each([
    ["203.0.113.7", "203.0.113.7"],
    ["2001:db8:1:2:aaaa:bbbb:cccc:dddd", "2001:db8:1:2"],
    ["2001:db8::1", "2001:db8:0:0"],
    ["2001:0DB8:0001:0002::", "2001:db8:1:2"],
    ["::1", "0:0:0:0"],
  ])("%s", (ip, bucket) => {
    expect(ipBucket(ip)).toBe(bucket);
  });
});
