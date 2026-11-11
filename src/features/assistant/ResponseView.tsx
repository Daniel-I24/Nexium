import { useState } from "react";

import { Button } from "@/components/Button";
import { DataRows } from "@/components/DataRows";
import { Scale } from "@/components/Scale";
import { api } from "@/services/api";
import type { Attribution, AssistantResponse, ReminderOption, ResponseBody } from "@/types/api";

import { toLocalInput } from "@/utils/dates";

import { FileList } from "./FileList";

export interface ResponseActions {
  onSaveReminder: (content: string, dueAt: string) => void;
  onAuthorizeFolders: (folders: string[], pendingRequest: string) => void;
}

function AttributionLine({ attribution }: { attribution: Attribution | null }) {
  if (!attribution) return null;
  return (
    <p className="m-0 text-sm text-muted">
      <button type="button" onClick={() => void api.openAttribution()} className="bg-transparent text-left text-muted underline">
        {attribution.text}
      </button>
    </p>
  );
}

function ReadingValue({ value }: { value: string }) {
  const [whole, decimals] = value.split(",");
  if (decimals === undefined) return value;
  return (
    <>
      {whole}
      <span className="-mx-reading-comma">,</span>
      {decimals}
    </>
  );
}

function ReminderDraft({ body, onSave }: { body: { content: string; dueAt: string | null; options: ReminderOption[] }; onSave: ResponseActions["onSaveReminder"] }) {
  const [content, setContent] = useState(body.content);
  const [due, setDue] = useState(toLocalInput(body.dueAt));
  const valid = content.trim().length > 0 && due.length > 0;
  return (
    <form
      className="flex max-w-reading flex-col gap-3"
      onSubmit={(event) => {
        event.preventDefault();
        if (valid) onSave(content.trim(), new Date(due).toISOString());
      }}
    >
      <label className="flex flex-col gap-1 text-sm text-muted">
        Recordatorio
        <input
          value={content}
          maxLength={300}
          onChange={(event) => setContent(event.target.value)}
          className="h-11 rounded-sm border border-line bg-panel px-3 text-md text-ink"
        />
      </label>
      <label className="flex flex-col gap-1 text-sm text-muted">
        Fecha y hora
        <input
          type="datetime-local"
          value={due}
          onChange={(event) => setDue(event.target.value)}
          className="h-11 rounded-sm border border-line bg-panel px-3 font-mono text-md text-ink"
        />
      </label>
      <div>
        <Button type="submit" variant="primary" disabled={!valid}>
          Guardar recordatorio
        </Button>
      </div>
    </form>
  );
}

function ScreenAnalysisView({ summary, cause, steps }: { summary: string; cause: string | null; steps: string[] }) {
  return (
    <div className="flex max-w-reading flex-col gap-5">
      <p className="m-0 text-lg font-semibold leading-snug">{summary}</p>
      {cause && (
        <section className="flex flex-col gap-1">
          <h2 className="m-0 text-sm font-medium text-muted">Causa probable</h2>
          <p className="m-0 text-md leading-normal">{cause}</p>
        </section>
      )}
      {steps.length > 0 && (
        <section className="flex flex-col gap-1">
          <h2 className="m-0 text-sm font-medium text-muted">Qué hacer</h2>
          <ol className="m-0 list-none p-0">
            {steps.map((step, index) => (
              <li key={index} className="flex gap-3 border-t border-hairline py-2 text-md">
                <span aria-hidden="true" className="font-mono text-muted">
                  {index + 1}
                </span>
                <span>{step}</span>
              </li>
            ))}
          </ol>
        </section>
      )}
    </div>
  );
}

function Body({ body, actions }: { body: ResponseBody; actions: ResponseActions }) {
  switch (body.kind) {
    case "reading":
      return (
        <>
          <div className="flex flex-wrap items-baseline gap-x-5 gap-y-2">
            <span className="font-mono text-reading leading-none tracking-tightest">
              <ReadingValue value={body.value} />
              {body.unit && <span className="ml-2 text-unit tracking-normal">{body.unit}</span>}
            </span>
            <span className="text-lg text-muted">{body.context}</span>
          </div>
          {body.percent !== null && <Scale percent={body.percent} label={`${body.value} ${body.unit} ${body.context}`} />}
          {body.rows.length > 0 && <DataRows rows={body.rows} />}
          <AttributionLine attribution={body.attribution} />
        </>
      );
    case "rows":
      return (
        <>
          <DataRows rows={body.rows} />
          {body.note && <p className="m-0 max-w-reading text-sm text-muted">{body.note}</p>}
          <AttributionLine attribution={body.attribution} />
        </>
      );
    case "text":
      return <p className="m-0 max-w-reading whitespace-pre-wrap text-lg leading-normal">{body.text}</p>;
    case "screenAnalysis":
      return <ScreenAnalysisView summary={body.summary} cause={body.cause} steps={body.steps} />;
    case "files":
      return (
        <>
          <p className="m-0 text-sm text-muted">{body.summary}</p>
          <FileList files={body.files} />
          {body.note && <p className="m-0 max-w-reading text-sm text-muted">{body.note}</p>}
        </>
      );
    case "reminderDraft":
      return <ReminderDraft body={body} onSave={actions.onSaveReminder} />;
    case "reminderSaved":
      return <p className="m-0 text-lg">Guardado: {body.reminder.content}</p>;
    case "clarification":
      return <p className="m-0 max-w-reading text-lg text-query">{body.question}</p>;
    case "folderConsent":
      return (
        <div className="flex max-w-reading flex-col gap-3">
          <p className="m-0 text-lg">Para buscar archivos necesito tu permiso. Solo miraré nombres de archivo, sin abrirlos ni modificarlos, en estas carpetas:</p>
          <ul className="m-0 list-none p-0 font-mono text-sm">
            {body.folders.map((folder) => (
              <li key={folder} className="border-t border-hairline py-2">
                {folder}
              </li>
            ))}
          </ul>
          <div>
            <Button variant="primary" onClick={() => actions.onAuthorizeFolders(body.folders, body.pendingRequest)}>
              Autorizar y buscar
            </Button>
          </div>
        </div>
      );
    case "captureRequest":
      return <p className="m-0 text-lg">Necesito tu permiso para tomar una captura.</p>;
    case "error":
      return (
        <div className="flex max-w-reading flex-col gap-1 border-t-2 border-error pt-3">
          <p className="m-0 text-lg text-error">{body.message}</p>
          {body.hint && <p className="m-0 text-muted">{body.hint}</p>}
        </div>
      );
  }
}

export function ResponseView({ response, actions }: { response: AssistantResponse; actions: ResponseActions }) {
  const time = new Date(response.createdAt).toLocaleTimeString("es", { hour: "2-digit", minute: "2-digit" });
  return (
    <article className="flex flex-col gap-6">
      <div className="flex flex-wrap items-baseline justify-between gap-x-6 gap-y-2">
        <h1 className="m-0 text-lg font-medium">{response.request}</h1>
        <span className="font-mono text-sm text-muted">{time}</span>
      </div>
      <Body body={response.body} actions={actions} />
      {response.body.kind !== "error" && <p className="m-0 text-sm text-muted">Origen: {response.originLabel}</p>}
    </article>
  );
}
