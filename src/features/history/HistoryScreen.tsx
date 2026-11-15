import { useCallback, useEffect, useState } from "react";

import { Button } from "@/components/Button";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { Notice } from "@/components/Notice";
import { Loading } from "@/components/Progress";
import { ScreenLayout } from "@/components/ScreenLayout";
import { StatusLabel, type Status } from "@/components/StatusLabel";
import { api, errorMessage } from "@/services/api";
import type { Category, HistoryEntry, InteractionStatus } from "@/types/api";
import { exitThen, stagger } from "@/utils/motion";
import { count } from "@/utils/plural";

const statusLabels: Record<InteractionStatus, { label: string; status: Status }> = {
  completed: { label: "Completada", status: "completed" },
  pending: { label: "Pendiente", status: "pending" },
  failed: { label: "Fallida", status: "failed" },
  cancelled: { label: "Cancelada", status: "cancelled" },
};

const categoryLabels: Record<Category, string> = {
  time: "Hora",
  system: "Sistema",
  files: "Archivos",
  weather: "Clima",
  reminders: "Recordatorios",
  screen: "Pantalla",
  general: "General",
};

const filters: { value: InteractionStatus | null; label: string; empty: string }[] = [
  { value: null, label: "Todas", empty: "Todavía no hay nada en el historial. Cada solicitud que hagas aparecerá aquí." },
  { value: "completed", label: "Completadas", empty: "No hay solicitudes completadas." },
  { value: "pending", label: "Pendientes", empty: "No hay solicitudes pendientes." },
  { value: "failed", label: "Fallidas", empty: "No hay solicitudes fallidas." },
];

/** Nombra exactamente lo que se borrará: con un filtro activo, también lo que no se ve. */
export function clearDescription(filter: InteractionStatus | null, visible: number): string {
  const option = filters.find((item) => item.value === filter);
  const what =
    filter === null || !option
      ? `${visible === 1 ? "Se borrará la única entrada" : `Se borrarán las ${count(visible, "entrada", "entradas")}`} del historial de este equipo.`
      : `Se borrarán todas las entradas del historial de este equipo, también las que no muestra el filtro «${option.label}».`;
  return `${what} También se vacía la conversación del Asistente. Los recordatorios guardados no se tocan. No se puede deshacer.`;
}

/** `onCleared`: tras Borrar todo, la conversación del Asistente también se vacía. */
export function HistoryScreen({ onCleared }: { onCleared: () => void }) {
  const [entries, setEntries] = useState<HistoryEntry[] | null>(null);
  const [filter, setFilter] = useState<InteractionStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);
  /** Entradas que se están yendo: se animan antes de borrarse. `all` cuando se borra todo. */
  const [leaving, setLeaving] = useState<Set<number> | "all">(new Set());

  const load = useCallback(() => {
    api
      .listHistory(filter)
      .then((items) => {
        setEntries(items);
        setError(null);
      })
      .catch((reason: unknown) => setError(errorMessage(reason)));
  }, [filter]);

  useEffect(load, [load]);

  const act = (action: Promise<void>) =>
    action
      .then(load)
      .catch((reason: unknown) => setError(errorMessage(reason)))
      .finally(() => setLeaving(new Set()));

  const remove = (id: number) =>
    exitThen(
      () => setLeaving((current) => new Set(current === "all" ? [] : [...current, id]).add(id)),
      () => void act(api.deleteHistory(id)),
    );
  const isLeaving = (id: number) => leaving === "all" || leaving.has(id);
  const emptyText = filters.find((item) => item.value === filter)?.empty ?? "";

  return (
    <ScreenLayout>
      <div className="flex flex-col gap-4">
        <h1 className="m-0 text-xl font-medium">Historial</h1>
        <p className="m-0 max-w-reading text-sm text-muted">
          Se guarda solo en este equipo, con las últimas 500 solicitudes. No incluye audio ni capturas, aunque una respuesta sobre tu pantalla sí puede describir lo que se veía en ella.
        </p>
        <div className="flex flex-wrap items-center gap-2">
          <div role="group" aria-label="Filtrar por estado" className="flex flex-wrap gap-2">
            {filters.map((option) => (
              <Button key={option.label} variant={option.value === filter ? "primary" : "secondary"} aria-pressed={option.value === filter} onClick={() => setFilter(option.value)}>
                {option.label}
              </Button>
            ))}
          </div>
          {entries && entries.length > 0 && (
            <Button variant="danger" className="ml-auto" onClick={() => setConfirmClear(true)}>
              Borrar todo
            </Button>
          )}
        </div>
        {error && <Notice tone="error">{error}</Notice>}
        {entries === null && !error && <Loading>Cargando el historial…</Loading>}
        {entries?.length === 0 && <p className="m-0 animate-enter text-muted">{emptyText}</p>}
        {entries && entries.length > 0 && (
          <ul aria-label="Solicitudes" className="m-0 flex list-none flex-col border-b border-hairline p-0">
            {entries.map((entry, index) => {
              const status = statusLabels[entry.status];
              return (
                <li
                  key={entry.id}
                  style={isLeaving(entry.id) ? undefined : stagger(index)}
                  className={`flex flex-wrap items-start gap-x-4 gap-y-2 border-t border-hairline py-3 ${isLeaving(entry.id) ? "animate-leave" : "animate-enter"}`}
                >
                  <div className="flex min-w-0 flex-1 basis-60 flex-col gap-1">
                    <strong className="break-words font-medium">{entry.request}</strong>
                    <span className="whitespace-pre-wrap break-words text-sm">{entry.response}</span>
                    <span className="flex flex-wrap items-center gap-x-2 text-sm text-muted">
                      <span className="font-mono">{new Date(entry.createdAt).toLocaleString("es", { dateStyle: "short", timeStyle: "short" })}</span>
                      <span aria-hidden="true">·</span>
                      <span>{categoryLabels[entry.category]}</span>
                      <span aria-hidden="true">·</span>
                      <StatusLabel status={status.status}>{status.label}</StatusLabel>
                    </span>
                  </div>
                  <Button variant="danger" aria-label={`Eliminar del historial: ${entry.request}`} disabled={isLeaving(entry.id)} onClick={() => remove(entry.id)}>
                    Eliminar
                  </Button>
                </li>
              );
            })}
          </ul>
        )}
        <ConfirmDialog
          open={confirmClear}
          title="¿Borrar todo el historial?"
          description={clearDescription(filter, entries?.length ?? 0)}
          confirmLabel="Borrar todo"
          onCancel={() => setConfirmClear(false)}
          onConfirm={() => {
            setConfirmClear(false);
            exitThen(
              () => setLeaving("all"),
              () => void act(api.clearHistory().then(onCleared)),
            );
          }}
        />
      </div>
    </ScreenLayout>
  );
}
