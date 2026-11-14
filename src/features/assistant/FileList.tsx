import { StatusLabel } from "@/components/StatusLabel";
import type { FileResult } from "@/types/api";

export function FileList({ files }: { files: FileResult[] }) {
  return (
    <ul className="m-0 flex max-w-reading list-none flex-col border-b border-hairline p-0">
      {files.map((file) => (
        <li key={file.path} className="flex flex-col gap-0.5 border-t border-hairline py-3">
          <div className="flex flex-wrap justify-between gap-x-4">
            <strong className="min-w-0 break-words font-semibold">{file.name}</strong>
            <span className="font-mono text-sm">{file.extension}</span>
          </div>
          <span className="break-all font-mono text-xs text-muted">{file.path}</span>
          <span className="flex flex-wrap items-center gap-x-1 text-sm">
            <StatusLabel status={file.exists ? "completed" : "failed"}>{file.exists ? "Existe" : "Ya no existe"}</StatusLabel>
            <span className="text-muted">· en {file.location}</span>
          </span>
        </li>
      ))}
    </ul>
  );
}
