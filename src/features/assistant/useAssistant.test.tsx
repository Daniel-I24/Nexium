import { act, renderHook, waitFor } from "@testing-library/react";
import { StrictMode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { AssistantResponse, ResponseBody } from "@/types/api";

import { useAssistant } from "./useAssistant";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => undefined)) }));

const reply = (body: ResponseBody, status: AssistantResponse["status"] = "completed"): AssistantResponse => ({
  id: 1,
  request: "x",
  createdAt: "2026-11-05T14:05:00-05:00",
  category: "general",
  status,
  originLabel: "o",
  summary: "s",
  body,
});

describe("useAssistant", () => {
  beforeEach(() => invoke.mockReset());

  it("presenta una respuesta y vuelve a En espera", async () => {
    invoke.mockResolvedValueOnce(reply({ kind: "text", text: "hola" }));
    const { result } = renderHook(() => useAssistant());
    await act(() => result.current.submit("hola"));
    expect(invoke).toHaveBeenCalledWith("assistant_submit", { text: "hola", continues: null });
    expect(result.current.current?.summary).toBe("s");
    expect(result.current.bar).toEqual({ kind: "idle" });
  });

  it("una orden para abrir una sección cambia de pantalla", async () => {
    invoke.mockResolvedValueOnce(reply({ kind: "navigate", section: "reminders" }));
    const onNavigate = vi.fn();
    const { result } = renderHook(() => useAssistant({ onNavigate }));
    await act(() => result.current.submit("Abre los recordatorios"));
    expect(onNavigate).toHaveBeenCalledWith("reminders");
    expect(result.current.bar).toEqual({ kind: "idle" });
  });

  it("al dar permiso para Documentos y Escritorio, la solicitud pendiente sigue sola", async () => {
    invoke.mockResolvedValueOnce(["C:\\Users\\a\\Documents"]).mockResolvedValueOnce(reply({ kind: "text", text: "hecho" }));
    const { result } = renderHook(() => useAssistant());
    await act(() => result.current.authorizeFolders("known", "Busca mi informe"));
    expect(invoke).toHaveBeenNthCalledWith(1, "folders_allow_known");
    expect(invoke).toHaveBeenNthCalledWith(2, "assistant_submit", { text: "Busca mi informe", continues: null });
  });

  it("si se cierra el selector de carpetas sin elegir, no se repite la solicitud", async () => {
    invoke.mockResolvedValueOnce(null);
    const { result } = renderHook(() => useAssistant());
    await act(() => result.current.authorizeFolders("pick", "Busca mi informe"));
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith("folders_pick");
  });

  it("confirmar un documento lo envía por su identificador, nunca por su ruta", async () => {
    invoke.mockResolvedValueOnce(reply({ kind: "documentAnalysis", fileName: "a.pdf", task: "summarize", summary: "s", points: [], details: [], truncated: false }));
    const { result } = renderHook(() => useAssistant());
    await act(() => result.current.analyzeDocument({ token: "t-1", name: "a", extension: ".pdf", location: "Documentos" }, "summarize"));
    expect(invoke).toHaveBeenCalledWith("document_analyze", { token: "t-1", task: "summarize", continues: null });
    expect(result.current.current?.body.kind).toBe("documentAnalysis");
  });

  it("vaciar la conversación quita la respuesta actual y la cinta", async () => {
    invoke.mockResolvedValue(reply({ kind: "text", text: "hola" }));
    const { result } = renderHook(() => useAssistant());
    await act(() => result.current.submit("uno"));
    await act(() => result.current.submit("dos"));
    act(() => result.current.clearConversation());
    expect(result.current.current).toBeNull();
    expect(result.current.tape).toEqual([]);
  });

  it("completar una solicitud pendiente la sustituye: no queda una respuesta más en la cinta", async () => {
    invoke
      .mockResolvedValueOnce({ ...reply({ kind: "clarification", question: "¿Qué ciudad?", suggestions: [{ label: "Cali", request: "clima en Cali" }] }, "pending"), id: 5 })
      .mockResolvedValueOnce({ ...reply({ kind: "text", text: "22 °C" }), id: 5 });
    const { result } = renderHook(() => useAssistant());
    await act(() => result.current.submit("Dame el clima"));
    await act(async () => result.current.chooseSuggestion({ label: "Cali", request: "clima en Cali" }));
    await waitFor(() => expect(result.current.current?.status).toBe("completed"));
    expect(invoke).toHaveBeenLastCalledWith("assistant_submit", { text: "clima en Cali", continues: 5 });
    expect(result.current.tape).toEqual([]);
  });

  it("cancelar una solicitud pendiente la cierra como cancelada en su entrada", async () => {
    invoke.mockResolvedValueOnce({ ...reply({ kind: "folderConsent", folders: [], pendingRequest: "Busca x" }, "pending"), id: 8 }).mockResolvedValue(undefined);
    const { result } = renderHook(() => useAssistant());
    await act(() => result.current.submit("Busca x"));
    act(() => result.current.dismissCurrent("Cancelado. No se buscó nada."));
    expect(invoke).toHaveBeenCalledWith("history_cancel", { id: 8 });
    expect(result.current.current?.status).toBe("cancelled");
  });

  it("el botón Analizar pantalla no continúa ninguna solicitud", async () => {
    const settings = { provider: { displayName: "Google Gemini", dataNotice: "Aviso." } };
    invoke.mockImplementation((command: string) => Promise.resolve(command === "settings_get" ? settings : reply({ kind: "text", text: "x" })));
    const { result } = renderHook(() => useAssistant());
    act(() => result.current.analyzeScreenButton());
    await waitFor(() => expect(result.current.bar.kind).toBe("captureConsent"));
    await act(() => result.current.confirmCapture());
    expect(invoke).toHaveBeenCalledWith("screen_analyze", { question: "Analiza lo que aparece en mi pantalla", continues: null });
  });

  it("la cinta no duplica entradas ni pasa de cinco, también en modo estricto", async () => {
    const { result } = renderHook(() => useAssistant(), { wrapper: StrictMode });
    for (const id of [1, 2, 3, 4, 5, 6, 7]) {
      invoke.mockResolvedValueOnce({ ...reply({ kind: "text", text: "hola" }), id });
      await act(() => result.current.submit("hola"));
    }
    expect(result.current.current?.id).toBe(7);
    expect(result.current.tape.map((entry) => entry.id)).toEqual([2, 3, 4, 5, 6]);
  });

  it("la cuota agotada no es reintentable", async () => {
    invoke.mockResolvedValueOnce(reply({ kind: "error", code: "quotaExhausted", message: "Cuota", hint: null }, "failed"));
    const { result } = renderHook(() => useAssistant());
    await act(() => result.current.submit("hola"));
    expect(result.current.bar).toEqual({ kind: "error", message: null, retryable: false });
  });

  it("una región sin servicio no es reintentable", async () => {
    invoke.mockResolvedValueOnce(reply({ kind: "error", code: "regionUnavailable", message: "Región", hint: null }, "failed"));
    const { result } = renderHook(() => useAssistant());
    await act(() => result.current.submit("hola"));
    expect(result.current.bar).toEqual({ kind: "error", message: null, retryable: false });
  });

  it("una solicitud de pantalla pide permiso y no captura sola", async () => {
    const settings = { provider: { displayName: "Google Gemini", dataNotice: "Aviso." } };
    invoke.mockImplementation((command: string) =>
      Promise.resolve(command === "settings_get" ? settings : reply({ kind: "captureRequest", question: "¿Qué es?" }, "pending")),
    );
    const { result } = renderHook(() => useAssistant());
    await act(() => result.current.submit("Explícame este error"));
    await waitFor(() => expect(result.current.bar.kind).toBe("captureConsent"));
    expect(result.current.bar).toEqual({ kind: "captureConsent", provider: "Google Gemini", notice: "Aviso." });
    expect(invoke).not.toHaveBeenCalledWith("screen_analyze", expect.anything());
    await act(() => result.current.confirmCapture());
    expect(invoke).toHaveBeenCalledWith("screen_analyze", { question: "¿Qué es?", continues: 1 });
  });

  it("una solicitud cancelada no se presenta", async () => {
    invoke.mockResolvedValueOnce(reply({ kind: "error", code: "cancelled", message: "Cancelada", hint: null }, "cancelled"));
    const { result } = renderHook(() => useAssistant());
    await act(() => result.current.submit("hola"));
    expect(result.current.current).toBeNull();
    expect(result.current.bar).toEqual({ kind: "idle" });
  });

  it("la cinta conserva la respuesta anterior", async () => {
    invoke.mockResolvedValueOnce(reply({ kind: "text", text: "a" })).mockResolvedValueOnce({ ...reply({ kind: "text", text: "b" }), id: 2 });
    const { result } = renderHook(() => useAssistant());
    await act(() => result.current.submit("a"));
    await act(() => result.current.submit("b"));
    expect(result.current.current?.id).toBe(2);
    expect(result.current.tape.map((entry) => entry.id)).toEqual([1]);
  });

  it("un fallo del micrófono queda en la conversación, sin repetirse en la barra", async () => {
    invoke.mockRejectedValueOnce("Falta el modelo de voz.");
    const { result } = renderHook(() => useAssistant());
    await act(() => result.current.speak());
    expect(result.current.current).toMatchObject({ request: "Solicitud por voz", status: "failed", body: { kind: "error", message: "Falta el modelo de voz." } });
    expect(result.current.bar).toEqual({ kind: "error", message: null, retryable: true });
  });

  it("un fallo al transcribir conserva la respuesta anterior en la cinta", async () => {
    invoke.mockResolvedValueOnce(reply({ kind: "text", text: "hola" }));
    const { result } = renderHook(() => useAssistant());
    await act(() => result.current.submit("hola"));
    invoke.mockResolvedValueOnce(undefined).mockRejectedValueOnce("El micrófono no captó ningún sonido.");
    await act(() => result.current.speak());
    await act(() => result.current.stopListening());
    expect(result.current.current).toMatchObject({ request: "Solicitud por voz", body: { message: "El micrófono no captó ningún sonido." } });
    expect(result.current.tape.map((entry) => entry.id)).toEqual([1]);
  });

  it("un fallo de una solicitud escrita conserva el texto que se intentó", async () => {
    invoke.mockRejectedValueOnce("Sin conexión.");
    const { result } = renderHook(() => useAssistant());
    await act(() => result.current.submit("¿Qué tiempo hace?"));
    expect(result.current.current).toMatchObject({ request: "¿Qué tiempo hace?", body: { kind: "error", message: "Sin conexión." } });
  });

  it("un fallo que se ve en la conversación queda en el historial como Fallida, con su causa", async () => {
    invoke.mockRejectedValueOnce("Falta el modelo de voz.");
    const { result } = renderHook(() => useAssistant());
    await act(() => result.current.speak());
    expect(invoke).toHaveBeenCalledWith("history_record_failure", { request: "Solicitud por voz", cause: "Falta el modelo de voz." });
  });

  it("si el historial no puede guardar el fallo, la conversación lo sigue mostrando", async () => {
    invoke.mockRejectedValueOnce("Sin conexión.").mockRejectedValueOnce("Base de datos bloqueada.");
    const { result } = renderHook(() => useAssistant());
    await act(() => result.current.submit("hola"));
    expect(result.current.current).toMatchObject({ status: "failed", body: { message: "Sin conexión." } });
    expect(result.current.bar).toEqual({ kind: "error", message: null, retryable: true });
  });

  it("una respuesta fallida del núcleo no se registra dos veces", async () => {
    invoke.mockResolvedValueOnce(reply({ kind: "error", code: "network", message: "Sin red", hint: null }, "failed"));
    const { result } = renderHook(() => useAssistant());
    await act(() => result.current.submit("hola"));
    expect(invoke).not.toHaveBeenCalledWith("history_record_failure", expect.anything());
  });

  it("dos fallos seguidos tienen identificadores distintos y negativos", async () => {
    invoke.mockRejectedValueOnce("a").mockRejectedValueOnce("b");
    const { result } = renderHook(() => useAssistant());
    await act(() => result.current.submit("uno"));
    await act(() => result.current.submit("dos"));
    const ids = [result.current.tape[0]?.id, result.current.current?.id];
    expect(new Set(ids).size).toBe(2);
    expect(ids.every((id) => id !== undefined && id < 0)).toBe(true);
  });
});
