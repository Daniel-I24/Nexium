import { fileURLToPath } from "node:url";

import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  // Las dependencias de la app y de la página de las pruebas de diseño se analizan al arrancar el servidor. Si
  // no, Vite las descubre en la primera carga y recarga la página a mitad de una prueba.
  optimizeDeps: { entries: ["index.html", "tests/layout/harness.html"] },
  build: { target: "es2022", sourcemap: false },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./tests/setup.ts"],
    css: false,
    include: ["src/**/*.test.{ts,tsx}", "tests/**/*.test.{ts,tsx}"],
    exclude: ["tests/layout/**", "node_modules/**"],
    coverage: { provider: "v8", include: ["src/**/*.{ts,tsx}"], exclude: ["src/**/*.test.{ts,tsx}"], reporter: ["text-summary", "json-summary"], reportsDirectory: "coverage" },
  },
});
