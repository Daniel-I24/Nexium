import { useEffect, useId, useRef } from "react";

import { Button } from "./Button";

interface ConfirmDialogProps {
  open: boolean;
  title: string;
  /** Qué se va a borrar, con su nombre o su cantidad, y si se puede deshacer. */
  description: string;
  confirmLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * Confirmación de una acción destructiva. Usa el `<dialog>` modal del sistema: el resto de la ventana
 * queda inerte, Escape cancela y el foco empieza en Cancelar, para que Intro no borre por accidente.
 * Al cerrarse, el foco vuelve al botón que lo abrió.
 */
export function ConfirmDialog({ open, title, description, confirmLabel, onConfirm, onCancel }: ConfirmDialogProps) {
  const dialog = useRef<HTMLDialogElement>(null);
  const cancel = useRef<HTMLButtonElement>(null);
  const titleId = useId();
  const descriptionId = useId();

  useEffect(() => {
    const element = dialog.current;
    if (!element || !open) return;
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    if (!element.open) element.showModal();
    cancel.current?.focus();
    return () => {
      if (element.open) element.close();
      opener?.focus();
    };
  }, [open]);

  if (!open) return null;

  return (
    <dialog
      ref={dialog}
      role="alertdialog"
      aria-modal="true"
      aria-labelledby={titleId}
      aria-describedby={descriptionId}
      onCancel={(event) => {
        event.preventDefault();
        onCancel();
      }}
      className="m-auto w-full max-w-dialog animate-dialog rounded-sm border border-line bg-panel p-0 text-ink backdrop:animate-fade"
    >
      <div className="flex flex-col gap-4 p-6">
        <h2 id={titleId} className="m-0 text-lg font-semibold">
          {title}
        </h2>
        <p id={descriptionId} className="m-0 break-words">
          {description}
        </p>
        <div className="flex flex-wrap justify-end gap-3">
          <Button ref={cancel} onClick={onCancel}>
            Cancelar
          </Button>
          <Button variant="danger" onClick={onConfirm}>
            {confirmLabel}
          </Button>
        </div>
      </div>
    </dialog>
  );
}
