import { useEffect, useState, type ReactNode } from "react";

import { Button } from "@/components/Button";
import { Icon } from "@/components/Icon";
import { Notice } from "@/components/Notice";
import { Loading } from "@/components/Progress";
import { ScreenLayout } from "@/components/ScreenLayout";
import { StatusLabel } from "@/components/StatusLabel";
import { api, errorMessage, events } from "@/services/api";
import type { AiServiceStatus, DownloadProgress, InputDevice, Settings, SettingsView } from "@/types/api";
import { count } from "@/utils/plural";

const statusLabels: Record<AiServiceStatus, string> = {
  notConfigured: "No incluido en esta versión",
  notUsedYet: "Incluido. Sin usar todavía en esta sesión",
  working: "Funcionando",
  limited: "Límite de uso alcanzado por ahora",
  quotaExhausted: "Cuota diaria agotada. Vuelve a intentarlo mañana",
  regionUnavailable: "No disponible en tu región o sin país identificado",
  unreachable: "Sin conexión con el servicio",
};

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex max-w-reading flex-col gap-3 border-t border-line pt-4">
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

const inputStyle = "h-11 w-full min-w-0 rounded-sm border border-line bg-panel px-3 text-md text-ink";

function deviceLabel(device: InputDevice) {
  const notes = [device.isDefault && "predeterminado de Windows", !device.isMicrophone && "graba el sonido del equipo, no tu voz"].filter(Boolean);
  return notes.length > 0 ? `${device.name} (${notes.join(", ")})` : device.name;
}

/** Qué micrófono usará Nexium, y si el elegido no está conectado. */
function MicrophoneNote({ chosen, active }: { chosen: string | null; active: string | null }) {
  if (!active) return <p className="m-0 text-sm text-error">No se encontró ningún micrófono. Conecta uno y vuelve a abrir Ajustes.</p>;
  if (chosen && chosen !== active) {
    return (
      <p className="m-0 text-sm text-error">
        El micrófono elegido, {chosen}, no está conectado. Nexium usará {active}.
      </p>
    );
  }
  return <p className="m-0 text-sm text-muted">Nexium usará {active}.</p>;
}

export function SettingsScreen() {
  const [view, setView] = useState<SettingsView | null>(null);
  const [draft, setDraft] = useState<Settings | null>(null);
  const [message, setMessage] = useState<{ text: string; error: boolean; at: number } | null>(null);
  const [download, setDownload] = useState<DownloadProgress | null>(null);
  /** Lo guardado coincide con lo que se ve: el botón lo confirma hasta el siguiente cambio. */
  const [saved, setSaved] = useState(false);

  const apply = (next: SettingsView) => {
    setView(next);
    setDraft(next.settings);
  };

  useEffect(() => {
    api.getSettings().then(apply).catch((error: unknown) => setMessage({ text: errorMessage(error), error: true, at: Date.now() }));
    const subscription = events.onDownload(setDownload);
    return () => void subscription.then((unlisten) => unlisten());
  }, []);

  const run = async (action: () => Promise<SettingsView>, done: string) => {
    try {
      apply(await action());
      setSaved(true);
      setMessage({ text: done, error: false, at: Date.now() });
    } catch (error) {
      setMessage({ text: errorMessage(error), error: true, at: Date.now() });
    }
  };

  if (!view || !draft) {
    return (
      <ScreenLayout>
        <div className="flex flex-col gap-8">
          <h1 className="m-0 text-xl font-medium">Ajustes</h1>
          {message ? (
            <Notice tone="error">{message.text}</Notice>
          ) : (
            <Loading>Cargando ajustes…</Loading>
          )}
        </div>
      </ScreenLayout>
    );
  }

  const update = <K extends keyof Settings>(key: K, value: Settings[K]) => {
    setSaved(false);
    setDraft({ ...draft, [key]: value });
  };

  /** Las carpetas se guardan al momento: no dependen de Guardar ajustes. */
  const changeFolders = async (action: () => Promise<string[] | null>, done: string) => {
    try {
      const folders = await action();
      if (!folders) return;
      setView({ ...view, settings: { ...view.settings, authorizedFolders: folders } });
      setDraft({ ...draft, authorizedFolders: folders });
      setMessage({ text: done, error: false, at: Date.now() });
    } catch (error) {
      setMessage({ text: errorMessage(error), error: true, at: Date.now() });
    }
  };
  const missingKnown = view.knownFolders.some((folder) => !draft.authorizedFolders.includes(folder));

  const startDownload = async (modelId: string) => {
    try {
      await api.downloadSpeechModel(modelId);
      setDownload(null);
      apply(await api.getSettings());
      setMessage({ text: "Modelo de voz descargado.", error: false, at: Date.now() });
    } catch (error) {
      setDownload(null);
      setMessage({ text: errorMessage(error), error: true, at: Date.now() });
    }
  };

  // Guardar va en el pie, fuera de la zona que se desplaza, y confirma en el propio botón: así el pie cabe en
  // una fila incluso en la ventana mínima con el texto ampliado. Los errores sí se muestran a su lado.
  const footer = (
    <>
      <Button variant="primary" data-save onClick={() => void run(() => api.saveSettings(draft), "Ajustes guardados.")}>
        {saved ? (
          <span key="saved" className="inline-flex animate-bar items-center gap-2">
            <Icon name="completed" />
            Ajustes guardados
          </span>
        ) : (
          "Guardar ajustes"
        )}
      </Button>
      {message?.error && (
        <div key={message.at} className="min-w-0 flex-1 basis-48">
          <Notice tone="error">{message.text}</Notice>
        </div>
      )}
      {message && !message.error && (
        <span key={message.at} role="status" className="sr-only">
          {message.text}
        </span>
      )}
    </>
  );

  return (
    <ScreenLayout footer={footer}>
      <div className="flex flex-col gap-8">
        <h1 className="m-0 text-xl font-medium">Ajustes</h1>

        <Section title={`Inteligencia artificial: ${view.provider.displayName}`}>
          <p className="m-0 border-t-2 border-query pt-2 text-sm">{view.provider.dataNotice}</p>
          <p className="m-0 text-sm">Estado del servicio: {statusLabels[view.aiStatus]}.</p>
          <Field label="Límite diario de solicitudes de IA">
            <input type="number" min={1} max={10000} value={draft.dailyAiLimit} onChange={(event) => update("dailyAiLimit", Number(event.target.value))} className={`font-mono ${inputStyle}`} />
          </Field>
          <p className="m-0 font-mono text-sm text-muted">
            Hoy: {count(view.aiRequestsToday, "solicitud", "solicitudes")} · {count(view.tokensToday, "token", "tokens")}
          </p>
        </Section>

        <Section title="Carpetas autorizadas">
        <p className="m-0 text-sm text-muted">
          Nexium busca y lee archivos solo en estas carpetas: por nombre y, en los de texto, también por contenido. Nunca modifica, mueve ni elimina nada. Un documento
          solo se envía a la IA si lo confirmas.
        </p>
        {draft.authorizedFolders.length === 0 ? (
          <p className="m-0 text-sm">Todavía no hay ninguna carpeta autorizada.</p>
        ) : (
          <ul aria-label="Carpetas autorizadas" className="m-0 flex list-none flex-col p-0">
            {draft.authorizedFolders.map((folder) => (
              <li key={folder} className="flex flex-wrap items-center gap-x-4 gap-y-2 border-t border-hairline py-2">
                <span className="min-w-0 flex-1 basis-48 break-all font-mono text-sm">{folder}</span>
                <Button aria-label={`Quitar la carpeta ${folder}`} onClick={() => void changeFolders(() => api.removeFolder(folder), "Carpeta quitada.")}>
                  Quitar
                </Button>
              </li>
            ))}
          </ul>
        )}
        <div className="flex flex-wrap gap-2">
          <Button onClick={() => void changeFolders(api.pickFolder, "Carpeta añadida.")}>
            <Icon name="folder" />
            Añadir carpeta
          </Button>
          {missingKnown && <Button onClick={() => void changeFolders(api.allowKnownFolders, "Documentos y Escritorio añadidos.")}>Permitir Documentos y Escritorio</Button>}
        </div>
      </Section>

      <Section title="Voz">
          <p className="m-0 text-sm text-muted">La voz se transcribe en este equipo. El audio no sale de él ni se guarda.</p>
          <ul className="m-0 flex list-none flex-col p-0">
            {view.speechModels.map((model) => (
              <li key={model.id} className="flex flex-wrap items-center gap-x-4 gap-y-2 border-t border-hairline py-3">
                <label className="flex min-h-11 min-w-0 flex-1 basis-48 items-center gap-3">
                  <input type="radio" name="speech-model" className="size-4 shrink-0 accent-ink" disabled={!model.downloaded} checked={draft.speechModel === model.id} onChange={() => update("speechModel", model.id)} />
                  <span>
                    {model.displayName} <span className="font-mono text-sm text-muted">{model.sizeMb} MB</span>
                  </span>
                </label>
                {model.downloaded ? (
                  <span className="text-sm">
                    <StatusLabel status="completed">Disponible</StatusLabel>
                  </span>
                ) : (
                  <Button disabled={download !== null} onClick={() => void startDownload(model.id)}>
                    {download?.modelId === model.id ? `${download.downloadedMb} de ${download.totalMb} MB` : model.damaged ? "Volver a descargar" : "Descargar"}
                  </Button>
                )}
              </li>
            ))}
          </ul>
          <Field label="Micrófono">
            <select value={draft.microphone ?? ""} onChange={(event) => update("microphone", event.target.value || null)} className={inputStyle}>
              <option value="">Automático: el primer micrófono disponible</option>
              {view.microphones.map((device) => (
                <option key={device.name} value={device.name}>
                  {deviceLabel(device)}
                </option>
              ))}
            </select>
          </Field>
          <MicrophoneNote chosen={view.settings.microphone} active={view.activeMicrophone} />
          <Field label="Liberar el modelo tras estos minutos sin uso">
            <input type="number" min={1} max={240} value={draft.speechIdleMinutes} onChange={(event) => update("speechIdleMinutes", Number(event.target.value))} className={`font-mono ${inputStyle}`} />
          </Field>
          <p className="m-0 text-sm text-muted">El modelo se carga la primera vez que hablas y se mantiene en memoria para responder más rápido.</p>
        </Section>

        <Section title="Clima">
          <Field label="Ciudad para el clima">
            <input value={draft.weatherCity} maxLength={100} onChange={(event) => update("weatherCity", event.target.value)} className={inputStyle} />
          </Field>
          <p className="m-0 text-sm text-muted">Se usa cuando no nombras una ciudad. Al guardar, Nexium la comprueba con Open-Meteo y guarda su nombre oficial.</p>
          <p className="m-0 text-sm text-muted">
            Datos meteorológicos de{" "}
            <button type="button" onClick={() => void api.openAttribution()} className="bg-transparent text-muted underline">
              Open-Meteo
            </button>{" "}
            (CC BY 4.0). Su uso gratuito es solo para fines no comerciales.
          </p>
        </Section>
      </div>
    </ScreenLayout>
  );
}
