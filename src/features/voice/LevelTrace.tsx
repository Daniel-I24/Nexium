/**
 * Trazo del nivel real del micrófono: cada barra es una lectura enviada por el backend.
 * Las barras ocupan todo el alto y se escalan en vertical: solo cambia una transformación, sin recalcular el diseño.
 */
export function LevelTrace({ levels }: { levels: number[] }) {
  return (
    <div role="img" aria-label="Nivel del micrófono" data-clip-ok className="flex h-11 min-w-12 flex-1 basis-12 items-center gap-0.5 overflow-hidden">
      {levels.map((level, index) => (
        <span key={index} className="h-full min-w-0.5 flex-1 bg-on-signal" style={{ transform: `scaleY(${Math.max(0.06, Math.min(1, level))})` }} />
      ))}
    </div>
  );
}
