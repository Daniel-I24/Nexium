import type { Stage } from "@/types/api";

export type BarState =
  | { kind: "idle" }
  | { kind: "listening" }
  | { kind: "processing"; step: string }
  | { kind: "responding" }
  | { kind: "captureConsent"; provider: string; notice: string }
  /** Nexium espera una respuesta: las opciones están en la conversación. */
  | { kind: "clarify" }
  /** `message` es null cuando el error ya se ve en la conversación: la barra no lo repite. */
  | { kind: "error"; message: string | null; retryable: boolean };

const stepLabels: Partial<Record<Stage, string>> = {
  transcribing: "Transcribiendo tu voz en este equipo",
  interpreting: "Interpretando tu solicitud",
  executing: "Consultando",
  capturing: "Tomando una sola captura",
  analyzing: "Analizando la captura",
};

export function barStateFromStage(stage: Stage): BarState {
  if (stage === "idle") return { kind: "idle" };
  if (stage === "listening") return { kind: "listening" };
  if (stage === "responding") return { kind: "responding" };
  return { kind: "processing", step: stepLabels[stage] ?? "Procesando" };
}

export function isSensing(state: BarState): boolean {
  return state.kind === "listening" || state.kind === "captureConsent";
}
