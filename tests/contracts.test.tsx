import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ResponseView } from "@/features/assistant/ResponseView";
import type { AssistantResponse } from "@/types/api";

import fixtures from "./contracts/responses.json";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => undefined)) }));

const actions = { onSaveReminder: vi.fn(), onAuthorizeFolders: vi.fn() };
const show = (key: keyof typeof fixtures) => render(<ResponseView response={fixtures[key] as AssistantResponse} actions={actions} />);

describe("respuestas con el contrato que produce Rust", () => {
  it("muestra una lectura con su origen", () => {
    show("reading");
    expect(screen.getByText("11,4")).toBeInTheDocument();
    expect(screen.getByRole("meter")).toHaveAttribute("aria-valuenow", "71");
    expect(screen.getByText(/lectura directa del equipo, sin IA/)).toBeInTheDocument();
  });

  it("indica lo no disponible sin inventar un valor", () => {
    show("unavailable");
    expect(screen.getByText("No disponible")).toBeInTheDocument();
    expect(screen.getByText("Este equipo no tiene batería.")).toBeInTheDocument();
  });

  it("muestra cada dato de un archivo", () => {
    show("files");
    expect(screen.getByText("Informe")).toBeInTheDocument();
    expect(screen.getByText(".pdf")).toBeInTheDocument();
    expect(screen.getByText("C:\\a\\Informe.pdf")).toBeInTheDocument();
    expect(screen.getByText(/Existe · en Documentos/)).toBeInTheDocument();
  });

  it("no permite guardar un borrador sin fecha", () => {
    show("draft");
    expect(screen.getByRole("button", { name: "Guardar recordatorio" })).toBeDisabled();
  });

  it("explica la cuota agotada", () => {
    show("quota");
    expect(screen.getByText(/Se agotó la cuota diaria gratuita de Google Gemini/)).toBeInTheDocument();
    expect(screen.getByText(/siguen funcionando/)).toBeInTheDocument();
  });

  it("atribuye el clima a Open-Meteo", () => {
    show("weather");
    expect(screen.getByRole("button", { name: /Open-Meteo \(CC BY 4.0\)/ })).toBeInTheDocument();
  });
});

describe("fuentes incluidas en la aplicación", () => {
  function* files(dir: string): Generator<string> {
    for (const name of readdirSync(dir)) {
      const path = join(dir, name);
      if (statSync(path).isDirectory()) yield* files(path);
      else yield path;
    }
  }

  it("ningún archivo de la app carga fuentes desde un servicio externo", () => {
    for (const file of [...files("src"), "index.html"]) {
      expect(readFileSync(file, "utf8"), file).not.toMatch(/fonts\.(googleapis|gstatic)\.com/);
    }
  });

  it("la hoja de estilos importa las fuentes locales", () => {
    const css = readFileSync("src/styles/theme.css", "utf8");
    expect(css).toContain("@fontsource-variable/instrument-sans");
    expect(css).toContain("@fontsource/ibm-plex-mono");
  });
});
