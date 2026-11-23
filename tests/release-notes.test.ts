import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

// @ts-expect-error: script de Node sin tipos, el mismo que usa el flujo de publicación.
import { notesFor, renderNotes } from "../scripts/release-notes.mjs";

const version = JSON.parse(readFileSync("package.json", "utf8")).version as string;

describe("Notas de la Release", () => {
  it("empiezan por la tabla de descarga, con un archivo por sistema y sin .msi", () => {
    const notes: string = notesFor(version);
    expect(notes.startsWith("## Descargar")).toBe(true);
    for (const file of [`Nexium_${version}_x64-setup.exe`, `Nexium_${version}_aarch64.dmg`, `Nexium_${version}_x64.dmg`, `Nexium_${version}_amd64.deb`, `Nexium_${version}_amd64.AppImage`]) {
      expect(notes).toContain(file);
    }
    expect(notes).not.toContain(".msi");
    expect(notes).not.toMatch(/__[A-Z]+__/);
  });

  it("explican SmartScreen, Gatekeeper y los archivos técnicos", () => {
    const notes: string = notesFor(version);
    expect(notes).toContain("**Más información** y después **Ejecutar de todas formas**");
    expect(notes).toContain("**Abrir igualmente**");
    expect(notes).toContain("no hace falta descargarlos");
  });

  it("la versión actual tiene sus novedades", () => {
    expect(notesFor(version)).toContain(`## Novedades de la ${version}`);
  });

  it("rechaza una versión mal escrita", () => {
    expect(() => renderNotes("x", "v1.0", "")).toThrow();
  });
});
