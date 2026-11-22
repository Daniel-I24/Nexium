import { useCallback, useEffect, useState } from "react";

import { Button } from "@/components/Button";
import { Icon } from "@/components/Icon";
import { Notice } from "@/components/Notice";
import { Loading } from "@/components/Progress";
import { ScreenLayout } from "@/components/ScreenLayout";
import { api, errorMessage, events } from "@/services/api";
import type { Reminder } from "@/types/api";

import { ReminderRow } from "./ReminderRow";


/** Las indicaciones de las notificaciones dependen del sistema. */
function isWindows() {
  return /Windows/i.test(navigator.userAgent);
}

const EXAMPLE = "Recuérdame entregar el proyecto el viernes a las nueve";
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

interface RemindersScreenProps {
  onPickExample: (text: string) => void;
  /** El recordatorio que se abrió desde su aviso, para resaltarlo. */
  highlight?: number | null;
}

export function RemindersScreen({ onPickExample, highlight = null }: RemindersScreenProps) {
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
    <ScreenLayout>
      <div className="flex flex-col gap-4">
        <h1 className="m-0 text-xl font-medium">Recordatorios</h1>
        <p className="m-0 max-w-reading text-sm text-muted">
          Nexium avisa a la hora indicada mientras está abierto o en la bandeja del sistema. <strong className="font-medium text-ink">Pendiente</strong>: la
          hora no ha llegado. <strong className="font-medium text-ink">Avisado</strong>: el aviso se envió a su hora. <strong className="font-medium text-ink">Vencido</strong>: la
          hora pasó sin poder avisar, por ejemplo porque Nexium estaba cerrado con Salir.
        </p>
        {isWindows() && (
          <p className="m-0 max-w-reading text-sm text-muted">
            En Windows, el aviso se queda en pantalla hasta que lo descartes. Si solo aparece en el centro de notificaciones, revisa en Configuración &gt; Sistema &gt;
            Notificaciones que Nexium tenga activados los banners y que «No molestar» esté desactivado o permita los recordatorios.
          </p>
        )}
        {error && <Notice tone="error">{error}</Notice>}
        {reminders === null && !error && <Loading>Cargando recordatorios…</Loading>}
        {reminders?.length === 0 && <EmptyReminders onPickExample={onPickExample} />}
        {reminders && reminders.length > 0 && (
          <ul aria-label="Recordatorios" className="m-0 flex list-none flex-col border-b border-hairline p-0">
            {reminders.map((reminder, index) => (
              <ReminderRow key={`${reminder.id}-${reminder.dueAt}`} reminder={reminder} index={index} highlighted={reminder.id === highlight} onChanged={load} onError={setError} />
            ))}
          </ul>
        )}
      </div>
    </ScreenLayout>
  );
}
