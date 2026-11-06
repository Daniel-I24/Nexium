/** Trazo del nivel real del micrófono: cada barra es una lectura enviada por el backend. */
export function LevelTrace({ levels }: { levels: number[] }) {
  return (
    <div role="img" aria-label="Nivel del micrófono" className="flex h-11 min-w-48 flex-1 items-center gap-0.5 overflow-hidden">
      {levels.map((level, index) => (
        <span
          key={index}
          className="min-w-0.5 flex-1 bg-on-signal"
          style={{ height: `${Math.max(6, Math.min(100, level * 100))}%` }}
        />
      ))}
    </div>
  );
}
