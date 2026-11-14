import { useEffect, useRef, useState, type FormEvent, type ReactNode } from "react";

import { Button } from "@/components/Button";
import { Icon } from "@/components/Icon";

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
}

type Tone = "neutral" | "signal" | "query" | "error";

/** Línea superior de cada tono. El fondo amarillo es una capa aparte, que se enciende por opacidad. */
const toneBorders: Record<Tone, string> = {
  neutral: "border-t border-line",
  signal: "border-y border-on-signal text-on-signal focus-on-signal",
  query: "border-t-2 border-query",
  error: "border-t-2 border-error",
};

/** La tecla principal: ancha y alta cuando hay sitio; cuadrada, solo con el icono, cuando no. */
const mainKey = "shrink-0 @bar-labels:min-h-16 @bar-labels:w-34";

/** Control que recibe el foco cuando cambia el estado y el foco estaba en la barra (o se perdió con el cambio). */
const AUTOFOCUS = "data-bar-focus";

/** Nombre visible solo cuando cabe. Si no, sigue siendo el nombre accesible del botón. */
function Label({ children }: { children: string }) {
  return <span className="sr-only @bar-labels:not-sr-only">{children}</span>;
}

/** Estado en texto. En barras estrechas solo lo oyen los lectores de pantalla: los controles ya lo muestran. */
function Status({ text, className = "" }: { text: string; className?: string }) {
  return (
    <span role="status" aria-live="polite" className={`sr-only font-mono text-sm @bar-status:not-sr-only @bar-status:w-34 @bar-status:shrink-0 ${className}`}>
      {text}
    </span>
  );
}

interface Content {
  tone: Tone;
  /** Texto que explica el estado. Si es largo, se desplaza por dentro: los controles nunca se cortan. */
  message?: ReactNode;
  controls: ReactNode;
}

function IdleControls({ onSpeak, onSubmitText, onAnalyzeScreen }: Pick<SignalBarProps, "onSpeak" | "onSubmitText" | "onAnalyzeScreen">) {
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
      <Button variant="primary" title="Hablar" className={mainKey} onClick={onSpeak}>
        <Icon name="mic" size="md" />
        <Label>Hablar</Label>
      </Button>
      <form onSubmit={submit} className="min-w-24 flex-1 basis-24">
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
      <Button title="Analizar pantalla" className="shrink-0" onClick={onAnalyzeScreen}>
        <Icon name="screen" size="md" />
        <Label>Analizar pantalla</Label>
      </Button>
      <Status text="En espera" className="text-muted" />
    </>
  );
}

function ProgressControls({ step, filled, status, onCancel }: { step: string; filled: boolean; status: string; onCancel: () => void }) {
  return (
    <>
      <div className="flex min-w-24 flex-1 basis-24 flex-col gap-1.5">
        <span className="break-words">{step}</span>
        {/* Indeterminado mientras procesa; lleno al responder. Con Reducir movimiento queda quieto y el texto manda. */}
        <div aria-hidden="true" data-clip-ok className={`h-0.5 overflow-hidden ${filled ? "bg-ink" : "bg-hairline"}`}>
          {!filled && <div className="h-full w-1/4 animate-sweep bg-ink" />}
        </div>
      </div>
      <Button className="shrink-0" onClick={onCancel} {...{ [AUTOFOCUS]: "" }}>
        Cancelar
      </Button>
      <Status text={status} className={filled ? "font-medium text-ink" : "text-muted"} />
    </>
  );
}

/** Una frase basta para decidir. El aviso completo de privacidad queda a un clic. */
function useCaptureConsent(provider: string, notice: string, onConfirm: () => void, onCancel: () => void): Content {
  const [details, setDetails] = useState(false);
  return {
    tone: "signal",
    message: (
      <>
        <p className="m-0">
          Se tomará una sola imagen de tu pantalla y se enviará a {provider}. No captures información personal.{" "}
          <button type="button" aria-expanded={details} onClick={() => setDetails(!details)} className="bg-transparent p-0 font-semibold underline">
            {details ? "Menos información" : "Más información"}
          </button>
        </p>
        {details && <p className="m-0 mt-2 animate-fade text-sm">{notice} Nexium no guarda la imagen.</p>}
      </>
    ),
    controls: (
      <>
        <Button variant="signal" className="shrink-0" onClick={onConfirm} {...{ [AUTOFOCUS]: "" }}>
          Capturar
        </Button>
        <Button variant="onSignal" className="shrink-0" onClick={onCancel}>
          Cancelar
        </Button>
        <span className="flex-1" />
        <Status text="Esperando permiso" className="font-medium" />
      </>
    ),
  };
}

function useContent(props: SignalBarProps): Content {
  const { state, levels, onCancel } = props;
  const consent = useCaptureConsent(
    state.kind === "captureConsent" ? state.provider : "",
    state.kind === "captureConsent" ? state.notice : "",
    props.onConfirmCapture,
    onCancel,
  );
  switch (state.kind) {
    case "idle":
      return { tone: "neutral", controls: <IdleControls {...props} /> };
    case "listening":
      return {
        tone: "signal",
        controls: (
          <>
            <Button variant="signal" className={mainKey} onClick={props.onStopListening} {...{ [AUTOFOCUS]: "" }}>
              Terminar
            </Button>
            <LevelTrace levels={levels} />
            <Button variant="onSignal" className="shrink-0" onClick={onCancel}>
              Cancelar
            </Button>
            <Status text="Escuchando" className="font-medium" />
          </>
        ),
      };
    case "processing":
      return { tone: "neutral", controls: <ProgressControls step={state.step} filled={false} status="Procesando" onCancel={onCancel} /> };
    case "responding":
      return { tone: "neutral", controls: <ProgressControls step="Escribiendo la respuesta" filled status="Respondiendo" onCancel={onCancel} /> };
    case "captureConsent":
      return consent;
    case "clarify":
      // Las opciones están en la conversación, junto a la pregunta: la barra solo indica que falta elegir.
      return {
        tone: "query",
        controls: (
          <>
            <p className="m-0 min-w-24 flex-1 basis-24 font-semibold text-query">Elige una opción en la conversación.</p>
            <Button className="shrink-0" onClick={onCancel} {...{ [AUTOFOCUS]: "" }}>
              Cancelar
            </Button>
            <Status text="Pendiente" className="text-query" />
          </>
        ),
      };
    case "error":
      // Un error se lee una sola vez: si ya está en la conversación, la barra solo ofrece qué hacer.
      return {
        tone: "error",
        message: state.message ? (
          <p className="m-0 flex items-start gap-2 text-error">
            <span className="flex h-lh items-center">
              <Icon name="failed" />
            </span>
            <span className="min-w-0 break-words">{state.message}</span>
          </p>
        ) : undefined,
        controls: (
          <>
            {state.retryable && (
              <Button variant="primary" className="shrink-0" onClick={props.onRetry} {...{ [AUTOFOCUS]: "" }}>
                Reintentar
              </Button>
            )}
            <span role="status" aria-live="polite" className="sr-only">
              {state.message ? `No se pudo completar la solicitud. ${state.message}` : "No se pudo completar la solicitud. El motivo aparece en la conversación."}
            </span>
            <span className="flex-1" />
            <Button className="shrink-0" onClick={onCancel} {...(state.retryable ? {} : { [AUTOFOCUS]: "" })}>
              Cerrar
            </Button>
          </>
        ),
      };
  }
}

/**
 * Barra de señal: siempre entera al pie de la ventana. Tiene dos partes:
 *
 * - el mensaje del estado, si lo hay, que se desplaza por dentro cuando no cabe;
 * - la fila de controles, que nunca se desplaza ni se corta. Cuando la barra es estrecha, los botones muestran
 *   solo su icono y el estado en texto queda para los lectores de pantalla.
 *
 * El contenedor no cambia entre estados: la capa amarilla se enciende y se apaga con un fundido, y el contenido
 * de cada estado entra con otro. Si el foco estaba en la barra, pasa al control principal del estado nuevo.
 */
export function SignalBar(props: SignalBarProps) {
  const { tone, message, controls } = useContent(props);
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
    <section ref={bar} aria-label="Barra de señal" data-tone={tone} className={`@container relative isolate flex shrink-0 flex-col bg-base ${toneBorders[tone]}`}>
      <span
        aria-hidden="true"
        className={`absolute inset-0 -z-10 bg-signal transition-opacity duration-fast ease-instrument ${tone === "signal" ? "opacity-100" : "opacity-0"}`}
      />
      <div key={kind} className="flex animate-fade flex-col">
        {message && (
          <div data-bar-message className="max-h-bar-message overflow-y-auto px-4 pt-3 short:pt-2">
            {message}
          </div>
        )}
        <div className="flex flex-wrap items-center gap-x-3 gap-y-2 px-4 py-3 short:py-2 @bar-labels:gap-x-4">{controls}</div>
      </div>
    </section>
  );
}
