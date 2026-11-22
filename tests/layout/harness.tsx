// Arnés de las pruebas de diseño: monta la app real con un backend simulado.
// Los datos son de ejemplo y exageran a propósito (nombres, rutas y textos largos) para forzar el diseño.
import { emit } from "@tauri-apps/api/event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "@/app/App";
import "@/styles/theme.css";
import type { AssistantResponse, HistoryEntry, Reminder, ResponseBody, SettingsView, Stage } from "@/types/api";

const now = Date.now();
const iso = (offsetHours: number) => new Date(now + offsetHours * 3_600_000).toISOString();
const longPath = "C:\\Users\\usuario.con.un.nombre.muy.largo\\OneDrive - Universidad Nacional de Colombia\\Documentos\\Semestre 2026-2\\Bases de datos\\Proyecto final\\Entregas\\Informe_final_bases_de_datos_version_definitiva_revisada.docx";

const response = (request: string, body: ResponseBody, status: AssistantResponse["status"] = "completed"): AssistantResponse => ({
  id: Math.floor(Math.random() * 1_000_000),
  request,
  createdAt: new Date().toISOString(),
  category: "general",
  status,
  originLabel: "lectura directa del equipo, sin IA",
  summary: "Resumen de la respuesta",
  body,
});

/** Cada palabra clave de la solicitud elige una respuesta de ejemplo. */
const responses: Record<string, () => AssistantResponse> = {
  ram: () =>
    response("¿Cuánta memoria RAM estoy utilizando?", {
      kind: "reading",
      value: "11,4",
      unit: "GB",
      context: "en uso de 16,0 GB",
      percent: 71,
      rows: [
        { label: "Disponible", value: "4,6 GB", mono: true, unavailable: false },
        { label: "Procesos con más memoria", value: "Navegador web, editor de código y servicio de sincronización", mono: false, unavailable: false },
      ],
      attribution: null,
    }),
  clima: () => ({
    ...response("¿Cómo está el clima hoy en San José de las Lajas del Río Grande?", {
      kind: "reading",
      value: "22",
      unit: "°C",
      context: "parcialmente nublado en San José de las Lajas del Río Grande, ahora",
      percent: null,
      rows: [{ label: "Pronóstico de hoy", value: "12 °C a 23 °C", mono: true, unavailable: false }],
      attribution: { text: "Datos meteorológicos de Open-Meteo (CC BY 4.0)", url: "https://open-meteo.com" },
    }),
    originLabel: "Open-Meteo, sin IA",
  }),
  archivos: () =>
    response("Busca mi informe de bases de datos", {
      kind: "files",
      summary: "2 coincidencias en Documentos y Escritorio",
      note: "Hay más coincidencias o carpetas muy grandes. Prueba con términos más concretos.",
      files: [
        { name: "Informe_final_bases_de_datos_version_definitiva_revisada", extension: ".docx", location: "Documentos", path: longPath, exists: true, matchedInContent: false },
        { name: "informe_bd", extension: ".pdf", location: "Escritorio", path: "C:\\Users\\usuario\\Desktop\\informe_bd.pdf", exists: false, matchedInContent: true },
      ],
    }),
  pantalla: () =>
    response("Explícame este error", {
      kind: "screenAnalysis",
      summary: "El programa Inventario no puede iniciarse porque falta su archivo de configuración.",
      cause: "El mensaje muestra el código 0x80070002, que en Windows significa que no se encuentra un archivo: en este caso, C:\\Program Files\\Inventario\\configuracion.ini.",
      steps: ["Comprueba que el archivo exista en la carpeta del programa.", "Si no está, reinstala Inventario desde su instalador original.", "Vuelve a abrir el programa y comprueba si el mensaje desaparece."],
    }),
  error: () =>
    response(
      "Busca mi informe de programación",
      { kind: "error", code: "quotaExhausted", message: "Se agotó la cuota diaria gratuita de Google Gemini.", hint: "Se renueva a medianoche, hora del Pacífico. Las consultas de hora, sistema y red siguen funcionando." },
      "failed",
    ),
  recordatorio: () =>
    response(
      "Recuérdame entregar el proyecto el jueves",
      {
        kind: "reminderDraft",
        content: "Entregar el proyecto final de bases de datos al profesor",
        dueAt: null,
        question: "¿A qué hora del jueves 19 de noviembre quieres el aviso?",
        options: [
          { label: "08:00", dueAt: iso(24) },
          { label: "18:00", dueAt: iso(34) },
        ],
      },
      "pending",
    ),
  aclara: () =>
    response(
      "Abre eso",
      {
        kind: "clarification",
        question: "¿Qué quieres que haga? No puedo abrir programas, pero puedo ayudarte de otra forma.",
        suggestions: [
          { label: "Buscar un archivo", request: "Busca un archivo" },
          { label: "Analizar la pantalla", request: "Analiza mi pantalla" },
          { label: "Ver mis recordatorios pendientes", request: "Mis recordatorios" },
        ],
      },
      "pending",
    ),
  carpetas: () =>
    response(
      "Busca mi informe",
      { kind: "folderConsent", folders: [longPath.replace(/\\[^\\]+$/, ""), "C:\\Users\\usuario\\Desktop"], pendingRequest: "Busca mi informe" },
      "pending",
    ),
  "sin ciudad": () => response("Dame el clima en la ciudad en la que me encuentro", { kind: "weatherCityQuestion", when: "now" }, "pending"),
  documento: () =>
    response(
      "Resume mi documento de bases de datos",
      {
        kind: "documentConsent",
        task: "summarize",
        provider: "Google Gemini",
        candidates: [
          { token: "t-1", name: "Informe_final_bases_de_datos_version_definitiva_revisada", extension: ".docx", location: "Proyecto final" },
          { token: "t-2", name: "Apuntes de bases de datos", extension: ".pdf", location: "Escritorio" },
        ],
        maxCharacters: 30000,
        note: null,
        preselected: null,
      },
      "pending",
    ),
  resumen: () =>
    response("Resumen de «Informe_final_bases_de_datos_version_definitiva_revisada.docx»", {
      kind: "documentAnalysis",
      fileName: "Informe_final_bases_de_datos_version_definitiva_revisada.docx",
      task: "summarize",
      summary: "El informe describe el diseño de la base de datos de una biblioteca universitaria y justifica cada decisión de normalización.",
      points: [
        "Modelo entidad-relación: parte de nueve entidades, entre ellas Libro, Ejemplar, Socio y Préstamo.",
        "Normalización: lleva las tablas hasta la tercera forma normal para evitar datos repetidos.",
        "Índices: propone índices para las consultas más frecuentes de préstamo y devolución.",
        "Restricciones: un socio no puede tener más de cinco préstamos activos a la vez.",
        "Una idea sin nombre propio también se muestra completa, aunque sea larga y no tenga dos puntos.",
      ],
      details: ["Entrega final: 28 de noviembre de 2026.", "Peso en la nota: 40 %.", "Tercera forma normal: ninguna columna depende de otra que no sea la clave."],
      truncated: true,
    }),
};

const history: HistoryEntry[] = Array.from({ length: 12 }, (_, index) => ({
  id: index + 1,
  request: index === 0 ? "Explícame qué significa el mensaje de error que aparece al abrir el programa de inventario de la empresa" : `Solicitud de ejemplo número ${index + 1}`,
  createdAt: iso(-index),
  category: (["screen", "system", "files", "weather"] as const)[index % 4] ?? "general",
  status: (["completed", "failed", "pending", "cancelled"] as const)[index % 4] ?? "completed",
  response: index === 0 ? `Error 0x80070002: falta el archivo ${longPath}` : "Respuesta breve.",
}));

const reminders: Reminder[] = [
  { id: 1, content: "Pagar la factura de la luz antes de que corten el servicio, con el número de referencia que llegó por correo", dueAt: iso(-30), status: "missed" },
  { id: 4, content: "Tomar agua", dueAt: iso(-2), status: "notified" },
  { id: 2, content: "Entregar el proyecto", dueAt: iso(20), status: "pending" },
  { id: 3, content: "Llamar al dentista", dueAt: iso(200), status: "pending" },
];

const settings: SettingsView = {
  settings: { speechModel: "small", speechIdleMinutes: 10, weatherCity: "Medellín", authorizedFolders: [longPath.replace(/\\[^\\]+$/, "")], microphone: null },
  provider: {
    displayName: "Google Gemini",
    dataNotice:
      "Tus solicitudes y capturas pasan por el servidor de Nexium hacia Google Gemini, en su capa gratuita. Google puede usar lo que envías para mejorar sus productos, y personas pueden revisarlo. No envíes información personal ni confidencial.",
  },
  aiStatus: "working",
  aiRequestsToday: 1,
  tokensToday: 12_345,
  knownFolders: [longPath.replace(/\\[^\\]+$/, ""), "C:\\Users\\usuario\\Desktop"],
  speechModels: [
    { id: "base", displayName: "Rápido", sizeMb: 142, path: "", downloaded: true, damaged: false, isActive: false },
    { id: "small", displayName: "Preciso (recomendado)", sizeMb: 466, path: "", downloaded: true, damaged: false, isActive: true },
    { id: "medium", displayName: "Muy preciso, más lento y con más memoria", sizeMb: 1462, path: "", downloaded: false, damaged: false, isActive: false },
  ],
  microphones: [
    { name: "Mezcla estéreo (Realtek(R) Audio) con un nombre de dispositivo muy largo", isMicrophone: false, isDefault: true },
    { name: "Matriz de micrófonos (Tecnología Intel® Smart Sound para micrófonos digitales)", isMicrophone: true, isDefault: false },
  ],
  activeMicrophone: "Matriz de micrófonos (Tecnología Intel® Smart Sound para micrófonos digitales)",
};

mockIPC(
  (command, args) => {
    const payload = (args ?? {}) as Record<string, unknown>;
    switch (command) {
      case "assistant_submit": {
        const text = String(payload.text ?? "").toLowerCase();
        const key = Object.keys(responses).find((word) => text.includes(word));
        return (responses[key ?? "ram"] ?? responses.ram)?.();
      }
      case "screen_analyze":
        return responses.pantalla?.();
      case "history_list":
        return payload.status ? history.filter((entry) => entry.status === payload.status) : history;
      case "reminders_list":
        return new URLSearchParams(location.search).has("sin-recordatorios") ? [] : reminders;
      case "settings_get":
      case "settings_save":
        return settings;
      case "voice_start":
      case "voice_cancel":
      case "assistant_cancel":
        return null;
      default:
        return null;
    }
  },
  { shouldMockEvents: true },
);

/** Las pruebas cambian el estado de la barra como lo haría el backend. */
declare global {
  interface Window {
    harness: { stage: (stage: Stage) => Promise<void>; level: (value: number) => Promise<void> };
  }
}
window.harness = {
  stage: (stage) => emit("assistant://stage", { stage }),
  level: (value) => emit("voice://level", value),
};

const root = document.getElementById("root");
if (root)
  createRoot(root).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
