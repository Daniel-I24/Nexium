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
    settings: { dailyAiLimit: 200, speechModel: "small", speechIdleMinutes: 10, weatherCity: "", authorizedFolders: [], foldersConfirmed: false, microphone },
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
    expect(screen.getByRole("option", { name: "Stereo Mix (predeterminado de Windows, no parece un micrófono)" })).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Micrófono" })).toHaveValue("");
  });

  it("guarda el micrófono elegido", async () => {
    invoke.mockResolvedValue(view());
    render(<SettingsScreen />);
    await userEvent.selectOptions(await screen.findByRole("combobox", { name: "Micrófono" }), "Microphone Array");
    await userEvent.click(screen.getByRole("button", { name: "Guardar ajustes" }));
    expect(invoke).toHaveBeenCalledWith("settings_save", { settings: expect.objectContaining({ microphone: "Microphone Array" }) });
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
