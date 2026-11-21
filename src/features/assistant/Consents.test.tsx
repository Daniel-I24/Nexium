import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { DocumentConsent, FolderConsent } from "./Consents";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

const pdf = { token: "t-1", name: "Bases de datos", extension: ".pdf", location: "Documentos" };
const docx = { token: "t-2", name: "Apuntes", extension: ".docx", location: "Escritorio" };

describe("permiso de carpetas", () => {
  it("permitir Documentos y Escritorio o elegir otra carpeta continúa la misma solicitud", async () => {
    const onAuthorize = vi.fn();
    render(<FolderConsent folders={["C:\\Users\\a\\Documents"]} pendingRequest="Busca mi informe" onAuthorize={onAuthorize} onDismiss={vi.fn()} />);
    expect(screen.getByText(/nunca modifica, mueve ni elimina nada/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Permitir Documentos y Escritorio" }));
    expect(onAuthorize).toHaveBeenCalledWith("known", "Busca mi informe");
    // Una sola respuesta: los botones quedan inactivos.
    expect(screen.getByRole("button", { name: "Elegir otra carpeta" })).toBeDisabled();
  });

  it("elegir otra carpeta usa el selector del sistema, y cancelar no hace nada", async () => {
    const onAuthorize = vi.fn();
    const onDismiss = vi.fn();
    const { unmount } = render(<FolderConsent folders={[]} pendingRequest="x" onAuthorize={onAuthorize} onDismiss={onDismiss} />);
    await userEvent.click(screen.getByRole("button", { name: "Elegir otra carpeta" }));
    expect(onAuthorize).toHaveBeenCalledWith("pick", "x");
    unmount();
    render(<FolderConsent folders={[]} pendingRequest="x" onAuthorize={onAuthorize} onDismiss={onDismiss} />);
    await userEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(onDismiss).toHaveBeenCalledWith("Cancelado. No se buscó nada.");
  });
});

describe("confirmación de un documento", () => {
  beforeEach(() => invoke.mockReset());
  const props = { task: "summarize" as const, provider: "Google Gemini", maxCharacters: 30000, note: null, preselected: null, onDismiss: vi.fn() };

  it("una coincidencia clara viene marcada", () => {
    render(<DocumentConsent {...props} preselected="t-2" candidates={[pdf, docx]} onAnalyze={vi.fn()} />);
    expect(screen.getByRole("radio", { name: /Apuntes\.docx/ })).toBeChecked();
    expect(screen.getByRole("button", { name: "Resumir «Apuntes.docx»" })).toBeInTheDocument();
  });

  it("con varios candidatos sin uno claro, nada se envía hasta elegir", async () => {
    const onAnalyze = vi.fn();
    render(<DocumentConsent {...props} candidates={[pdf, docx]} onAnalyze={onAnalyze} />);
    expect(screen.getByText("Encontré 2 documentos. ¿Cuál quieres resumir?")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Resumir/ })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("radio", { name: /Apuntes\.docx/ }));
    expect(screen.getByText(/leerá «Apuntes\.docx» en este equipo y enviará su texto a Google Gemini/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Resumir «Apuntes.docx»" }));
    expect(onAnalyze).toHaveBeenCalledWith(docx, "summarize");
    expect(screen.getByRole("button", { name: "Resumir «Apuntes.docx»" })).toBeDisabled();
  });

  it("sin candidatos ofrece el selector del sistema, y elegir no envía nada", async () => {
    const onAnalyze = vi.fn();
    invoke.mockResolvedValueOnce(docx);
    render(<DocumentConsent {...props} task="analyze" candidates={[]} onAnalyze={onAnalyze} />);
    expect(screen.getByText("Elige el documento que quieres analizar.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Elegir archivo" }));
    expect(invoke).toHaveBeenCalledWith("document_pick");
    expect(onAnalyze).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Analizar «Apuntes.docx»" })).toBeInTheDocument();
  });

  it("cerrar el selector sin elegir deja todo igual", async () => {
    invoke.mockResolvedValueOnce(null);
    render(<DocumentConsent {...props} candidates={[pdf]} onAnalyze={vi.fn()} />);
    await userEvent.click(screen.getByRole("button", { name: "Elegir otro archivo" }));
    expect(screen.getByRole("button", { name: "Resumir «Bases de datos.pdf»" })).toBeEnabled();
  });
});
