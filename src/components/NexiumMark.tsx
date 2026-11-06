interface NexiumMarkProps {
  /** Nexium te oye o va a ver tu pantalla: solo entonces se enciende el nodo. */
  sensing?: boolean;
  size?: number;
}

export function NexiumMark({ sensing = false, size = 28 }: NexiumMarkProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 28 28" fill="none" role="img" aria-label="Símbolo de Nexium">
      <rect x="3" y="3" width="4" height="22" fill="currentColor" />
      <rect x="21" y="3" width="4" height="22" fill="currentColor" />
      <path d="M7 5 21 23" stroke="currentColor" strokeWidth="4" />
      <rect
        x="10.5"
        y="10.5"
        width="7"
        height="7"
        stroke="currentColor"
        strokeWidth="1.5"
        className={sensing ? "fill-signal" : "fill-base"}
      />
    </svg>
  );
}
