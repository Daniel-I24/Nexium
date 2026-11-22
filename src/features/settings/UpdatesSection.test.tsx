import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { UpdatesSection } from "./UpdatesSection";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

describe("Ajustes: actualizaciones", () => {
  beforeEach(() => invoke.mockReset());

  it("no comprueba nada hasta que se pide", () => {
    render(<UpdatesSection />);
    expect(invoke).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Buscar actualizaciones" })).toBeInTheDocument();
  });

  it("dice si ya tienes la última versión", async () => {
    invoke.mockResolvedValueOnce({ current: "1.0.0", published: true, available: null });
    render(<UpdatesSection />);
    await userEvent.click(screen.getByRole("button", { name: "Buscar actualizaciones" }));
    expect(await screen.findByText("Tienes la última versión, la 1.0.0.")).toBeInTheDocument();
  });

  it("ofrece instalar una versión nueva y la instala al pedirlo", async () => {
    invoke.mockResolvedValueOnce({ current: "1.0.0", published: true, available: "1.0.1" }).mockResolvedValueOnce(undefined);
    render(<UpdatesSection />);
    await userEvent.click(screen.getByRole("button", { name: "Buscar actualizaciones" }));
    await userEvent.click(await screen.findByRole("button", { name: "Instalar la versión 1.0.1" }));
    expect(invoke).toHaveBeenLastCalledWith("update_install");
  });

  it("sin ninguna versión publicada lo dice, sin culpar a la conexión", async () => {
    invoke.mockResolvedValueOnce({ current: "1.0.0", published: false, available: null });
    render(<UpdatesSection />);
    await userEvent.click(screen.getByRole("button", { name: "Buscar actualizaciones" }));
    expect(await screen.findByText("Todavía no hay ninguna versión publicada en GitHub. Tienes la 1.0.0.")).toBeInTheDocument();
    expect(screen.queryByText(/conexión/)).toBeNull();
  });

  it("explica un fallo de conexión o de firma con el mensaje de Nexium", async () => {
    invoke.mockRejectedValueOnce("No hay conexión con GitHub, donde se publican las versiones. Revisa tu conexión a Internet.");
    render(<UpdatesSection />);
    await userEvent.click(screen.getByRole("button", { name: "Buscar actualizaciones" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("No hay conexión con GitHub");
  });
});
