import { DurableObject } from "cloudflare:workers";

import type { Env, Limits, OperationLimits } from "./env";
import { ApiError } from "./errors";

const MINUTE_MS = 60_000;
const DAY_MS = 86_400_000;
const STATE_KEY = "state";

interface CounterState {
  minute: number;
  minuteCount: number;
  day: number;
  dayCount: number;
}

export type ConsumeResult = { allowed: true } | { allowed: false; window: "minute" | "day"; retryAfterSeconds: number };

/** Un contador por instalación, por red y global. Solo guarda números y se borra solo. */
export class UsageCounter extends DurableObject<Env> {
  async consume(limits: Limits, now: number = Date.now()): Promise<ConsumeResult> {
    const minute = Math.floor(now / MINUTE_MS);
    const day = Math.floor(now / DAY_MS);
    const stored = await this.ctx.storage.get<CounterState>(STATE_KEY);
    const minuteCount = stored?.minute === minute ? stored.minuteCount : 0;
    const dayCount = stored?.day === day ? stored.dayCount : 0;

    if (dayCount >= limits.perDay) return { allowed: false, window: "day", retryAfterSeconds: Math.ceil(((day + 1) * DAY_MS - now) / 1000) };
    if (minuteCount >= limits.perMinute) return { allowed: false, window: "minute", retryAfterSeconds: Math.ceil(((minute + 1) * MINUTE_MS - now) / 1000) };

    await this.ctx.storage.put<CounterState>(STATE_KEY, { minute, minuteCount: minuteCount + 1, day, dayCount: dayCount + 1 });
    if (!stored) await this.ctx.storage.setAlarm(this.expiry(day));
    return { allowed: true };
  }

  override async alarm(): Promise<void> {
    const stored = await this.ctx.storage.get<CounterState>(STATE_KEY);
    const today = Math.floor(Date.now() / DAY_MS);
    if (stored && stored.day >= today) await this.ctx.storage.setAlarm(this.expiry(today));
    else await this.ctx.storage.deleteAll();
  }

  private expiry(day: number): number {
    return (day + 1) * DAY_MS + MINUTE_MS;
  }
}

function assertAllowed(result: ConsumeResult, globalScope: boolean): void {
  if (result.allowed) return;
  if (result.window === "minute") throw new ApiError("rate_limited", result.retryAfterSeconds);
  throw new ApiError(globalScope ? "service_quota" : "daily_limit", result.retryAfterSeconds);
}

/** La instalación y la red se comprueban antes: quien abusa no consume el cupo común. */
export async function enforceLimits(env: Env, limits: OperationLimits, operation: string, install: string, ip: string): Promise<void> {
  const counter = (name: string) => env.USAGE_COUNTER.getByName(`${name}:${operation}`);
  const [byInstall, byIp] = await Promise.all([counter(`install:${install}`).consume(limits.install), counter(`ip:${ip}`).consume(limits.ip)]);
  assertAllowed(byInstall, false);
  assertAllowed(byIp, false);
  assertAllowed(await counter("global").consume(limits.global), true);
}
