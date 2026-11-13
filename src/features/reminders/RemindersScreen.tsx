import { useCallback, useEffect, useState } from "react";

import { Button } from "@/components/Button";
import { api, errorMessage, events } from "@/services/api";
import type { Reminder } from "@/types/api";
import { toLocalInput } from "@/utils/dates";

function ReminderRow({ reminder, onChanged, onError }: { reminder: Reminder; onChanged: () => void; onError: (message: string) => void }) {
  const [editing, setEditing] = useState(false);
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
        <label className="flex min-w-48 flex-1 flex-col gap-1 text-sm text-muted">
          Recordatorio
          <input value={content} maxLength={300} onChange={(event) => setContent(event.target.value)} className="h-11 rounded-sm border border-line bg-panel px-3 text-md text-ink" />
        </label>
        <label className="flex flex-col gap-1 text-sm text-muted">
          Fecha y hora
          <input type="datetime-local" value={due} onChange={(event) => setDue(event.target.value)} className="h-11 rounded-sm border border-line bg-panel px-3 font-mono text-md text-ink" />
        </label>
        <Button variant="primary" disabled={!content.trim() || !due} onClick={() => void save()}>
          Guardar
        </Button>
        <Button onClick={() => setEditing(false)}>Cancelar</Button>
      </li>
    );
  }

  const overdue = reminder.status === "notified" || new Date(reminder.dueAt) < new Date();
  return (
    <li className="flex flex-wrap items-center gap-x-4 gap-y-2 border-t border-hairline py-3">
      <div className="flex min-w-48 flex-1 flex-col gap-0.5">
        <span>{reminder.content}</span>
        <span className="font-mono text-sm text-muted">
          {new Date(reminder.dueAt).toLocaleString("es", { dateStyle: "full", timeStyle: "short" })}
        </span>
        {overdue && <span className="text-sm text-muted">Vencido</span>}
      </div>
      <Button onClick={() => setEditing(true)}>Editar</Button>
      <Button variant="danger" onClick={() => void remove()}>
        Eliminar
      </Button>
    </li>
  );
}

export function RemindersScreen() {
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
      <h1 className="m-0 text-xl font-medium leading-tight">Recordatorios</h1>
      <p className="m-0 text-sm text-muted">
        Los avisos solo se emiten mientras Nexium está abierto o en la bandeja del sistema. Si lo cierras con Salir, verás los vencidos al volver a abrirlo.
      </p>
      {error && <p className="m-0 text-error">{error}</p>}
      {reminders?.length === 0 && <p className="m-0 text-muted">No tienes recordatorios pendientes.</p>}
      {reminders && reminders.length > 0 && (
        <ul className="m-0 flex list-none flex-col border-b border-hairline p-0">
          {reminders.map((reminder) => (
            <ReminderRow key={`${reminder.id}-${reminder.dueAt}`} reminder={reminder} onChanged={load} onError={setError} />
          ))}
        </ul>
      )}
    </div>
  );
}
