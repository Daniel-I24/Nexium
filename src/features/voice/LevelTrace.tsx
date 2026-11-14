/** Barras del trazo: 2,4 s de voz, una lectura del micrófono cada 50 ms. */
export const TRACE_BARS = 48;

/** Las barras del borde izquierdo, las más antiguas, se apagan: el trazo avanza hacia la derecha. */
const FADING_BARS = 10;

/** Amplitud visible: un mínimo para que el silencio se lea como una línea, y raíz cuadrada para que la voz baja se vea. */
function amplitude(level: number) {
  return Math.max(0.04, Math.sqrt(Math.min(1, Math.max(0, level))));
}

/**
 * Trazo del nivel real del micrófono, simétrico respecto a una línea central, como en un instrumento de medida.
 * Cada barra ocupa todo el alto y se escala en vertical desde el centro: solo cambia una transformación, que se
 * interpola en 150 ms, así que el trazo fluye entre lecturas sin recalcular el diseño. Si falta sitio, se recortan
 * las barras más antiguas, nunca las recientes.
 */
export function LevelTrace({ levels }: { levels: number[] }) {
  const recent = levels.slice(-TRACE_BARS);
  const bars = [...Array<number>(TRACE_BARS - recent.length).fill(0), ...recent];
  return (
    <div role="img" aria-label="Nivel del micrófono" data-clip-ok className="relative flex h-11 min-w-12 flex-1 basis-12 items-center justify-end gap-0.5 overflow-hidden">
      <span aria-hidden="true" className="absolute inset-x-0 top-1/2 h-px bg-on-signal opacity-40" />
      {bars.map((level, index) => (
        <span
          key={index}
          className="h-full w-0.5 min-w-0.5 flex-1 rounded-sm bg-on-signal transition-transform duration-fast ease-instrument"
          style={{ transform: `scaleY(${amplitude(level)})`, opacity: Math.min(1, (index + 1) / FADING_BARS) }}
        />
      ))}
    </div>
  );
}
