import type { DataRowContent } from "@/types/api";

export function DataRows({ rows }: { rows: DataRowContent[] }) {
  return (
    <dl className="m-0 flex max-w-reading flex-col border-b border-hairline">
      {rows.map((row) => (
        <div key={row.label} className="flex flex-wrap justify-between gap-x-6 gap-y-1 border-t border-hairline py-2">
          <dt className="text-muted">{row.label}</dt>
          <dd className={`m-0 ${row.mono && !row.unavailable ? "font-mono" : ""} ${row.unavailable ? "text-muted" : ""}`}>
            {row.value}
          </dd>
        </div>
      ))}
    </dl>
  );
}
