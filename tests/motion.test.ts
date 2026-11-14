import { readFileSync } from "node:fs";

import { afterEach, describe, expect, it, vi } from "vitest";

import { exitThen, withViewTransition } from "@/utils/motion";

/** El sistema de movimiento, leído de la hoja de estilos real (docs/DISENO_VISUAL.md, sección 4). */
const css = readFileSync("src/styles/theme.css", "utf8");

describe("tokens de movimiento", () => {
  it("todas las duraciones están entre 150 y 300 ms", () => {
    const durations = [...css.matchAll(/--transition-duration-([a-z]+):\s*(\d+)ms;/g)];
    expect(durations.map(([, name]) => name)).toEqual(["fast", "base", "slow"]);
    for (const [, name, value] of durations) expect(Number(value), name).toBeGreaterThanOrEqual(150);
    for (const [, name, value] of durations) expect(Number(value), name).toBeLessThanOrEqual(300);
  });

  it("los fotogramas clave solo animan opacidad y transformaciones", () => {
    const frames = [...css.matchAll(/@keyframes ([a-z-]+) \{([\s\S]*?)\n {2}\}/g)];
    expect(frames.length).toBeGreaterThan(8);
    for (const [, name, body] of frames) {
      const properties = [...(body ?? "").matchAll(/([a-z-]+):/g)].map(([, property]) => property);
      expect(properties.every((property) => property === "opacity" || property === "transform"), `${name}: ${properties.join(", ")}`).toBe(true);
    }
  });

  it("ninguna curva rebota: los puntos de control quedan entre 0 y 1", () => {
    for (const [, , values] of css.matchAll(/--ease-([a-z]+):\s*cubic-bezier\(([^)]+)\)/g)) {
      for (const value of (values ?? "").split(",").map(Number)) {
        expect(value).toBeGreaterThanOrEqual(0);
        expect(value).toBeLessThanOrEqual(1);
      }
    }
  });

  it("con Reducir movimiento se anula todo, también las transiciones de vista", () => {
    const reduced = css.slice(css.indexOf("@media (prefers-reduced-motion: reduce)"));
    expect(reduced).toContain("animation: none !important");
    expect(reduced).toContain("transition: none !important");
    expect(reduced).toContain("::view-transition-new(*)");
  });
});

describe("transiciones en código", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    Reflect.deleteProperty(document, "startViewTransition");
  });

  const motion = (reduce: boolean) => vi.stubGlobal("matchMedia", (query: string) => ({ matches: reduce && query.includes("reduce") }));

  it("el cambio de sección usa una transición de vista si se permite el movimiento", () => {
    motion(false);
    const start = vi.fn((update: () => void) => update());
    Object.assign(document, { startViewTransition: start });
    const update = vi.fn();
    withViewTransition(update);
    expect(start).toHaveBeenCalledOnce();
    expect(update).toHaveBeenCalledOnce();
  });

  it("con Reducir movimiento el cambio es directo", () => {
    motion(true);
    const start = vi.fn();
    Object.assign(document, { startViewTransition: start });
    const update = vi.fn();
    withViewTransition(update);
    expect(start).not.toHaveBeenCalled();
    expect(update).toHaveBeenCalledOnce();
  });

  it("una salida se anima antes de la acción, salvo con Reducir movimiento", () => {
    vi.useFakeTimers();
    motion(false);
    const mark = vi.fn();
    const action = vi.fn();
    exitThen(mark, action);
    expect(mark).toHaveBeenCalledOnce();
    expect(action).not.toHaveBeenCalled();
    vi.advanceTimersByTime(200);
    expect(action).toHaveBeenCalledOnce();

    motion(true);
    const immediate = vi.fn();
    exitThen(mark, immediate);
    expect(immediate).toHaveBeenCalledOnce();
    vi.useRealTimers();
  });
});
