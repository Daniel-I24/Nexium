import { useCallback, useEffect, useRef, useState } from "react";

import { barStateFromStage, type BarState } from "@/features/voice/barState";
import { TRACE_BARS } from "@/features/voice/LevelTrace";
import { api, errorMessage, events } from "@/services/api";
import type { AssistantResponse, DocumentCandidate, DocumentTask, ReminderOption, Section, SuggestedRequest } from "@/types/api";

export type FolderChoice = "known" | "pick";

const CAPTURE_REQUEST = "Analiza lo que aparece en mi pantalla";
const VOICE_REQUEST = "Solicitud por voz";
const TAPE_LENGTH = 5;
const notRetryable = new Set(["quotaExhausted", "notConfigured", "regionUnavailable", "regionUnknown"]);

/** Identificadores negativos: un intento fallido en la interfaz nunca coincide con uno del historial. */
let nextLocalId = -1;

/** Lo que se intentó y por qué falló, para que no desaparezca de la conversación. */
function failedAttempt(request: string, error: unknown): AssistantResponse {
  const message = errorMessage(error);
  return {
    id: nextLocalId--,
    request,
    createdAt: new Date().toISOString(),
    category: "general",
    status: "failed",
    originLabel: "",
    summary: message,
    body: { kind: "error", code: "failed", message, hint: null },
  };
}

/** El historial también guarda lo que falló antes de llegar al núcleo. Si no se puede guardar, la conversación sigue igual. */
async function recordFailure(request: string, cause: string) {
  try {
    await api.recordFailure(request, cause);
  } catch {
    // El fallo ya se muestra en la conversación: no se añade un segundo error por no poder registrarlo.
  }
}

/** Respuesta actual y cinta cambian juntas: separadas, un actualizador tendría que modificar la otra. */
interface Session {
  current: AssistantResponse | null;
  tape: AssistantResponse[];
}

interface AssistantOptions {
  /** Una orden como "Abre los recordatorios" cambia de sección. */
  onNavigate?: (section: Section) => void;
}

export function useAssistant({ onNavigate }: AssistantOptions = {}) {
  const [bar, setBar] = useState<BarState>({ kind: "idle" });
  const [levels, setLevels] = useState<number[]>([]);
  const [{ current, tape }, setSession] = useState<Session>({ current: null, tape: [] });
  const lastRequest = useRef<string | null>(null);
  const captureQuestion = useRef(CAPTURE_REQUEST);
  const requestCaptureRef = useRef<(question: string) => Promise<void>>(() => Promise.resolve());
  const navigateRef = useRef(onNavigate);

  useEffect(() => {
    navigateRef.current = onNavigate;
  }, [onNavigate]);

  useEffect(() => {
    const subscriptions = [
      events.onStage(({ stage }) => setBar((previous) => (previous.kind === "captureConsent" ? previous : barStateFromStage(stage)))),
      events.onLevel((level) => setLevels((previous) => [...previous, level].slice(-TRACE_BARS))),
    ];
    return () => subscriptions.forEach((subscription) => void subscription.then((unlisten) => unlisten()));
  }, []);

  const present = useCallback((response: AssistantResponse) => {
    if (response.status === "cancelled") {
      setBar({ kind: "idle" });
      return;
    }
    setSession(({ current: previous, tape: entries }) => ({
      current: response,
      tape: previous ? [...entries, previous].slice(-TAPE_LENGTH) : entries,
    }));
    const { body } = response;
    if (body.kind === "reminderDraft" && body.question) {
      setBar({ kind: "clarify" });
    } else if (body.kind === "clarification") {
      setBar({ kind: "clarify" });
    } else if (body.kind === "captureRequest") {
      void requestCaptureRef.current(body.question);
    } else if (body.kind === "navigate") {
      setBar({ kind: "idle" });
      navigateRef.current?.(body.section);
    } else if (body.kind === "error") {
      setBar({ kind: "error", message: null, retryable: !notRetryable.has(body.code) });
    } else {
      setBar({ kind: "idle" });
    }
  }, []);

  /** Sin solicitud, el error solo cabe en la barra. Con ella, queda en la conversación junto a lo que se intentó. */
  const fail = useCallback(
    (error: unknown, request?: string) => {
      if (request) {
        const attempt = failedAttempt(request, error);
        present(attempt);
        void recordFailure(request, attempt.summary);
      } else setBar({ kind: "error", message: errorMessage(error), retryable: true });
    },
    [present],
  );

  const submit = useCallback(
    async (text: string) => {
      lastRequest.current = text;
      try {
        present(await api.submitRequest(text));
      } catch (error) {
        fail(error, text);
      }
    },
    [fail, present],
  );

  const speak = useCallback(async () => {
    setLevels([]);
    lastRequest.current = null;
    try {
      await api.startVoice();
    } catch (error) {
      fail(error, VOICE_REQUEST);
    }
  }, [fail]);

  const stopListening = useCallback(async () => {
    try {
      const response = await api.stopVoice();
      if (response) present(response);
      else setBar({ kind: "idle" });
    } catch (error) {
      fail(error, VOICE_REQUEST);
    }
  }, [fail, present]);

  useEffect(() => {
    const subscription = events.onVoiceLimit(() => void stopListening());
    return () => void subscription.then((unlisten) => unlisten());
  }, [stopListening]);

  const cancel = useCallback(async () => {
    const kind = bar.kind;
    setBar({ kind: "idle" });
    if (kind === "listening") await api.cancelVoice().catch(fail);
    else if (kind === "processing" || kind === "responding") await api.cancel().catch(fail);
  }, [bar.kind, fail]);

  const requestCapture = useCallback(
    async (question: string = CAPTURE_REQUEST) => {
      captureQuestion.current = question;
      try {
        const { provider } = await api.getSettings();
        setBar({ kind: "captureConsent", provider: provider.displayName, notice: provider.dataNotice });
      } catch (error) {
        fail(error);
      }
    },
    [fail],
  );

  useEffect(() => {
    requestCaptureRef.current = requestCapture;
  }, [requestCapture]);

  const confirmCapture = useCallback(async () => {
    setBar({ kind: "processing", step: "Tomando una sola captura…" });
    try {
      present(await api.analyzeScreen(captureQuestion.current));
    } catch (error) {
      fail(error, captureQuestion.current);
    }
  }, [fail, present]);

  const retry = useCallback(() => {
    if (lastRequest.current) void submit(lastRequest.current);
    else void speak();
  }, [speak, submit]);

  const currentId = current?.id ?? null;

  const saveReminder = useCallback(
    async (content: string, dueAt: string) => {
      try {
        const reminder = await api.createReminder(content, dueAt, currentId);
        setSession((session) => ({
          ...session,
          current: session.current ? { ...session.current, status: "completed", summary: `Guardado: ${reminder.content}`, body: { kind: "reminderSaved", reminder } } : null,
        }));
        setBar({ kind: "idle" });
      } catch (error) {
        fail(error);
      }
    },
    [currentId, fail],
  );

  const chooseReminderOption = useCallback(
    (option: ReminderOption) => {
      if (current?.body.kind === "reminderDraft") void saveReminder(current.body.content, option.dueAt);
    },
    [current, saveReminder],
  );

  const chooseSuggestion = useCallback((suggestion: SuggestedRequest) => void submit(suggestion.request), [submit]);

  /** Da permiso (Documentos y Escritorio, u otra carpeta elegida en el sistema) y continúa la solicitud sola. */
  const authorizeFolders = useCallback(
    async (choice: FolderChoice, pendingRequest: string) => {
      try {
        const folders = choice === "known" ? await api.allowKnownFolders() : await api.pickFolder();
        if (folders) await submit(pendingRequest);
      } catch (error) {
        fail(error);
      }
    },
    [fail, submit],
  );

  /** Lee el archivo confirmado y lo envía a resumir o analizar. */
  const analyzeDocument = useCallback(
    async (candidate: DocumentCandidate, task: DocumentTask) => {
      setBar(barStateFromStage("reading"));
      try {
        present(await api.analyzeDocument(candidate.token, task));
      } catch (error) {
        fail(error, `${task === "summarize" ? "Resumen" : "Análisis"} de «${candidate.name}${candidate.extension}»`);
      }
    },
    [fail, present],
  );

  /** "Cancelar" en un permiso de la conversación: no se hizo ni se envió nada. */
  const dismissCurrent = useCallback((message: string) => {
    setSession((session) =>
      session.current ? { ...session, current: { ...session.current, status: "cancelled", summary: message, body: { kind: "text", text: message } } } : session,
    );
    setBar({ kind: "idle" });
  }, []);

  return {
    bar,
    levels,
    current,
    tape,
    submit,
    speak,
    stopListening,
    cancel,
    requestCapture,
    confirmCapture,
    retry,
    saveReminder,
    chooseReminderOption,
    chooseSuggestion,
    authorizeFolders,
    analyzeDocument,
    dismissCurrent,
  };
}
