import { useEffect, useId, useRef, useState } from "react";

import { exitThen } from "@/utils/motion";

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
 * El fondo se oscurece mientras el panel entra; al cerrarse, los dos salen antes de que el foco vuelva
 * al botón que lo abrió.
 */
export function ConfirmDialog({ open, title, description, confirmLabel, onConfirm, onCancel }: ConfirmDialogProps) {
  const dialog = useRef<HTMLDialogElement>(null);
  const cancel = useRef<HTMLButtonElement>(null);
  const [closing, setClosing] = useState(false);
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
      setClosing(false);
      opener?.focus();
    };
  }, [open]);

  if (!open) return null;

  const close = (action: () => void) => {
    if (!closing) exitThen(() => setClosing(true), action);
  };

  return (
    <dialog
      ref={dialog}
      role="alertdialog"
      aria-modal="true"
      aria-labelledby={titleId}
      aria-describedby={descriptionId}
      onCancel={(event) => {
        event.preventDefault();
        close(onCancel);
      }}
      className={`m-auto w-full max-w-dialog rounded-sm border border-line bg-panel p-0 text-ink ${
        closing ? "animate-dialog-out backdrop:animate-backdrop-out" : "animate-dialog backdrop:animate-backdrop"
      }`}
    >
      <div className="flex flex-col gap-4 p-6">
        <h2 id={titleId} className="m-0 text-lg font-semibold">
          {title}
        </h2>
        <p id={descriptionId} className="m-0 wrap-anywhere">
          {description}
        </p>
        <div className="flex flex-wrap justify-end gap-3">
          <Button ref={cancel} onClick={() => close(onCancel)}>
            Cancelar
          </Button>
          <Button variant="danger" onClick={() => close(onConfirm)}>
            {confirmLabel}
          </Button>
        </div>
      </div>
    </dialog>
  );
}
