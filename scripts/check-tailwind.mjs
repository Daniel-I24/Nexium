// Falla si el código usa clases fuera del sistema visual (docs/DISENO_VISUAL.md, sección 7).
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

const palette =
  "slate|gray|zinc|neutral|stone|red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose|black|white";
const rules = [
  [new RegExp(`\\b(?:bg|text|border|ring|fill|stroke|outline|from|to|via)-(?:${palette})(?:-\\d+)?\\b`), "color fuera de los tokens"],
  [/(?:^|[\s"'`])[a-z:-]*-\[[^\]]+\]/, "valor arbitrario"],
  [/\brounded-(?:md|lg|xl|2xl|3xl|full)\b/, "radio mayor de 2 px"],
  [/\bshadow-(?:sm|md|lg|xl|2xl|inner)\b/, "sombra"],
  [/\bbg-gradient-|\bbg-linear-/, "degradado"],
  [/\bdark:/, "variante dark: (los temas solo reasignan variables)"],
];

function* sources(dir) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) yield* sources(path);
    else if (/\.tsx?$/.test(name) && !/\.test\.tsx?$/.test(name)) yield path;
  }
}

let failures = 0;
for (const file of sources("src")) {
  readFileSync(file, "utf8")
    .split("\n")
    .forEach((line, index) => {
      for (const [rule, reason] of rules) {
        if (rule.test(line)) {
          console.error(`${file}:${index + 1}: ${reason}: ${line.trim()}`);
          failures += 1;
        }
      }
    });
}
if (failures > 0) process.exit(1);
console.log("Tailwind: todas las clases respetan el sistema visual.");
