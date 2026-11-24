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

/** La última operación, con lo necesario para repetirla tal cual. */
type Operation =
  | { kind: "text"; text: string; continues: number | null }
  | { kind: "voice" }
  | { kind: "capture"; question: string; continues: number | null }
  | { kind: "document"; candidate: DocumentCandidate; task: DocumentTask; continues: number | null }
  | { kind: "reminder"; content: string; dueAt: string }
  | { kind: "folders"; choice: FolderChoice; request: string };

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
  /**
   * Cómo repetir exactamente la última operación, para «Reintentar»: la solicitud escrita, la grabación, la
   * captura (que vuelve a pedir permiso), el documento confirmado, el recordatorio o el permiso de carpetas.
   */
  const lastOperation = useRef<Operation | null>(null);
  const captureQuestion = useRef(CAPTURE_REQUEST);
  const requestCaptureRef = useRef<(question: string) => Promise<void>>(() => Promise.resolve());
  const navigateRef = useRef(onNavigate);
  /** La solicitud pendiente (permiso, aclaración, confirmación): lo que la complete actualiza su entrada. */
  const pendingId = useRef<number | null>(null);
  /** La entrada que continúa la captura, si la pidió una solicitud ("¿qué hay en mi pantalla?"). */
  const captureContinues = useRef<number | null>(null);

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
    pendingId.current = response.status === "pending" && response.id > 0 ? response.id : null;
    // Si completa la solicitud en curso, la sustituye: una solicitud, una sola respuesta.
    setSession(({ current: previous, tape: entries }) => ({
      current: response,
      tape: previous && previous.id !== response.id ? [...entries, previous].slice(-TAPE_LENGTH) : entries,
    }));
    const { body } = response;
    if (body.kind === "reminderDraft" && body.question) {
      setBar({ kind: "clarify" });
    } else if (body.kind === "clarification") {
      setBar({ kind: "clarify" });
    } else if (body.kind === "captureRequest") {
      captureContinues.current = response.id > 0 ? response.id : null;
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
    async (text: string, continues: number | null = null) => {
      lastOperation.current = { kind: "text", text, continues };
      try {
        present(await api.submitRequest(text, continues));
      } catch (error) {
        fail(error, text);
      }
    },
    [fail, present],
  );

  const speak = useCallback(async () => {
    setLevels([]);
    lastOperation.current = { kind: "voice" };
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

  /** "Cancelar" en un permiso de la conversación: no se hizo ni se envió nada. */
  const dismissCurrent = useCallback((message: string) => {
    if (pendingId.current !== null) void api.cancelEntry(pendingId.current).catch(() => undefined);
    pendingId.current = null;
    setSession((session) =>
      session.current ? { ...session, current: { ...session.current, status: "cancelled", summary: message, body: { kind: "text", text: message } } } : session,
    );
    setBar({ kind: "idle" });
  }, []);

  const cancel = useCallback(async () => {
    const kind = bar.kind;
    setBar({ kind: "idle" });
    if (kind === "listening") await api.cancelVoice().catch(fail);
    else if (kind === "processing" || kind === "responding") await api.cancel().catch(fail);
    // Cancelar una pregunta o un permiso pendiente cierra su solicitud como cancelada.
    else if ((kind === "clarify" || kind === "captureConsent") && pendingId.current !== null) dismissCurrent("Cancelado.");
  }, [bar.kind, fail, dismissCurrent]);

  /** Desde el botón Analizar pantalla no continúa ninguna solicitud; desde la conversación, sí (ver present). */
  const analyzeScreenButton = useCallback(() => {
    captureContinues.current = null;
    void requestCaptureRef.current(CAPTURE_REQUEST);
  }, []);

  const requestCapture = useCallback(
    async (question: string = CAPTURE_REQUEST) => {
      captureQuestion.current = question;
      // Una captura nunca se repite sola: el reintento vuelve a pedir permiso, para la misma pregunta y entrada.
      lastOperation.current = { kind: "capture", question, continues: captureContinues.current };
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
      present(await api.analyzeScreen(captureQuestion.current, captureContinues.current));
    } catch (error) {
      fail(error, captureQuestion.current);
    }
  }, [fail, present]);


  const currentId = current?.id ?? null;

  const saveReminder = useCallback(
    async (content: string, dueAt: string) => {
      lastOperation.current = { kind: "reminder", content, dueAt };
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

  const chooseSuggestion = useCallback((suggestion: SuggestedRequest) => void submit(suggestion.request, pendingId.current), [submit]);

  /** Da permiso (Documentos y Escritorio, u otra carpeta elegida en el sistema) y continúa la solicitud sola. */
  const authorizeFolders = useCallback(
    async (choice: FolderChoice, pendingRequest: string) => {
      lastOperation.current = { kind: "folders", choice, request: pendingRequest };
      try {
        const folders = choice === "known" ? await api.allowKnownFolders() : await api.pickFolder();
        if (folders) await submit(pendingRequest, pendingId.current);
      } catch (error) {
        fail(error);
      }
    },
    [fail, submit],
  );

  /** Lee el archivo confirmado y lo envía a resumir o analizar. */
  const analyzeDocument = useCallback(
    async (candidate: DocumentCandidate, task: DocumentTask, continues: number | null = pendingId.current) => {
      // El archivo ya está confirmado: el reintento lo vuelve a leer y enviar, en la misma entrada.
      lastOperation.current = { kind: "document", candidate, task, continues };
      setBar(barStateFromStage("reading"));
      try {
        present(await api.analyzeDocument(candidate.token, task, continues));
      } catch (error) {
        fail(error, `${task === "summarize" ? "Resumen" : "Análisis"} de «${candidate.name}${candidate.extension}»`);
      }
    },
    [fail, present],
  );

  /** Borrar todo el historial vacía también la conversación visible. */
  const clearConversation = useCallback(() => {
    setSession({ current: null, tape: [] });
    lastOperation.current = null;
  }, []);


  /** «Reintentar» repite exactamente la última operación. Una captura vuelve a pedir permiso. */
  const retry = useCallback(() => {
    const operation = lastOperation.current;
    if (!operation) return;
    if (operation.kind === "text") void submit(operation.text, operation.continues);
    else if (operation.kind === "voice") void speak();
    else if (operation.kind === "capture") {
      captureContinues.current = operation.continues;
      void requestCapture(operation.question);
    } else if (operation.kind === "document") void analyzeDocument(operation.candidate, operation.task, operation.continues);
    else if (operation.kind === "reminder") void saveReminder(operation.content, operation.dueAt);
    else void authorizeFolders(operation.choice, operation.request);
  }, [analyzeDocument, authorizeFolders, requestCapture, saveReminder, speak, submit]);

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
    analyzeScreenButton,
    confirmCapture,
    retry,
    saveReminder,
    chooseReminderOption,
    chooseSuggestion,
    authorizeFolders,
    analyzeDocument,
    dismissCurrent,
    clearConversation,
  };
}
