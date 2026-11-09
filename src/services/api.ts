import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  AssistantResponse,
  DownloadProgress,
  HistoryEntry,
  InteractionStatus,
  Reminder,
  Settings,
  SettingsView,
  StageEvent,
} from "@/types/api";

/** Único punto de contacto con el backend: ningún componente llama a invoke directamente. */
export const api = {
  submitRequest: (text: string) => invoke<AssistantResponse>("assistant_submit", { text }),
  cancel: () => invoke<undefined>("assistant_cancel"),
  authorizeFolders: (folders: string[]) => invoke<undefined>("folders_authorize", { folders }),
  openAttribution: () => invoke<undefined>("open_attribution"),

  startVoice: () => invoke<undefined>("voice_start"),
  stopVoice: () => invoke<AssistantResponse | null>("voice_stop"),
  cancelVoice: () => invoke<undefined>("voice_cancel"),
  downloadSpeechModel: (modelId: string) => invoke<undefined>("speech_model_download", { modelId }),

  analyzeScreen: (question: string) => invoke<AssistantResponse>("screen_analyze", { question }),

  listReminders: () => invoke<Reminder[]>("reminders_list"),
  createReminder: (content: string, dueAt: string, historyId: number | null) =>
    invoke<Reminder>("reminders_create", { content, dueAt, historyId }),
  updateReminder: (id: number, content: string, dueAt: string) => invoke<Reminder>("reminders_update", { id, content, dueAt }),
  deleteReminder: (id: number) => invoke<undefined>("reminders_delete", { id }),

  listHistory: (status: InteractionStatus | null) => invoke<HistoryEntry[]>("history_list", { status }),
  deleteHistory: (id: number) => invoke<undefined>("history_delete", { id }),
  clearHistory: () => invoke<undefined>("history_clear"),

  getSettings: () => invoke<SettingsView>("settings_get"),
  saveSettings: (settings: Settings) => invoke<SettingsView>("settings_save", { settings }),
};

export const events = {
  onStage: (handler: (event: StageEvent) => void): Promise<UnlistenFn> =>
    listen<StageEvent>("assistant://stage", (event) => handler(event.payload)),
  onLevel: (handler: (level: number) => void): Promise<UnlistenFn> =>
    listen<number>("voice://level", (event) => handler(event.payload)),
  onVoiceLimit: (handler: () => void): Promise<UnlistenFn> => listen("voice://limit", () => handler()),
  onDownload: (handler: (progress: DownloadProgress) => void): Promise<UnlistenFn> =>
    listen<DownloadProgress>("speech://download", (event) => handler(event.payload)),
  onRemindersChanged: (handler: () => void): Promise<UnlistenFn> => listen("reminders://changed", () => handler()),
};

/** Los errores de un comando llegan como texto en español ya listo para mostrar. */
export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "Ocurrió un error inesperado.";
}
