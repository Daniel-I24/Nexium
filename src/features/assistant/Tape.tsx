import type { AssistantResponse } from "@/types/api";

/** Respuestas anteriores comprimidas en una línea: hora, solicitud y resultado. */
export function Tape({ entries }: { entries: AssistantResponse[] }) {
  if (entries.length === 0) return null;
  return (
    <ol aria-label="Cinta de la sesión" className="m-0 flex list-none flex-col p-0">
      {entries.map((entry) => (
        <li key={entry.id} className="flex flex-wrap gap-x-6 gap-y-1 border-b border-hairline py-2 text-sm text-muted">
          <span className="w-12 shrink-0 font-mono">
            {new Date(entry.createdAt).toLocaleTimeString("es", { hour: "2-digit", minute: "2-digit" })}
          </span>
          <span className="min-w-48 flex-1">{entry.request}</span>
          <span className={`min-w-48 flex-1 ${entry.status === "failed" ? "text-error" : "text-ink"}`}>{entry.summary}</span>
        </li>
      ))}
    </ol>
  );
}
