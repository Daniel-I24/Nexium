// Datos de ejemplo solo para revisar la composición. Nunca se empaquetan en la aplicación.
import { createRoot } from "react-dom/client";

import { Shell } from "@/app/Shell";
import { AssistantScreen } from "@/features/assistant/AssistantScreen";
import { isSensing, type BarState } from "@/features/voice/barState";
import { SignalBar } from "@/features/voice/SignalBar";
import "@/styles/theme.css";
import type { AssistantResponse, ResponseBody } from "@/types/api";

const response = (id: number, request: string, summary: string, body: ResponseBody): AssistantResponse => ({
  id,
  request,
  summary,
  body,
  createdAt: "2026-11-05T14:05:00",
  category: "system",
  status: "completed",
  originLabel: "lectura directa del equipo, sin IA",
});

const ram = response(3, "¿Cuánta memoria RAM estoy utilizando?", "11,4 GB en uso", {
  kind: "reading",
  value: "11,4",
  unit: "GB",
  context: "en uso de 16 GB",
  percent: 71,
  rows: [{ label: "Disponible", value: "4,6 GB", mono: true, unavailable: false }],
  attribution: null,
});

const tape = [
  response(1, "¿Qué hora es?", "14:02 · jueves 5 de noviembre", { kind: "text", text: "" }),
  response(2, "¿Estoy conectado a Internet?", "Conectado por Wi-Fi", { kind: "text", text: "" }),
];

const bodies: Record<string, AssistantResponse | null> = {
  empty: null,
  ram,
  files: response(4, "Busca mi informe de programación", "2 coincidencias", {
    kind: "files",
    summary: "2 coincidencias en Documentos y Escritorio",
    note: null,
    files: [
      { name: "Informe programación final", extension: ".docx", location: "Documentos", path: "C:\\Users\\usuario\\Documents\\Clase\\Informe programación final.docx", exists: true },
      { name: "informe_prog_v2", extension: ".pdf", location: "Escritorio", path: "C:\\Users\\usuario\\Desktop\\informe_prog_v2.pdf", exists: true },
    ],
  }),
  draft: response(5, "Recuérdame entregar el proyecto el jueves", "Pendiente de aclaración", {
    kind: "reminderDraft",
    content: "Entregar el proyecto",
    dueAt: null,
    question: "¿A qué hora del jueves 12 de noviembre quieres el aviso?",
    options: [
      { label: "08:00", dueAt: "2026-11-12T08:00:00" },
      { label: "18:00", dueAt: "2026-11-12T18:00:00" },
    ],
  }),
  weather: response(6, "¿Cómo está el clima hoy?", "22 °C", {
    kind: "reading",
    value: "22",
    unit: "°C",
    context: "parcialmente nublado en Bogotá, ahora",
    percent: null,
    rows: [{ label: "Pronóstico de hoy", value: "12 °C a 23 °C", mono: true, unavailable: false }],
    attribution: { text: "Datos meteorológicos de Open-Meteo (CC BY 4.0)", url: "https://open-meteo.com" },
  }),
  unavailable: response(7, "¿A qué red Wi-Fi estoy conectado?", "Conectado", {
    kind: "rows",
    rows: [
      { label: "Estado de Wi-Fi", value: "Conectado", mono: true, unavailable: false },
      { label: "Nombre de la red", value: "No disponible", mono: false, unavailable: true },
    ],
    note: "Windows solo muestra el nombre si activas “Permitir que las aplicaciones de escritorio accedan a tu ubicación”.",
    attribution: null,
  }),
  screen: response(8, "Explícame este error", "El programa no encuentra un archivo", {
    kind: "text",
    text: "El mensaje indica que el programa no encuentra un archivo en la ruta que le diste. Comprueba que el nombre esté bien escrito y que estés en la carpeta correcta.",
  }),
  quota: response(9, "Busca mi informe de programación", "Cuota agotada", {
    kind: "error",
    code: "quotaExhausted",
    message: "Se agotó la cuota diaria gratuita de Google Gemini.",
    hint: "Se renueva a medianoche, hora del Pacífico. Las consultas de hora, sistema y red siguen funcionando.",
  }),
};

const notice = "En la capa gratuita, Google puede usar lo que envías para mejorar sus productos, y personas pueden revisarlo. No captures información personal ni confidencial.";

const bars: Record<string, BarState> = {
  idle: { kind: "idle" },
  listening: { kind: "listening" },
  processing: { kind: "processing", step: "Transcribiendo tu voz en este equipo" },
  responding: { kind: "responding" },
  capture: { kind: "captureConsent", provider: "Google Gemini", notice },
  clarify: {
    kind: "clarify",
    question: "¿A qué hora del jueves 12 de noviembre quieres el aviso?",
    reminderOptions: [
      { label: "08:00", dueAt: "2026-11-12T08:00:00" },
      { label: "18:00", dueAt: "2026-11-12T18:00:00" },
    ],
    suggestions: [],
  },
  error: { kind: "error", message: "No puedo usar el micrófono: el sistema le ha negado el permiso a Nexium. Actívalo en la configuración de privacidad.", retryable: true },
};

const params = new URLSearchParams(location.search);
const bar = bars[params.get("bar") ?? "idle"] ?? { kind: "idle" };
const current = bodies[params.get("body") ?? "ram"] ?? null;
const levels = Array.from({ length: 72 }, (_, index) => Math.abs(Math.sin(index * 0.55) * Math.cos(index * 0.13)));
const noop = () => undefined;

const root = document.getElementById("root");
if (root)
  createRoot(root).render(
    <Shell
      screen="assistant"
      sensing={isSensing(bar)}
      onSelect={noop}
      bar={
        <SignalBar
          state={bar}
          levels={levels}
          onSpeak={noop}
          onStopListening={noop}
          onCancel={noop}
          onSubmitText={noop}
          onAnalyzeScreen={noop}
          onConfirmCapture={noop}
          onRetry={noop}
          onChooseReminderOption={noop}
          onChooseSuggestion={noop}
        />
      }
    >
      <AssistantScreen current={current} tape={current ? tape : []} onPickExample={noop} onSaveReminder={noop} onAuthorizeFolders={noop} />
    </Shell>,
  );
