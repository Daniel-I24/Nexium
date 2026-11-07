import { useEffect, useState, type FormEvent, type ReactNode } from "react";

import { Button } from "@/components/Button";
import { api, errorMessage, events } from "@/services/api";
import type { DownloadProgress, KeySource, Settings, SettingsView } from "@/types/api";

const keyLabels: Record<KeySource, string> = {
  secureStore: "Guardada en el almacén seguro del sistema",
  session: "Solo durante esta sesión: este sistema no ofrece almacén seguro",
  devEnv: "Leída de .env (solo en desarrollo)",
  none: "Sin configurar",
};

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-3 border-t border-line pt-4">
      <h2 className="m-0 text-lg font-medium">{title}</h2>
      {children}
    </section>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="flex flex-col gap-1 text-sm text-muted">
      {label}
      {children}
    </label>
  );
}

const inputStyle = "h-11 rounded-sm border border-line bg-panel px-3 text-md text-ink";

export function SettingsScreen() {
  const [view, setView] = useState<SettingsView | null>(null);
  const [draft, setDraft] = useState<Settings | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);
  const [download, setDownload] = useState<DownloadProgress | null>(null);

  const apply = (next: SettingsView) => {
    setView(next);
    setDraft(next.settings);
  };

  useEffect(() => {
    api.getSettings().then(apply).catch((error: unknown) => setMessage({ text: errorMessage(error), error: true }));
    const subscription = events.onDownload(setDownload);
    return () => void subscription.then((unlisten) => unlisten());
  }, []);

  const run = async (action: () => Promise<SettingsView>, done: string) => {
    try {
      apply(await action());
      setMessage({ text: done, error: false });
    } catch (error) {
      setMessage({ text: errorMessage(error), error: true });
    }
  };

  if (!view || !draft) {
    return message ? <p className="m-0 text-error">{message.text}</p> : <p className="m-0 text-muted">Cargando ajustes</p>;
  }

  const update = <K extends keyof Settings>(key: K, value: Settings[K]) => setDraft({ ...draft, [key]: value });

  const toggleFolder = (folder: string, enabled: boolean) =>
    update("authorizedFolders", enabled ? [...draft.authorizedFolders, folder] : draft.authorizedFolders.filter((item) => item !== folder));

  const saveKey = (event: FormEvent) => {
    event.preventDefault();
    const key = apiKey.trim();
    setApiKey("");
    if (key) void run(() => api.saveApiKey(key), "Clave guardada.");
  };

  const startDownload = async (modelId: string) => {
    try {
      await api.downloadSpeechModel(modelId);
      setDownload(null);
      apply(await api.getSettings());
      setMessage({ text: "Modelo de voz descargado.", error: false });
    } catch (error) {
      setDownload(null);
      setMessage({ text: errorMessage(error), error: true });
    }
  };

  return (
    <div className="flex max-w-reading flex-col gap-8 pb-8">
      <h1 className="m-0 text-xl font-medium leading-tight">Ajustes</h1>
      {message && <p role="status" className={`m-0 ${message.error ? "text-error" : "text-ok"}`}>{message.text}</p>}

      <Section title={`Inteligencia artificial: ${view.provider.displayName}`}>
        <p className="m-0 border-t-2 border-query pt-2 text-sm">{view.provider.dataNotice}</p>
        <p className="m-0 text-sm text-muted">
          Clave: {keyLabels[view.keySource]}. Se crea en <span className="font-mono">{view.provider.keyUrl}</span>
        </p>
        <form onSubmit={saveKey} className="flex flex-wrap items-end gap-3">
          <div className="min-w-48 flex-1">
            <Field label="Clave de API">
              <input type="password" autoComplete="off" value={apiKey} onChange={(event) => setApiKey(event.target.value)} className={`w-full ${inputStyle}`} />
            </Field>
          </div>
          <Button type="submit" variant="primary" disabled={!apiKey.trim()}>
            Guardar clave
          </Button>
          {view.keySource === "secureStore" && (
            <Button variant="danger" onClick={() => void run(api.deleteApiKey, "Clave eliminada.")}>
              Eliminar clave
            </Button>
          )}
        </form>
        <div className="flex flex-col gap-3">
          <Field label="Modelo para interpretar solicitudes">
            <input value={draft.interpretModel} onChange={(event) => update("interpretModel", event.target.value)} className={`font-mono ${inputStyle}`} />
          </Field>
          <Field label="Modelo para analizar la pantalla">
            <input value={draft.visionModel} onChange={(event) => update("visionModel", event.target.value)} className={`font-mono ${inputStyle}`} />
          </Field>
          <Field label="Límite diario de solicitudes de IA">
            <input type="number" min={1} max={10000} value={draft.dailyAiLimit} onChange={(event) => update("dailyAiLimit", Number(event.target.value))} className={`font-mono ${inputStyle}`} />
          </Field>
        </div>
        <p className="m-0 font-mono text-sm text-muted">
          Hoy: {view.aiRequestsToday} solicitudes · {view.tokensToday} tokens
        </p>
      </Section>

      <Section title="Carpetas autorizadas para buscar">
        <p className="m-0 text-sm text-muted">Nexium solo mira nombres de archivo en estas carpetas. No abre, modifica, mueve ni elimina nada.</p>
        {view.knownFolders.map((folder) => (
          <label key={folder} className="flex min-h-11 items-center gap-3">
            <input type="checkbox" checked={draft.authorizedFolders.includes(folder)} onChange={(event) => toggleFolder(folder, event.target.checked)} />
            <span className="break-all font-mono text-sm">{folder}</span>
          </label>
        ))}
      </Section>

      <Section title="Voz">
        <p className="m-0 text-sm text-muted">La voz se transcribe en este equipo. El audio no sale de él ni se guarda.</p>
        <ul className="m-0 flex list-none flex-col p-0">
          {view.speechModels.map((model) => (
            <li key={model.id} className="flex flex-wrap items-center gap-x-4 gap-y-2 border-t border-hairline py-3">
              <label className="flex min-w-48 flex-1 items-center gap-3">
                <input type="radio" name="speech-model" disabled={!model.downloaded} checked={draft.speechModel === model.id} onChange={() => update("speechModel", model.id)} />
                <span>
                  {model.displayName} <span className="font-mono text-sm text-muted">{model.sizeMb} MB</span>
                </span>
              </label>
              {!model.downloaded && (
                <Button disabled={download !== null} onClick={() => void startDownload(model.id)}>
                  {download?.modelId === model.id ? `${download.downloadedMb} de ${download.totalMb} MB` : "Descargar"}
                </Button>
              )}
            </li>
          ))}
        </ul>
        <Field label="Liberar el modelo tras estos minutos sin uso">
          <input type="number" min={1} max={240} value={draft.speechIdleMinutes} onChange={(event) => update("speechIdleMinutes", Number(event.target.value))} className={`font-mono ${inputStyle}`} />
        </Field>
        <p className="m-0 text-sm text-muted">El modelo se carga la primera vez que hablas y se mantiene en memoria para responder más rápido.</p>
      </Section>

      <Section title="Clima">
        <Field label="Ciudad predeterminada">
          <input value={draft.weatherCity} maxLength={100} onChange={(event) => update("weatherCity", event.target.value)} className={inputStyle} />
        </Field>
        <p className="m-0 text-sm text-muted">
          Datos meteorológicos de{" "}
          <button type="button" onClick={() => void api.openAttribution()} className="bg-transparent text-muted underline">
            Open-Meteo
          </button>{" "}
          (CC BY 4.0). Su uso gratuito es solo para fines no comerciales.
        </p>
      </Section>

      <div>
        <Button variant="primary" onClick={() => void run(() => api.saveSettings(draft), "Ajustes guardados.")}>
          Guardar ajustes
        </Button>
      </div>
    </div>
  );
}
