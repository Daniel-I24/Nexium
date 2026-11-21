import { useId, useState } from "react";

import { Button } from "@/components/Button";
import { Notice } from "@/components/Notice";
import { api, errorMessage } from "@/services/api";
import type { DocumentCandidate, DocumentTask } from "@/types/api";
import type { FolderChoice } from "./useAssistant";

const fileName = (candidate: DocumentCandidate) => `${candidate.name}${candidate.extension}`;
const thousands = (value: number) => value.toLocaleString("es");

/**
 * Permiso para buscar en carpetas. Al aceptar, la solicitud que lo pidió sigue sola, sin repetirla.
 * Nexium solo lee: lo dice antes de pedir nada.
 */
export function FolderConsent({
  folders,
  pendingRequest,
  onAuthorize,
  onDismiss,
}: {
  folders: string[];
  pendingRequest: string;
  onAuthorize: (choice: FolderChoice, pendingRequest: string) => void;
  onDismiss: (message: string) => void;
}) {
  const [answered, setAnswered] = useState(false);
  const answer = (choice: FolderChoice) => {
    setAnswered(true);
    onAuthorize(choice, pendingRequest);
  };
  return (
    <div className="flex max-w-reading flex-col gap-3">
      <p className="m-0 text-lg">Para buscar y leer tus archivos necesito tu permiso para alguna carpeta. Nexium solo lee: nunca modifica, mueve ni elimina nada.</p>
      {folders.length > 0 && (
        <ul aria-label="Carpetas propuestas" className="m-0 list-none p-0 font-mono text-sm">
          {folders.map((folder) => (
            <li key={folder} className="break-all border-t border-hairline py-2">
              {folder}
            </li>
          ))}
        </ul>
      )}
      <div className="flex flex-wrap gap-2">
        {folders.length > 0 && (
          <Button variant="primary" autoFocus disabled={answered} onClick={() => answer("known")}>
            Permitir Documentos y Escritorio
          </Button>
        )}
        <Button disabled={answered} onClick={() => answer("pick")}>
          Elegir otra carpeta
        </Button>
        <Button disabled={answered} onClick={() => onDismiss("Cancelado. No se buscó nada.")}>
          Cancelar
        </Button>
      </div>
      <p className="m-0 text-sm text-muted">Puedes añadir o quitar carpetas cuando quieras en Ajustes.</p>
    </div>
  );
}

interface DocumentConsentProps {
  task: DocumentTask;
  provider: string;
  candidates: DocumentCandidate[];
  maxCharacters: number;
  note: string | null;
  /** Solo si la coincidencia es clara; si no, se elige antes de confirmar. */
  preselected: string | null;
  onAnalyze: (candidate: DocumentCandidate, task: DocumentTask) => void;
  onDismiss: (message: string) => void;
}

/**
 * Confirmación expresa antes de enviar un documento: nombra el archivo y adónde va su contenido.
 * Si hay varios candidatos, se elige uno; también se puede elegir otro con el selector del sistema.
 */
export function DocumentConsent({ task, provider, candidates: found, maxCharacters, note, preselected, onAnalyze, onDismiss }: DocumentConsentProps) {
  const [candidates, setCandidates] = useState(found);
  const [selected, setSelected] = useState<string | null>(preselected ?? (found.length === 1 ? (found[0]?.token ?? null) : null));
  const [sent, setSent] = useState(false);
  const [pickError, setPickError] = useState<string | null>(null);
  const groupName = useId();
  const chosen = candidates.find((candidate) => candidate.token === selected) ?? null;
  const verb = task === "summarize" ? "Resumir" : "Analizar";

  const pick = async () => {
    setPickError(null);
    try {
      const candidate = await api.pickDocument();
      if (!candidate) return;
      setCandidates((current) => [candidate, ...current.filter((item) => item.token !== candidate.token)]);
      setSelected(candidate.token);
    } catch (error) {
      setPickError(errorMessage(error));
    }
  };

  const question =
    candidates.length === 0
      ? (note ?? `Elige el documento que quieres ${task === "summarize" ? "resumir" : "analizar"}.`)
      : candidates.length === 1 && chosen
        ? `Encontré «${fileName(chosen)}» en ${chosen.location}.`
        : `Encontré ${candidates.length} documentos. ¿Cuál quieres ${task === "summarize" ? "resumir" : "analizar"}?`;

  return (
    <div className="flex max-w-reading flex-col gap-3">
      <p className="m-0 text-lg wrap-anywhere">{question}</p>
      {candidates.length > 1 && (
        <fieldset className="m-0 flex flex-col border-0 p-0">
          <legend className="sr-only">Documentos encontrados</legend>
          {candidates.map((candidate) => (
            <label key={candidate.token} className="flex min-h-11 items-center gap-3 border-t border-hairline py-1">
              <input
                type="radio"
                name={groupName}
                className="size-4 shrink-0 accent-ink"
                checked={candidate.token === selected}
                disabled={sent}
                onChange={() => setSelected(candidate.token)}
              />
              <span className="min-w-0 wrap-anywhere">
                {fileName(candidate)} <span className="text-sm text-muted">· en {candidate.location}</span>
              </span>
            </label>
          ))}
        </fieldset>
      )}
      {chosen && (
        <p className="m-0 border-t-2 border-query pt-2 text-sm wrap-anywhere">
          Al confirmar, Nexium leerá «{fileName(chosen)}» en este equipo y enviará su texto a {provider}, como mucho los primeros {thousands(maxCharacters)} caracteres. El
          historial guarda el nombre y el resumen, no el contenido.
        </p>
      )}
      {pickError && <Notice tone="error">{pickError}</Notice>}
      <div className="flex flex-wrap gap-2">
        {chosen && (
          <Button
            variant="primary"
            autoFocus
            disabled={sent}
            onClick={() => {
              setSent(true);
              onAnalyze(chosen, task);
            }}
          >
            <span className="min-w-0 wrap-anywhere">
              {verb} «{fileName(chosen)}»
            </span>
          </Button>
        )}
        <Button disabled={sent} onClick={() => void pick()}>
          {candidates.length > 0 ? "Elegir otro archivo" : "Elegir archivo"}
        </Button>
        <Button disabled={sent} onClick={() => onDismiss("Cancelado. No se leyó ni se envió nada.")}>
          Cancelar
        </Button>
      </div>
    </div>
  );
}
