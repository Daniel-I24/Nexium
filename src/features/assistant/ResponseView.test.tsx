import { render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { AssistantResponse, ResponseBody } from "@/types/api";

import { originSentence, ResponseView } from "./ResponseView";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const response = (body: ResponseBody): AssistantResponse => ({
  id: 1,
  request: "¿Qué me dice este cuadro de error?",
  createdAt: "2026-11-11T10:00:00-05:00",
  category: "screen",
  status: "completed",
  originLabel: "una captura analizada con IA (Google Gemini). Nexium ya descartó la imagen",
  summary: "s",
  body,
});

const actions = { onSaveReminder: vi.fn(), onAuthorizeFolders: vi.fn() };

describe("origen", () => {
  it("todas las líneas de origen terminan con un punto, sin duplicarlo", () => {
    expect(originSentence("reglas locales, sin IA")).toBe("reglas locales, sin IA.");
    expect(originSentence("Open-Meteo; solicitud interpretada con IA (Google Gemini)")).toBe("Open-Meteo; solicitud interpretada con IA (Google Gemini).");
    expect(originSentence("una captura analizada con IA (Google Gemini). Nexium ya descartó la imagen")).toBe("una captura analizada con IA (Google Gemini). Nexium ya descartó la imagen.");
    expect(originSentence("ya termina.")).toBe("ya termina.");
  });
});

describe("errores", () => {
  it("no muestran un origen sin datos: el mensaje ya explica la causa", () => {
    render(<ResponseView response={{ ...response({ kind: "error", code: "unavailable", message: "El servidor de IA no responde en este momento.", hint: "Vuelve a intentarlo." }), originLabel: "sin datos" }} actions={actions} />);
    expect(screen.getByText("El servidor de IA no responde en este momento.")).toBeInTheDocument();
    expect(screen.queryByText(/Origen/)).not.toBeInTheDocument();
  });

  it("una respuesta correcta sí muestra su origen", () => {
    render(<ResponseView response={response({ kind: "text", text: "Hola" })} actions={actions} />);
    expect(screen.getByText("Origen: una captura analizada con IA (Google Gemini). Nexium ya descartó la imagen.")).toBeInTheDocument();
  });
});

describe("análisis de pantalla", () => {
  it("muestra el resumen, la causa y los pasos numerados", () => {
    render(<ResponseView response={response({ kind: "screenAnalysis", summary: "Falta configuracion.ini.", cause: "El programa no encuentra su archivo.", steps: ["Comprueba la carpeta.", "Reinstala el programa."] })} actions={actions} />);
    expect(screen.getByText("Falta configuracion.ini.")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Causa probable" })).toBeInTheDocument();
    expect(screen.getByText("El programa no encuentra su archivo.")).toBeInTheDocument();
    const steps = within(screen.getByRole("list")).getAllByRole("listitem");
    expect(steps.map((step) => step.textContent)).toEqual(["1Comprueba la carpeta.", "2Reinstala el programa."]);
  });

  it("sin problema no muestra causa ni pasos", () => {
    render(<ResponseView response={response({ kind: "screenAnalysis", summary: "Un escritorio sin ventanas abiertas.", cause: null, steps: [] })} actions={actions} />);
    expect(screen.queryByRole("heading", { name: "Causa probable" })).not.toBeInTheDocument();
    expect(screen.queryByRole("list")).not.toBeInTheDocument();
  });

  it("muestra el texto tal cual: no interpreta HTML ni Markdown", () => {
    render(<ResponseView response={response({ kind: "screenAnalysis", summary: "<b>hola</b>", cause: null, steps: [] })} actions={actions} />);
    expect(screen.getByText("<b>hola</b>")).toBeInTheDocument();
    expect(document.querySelector("b")).toBeNull();
  });
});
