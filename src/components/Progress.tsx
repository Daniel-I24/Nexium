/**
 * Línea de progreso. Indeterminada mientras algo carga o procesa (el único movimiento continuo de Nexium);
 * llena cuando termina. Siempre va junto a un texto que dice qué ocurre: con Reducir movimiento queda quieta
 * y el texto manda.
 */
export function ProgressLine({ filled = false, className = "" }: { filled?: boolean; className?: string }) {
  return (
    <div aria-hidden="true" data-clip-ok className={`h-0.5 overflow-hidden ${filled ? "bg-ink" : "bg-hairline"} ${className}`}>
      {!filled && <div className="h-full w-1/4 animate-sweep bg-ink" />}
    </div>
  );
}

/** Estado de carga de una pantalla: qué se está cargando y la línea de progreso. */
export function Loading({ children }: { children: string }) {
  return (
    <div role="status" className="flex max-w-reading flex-col gap-2 text-muted">
      <span>{children}</span>
      <ProgressLine className="w-40 max-w-full" />
    </div>
  );
}
