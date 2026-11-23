// Notas de una Release: la plantilla .github/release-notes.md con la versión y sus novedades
// (.github/changes/<versión>.md). Uso: node scripts/release-notes.mjs 1.0.1 > notas.md
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

// Se ejecuta desde la raíz del repositorio, como el flujo de publicación y las pruebas.
const root = process.cwd();

export function renderNotes(template, version, changes) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error(`Versión no válida: ${version}`);
  return template.replaceAll("__VERSION__", version).replace("__CHANGES__", changes.trim());
}

export function notesFor(version) {
  const template = readFileSync(join(root, ".github/release-notes.md"), "utf8");
  const changes = readFileSync(join(root, ".github/changes", `${version}.md`), "utf8");
  return renderNotes(template, version, changes);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  process.stdout.write(notesFor(process.argv[2]));
}
