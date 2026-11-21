import { mkdirSync } from "node:fs";

import { expect, test, type Page } from "@playwright/test";

import { auditLayout } from "./audit";
import { auditFrame } from "./frame";

/**
 * Pruebas de diseño en un navegador real: cada pantalla, diálogo y estado de la barra, en varios tamaños de
 * ventana y de texto. Las escalas de pantalla (100 % a 200 %) y los temas son proyectos de playwright.config.ts.
 *
 * Los tamaños son de ventana en píxeles lógicos: lo que ve la interfaz con cualquier escala del sistema.
 */
const windows = [
  { name: "minima", width: 640, height: 400 },
  { name: "pequena", width: 800, height: 600 },
  { name: "inicial", width: 1100, height: 720 },
  { name: "portatil", width: 1366, height: 768 },
  { name: "maximizada-1080p", width: 1920, height: 1032 },
  { name: "maximizada-1440p", width: 2560, height: 1392 },
];

/**
 * Tamaño de texto del sistema. Se cambia la letra predeterminada del navegador, como hace el sistema: así cambian
 * a la vez los rem de la interfaz y los puntos de corte de las media queries, que no siguen al estilo de `html`.
 */
const textScales = [1, 1.5, 2];

async function setTextScale(page: Page, scale: number) {
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Page.setFontSizes", { fontSizes: { standard: Math.round(16 * scale), fixed: Math.round(13 * scale) } });
  await cdp.detach();
}

async function submit(page: Page, text: string) {
  await page.getByRole("button", { name: "Asistente" }).click();
  const input = page.getByLabel("Escribe tu solicitud");
  await input.fill(text);
  await input.press("Enter");
  await expect(page.locator("article h1")).toBeVisible();
}

async function screen(page: Page, name: string) {
  await page.getByRole("button", { name, exact: true }).click();
  await expect(page.getByRole("heading", { level: 1, name })).toBeVisible();
}

const views: Record<string, (page: Page) => Promise<void>> = {
  "asistente-vacio": async () => undefined,
  "asistente-lectura": (page) => submit(page, "ram"),
  "asistente-clima": (page) => submit(page, "clima"),
  "asistente-archivos": (page) => submit(page, "archivos"),
  "asistente-pantalla": (page) => submit(page, "pantalla"),
  "asistente-error": async (page) => {
    await submit(page, "error");
    await expect(page.getByRole("button", { name: "Cerrar" })).toBeVisible();
  },
  "asistente-borrador": async (page) => {
    await submit(page, "recordatorio");
    await expect(page.getByRole("button", { name: "08:00" })).toBeVisible();
  },
  "asistente-aclaracion": async (page) => {
    await submit(page, "aclara");
    await expect(page.getByRole("button", { name: "Buscar un archivo" })).toBeVisible();
  },
  "asistente-carpetas": (page) => submit(page, "carpetas"),
  "asistente-documento": async (page) => {
    await submit(page, "documento");
    await expect(page.getByRole("radio", { name: /Apuntes de bases de datos/ })).toBeVisible();
  },
  "asistente-resumen": (page) => submit(page, "resumen"),
  "asistente-clima-sin-ciudad": async (page) => {
    await submit(page, "sin ciudad");
    await expect(page.getByRole("button", { name: "Consultar el clima" })).toBeVisible();
  },
  "barra-escuchando": async (page) => {
    await page.getByRole("button", { name: "Hablar" }).click();
    await page.evaluate(async () => {
      await window.harness.stage("listening");
      for (let index = 0; index < 72; index += 1) await window.harness.level(Math.abs(Math.sin(index / 3)));
    });
    await expect(page.getByRole("button", { name: "Terminar" })).toBeVisible();
  },
  "barra-procesando": async (page) => {
    await page.evaluate(() => window.harness.stage("transcribing"));
    await expect(page.getByText("Transcribiendo tu voz en este equipo…")).toBeVisible();
  },
  "barra-respondiendo": async (page) => {
    await page.evaluate(() => window.harness.stage("responding"));
    await expect(page.getByText("Escribiendo la respuesta…")).toBeVisible();
  },
  "barra-captura": async (page) => {
    await page.getByRole("button", { name: "Analizar pantalla" }).click();
    await page.getByRole("button", { name: "Más información" }).click();
    await expect(page.getByRole("button", { name: "Capturar" })).toBeVisible();
  },
  recordatorios: (page) => screen(page, "Recordatorios"),
  "recordatorios-editar": async (page) => {
    await screen(page, "Recordatorios");
    await page.getByRole("button", { name: /^Editar: Entregar/ }).click();
    await expect(page.getByLabel("Hora")).toBeVisible();
  },
  "recordatorios-confirmar": async (page) => {
    await screen(page, "Recordatorios");
    await page.getByRole("button", { name: /^Eliminar: Pagar/ }).click();
    await expect(page.getByRole("alertdialog")).toBeVisible();
  },
  historial: (page) => screen(page, "Historial"),
  "historial-confirmar": async (page) => {
    await screen(page, "Historial");
    await page.getByRole("button", { name: "Borrar todo" }).click();
    await expect(page.getByRole("alertdialog")).toBeVisible();
  },
  ajustes: (page) => screen(page, "Ajustes"),
  "ajustes-guardado": async (page) => {
    await screen(page, "Ajustes");
    await page.getByRole("button", { name: "Guardar ajustes" }).click();
    await expect(page.getByRole("button", { name: "Ajustes guardados" })).toBeVisible();
  },
};

const shots = process.env.NEXIUM_SHOTS ? "test-results/capturas" : null;

for (const [name, open] of Object.entries(views)) {
  test(name, async ({ page }, info) => {
    await page.setViewportSize({ width: 1100, height: 720 });
    await page.goto("/tests/layout/harness.html");
    // La primera carga compila la app en el servidor de desarrollo: en la CI puede pasar de 5 s.
    await expect(page.getByRole("navigation", { name: "Secciones" })).toBeVisible({ timeout: 30_000 });
    await open(page);

    const problems: string[] = [];
    for (const size of windows) {
      for (const text of textScales) {
        await page.setViewportSize({ width: size.width, height: size.height });
        await setTextScale(page, text);
        // Dos fotogramas: el diseño ya se recalculó y el desplazamiento se asentó.
        await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
        // Primero el marco, sin desplazar nada; después, cada control desplazándose hasta él.
        const found = [...(await page.evaluate(auditFrame)), ...(await page.evaluate(auditLayout))];
        problems.push(...found.map((problem) => `${size.name} ${size.width}×${size.height}, texto ${text * 100} %: ${problem}`));
        if (shots) {
          const folder = `${shots}/${info.project.name}`;
          mkdirSync(folder, { recursive: true });
          await page.evaluate(() => document.querySelectorAll("[data-scroll-area]").forEach((area) => area.scrollTo({ top: 0 })));
          await page.screenshot({ path: `${folder}/${name}-${size.width}x${size.height}-texto${text * 100}.png` });
        }
      }
    }
    expect(problems, problems.join("\n")).toEqual([]);
  });
}
