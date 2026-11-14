import { useEffect, useRef, useState, type FormEvent, type ReactNode } from "react";

import { Button } from "@/components/Button";
import { Icon } from "@/components/Icon";
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

/** Línea superior de cada tono. El fondo amarillo es una capa aparte, que se enciende por opacidad. */
const toneBorders: Record<Tone, string> = {
  neutral: "border-t border-line",
  signal: "border-y border-on-signal text-on-signal focus-on-signal",
  query: "border-t-2 border-query",
  error: "border-t-2 border-error",
};

const voiceKey = "min-h-16 w-34 shrink-0";

/** Control que recibe el foco cuando cambia el estado y el foco estaba en la barra (o se perdió con el cambio). */
const AUTOFOCUS = "data-bar-focus";

function Status({ text, className = "" }: { text: string; className?: string }) {
  return (
    <span role="status" aria-live="polite" className={`w-34 shrink-0 font-mono text-sm ${className}`}>
      {text}
    </span>
  );
}

/** Un error se lee una sola vez: si ya está en la conversación, la barra solo ofrece qué hacer. */
function ErrorContent({ message, retryable, onRetry, onClose }: { message: string | null; retryable: boolean; onRetry: () => void; onClose: () => void }) {
  return (
    <>
      {retryable && (
        <Button variant="primary" className={voiceKey} onClick={onRetry} {...{ [AUTOFOCUS]: "" }}>
          Reintentar
        </Button>
      )}
      {message ? (
        <p role="status" aria-live="polite" className="m-0 flex min-w-0 flex-1 basis-48 items-start gap-2 text-error">
          <span className="flex h-lh items-center">
            <Icon name="failed" />
          </span>
          <span>{message}</span>
        </p>
      ) : (
        <span role="status" aria-live="polite" className="sr-only">
          No se pudo completar la solicitud. El motivo aparece en la conversación.
        </span>
      )}
      <Button className={message ? undefined : "ml-auto"} onClick={onClose} {...(retryable ? {} : { [AUTOFOCUS]: "" })}>
        Cerrar
      </Button>
    </>
  );
}

/** Una frase basta para decidir. El aviso completo de privacidad queda a un clic. */
function CaptureConsent({ provider, notice, onConfirm, onCancel }: { provider: string; notice: string; onConfirm: () => void; onCancel: () => void }) {
  const [details, setDetails] = useState(false);
  return (
    <>
      <Button variant="signal" className={voiceKey} onClick={onConfirm} {...{ [AUTOFOCUS]: "" }}>
        Capturar
      </Button>
      {/* El aviso es lo que hay que leer: en una ventana estrecha ocupa su propia fila. */}
      <div className="min-w-0 flex-1 basis-80">
        <p className="m-0">
          Se tomará una sola imagen de tu pantalla y se enviará a {provider}. No captures información personal.{" "}
          <button type="button" aria-expanded={details} onClick={() => setDetails(!details)} className="bg-transparent p-0 font-semibold underline">
            {details ? "Menos información" : "Más información"}
          </button>
        </p>
        {details && <p className="m-0 mt-2 animate-fade text-sm">{notice} Nexium no guarda la imagen.</p>}
      </div>
      <Button variant="onSignal" onClick={onCancel}>
        Cancelar
      </Button>
      <Status text="Esperando permiso" className="font-medium" />
    </>
  );
}

function IdleContent({ onSpeak, onSubmitText, onAnalyzeScreen }: Pick<SignalBarProps, "onSpeak" | "onSubmitText" | "onAnalyzeScreen">) {
  const [text, setText] = useState("");
  const submit = (event: FormEvent) => {
    event.preventDefault();
    const value = text.trim();
    if (!value) return;
    onSubmitText(value);
    setText("");
  };
  return (
    <>
      <Button variant="primary" className={voiceKey} onClick={onSpeak}>
        <Icon name="mic" size="md" />
        Hablar
      </Button>
      <form onSubmit={submit} className="min-w-0 flex-1 basis-48">
        <input
          aria-label="Escribe tu solicitud"
          placeholder="O escribe tu solicitud"
          maxLength={500}
          value={text}
          onChange={(event) => setText(event.target.value)}
          className="h-11 w-full min-w-0 rounded-sm border border-line bg-panel px-3 text-ink placeholder:text-muted"
          {...{ [AUTOFOCUS]: "" }}
        />
      </form>
      <Button onClick={onAnalyzeScreen}>
        <Icon name="screen" size="md" />
        Analizar pantalla
      </Button>
      <Status text="En espera" className="text-muted" />
    </>
  );
}

function ProgressContent({ step, filled, status, onCancel }: { step: string; filled: boolean; status: string; onCancel: () => void }) {
  return (
    <>
      <Button disabled className={voiceKey}>
        <Icon name="mic" size="md" />
        Hablar
      </Button>
      <div className="flex min-w-0 flex-1 basis-48 flex-col gap-1.5">
        <span>{step}</span>
        {/* Indeterminado mientras procesa; lleno al responder. Con Reducir movimiento queda quieto y el texto manda. */}
        <div aria-hidden="true" data-clip-ok className={`h-0.5 overflow-hidden ${filled ? "bg-ink" : "bg-hairline"}`}>
          {!filled && <div className="h-full w-1/4 animate-sweep bg-ink" />}
        </div>
      </div>
      <Button onClick={onCancel} {...{ [AUTOFOCUS]: "" }}>
        Cancelar
      </Button>
      <Status text={status} className={filled ? "font-medium text-ink" : "text-muted"} />
    </>
  );
}

function content(props: SignalBarProps): { tone: Tone; body: ReactNode } {
  const { state, levels, onCancel } = props;
  switch (state.kind) {
    case "idle":
      return { tone: "neutral", body: <IdleContent {...props} /> };
    case "listening":
      return {
        tone: "signal",
        body: (
          <>
            <Button variant="signal" className={voiceKey} onClick={props.onStopListening} {...{ [AUTOFOCUS]: "" }}>
              Terminar
            </Button>
            <LevelTrace levels={levels} />
            <Button variant="onSignal" onClick={onCancel}>
              Cancelar
            </Button>
            <Status text="Escuchando" className="font-medium" />
          </>
        ),
      };
    case "processing":
      return { tone: "neutral", body: <ProgressContent step={state.step} filled={false} status="Procesando" onCancel={onCancel} /> };
    case "responding":
      return { tone: "neutral", body: <ProgressContent step="Escribiendo la respuesta" filled status="Respondiendo" onCancel={onCancel} /> };
    case "captureConsent":
      return { tone: "signal", body: <CaptureConsent provider={state.provider} notice={state.notice} onConfirm={props.onConfirmCapture} onCancel={onCancel} /> };
    case "clarify":
      return {
        tone: "query",
        body: (
          <>
            <p className="m-0 min-w-0 flex-1 basis-48 font-semibold text-query">{state.question}</p>
            {state.reminderOptions.map((option, index) => (
              <Button key={option.dueAt} onClick={() => props.onChooseReminderOption(option)} {...(index === 0 ? { [AUTOFOCUS]: "" } : {})}>
                {option.label}
              </Button>
            ))}
            {state.suggestions.map((suggestion, index) => (
              <Button
                key={suggestion.request}
                onClick={() => props.onChooseSuggestion(suggestion)}
                {...(index === 0 && state.reminderOptions.length === 0 ? { [AUTOFOCUS]: "" } : {})}
              >
                {suggestion.label}
              </Button>
            ))}
            <Button onClick={onCancel}>Cancelar</Button>
            <Status text="Pendiente" className="text-query" />
          </>
        ),
      };
    case "error":
      return { tone: "error", body: <ErrorContent message={state.message} retryable={state.retryable} onRetry={props.onRetry} onClose={onCancel} /> };
  }
}

/**
 * Barra de señal. El contenedor no cambia entre estados: así la capa amarilla se enciende y se apaga con
 * un fundido, y el contenido de cada estado entra con otro. Si el foco estaba en la barra, pasa al control
 * principal del estado nuevo en lugar de perderse.
 */
export function SignalBar(props: SignalBarProps) {
  const { tone, body } = content(props);
  const bar = useRef<HTMLElement>(null);
  const kind = props.state.kind;
  const mounted = useRef(false);

  useEffect(() => {
    const element = bar.current;
    // Al abrir Nexium no se mueve el foco: solo cuando cambia el estado.
    if (!mounted.current) {
      mounted.current = true;
      return;
    }
    if (!element) return;
    const active = document.activeElement;
    const focusLost = active === null || active === document.body;
    if (focusLost || element.contains(active)) {
      element.querySelector<HTMLElement>(`[${AUTOFOCUS}]:not(:disabled)`)?.focus();
    }
  }, [kind]);

  return (
    <section
      ref={bar}
      aria-label="Barra de señal"
      data-tone={tone}
      className={`relative isolate flex max-h-half-window min-w-0 flex-col bg-base ${toneBorders[tone]}`}
    >
      <span
        aria-hidden="true"
        className={`absolute inset-0 -z-10 bg-signal transition-opacity duration-fast ease-instrument ${tone === "signal" ? "opacity-100" : "opacity-0"}`}
      />
      {/* Si el texto es muy grande y la ventana muy baja, la barra se desplaza por dentro en lugar de cortar controles. */}
      <div className="min-h-0 overflow-y-auto">
        <div key={kind} className="flex animate-fade flex-wrap items-center gap-x-4 gap-y-3 px-4 py-3">
          {body}
        </div>
      </div>
    </section>
  );
}
