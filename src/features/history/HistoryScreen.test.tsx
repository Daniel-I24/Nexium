import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { HistoryEntry } from "@/types/api";

import { clearDescription, HistoryScreen } from "./HistoryScreen";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

const entry = (id: number, status: HistoryEntry["status"], response = "ok"): HistoryEntry => ({
  id,
  request: `Solicitud ${id}`,
  createdAt: "2026-11-13T10:00:00-05:00",
  category: "general",
  status,
  response,
});

function backend(entries: HistoryEntry[]) {
  invoke.mockImplementation((command: string) => Promise.resolve(command === "history_list" ? entries : undefined));
}

beforeEach(() => invoke.mockReset());

describe("Historial: Borrar todo", () => {
  it("pide confirmación y nombra cuántas entradas se borrarán", async () => {
    backend([entry(1, "completed"), entry(2, "failed")]);
    render(<HistoryScreen onCleared={vi.fn()} />);
    await userEvent.click(await screen.findByRole("button", { name: "Borrar todo" }));
    const dialog = screen.getByRole("alertdialog", { name: "¿Borrar todo el historial?" });
    expect(dialog).toHaveAccessibleDescription(/Se borrarán las 2 entradas del historial de este equipo\. .*No se puede deshacer\./);
    expect(invoke).not.toHaveBeenCalledWith("history_clear");
  });

  it("el foco empieza en Cancelar y cancelar no borra nada", async () => {
    backend([entry(1, "completed")]);
    render(<HistoryScreen onCleared={vi.fn()} />);
    await userEvent.click(await screen.findByRole("button", { name: "Borrar todo" }));
    const cancel = within(screen.getByRole("alertdialog")).getByRole("button", { name: "Cancelar" });
    expect(cancel).toHaveFocus();
    await userEvent.keyboard("{Enter}");
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith("history_clear");
  });

  it("Escape cierra el diálogo sin borrar", async () => {
    backend([entry(1, "completed")]);
    render(<HistoryScreen onCleared={vi.fn()} />);
    await userEvent.click(await screen.findByRole("button", { name: "Borrar todo" }));
    screen.getByRole("alertdialog").dispatchEvent(new Event("cancel", { cancelable: true }));
    expect(await screen.findByRole("button", { name: "Borrar todo" })).toBeInTheDocument();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith("history_clear");
  });

  it("confirmar borra todo, vacía la conversación y vuelve a cargar", async () => {
    backend([entry(1, "completed")]);
    const onCleared = vi.fn();
    render(<HistoryScreen onCleared={onCleared} />);
    await userEvent.click(await screen.findByRole("button", { name: "Borrar todo" }));
    expect(screen.getByRole("alertdialog")).toHaveAccessibleDescription(/También se vacía la conversación del Asistente\./);
    await userEvent.click(within(screen.getByRole("alertdialog")).getByRole("button", { name: "Borrar todo" }));
    expect(invoke).toHaveBeenCalledWith("history_clear");
    await waitFor(() => expect(onCleared).toHaveBeenCalledOnce());
  });

  it("con un filtro activo advierte que también se borra lo que no se ve", () => {
    expect(clearDescription("failed", 1)).toContain("también las que no muestra el filtro «Fallidas»");
    expect(clearDescription(null, 1)).toContain("Se borrará la única entrada del historial");
  });
});

describe("Historial: lectura", () => {
  it("cada estado lleva icono y palabra, no solo color", async () => {
    backend([entry(1, "completed"), entry(2, "failed"), entry(3, "pending")]);
    render(<HistoryScreen onCleared={vi.fn()} />);
    for (const label of ["Completada", "Fallida", "Pendiente"]) {
      const status = await screen.findByText(label);
      expect(status.closest("[data-status]")?.querySelector("svg")).not.toBeNull();
    }
  });

  it("muestra que carga y no dice que está vacío antes de tiempo", async () => {
    let finish: (value: unknown[]) => void = () => undefined;
    invoke.mockReturnValue(new Promise((resolve) => (finish = resolve)));
    render(<HistoryScreen onCleared={vi.fn()} />);
    expect(screen.getByRole("status")).toHaveTextContent("Cargando el historial");
    expect(screen.queryByText(/Todavía no hay nada/)).not.toBeInTheDocument();
    await act(async () => finish([]));
  });

  it("el estado vacío depende del filtro", async () => {
    backend([]);
    render(<HistoryScreen onCleared={vi.fn()} />);
    expect(await screen.findByText(/Todavía no hay nada en el historial/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Fallidas" }));
    expect(await screen.findByText("No hay solicitudes fallidas.")).toBeInTheDocument();
  });

  it("no menciona claves: Nexium no guarda ninguna", async () => {
    backend([]);
    render(<HistoryScreen onCleared={vi.fn()} />);
    await screen.findByText(/Todavía no hay nada/);
    expect(document.body).not.toHaveTextContent(/clave/i);
  });

  it("cada Eliminar nombra la solicitud que borra", async () => {
    backend([entry(1, "completed")]);
    render(<HistoryScreen onCleared={vi.fn()} />);
    expect(await screen.findByRole("button", { name: "Eliminar del historial: Solicitud 1" })).toBeInTheDocument();
  });
});
