import { useEffect, useRef, useState } from "react";

import { Button } from "@/components/Button";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { DueFields } from "@/components/DueFields";
import { StatusLabel } from "@/components/StatusLabel";
import { api, errorMessage } from "@/services/api";
import type { Reminder } from "@/types/api";
import { isCompleteLocal, toLocalInput } from "@/utils/dates";
import { exitThen, stagger } from "@/utils/motion";

export function longDate(iso: string) {
  return new Date(iso).toLocaleString("es", { dateStyle: "full", timeStyle: "short" });
}

/** Vencido: ya avisado o con la fecha pasada. Se marca con icono, palabra y una línea lateral, no solo con color. */
export function isOverdue(reminder: Reminder, now: Date = new Date()) {
  return reminder.status === "notified" || new Date(reminder.dueAt) < now;
}

const inputStyle = "h-11 w-full min-w-0 rounded-sm border border-line bg-panel px-3 text-md text-ink";

interface RowProps {
  reminder: Reminder;
  index: number;
  /** Recién abierto desde su aviso: se resalta con un destello breve. */
  highlighted: boolean;
  onChanged: () => void;
  onError: (message: string) => void;
}

export function ReminderRow({ reminder, index, highlighted, onChanged, onError }: RowProps) {
  const [editing, setEditing] = useState(false);
  const item = useRef<HTMLLIElement>(null);

  // Abierto desde su aviso: la fila se lleva a la vista y recibe el foco, que también se ve sin animaciones.
  useEffect(() => {
    if (!highlighted) return;
    item.current?.scrollIntoView({ block: "center" });
    item.current?.focus({ preventScroll: true });
  }, [highlighted]);
  const [leaving, setLeaving] = useState(false);
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

  const remove = () => {
    setConfirmDelete(false);
    // La fila se aparta antes de borrarse; si el borrado falla, vuelve.
    exitThen(
      () => setLeaving(true),
      () =>
        void api
          .deleteReminder(reminder.id)
          .then(onChanged)
          .catch((error: unknown) => {
            setLeaving(false);
            onError(errorMessage(error));
          }),
    );
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
    <li
      ref={item}
      tabIndex={highlighted ? -1 : undefined}
      aria-current={highlighted ? "true" : undefined}
      style={leaving ? undefined : stagger(index)}
      className={`relative isolate flex flex-wrap items-center gap-x-4 gap-y-2 border-t border-hairline py-3 ${overdue ? "border-l-2 border-l-error pl-3" : ""} ${leaving ? "animate-leave" : "animate-enter"}`}
    >
      {highlighted && <span aria-hidden="true" data-highlight className="absolute inset-0 -z-10 animate-highlight bg-signal opacity-0" />}
      <div className="flex min-w-0 flex-1 basis-60 flex-col gap-1">
        <span className="wrap-anywhere">{reminder.content}</span>
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
        onConfirm={remove}
      />
    </li>
  );
}
