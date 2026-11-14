import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { Reminder } from "@/types/api";

import { isOverdue, RemindersScreen } from "./RemindersScreen";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => undefined)) }));

const future = new Date(Date.now() + 86_400_000).toISOString();
const past = new Date(Date.now() - 86_400_000).toISOString();
const reminder = (id: number, content: string, dueAt: string, status: Reminder["status"] = "pending"): Reminder => ({ id, content, dueAt, status });

function backend(reminders: Reminder[]) {
  invoke.mockImplementation((command: string) => Promise.resolve(command === "reminders_list" ? reminders : undefined));
}

beforeEach(() => invoke.mockReset());

describe("Recordatorios", () => {
  it("Eliminar pide confirmación y nombra el recordatorio", async () => {
    backend([reminder(7, "Pagar la luz", future)]);
    render(<RemindersScreen onPickExample={vi.fn()} />);
    await userEvent.click(await screen.findByRole("button", { name: "Eliminar: Pagar la luz" }));
    const dialog = screen.getByRole("alertdialog", { name: "¿Eliminar este recordatorio?" });
    expect(dialog).toHaveAccessibleDescription(/«Pagar la luz».*No se puede deshacer\./);
    expect(invoke).not.toHaveBeenCalledWith("reminders_delete", expect.anything());
    await userEvent.click(within(dialog).getByRole("button", { name: "Eliminar" }));
    expect(invoke).toHaveBeenCalledWith("reminders_delete", { id: 7 });
  });

  it("cancelar la confirmación no elimina", async () => {
    backend([reminder(7, "Pagar la luz", future)]);
    render(<RemindersScreen onPickExample={vi.fn()} />);
    await userEvent.click(await screen.findByRole("button", { name: "Eliminar: Pagar la luz" }));
    await userEvent.click(within(screen.getByRole("alertdialog")).getByRole("button", { name: "Cancelar" }));
    expect(invoke).not.toHaveBeenCalledWith("reminders_delete", expect.anything());
  });

  it("Vencido y Pendiente se distinguen con icono y palabra", async () => {
    backend([reminder(1, "Pasado", past, "notified"), reminder(2, "Futuro", future)]);
    render(<RemindersScreen onPickExample={vi.fn()} />);
    const overdue = await screen.findByText("Vencido");
    expect(overdue.closest("[data-status='overdue']")?.querySelector("svg")).not.toBeNull();
    const pending = screen.getByText("Pendiente");
    expect(pending.closest("[data-status='pending']")?.querySelector("svg")).not.toBeNull();
  });

  it("vencido es una fecha pasada o un aviso ya emitido", () => {
    const now = new Date("2026-11-13T12:00:00Z");
    expect(isOverdue(reminder(1, "a", "2026-11-13T11:00:00Z"), now)).toBe(true);
    expect(isOverdue(reminder(1, "a", "2026-11-14T11:00:00Z", "notified"), now)).toBe(true);
    expect(isOverdue(reminder(1, "a", "2026-11-14T11:00:00Z"), now)).toBe(false);
  });

  it("sin recordatorios explica cómo crear uno y ofrece un ejemplo", async () => {
    backend([]);
    const onPickExample = vi.fn();
    render(<RemindersScreen onPickExample={onPickExample} />);
    expect(await screen.findByText("No tienes recordatorios")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: /Recuérdame entregar el proyecto/ }));
    expect(onPickExample).toHaveBeenCalledWith("Recuérdame entregar el proyecto el viernes a las nueve");
  });

  it("muestra que carga mientras llega la lista", async () => {
    let finish: (value: unknown[]) => void = () => undefined;
    invoke.mockReturnValue(new Promise((resolve) => (finish = resolve)));
    render(<RemindersScreen onPickExample={vi.fn()} />);
    expect(screen.getByRole("status")).toHaveTextContent("Cargando recordatorios");
    await act(async () => finish([]));
  });
});
