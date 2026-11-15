import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { SettingsView } from "@/types/api";

import { SettingsScreen } from "./SettingsScreen";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => undefined)) }));

function view(overrides: Partial<SettingsView> = {}, microphone: string | null = null): SettingsView {
  return {
    settings: { dailyAiLimit: 200, speechModel: "small", speechIdleMinutes: 10, weatherCity: "", authorizedFolders: [], microphone },
    provider: { displayName: "Google Gemini", dataNotice: "Aviso." },
    aiStatus: "working",
    aiRequestsToday: 0,
    tokensToday: 0,
    knownFolders: [],
    speechModels: [],
    microphones: [
      { name: "Stereo Mix", isMicrophone: false, isDefault: true },
      { name: "Microphone Array", isMicrophone: true, isDefault: false },
    ],
    activeMicrophone: "Microphone Array",
    ...overrides,
  };
}

beforeEach(() => invoke.mockReset());

describe("Ajustes: modelos de voz", () => {
  const model = (overrides: object) => ({ id: "small", displayName: "Preciso (recomendado)", sizeMb: 181, path: "", downloaded: false, damaged: false, isActive: true, ...overrides });

  it("muestra el tamaño real y marca el modelo descargado como disponible", async () => {
    invoke.mockResolvedValue(view({ speechModels: [model({ downloaded: true })] }));
    render(<SettingsScreen />);
    expect(await screen.findByText("181 MB")).toBeInTheDocument();
    expect(screen.getByText("Disponible")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Descargar" })).not.toBeInTheDocument();
  });

  it("ofrece volver a descargar un modelo dañado y no deja elegirlo", async () => {
    invoke.mockResolvedValue(view({ speechModels: [model({ damaged: true })] }));
    render(<SettingsScreen />);
    expect(await screen.findByRole("button", { name: "Volver a descargar" })).toBeInTheDocument();
    expect(screen.getByRole("radio")).toBeDisabled();
  });
});

describe("Ajustes: micrófono", () => {
  it("dice qué micrófono usará Nexium y marca los dispositivos que no lo son", async () => {
    invoke.mockResolvedValue(view());
    render(<SettingsScreen />);
    expect(await screen.findByText("Nexium usará: Microphone Array.")).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Stereo Mix (predeterminado de Windows, graba el sonido del equipo, no tu voz)" })).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Micrófono" })).toHaveValue("");
  });

  it("guarda el micrófono elegido", async () => {
    invoke.mockResolvedValue(view());
    render(<SettingsScreen />);
    await userEvent.selectOptions(await screen.findByRole("combobox", { name: "Micrófono" }), "Microphone Array");
    await userEvent.click(screen.getByRole("button", { name: "Guardar ajustes" }));
    expect(invoke).toHaveBeenCalledWith("settings_save", { settings: expect.objectContaining({ microphone: "Microphone Array" }) });
  });

  it("el botón confirma lo guardado hasta el siguiente cambio", async () => {
    invoke.mockResolvedValue(view());
    render(<SettingsScreen />);
    await userEvent.click(await screen.findByRole("button", { name: "Guardar ajustes" }));
    expect(await screen.findByRole("button", { name: "Ajustes guardados" })).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("Ajustes guardados.");
    await userEvent.selectOptions(screen.getByRole("combobox", { name: "Micrófono" }), "Microphone Array");
    expect(screen.getByRole("button", { name: "Guardar ajustes" })).toBeInTheDocument();
  });

  it("avisa si el micrófono elegido no está conectado", async () => {
    invoke.mockResolvedValue(view({}, "Auriculares USB"));
    render(<SettingsScreen />);
    expect(await screen.findByText(/El micrófono elegido, Auriculares USB, no está conectado. Nexium usará Microphone Array/)).toBeInTheDocument();
  });

  it("avisa si no hay ningún micrófono", async () => {
    invoke.mockResolvedValue(view({ microphones: [], activeMicrophone: null }));
    render(<SettingsScreen />);
    expect(await screen.findByText(/No se encontró ningún micrófono/)).toBeInTheDocument();
  });
});

describe("Ajustes: uso de IA", () => {
  it("concuerda la cantidad de solicitudes y de tokens", async () => {
    invoke.mockResolvedValue(view({ aiRequestsToday: 1, tokensToday: 1 }));
    render(<SettingsScreen />);
    expect(await screen.findByText("Hoy: 1 solicitud · 1 token")).toBeInTheDocument();
  });
});

describe("Ajustes: carpetas autorizadas", () => {
  it("se añaden con el selector del sistema y se quitan al momento, sin Guardar", async () => {
    invoke.mockImplementation((command: string) => {
      if (command === "settings_get") return Promise.resolve(view({ knownFolders: ["C:\\Users\\a\\Documents"] }));
      if (command === "folders_pick") return Promise.resolve(["D:\\Proyectos"]);
      if (command === "folders_remove") return Promise.resolve([]);
      return Promise.resolve(null);
    });
    render(<SettingsScreen />);
    expect(await screen.findByText("Todavía no hay ninguna carpeta autorizada.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Añadir carpeta" }));
    expect(invoke).toHaveBeenCalledWith("folders_pick");
    expect(await screen.findByText("D:\\Proyectos")).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("Carpeta añadida.");
    expect(screen.getByRole("button", { name: "Permitir Documentos y Escritorio" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Quitar la carpeta D:\\Proyectos" }));
    expect(invoke).toHaveBeenCalledWith("folders_remove", { folder: "D:\\Proyectos" });
    expect(await screen.findByText("Todavía no hay ninguna carpeta autorizada.")).toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith("settings_save", expect.anything());
  });
});
