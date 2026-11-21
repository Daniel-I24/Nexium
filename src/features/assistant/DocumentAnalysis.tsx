import type { DocumentTask } from "@/types/api";

/** "Normalización: elimina datos repetidos" se muestra con el nombre de la idea destacado. */
export function splitIdea(point: string): { name: string | null; text: string } {
  const at = point.indexOf(": ");
  if (at > 0 && at <= 60) return { name: point.slice(0, at), text: point.slice(at + 2) };
  return { name: null, text: point };
}

function Ideas({ title, items }: { title: string; items: string[] }) {
  if (items.length === 0) return null;
  return (
    <section className="flex flex-col gap-1">
      <h2 className="m-0 text-sm font-medium text-muted">{title}</h2>
      <ol className="m-0 list-none p-0">
        {items.map((item, index) => {
          const { name, text } = splitIdea(item);
          return (
            <li key={index} className="flex gap-3 border-t border-hairline py-2 text-md">
              <span aria-hidden="true" className="font-mono text-muted">
                {index + 1}
              </span>
              <span className="min-w-0 wrap-anywhere">
                {name && <strong className="font-semibold">{name}: </strong>}
                {text}
              </span>
            </li>
          );
        })}
      </ol>
    </section>
  );
}

interface DocumentAnalysisProps {
  task: DocumentTask;
  summary: string;
  points: string[];
  details: string[];
  truncated: boolean;
}

/**
 * Resumen o análisis de un documento, con el formato estructurado de Nexium: el resumen, las ideas clave (u
 * observaciones) con su explicación y, si los hay, los datos importantes (fechas, cifras, definiciones).
 */
export function DocumentAnalysisView({ task, summary, points, details, truncated }: DocumentAnalysisProps) {
  return (
    <div className="flex max-w-reading flex-col gap-5">
      <p className="m-0 wrap-anywhere text-lg">{summary}</p>
      <Ideas title={task === "summarize" ? "Ideas clave" : "Observaciones"} items={points} />
      <Ideas title="Datos importantes" items={details} />
      {truncated && <p className="m-0 text-sm text-muted">El documento es largo: solo se envió su principio, y el resumen se basa en esa parte.</p>}
    </div>
  );
}
