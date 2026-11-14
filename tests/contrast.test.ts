import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

/** Contraste WCAG de los tokens de color en los dos temas, leídos de la hoja de estilos real. */
const css = readFileSync("src/styles/theme.css", "utf8");

function tokens(block: string): Record<string, string> {
  return Object.fromEntries([...block.matchAll(/--nx-([a-z-]+):\s*(#[0-9a-f]{6});/gi)].map((match) => [match[1] ?? "", match[2] ?? ""]));
}

const light = tokens(/^:root\s*\{([^}]*)\}/m.exec(css)?.[1] ?? "");
const dark = { ...light, ...tokens(/@media \(prefers-color-scheme: dark\)\s*\{\s*:root\s*\{([^}]*)\}/.exec(css)?.[1] ?? "") };
const fixed = { signal: "#ffd000", "on-signal": "#14181a" };

function luminance(hex: string) {
  const channels = [1, 3, 5].map((start) => parseInt(hex.slice(start, start + 2), 16) / 255);
  const [r = 0, g = 0, b = 0] = channels.map((value) => (value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function ratio(a: string, b: string) {
  const [high, low] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return ((high ?? 0) + 0.05) / ((low ?? 0) + 0.05);
}

it("lee los nueve colores de cada tema", () => {
  expect(Object.keys(light)).toHaveLength(9);
  expect(dark.base).toBe("#191d1e");
});

describe.each([
  ["claro", { ...light, ...fixed }],
  ["oscuro", { ...dark, ...fixed }],
])("contraste del tema %s", (_, theme: Record<string, string>) => {
  const color = (name: string) => {
    const value = theme[name];
    if (!value) throw new Error(`Falta el token ${name}`);
    return value;
  };

  it.each(["ink", "muted", "ok", "error", "query"])("el texto %s llega a 4,5:1 sobre base y panel", (name) => {
    expect(ratio(color(name), color("base"))).toBeGreaterThanOrEqual(4.5);
    expect(ratio(color(name), color("panel"))).toBeGreaterThanOrEqual(4.5);
  });

  it("los botones con fondo de color tienen texto legible", () => {
    expect(ratio(color("panel"), color("ink"))).toBeGreaterThanOrEqual(4.5);
    expect(ratio(color("on-signal"), color("signal"))).toBeGreaterThanOrEqual(4.5);
  });

  it("los bordes de los controles y el foco llegan a 3:1", () => {
    for (const background of ["base", "panel"]) {
      expect(ratio(color("line"), color(background))).toBeGreaterThanOrEqual(3);
      expect(ratio(color("ink"), color(background))).toBeGreaterThanOrEqual(3);
    }
    expect(ratio(color("on-signal"), color("signal"))).toBeGreaterThanOrEqual(3);
  });
});
