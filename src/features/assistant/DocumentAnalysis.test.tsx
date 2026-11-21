import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { DocumentAnalysisView, splitIdea } from "./DocumentAnalysis";

describe("resumen de un documento", () => {
  it("muestra el resumen, cada idea con su nombre destacado y los datos importantes", () => {
    render(
      <DocumentAnalysisView
        task="summarize"
        summary="Guía de la asignatura. Tiene tres unidades."
        points={["Normalización: evita datos repetidos.", "Índices: aceleran las consultas."]}
        details={["Parcial 1: 30 %."]}
        truncated={false}
      />,
    );
    expect(screen.getByText("Guía de la asignatura. Tiene tres unidades.")).toBeInTheDocument();
    const ideas = screen.getByRole("heading", { name: "Ideas clave" }).parentElement as HTMLElement;
    expect(within(ideas).getAllByRole("listitem")).toHaveLength(2);
    expect(within(ideas).getByText("Normalización:")).toHaveClass("font-semibold");
    expect(screen.getByRole("heading", { name: "Datos importantes" })).toBeInTheDocument();
  });

  it("sin datos importantes no muestra esa sección", () => {
    render(<DocumentAnalysisView task="analyze" summary="s" points={["a"]} details={[]} truncated />);
    expect(screen.getByRole("heading", { name: "Observaciones" })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Datos importantes" })).not.toBeInTheDocument();
    expect(screen.getByText(/solo se envió su principio/)).toBeInTheDocument();
  });

  it("solo separa el nombre de una idea si va al principio", () => {
    expect(splitIdea("Plazo: 28 de noviembre")).toEqual({ name: "Plazo", text: "28 de noviembre" });
    expect(splitIdea("Una idea sin nombre")).toEqual({ name: null, text: "Una idea sin nombre" });
  });
});
