import { useCallback, useEffect, useState } from "react";

import { Button } from "@/components/Button";
import { api, errorMessage } from "@/services/api";
import type { Category, HistoryEntry, InteractionStatus } from "@/types/api";

const statusLabels: Record<InteractionStatus, { label: string; style: string }> = {
  completed: { label: "Completada", style: "text-ok" },
  pending: { label: "Pendiente", style: "text-query" },
  failed: { label: "Fallida", style: "text-error" },
  cancelled: { label: "Cancelada", style: "text-muted" },
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

const filters: { value: InteractionStatus | null; label: string }[] = [
  { value: null, label: "Todas" },
  { value: "completed", label: "Completadas" },
  { value: "pending", label: "Pendientes" },
  { value: "failed", label: "Fallidas" },
];

export function HistoryScreen() {
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [filter, setFilter] = useState<InteractionStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

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

  const act = (action: Promise<void>) => action.then(load).catch((reason: unknown) => setError(errorMessage(reason)));

  return (
    <div className="flex flex-col gap-4">
      <h1 className="m-0 text-xl font-medium leading-tight">Historial</h1>
      <p className="m-0 text-sm text-muted">Se guarda en este equipo. No incluye claves, audio ni capturas. Una respuesta sobre tu pantalla sí puede describir lo que se veía en ella.</p>
      <div role="group" aria-label="Filtrar por estado" className="flex flex-wrap items-center gap-2">
        {filters.map((option) => (
          <Button key={option.label} variant={option.value === filter ? "primary" : "secondary"} aria-pressed={option.value === filter} onClick={() => setFilter(option.value)}>
            {option.label}
          </Button>
        ))}
        {entries.length > 0 && (
          <Button variant="danger" className="ml-auto" onClick={() => void act(api.clearHistory())}>
            Borrar todo
          </Button>
        )}
      </div>
      {error && <p className="m-0 text-error">{error}</p>}
      {entries.length === 0 && !error && <p className="m-0 text-muted">No hay registros.</p>}
      {entries.length > 0 && (
        <ul className="m-0 flex list-none flex-col border-b border-hairline p-0">
          {entries.map((entry) => {
            const status = statusLabels[entry.status];
            return (
              <li key={entry.id} className="flex flex-wrap items-start gap-x-4 gap-y-2 border-t border-hairline py-3">
                <div className="flex min-w-48 flex-1 flex-col gap-1">
                  <strong className="font-medium">{entry.request}</strong>
                  <span className="whitespace-pre-wrap text-sm">{entry.response}</span>
                  <span className="text-sm text-muted">
                    <span className="font-mono">{new Date(entry.createdAt).toLocaleString("es", { dateStyle: "short", timeStyle: "short" })}</span> · {categoryLabels[entry.category]} ·{" "}
                    <span className={status.style}>{status.label}</span>
                  </span>
                </div>
                <Button variant="danger" onClick={() => void act(api.deleteHistory(entry.id))}>
                  Eliminar
                </Button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
