import { useCallback, useEffect, useRef, useState } from "react";

import { barStateFromStage, type BarState } from "@/features/voice/barState";
import { api, errorMessage, events } from "@/services/api";
import type { AssistantResponse, ReminderOption, SuggestedRequest } from "@/types/api";

const LEVEL_HISTORY = 72;
const CAPTURE_REQUEST = "Analiza lo que aparece en mi pantalla";
const TAPE_LENGTH = 5;

/** Respuesta actual y cinta cambian juntas: separadas, un actualizador tendría que modificar la otra. */
interface Session {
  current: AssistantResponse | null;
  tape: AssistantResponse[];
}

export function useAssistant() {
  const [bar, setBar] = useState<BarState>({ kind: "idle" });
  const [levels, setLevels] = useState<number[]>([]);
  const [{ current, tape }, setSession] = useState<Session>({ current: null, tape: [] });
  const lastRequest = useRef<string | null>(null);
  const captureQuestion = useRef(CAPTURE_REQUEST);
  const requestCaptureRef = useRef<(question: string) => Promise<void>>(() => Promise.resolve());

  useEffect(() => {
    const subscriptions = [
      events.onStage(({ stage }) => setBar((previous) => (previous.kind === "captureConsent" ? previous : barStateFromStage(stage)))),
      events.onLevel((level) => setLevels((previous) => [...previous, level].slice(-LEVEL_HISTORY))),
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
      setBar({ kind: "clarify", question: body.question, reminderOptions: body.options, suggestions: [] });
    } else if (body.kind === "clarification") {
      setBar({ kind: "clarify", question: body.question, reminderOptions: [], suggestions: body.suggestions });
    } else if (body.kind === "captureRequest") {
      void requestCaptureRef.current(body.question);
    } else if (body.kind === "error") {
      setBar({ kind: "error", message: body.message, retryable: body.code !== "quotaExhausted" && body.code !== "noApiKey" });
    } else {
      setBar({ kind: "idle" });
    }
  }, []);

  const fail = useCallback((error: unknown) => {
    setBar({ kind: "error", message: errorMessage(error), retryable: true });
  }, []);

  const submit = useCallback(
    async (text: string) => {
      lastRequest.current = text;
      try {
        present(await api.submitRequest(text));
      } catch (error) {
        fail(error);
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
      fail(error);
    }
  }, [fail]);

  const stopListening = useCallback(async () => {
    try {
      const response = await api.stopVoice();
      if (response) present(response);
      else setBar({ kind: "idle" });
    } catch (error) {
      fail(error);
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
    setBar({ kind: "processing", step: "Tomando una sola captura" });
    try {
      present(await api.analyzeScreen(captureQuestion.current));
    } catch (error) {
      fail(error);
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

  const authorizeFolders = useCallback(
    async (folders: string[], pendingRequest: string) => {
      try {
        await api.authorizeFolders(folders);
        await submit(pendingRequest);
      } catch (error) {
        fail(error);
      }
    },
    [fail, submit],
  );

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
  };
}
