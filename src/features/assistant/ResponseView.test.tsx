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

const actions = { onSaveReminder: vi.fn(), onAuthorizeFolders: vi.fn(), onChooseReminderOption: vi.fn(), onChooseSuggestion: vi.fn(), onAnalyzeDocument: vi.fn(), onDismiss: vi.fn() };

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

describe("aclaraciones", () => {
  it("las opciones están junto a la pregunta y la primera recibe el foco", async () => {
    const { default: userEvent } = await import("@testing-library/user-event");
    const option = { label: "08:00", dueAt: "2026-11-12T08:00:00-05:00" };
    const body: ResponseBody = { kind: "reminderDraft", content: "Llamar a mamá", dueAt: null, question: "¿A qué hora?", options: [option, { label: "20:00", dueAt: "2026-11-12T20:00:00-05:00" }] };
    render(<ResponseView response={response(body)} actions={actions} />);
    const choices = screen.getByRole("group", { name: "¿A qué hora?" });
    expect(within(choices).getByRole("button", { name: "08:00" })).toHaveFocus();
    await userEvent.click(within(choices).getByRole("button", { name: "08:00" }));
    expect(actions.onChooseReminderOption).toHaveBeenCalledWith(option);
  });

  it("una sugerencia vuelve a enviar la solicitud completa", async () => {
    const { default: userEvent } = await import("@testing-library/user-event");
    const suggestion = { label: "Buscar un archivo", request: "Busca el archivo informe" };
    render(<ResponseView response={response({ kind: "clarification", question: "¿Qué quieres hacer?", suggestions: [suggestion] })} actions={actions} />);
    await userEvent.click(screen.getByRole("button", { name: "Buscar un archivo" }));
    expect(actions.onChooseSuggestion).toHaveBeenCalledWith(suggestion);
  });
});

describe("ResponseView: varias solicitudes", () => {
  it("muestra cada parte en su bloque, con su estado, y la que falló dice por qué", () => {
    const actions = { onSaveReminder: vi.fn(), onAuthorizeFolders: vi.fn(), onAnalyzeDocument: vi.fn(), onDismiss: vi.fn(), onChooseReminderOption: vi.fn(), onChooseSuggestion: vi.fn() };
    render(
      <ResponseView
        actions={actions}
        response={{
          id: 9,
          request: "¿Qué red tengo? Y dame el clima en Nolugar",
          createdAt: "2026-11-23T10:00:00-05:00",
          category: "general",
          status: "completed",
          originLabel: "cada solicitud indica el suyo",
          summary: "s",
          body: {
            kind: "multiple",
            parts: [
              { request: "¿Qué red tengo?", category: "system", status: "completed", originLabel: "lectura directa del equipo, sin IA", summary: "Casa", body: { kind: "text", text: "Conectado a Casa." } },
              { request: "dame el clima en Nolugar", category: "weather", status: "failed", originLabel: "sin datos", summary: "No", body: { kind: "error", code: "cityNotFound", message: "No encontré la ciudad «Nolugar».", hint: null } },
            ],
          },
        }}
      />,
    );
    const parts = within(screen.getByRole("list", { name: "Solicitudes de esta petición" })).getAllByRole("listitem");
    expect(parts).toHaveLength(2);
    const [network, weather] = parts as [HTMLElement, HTMLElement];
    expect(within(network).getByRole("heading", { name: /¿Qué red tengo\?$/ })).toBeInTheDocument();
    expect(within(network).getByText("Completada")).toBeInTheDocument();
    expect(within(network).getByText("Conectado a Casa.")).toBeInTheDocument();
    expect(within(weather).getByText("Falló")).toBeInTheDocument();
    expect(within(weather).getByText("No encontré la ciudad «Nolugar».")).toBeInTheDocument();
    // El origen va en cada parte, no en la petición entera.
    expect(screen.queryByText(/cada solicitud indica el suyo/)).toBeNull();
  });
});
