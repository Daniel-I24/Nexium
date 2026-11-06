import { Mic, ScanLine } from "lucide-react";
import { useState, type FormEvent, type ReactNode } from "react";

import { Button } from "@/components/Button";
import type { ReminderOption, SuggestedRequest } from "@/types/api";

import type { BarState } from "./barState";
import { LevelTrace } from "./LevelTrace";

interface SignalBarProps {
  state: BarState;
  levels: number[];
  onSpeak: () => void;
  onStopListening: () => void;
  onCancel: () => void;
  onSubmitText: (text: string) => void;
  onAnalyzeScreen: () => void;
  onConfirmCapture: () => void;
  onRetry: () => void;
  onChooseReminderOption: (option: ReminderOption) => void;
  onChooseSuggestion: (suggestion: SuggestedRequest) => void;
}

type Tone = "neutral" | "signal" | "query" | "error";

const toneStyles: Record<Tone, string> = {
  neutral: "bg-base border-t border-line",
  signal: "bg-signal text-on-signal border-y border-on-signal",
  query: "bg-base border-t-2 border-query",
  error: "bg-base border-t-2 border-error",
};

function Frame({ tone, status, statusClass, children }: { tone: Tone; status: string; statusClass: string; children: ReactNode }) {
  return (
    <section aria-label="Barra de señal" className={`flex flex-wrap items-center gap-x-4 gap-y-3 px-4 py-3 ${toneStyles[tone]}`}>
      {children}
      <span role="status" aria-live="polite" className={`w-34 shrink-0 font-mono text-sm ${statusClass}`}>
        {status}
      </span>
    </section>
  );
}

const voiceKey = "h-16 w-34 shrink-0";

function IdleBar({ onSpeak, onSubmitText, onAnalyzeScreen }: Pick<SignalBarProps, "onSpeak" | "onSubmitText" | "onAnalyzeScreen">) {
  const [text, setText] = useState("");
  const submit = (event: FormEvent) => {
    event.preventDefault();
    const value = text.trim();
    if (!value) return;
    onSubmitText(value);
    setText("");
  };
  return (
    <Frame tone="neutral" status="En espera" statusClass="text-muted">
      <Button variant="primary" className={voiceKey} onClick={onSpeak}>
        <Mic size={20} strokeWidth={1.75} aria-hidden="true" />
        Hablar
      </Button>
      <form onSubmit={submit} className="min-w-48 flex-1">
        <input
          aria-label="Escribe tu solicitud"
          placeholder="O escribe tu solicitud"
          maxLength={500}
          value={text}
          onChange={(event) => setText(event.target.value)}
          className="h-11 w-full rounded-sm border border-line bg-panel px-3 text-ink placeholder:text-muted"
        />
      </form>
      <Button onClick={onAnalyzeScreen}>
        <ScanLine size={20} strokeWidth={1.75} aria-hidden="true" />
        Analizar pantalla
      </Button>
    </Frame>
  );
}

function ProgressBar({ step, filled, status, onCancel }: { step: string; filled: boolean; status: string; onCancel: () => void }) {
  return (
    <Frame tone="neutral" status={status} statusClass={filled ? "font-medium text-ink" : "text-muted"}>
      <Button disabled className={voiceKey}>
        Hablar
      </Button>
      <div className="flex min-w-48 flex-1 flex-col gap-1.5">
        <span>{step}</span>
        <div aria-hidden="true" className={`h-0.5 overflow-hidden ${filled ? "bg-ink" : "bg-hairline"}`}>
          {!filled && <div className="h-full w-1/4 animate-sweep bg-ink" />}
        </div>
      </div>
      <Button onClick={onCancel}>Cancelar</Button>
    </Frame>
  );
}

export function SignalBar(props: SignalBarProps) {
  const { state, levels, onCancel } = props;
  switch (state.kind) {
    case "idle":
      return <IdleBar {...props} />;
    case "listening":
      return (
        <Frame tone="signal" status="Escuchando" statusClass="font-medium">
          <Button variant="signal" className={voiceKey} onClick={props.onStopListening}>
            Terminar
          </Button>
          <LevelTrace levels={levels} />
          <Button variant="onSignal" onClick={onCancel}>
            Cancelar
          </Button>
        </Frame>
      );
    case "processing":
      return <ProgressBar step={state.step} filled={false} status="Procesando" onCancel={onCancel} />;
    case "responding":
      return <ProgressBar step="Escribiendo la respuesta" filled status="Respondiendo" onCancel={onCancel} />;
    case "captureConsent":
      return (
        <Frame tone="signal" status="Esperando permiso" statusClass="font-medium">
          <Button variant="signal" className={voiceKey} onClick={props.onConfirmCapture}>
            Capturar
          </Button>
          <p className="m-0 min-w-48 flex-1">
            Se tomará una sola imagen de tu pantalla y se enviará a {state.provider} para analizarla. {state.notice} Nexium no
            guarda la imagen.
          </p>
          <Button variant="onSignal" onClick={onCancel}>
            Cancelar
          </Button>
        </Frame>
      );
    case "clarify":
      return (
        <Frame tone="query" status="Pendiente" statusClass="text-query">
          <p className="m-0 min-w-48 flex-1 font-semibold text-query">{state.question}</p>
          {state.reminderOptions.map((option) => (
            <Button key={option.dueAt} onClick={() => props.onChooseReminderOption(option)}>
              {option.label}
            </Button>
          ))}
          {state.suggestions.map((suggestion) => (
            <Button key={suggestion.request} onClick={() => props.onChooseSuggestion(suggestion)}>
              {suggestion.label}
            </Button>
          ))}
          <Button onClick={onCancel}>Cancelar</Button>
        </Frame>
      );
    case "error":
      return (
        <Frame tone="error" status="Error" statusClass="text-error">
          {state.retryable && (
            <Button variant="primary" className={voiceKey} onClick={props.onRetry}>
              Reintentar
            </Button>
          )}
          <p className="m-0 min-w-48 flex-1 text-error">{state.message}</p>
          <Button onClick={onCancel}>Cerrar</Button>
        </Frame>
      );
  }
}
