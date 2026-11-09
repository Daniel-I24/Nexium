import type { UsageCounter } from "./limiter";

export interface Limits {
  perMinute: number;
  perDay: number;
}

export interface OperationLimits {
  install: Limits;
  ip: Limits;
  global: Limits;
}

export interface Env {
  GEMINI_API_KEY?: string;
  LIMITS: { interpret: OperationLimits; analyze: OperationLimits };
  USAGE_COUNTER: DurableObjectNamespace<UsageCounter>;
}
