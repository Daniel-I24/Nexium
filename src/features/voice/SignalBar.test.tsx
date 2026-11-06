import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { barStateFromStage, isSensing, type BarState } from "./barState";
import { SignalBar } from "./SignalBar";

function setup(state: BarState) {
  const handlers = {
    onSpeak: vi.fn(),
    onStopListening: vi.fn(),
    onCancel: vi.fn(),
    onSubmitText: vi.fn(),
    onAnalyzeScreen: vi.fn(),
    onConfirmCapture: vi.fn(),
    onRetry: vi.fn(),
    onChooseReminderOption: vi.fn(),
    onChooseSuggestion: vi.fn(),
  };
  render(<SignalBar state={state} levels={[0.2, 0.8]} {...handlers} />);
  return handlers;
}

const status = () => screen.getByRole("status");

describe("SignalBar", () => {
  it("en espera permite hablar y escribir", async () => {
    const handlers = setup({ kind: "idle" });
    expect(status()).toHaveTextContent("En espera");
    await userEvent.type(screen.getByLabelText("Escribe tu solicitud"), "¿Qué hora es?{enter}");
    expect(handlers.onSubmitText).toHaveBeenCalledWith("¿Qué hora es?");
    await userEvent.click(screen.getByRole("button", { name: "Hablar" }));
    expect(handlers.onSpeak).toHaveBeenCalled();
  });

  it("no envía texto vacío", async () => {
    const handlers = setup({ kind: "idle" });
    await userEvent.type(screen.getByLabelText("Escribe tu solicitud"), "   {enter}");
    expect(handlers.onSubmitText).not.toHaveBeenCalled();
  });

  it("escuchando permite terminar y cancelar", async () => {
    const handlers = setup({ kind: "listening" });
    expect(status()).toHaveTextContent("Escuchando");
    expect(screen.getByRole("img", { name: "Nivel del micrófono" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Terminar" }));
    expect(handlers.onStopListening).toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(handlers.onCancel).toHaveBeenCalled();
  });

  it("procesando dice el paso y se puede cancelar", () => {
    setup({ kind: "processing", step: "Transcribiendo tu voz en este equipo" });
    expect(status()).toHaveTextContent("Procesando");
    expect(screen.getByText("Transcribiendo tu voz en este equipo")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Hablar" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Cancelar" })).toBeEnabled();
  });

  it("muestra el estado Respondiendo", () => {
    setup({ kind: "responding" });
    expect(status()).toHaveTextContent("Respondiendo");
    expect(screen.getByRole("button", { name: "Cancelar" })).toBeEnabled();
  });

  it("el aviso de captura nombra al proveedor y el uso de los datos antes de capturar", async () => {
    const notice = "En la capa gratuita, Google puede usar lo que envías para mejorar sus productos.";
    const handlers = setup({ kind: "captureConsent", provider: "Google Gemini", notice });
    expect(status()).toHaveTextContent("Esperando permiso");
    expect(screen.getByText(/se enviará a Google Gemini/)).toHaveTextContent(notice);
    expect(handlers.onConfirmCapture).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Capturar" }));
    expect(handlers.onConfirmCapture).toHaveBeenCalledTimes(1);
  });

  it("el aviso de captura usa cualquier proveedor configurado", () => {
    setup({ kind: "captureConsent", provider: "Otro Proveedor", notice: "Aviso." });
    expect(screen.getByText(/se enviará a Otro Proveedor/)).toBeInTheDocument();
    expect(screen.queryByText(/Google/)).not.toBeInTheDocument();
  });

  it("una aclaración ofrece opciones", async () => {
    const option = { label: "08:00", dueAt: "2026-11-12T08:00:00-05:00" };
    const handlers = setup({ kind: "clarify", question: "¿A qué hora?", reminderOptions: [option], suggestions: [] });
    expect(status()).toHaveTextContent("Pendiente");
    await userEvent.click(screen.getByRole("button", { name: "08:00" }));
    expect(handlers.onChooseReminderOption).toHaveBeenCalledWith(option);
  });

  it("un error explica la causa y permite reintentar", async () => {
    const handlers = setup({ kind: "error", message: "No puedo usar el micrófono.", retryable: true });
    expect(status()).toHaveTextContent("Error");
    await userEvent.click(screen.getByRole("button", { name: "Reintentar" }));
    expect(handlers.onRetry).toHaveBeenCalled();
  });

  it("la cuota agotada no ofrece reintentar", () => {
    setup({ kind: "error", message: "Se agotó la cuota.", retryable: false });
    expect(screen.queryByRole("button", { name: "Reintentar" })).not.toBeInTheDocument();
  });
});

describe("barState", () => {
  it("la señal solo se enciende cuando Nexium oye o va a ver", () => {
    expect(isSensing({ kind: "listening" })).toBe(true);
    expect(isSensing({ kind: "captureConsent", provider: "x", notice: "y" })).toBe(true);
    expect(isSensing({ kind: "idle" })).toBe(false);
    expect(isSensing({ kind: "processing", step: "x" })).toBe(false);
    expect(isSensing({ kind: "responding" })).toBe(false);
  });

  it("traduce cada etapa del backend", () => {
    expect(barStateFromStage("responding")).toEqual({ kind: "responding" });
    expect(barStateFromStage("transcribing")).toEqual({ kind: "processing", step: "Transcribiendo tu voz en este equipo" });
    expect(barStateFromStage("idle")).toEqual({ kind: "idle" });
  });
});
