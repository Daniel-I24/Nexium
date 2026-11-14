import { defineConfig, devices } from "@playwright/test";

/**
 * Pruebas de diseño (tests/layout). En Windows usan Microsoft Edge, el mismo motor que WebView2. En otros
 * sistemas, o con NEXIUM_PW_CHANNEL=chromium, usan el Chromium de Playwright (`npx playwright install chromium`).
 */
const channel = process.env.NEXIUM_PW_CHANNEL ?? (process.platform === "win32" ? "msedge" : undefined);
const browser = { ...devices["Desktop Chrome"], ...(channel && channel !== "chromium" ? { channel } : {}) };
const PORT = 1431;

export default defineConfig({
  testDir: "tests/layout",
  timeout: 180_000,
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  reporter: process.env.CI ? [["list"], ["github"]] : "list",
  use: {
    baseURL: `http://localhost:${PORT}`,
    // Las animaciones solo usan opacidad y transformaciones, pero una medición a mitad de una entrada daría posiciones falsas.
    reducedMotion: "reduce",
    locale: "es-ES",
  },
  // Escalas de pantalla del sistema (100 % a 200 %) y los dos temas.
  projects: [
    { name: "escala-100-claro", use: { ...browser, deviceScaleFactor: 1, colorScheme: "light" } },
    { name: "escala-100-oscuro", use: { ...browser, deviceScaleFactor: 1, colorScheme: "dark" } },
    { name: "escala-125", use: { ...browser, deviceScaleFactor: 1.25, colorScheme: "light" } },
    { name: "escala-150", use: { ...browser, deviceScaleFactor: 1.5, colorScheme: "dark" } },
    { name: "escala-175", use: { ...browser, deviceScaleFactor: 1.75, colorScheme: "light" } },
    { name: "escala-200", use: { ...browser, deviceScaleFactor: 2, colorScheme: "dark" } },
  ],
  webServer: {
    command: `npx vite --port ${PORT} --strictPort`,
    url: `http://localhost:${PORT}/tests/layout/harness.html`,
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
