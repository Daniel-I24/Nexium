export function Scale({ percent, label }: { percent: number; label: string }) {
  const bounded = Math.min(100, Math.max(0, percent));
  return (
    <div className="max-w-reading">
      <div
        role="meter"
        aria-label={label}
        aria-valuenow={Math.round(bounded)}
        aria-valuemin={0}
        aria-valuemax={100}
        className="h-3 border border-ink"
      >
        <div className="h-full bg-ink" style={{ width: `${bounded}%` }} />
      </div>
      <div aria-hidden="true" className="mt-1 flex justify-between font-mono text-xs text-muted">
        <span>0</span>
        <span>25</span>
        <span>50</span>
        <span>75</span>
        <span>100 %</span>
      </div>
    </div>
  );
}
