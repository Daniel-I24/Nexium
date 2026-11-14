import { Icon } from "@/components/Icon";
import type { AssistantResponse } from "@/types/api";

/**
 * Respuestas anteriores comprimidas en una línea: hora, solicitud y resultado. Un fallo lleva icono y su nombre
 * para lectores de pantalla. Cada línea nueva llega desde abajo, desde donde estaba la respuesta que resume.
 */
export function Tape({ entries }: { entries: AssistantResponse[] }) {
  if (entries.length === 0) return null;
  return (
    <ol aria-label="Cinta de la sesión" className="m-0 flex list-none flex-col p-0">
      {entries.map((entry) => {
        const failed = entry.status === "failed";
        return (
          <li key={entry.id} className="flex animate-to-tape flex-wrap gap-x-6 gap-y-1 border-b border-hairline py-2 text-sm text-muted">
            <span className="w-12 shrink-0 font-mono">
              {new Date(entry.createdAt).toLocaleTimeString("es", { hour: "2-digit", minute: "2-digit" })}
            </span>
            <span className="min-w-0 flex-1 basis-48 break-words">{entry.request}</span>
            <span className={`flex min-w-0 flex-1 basis-48 items-start gap-1 break-words ${failed ? "text-error" : "text-ink"}`}>
              {failed && (
                <span className="flex h-lh items-center">
                  <Icon name="failed" label="Fallida" />
                </span>
              )}
              <span className="min-w-0">{entry.summary}</span>
            </span>
          </li>
        );
      })}
    </ol>
  );
}
