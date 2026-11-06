import type { FileResult } from "@/types/api";

export function FileList({ files }: { files: FileResult[] }) {
  return (
    <ul className="m-0 flex max-w-reading list-none flex-col border-b border-hairline p-0">
      {files.map((file) => (
        <li key={file.path} className="flex flex-col gap-0.5 border-t border-hairline py-3">
          <div className="flex flex-wrap justify-between gap-x-4">
            <strong className="font-semibold">{file.name}</strong>
            <span className="font-mono text-sm">{file.extension}</span>
          </div>
          <span className="break-all font-mono text-xs text-muted">{file.path}</span>
          <span className={`text-sm ${file.exists ? "text-ok" : "text-error"}`}>
            {file.exists ? "Existe" : "Ya no existe"} · en {file.location}
          </span>
        </li>
      ))}
    </ul>
  );
}
