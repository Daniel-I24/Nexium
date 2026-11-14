import { useCallback, useEffect, useState } from "react";

import { Button } from "@/components/Button";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { DueFields } from "@/components/DueFields";
import { Icon } from "@/components/Icon";
import { Notice } from "@/components/Notice";
import { StatusLabel } from "@/components/StatusLabel";
import { api, errorMessage, events } from "@/services/api";
import type { Reminder } from "@/types/api";
import { isCompleteLocal, toLocalInput } from "@/utils/dates";

const EXAMPLE = "Recuérdame entregar el proyecto el viernes a las nueve";
const inputStyle = "h-11 w-full min-w-0 rounded-sm border border-line bg-panel px-3 text-md text-ink";

function longDate(iso: string) {
  return new Date(iso).toLocaleString("es", { dateStyle: "full", timeStyle: "short" });
}

/** Vencido: ya avisado o con la fecha pasada. Se marca con icono, palabra y una línea lateral, no solo con color. */
export function isOverdue(reminder: Reminder, now: Date = new Date()) {
  return reminder.status === "notified" || new Date(reminder.dueAt) < now;
}

interface RowProps {
  reminder: Reminder;
  onChanged: () => void;
  onError: (message: string) => void;
}

function ReminderRow({ reminder, onChanged, onError }: RowProps) {
  const [editing, setEditing] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [content, setContent] = useState(reminder.content);
  const [due, setDue] = useState(toLocalInput(reminder.dueAt));

  const save = async () => {
    try {
      await api.updateReminder(reminder.id, content.trim(), new Date(due).toISOString());
      setEditing(false);
      onChanged();
    } catch (error) {
      onError(errorMessage(error));
    }
  };

  const remove = async () => {
    setConfirmDelete(false);
    try {
      await api.deleteReminder(reminder.id);
      onChanged();
    } catch (error) {
      onError(errorMessage(error));
    }
  };

  if (editing) {
    return (
      <li className="flex flex-wrap items-end gap-3 border-t border-hairline py-3">
        <label className="flex min-w-0 flex-1 basis-60 flex-col gap-1 text-sm text-muted">
          Recordatorio
          <input value={content} maxLength={300} onChange={(event) => setContent(event.target.value)} className={inputStyle} />
        </label>
        <div className="min-w-0 flex-1 basis-72">
          <DueFields value={due} onChange={setDue} />
        </div>
        <div className="flex flex-wrap gap-3">
          <Button variant="primary" disabled={!content.trim() || !isCompleteLocal(due)} onClick={() => void save()}>
            Guardar
          </Button>
          <Button onClick={() => setEditing(false)}>Cancelar</Button>
        </div>
      </li>
    );
  }

  const overdue = isOverdue(reminder);
  return (
    <li className={`flex flex-wrap items-center gap-x-4 gap-y-2 border-t border-hairline py-3 ${overdue ? "border-l-2 border-l-error pl-3" : ""}`}>
      <div className="flex min-w-0 flex-1 basis-60 flex-col gap-1">
        <span className="break-words">{reminder.content}</span>
        <span className="font-mono text-sm text-muted">{longDate(reminder.dueAt)}</span>
        <span className="text-sm">{overdue ? <StatusLabel status="overdue">Vencido</StatusLabel> : <StatusLabel status="pending">Pendiente</StatusLabel>}</span>
      </div>
      <div className="flex flex-wrap gap-3">
        <Button aria-label={`Editar: ${reminder.content}`} onClick={() => setEditing(true)}>
          Editar
        </Button>
        <Button variant="danger" aria-label={`Eliminar: ${reminder.content}`} onClick={() => setConfirmDelete(true)}>
          Eliminar
        </Button>
      </div>
      <ConfirmDialog
        open={confirmDelete}
        title="¿Eliminar este recordatorio?"
        description={`«${reminder.content}», ${longDate(reminder.dueAt)}. Nexium ya no te avisará. No se puede deshacer.`}
        confirmLabel="Eliminar"
        onCancel={() => setConfirmDelete(false)}
        onConfirm={() => void remove()}
      />
    </li>
  );
}

/** Sin recordatorios: qué hace esta pantalla y cómo se crea uno, con un ejemplo que se puede pulsar. */
function EmptyReminders({ onPickExample }: { onPickExample: (text: string) => void }) {
  return (
    <div className="flex max-w-reading animate-enter flex-col gap-3 border-y border-hairline py-6">
      <p className="m-0 flex items-center gap-2 text-lg font-medium">
        <Icon name="reminder" size="md" />
        No tienes recordatorios
      </p>
      <p className="m-0 text-muted">Pídeselo a Nexium con tu voz o por escrito, con lo que tienes que hacer y cuándo. Por ejemplo:</p>
      <div>
        <Button onClick={() => onPickExample(EXAMPLE)}>{EXAMPLE}</Button>
      </div>
    </div>
  );
}

export function RemindersScreen({ onPickExample }: { onPickExample: (text: string) => void }) {
  const [reminders, setReminders] = useState<Reminder[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    api
      .listReminders()
      .then((items) => {
        setReminders(items);
        setError(null);
      })
      .catch((reason: unknown) => setError(errorMessage(reason)));
  }, []);

  useEffect(() => {
    load();
    const subscription = events.onRemindersChanged(load);
    return () => void subscription.then((unlisten) => unlisten());
  }, [load]);

  return (
    <div className="flex flex-col gap-4">
      <h1 className="m-0 text-xl font-medium">Recordatorios</h1>
      <p className="m-0 max-w-reading text-sm text-muted">
        Los avisos solo se emiten mientras Nexium está abierto o en la bandeja del sistema. Si lo cierras con Salir, verás los vencidos al volver a abrirlo.
      </p>
      {error && <Notice tone="error">{error}</Notice>}
      {reminders === null && !error && (
        <p role="status" className="m-0 text-muted">
          Cargando recordatorios…
        </p>
      )}
      {reminders?.length === 0 && <EmptyReminders onPickExample={onPickExample} />}
      {reminders && reminders.length > 0 && (
        <ul aria-label="Recordatorios" className="m-0 flex list-none flex-col border-b border-hairline p-0">
          {reminders.map((reminder) => (
            <ReminderRow key={`${reminder.id}-${reminder.dueAt}`} reminder={reminder} onChanged={load} onError={setError} />
          ))}
        </ul>
      )}
    </div>
  );
}
