// Contratos con el backend. Rust los serializa en camelCase (ver src-tauri/src/models).

export type Category =
  | "time"
  | "system"
  | "files"
  | "weather"
  | "reminders"
  | "screen"
  | "general";

export type InteractionStatus = "completed" | "pending" | "failed" | "cancelled";

export type Origin = "local" | "ai" | "external";

export interface DataRowContent {
  label: string;
  value: string;
  mono: boolean;
  unavailable: boolean;
}

export interface FileResult {
  name: string;
  extension: string;
  location: string;
  path: string;
  exists: boolean;
}

export interface ReminderOption {
  label: string;
  dueAt: string;
}

export interface SuggestedRequest {
  label: string;
  request: string;
}

export interface Attribution {
  text: string;
  url: string;
}

export type ResponseBody =
  | { kind: "reading"; value: string; unit: string; context: string; percent: number | null; rows: DataRowContent[]; attribution: Attribution | null }
  | { kind: "rows"; rows: DataRowContent[]; note: string | null; attribution: Attribution | null }
  | { kind: "text"; text: string }
  | { kind: "files"; summary: string; files: FileResult[]; note: string | null }
  | { kind: "reminderDraft"; content: string; dueAt: string | null; question: string | null; options: ReminderOption[] }
  | { kind: "reminderSaved"; reminder: Reminder }
  | { kind: "clarification"; question: string; suggestions: SuggestedRequest[] }
  | { kind: "folderConsent"; folders: string[]; pendingRequest: string }
  | { kind: "captureRequest"; question: string }
  | { kind: "error"; code: string; message: string; hint: string | null };

export interface AssistantResponse {
  id: number;
  request: string;
  createdAt: string;
  category: Category;
  status: InteractionStatus;
  originLabel: string;
  summary: string;
  body: ResponseBody;
}

export type Stage =
  | "idle"
  | "listening"
  | "transcribing"
  | "interpreting"
  | "executing"
  | "capturing"
  | "analyzing"
  | "responding";

export interface StageEvent {
  stage: Stage;
}

export interface Reminder {
  id: number;
  content: string;
  dueAt: string;
  status: "pending" | "notified";
}

export interface HistoryEntry {
  id: number;
  request: string;
  createdAt: string;
  category: Category;
  status: InteractionStatus;
  response: string;
}

export interface ProviderInfo {
  id: string;
  displayName: string;
  dataNotice: string;
  keyUrl: string;
}

export type KeySource = "secureStore" | "session" | "devEnv" | "none";

export interface SpeechModelStatus {
  id: string;
  displayName: string;
  sizeMb: number;
  path: string;
  downloaded: boolean;
  isActive: boolean;
}

export interface Settings {
  aiProvider: string;
  interpretModel: string;
  visionModel: string;
  dailyAiLimit: number;
  speechModel: string;
  speechIdleMinutes: number;
  weatherCity: string;
  authorizedFolders: string[];
  foldersConfirmed: boolean;
}

export interface SettingsView {
  settings: Settings;
  provider: ProviderInfo;
  keySource: KeySource;
  secureStoreAvailable: boolean;
  aiRequestsToday: number;
  tokensToday: number;
  knownFolders: string[];
  speechModels: SpeechModelStatus[];
}

export interface DownloadProgress {
  modelId: string;
  downloadedMb: number;
  totalMb: number;
}
