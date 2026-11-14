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
  // Movimiento: solo opacidad y transformaciones, con las duraciones y curvas del tema.
  [/\btransition(?:-(?:colors|all|shadow|discrete))?(?=["'`\s])/, "transición de algo que no es opacidad ni transformación"],
  [/\bduration-\d+\b/, "duración fuera de los tokens (duration-fast, duration-base, duration-slow)"],
  [/\b(?:ease-(?:in|out|in-out|linear)|animate-(?:spin|ping|pulse|bounce))\b/, "curva o animación fuera de los tokens"],
];

// Un único conjunto de iconos: solo src/components/Icon.tsx importa lucide-react.
const iconRule = [/from\s+["']lucide-react["']/, "icono importado fuera de src/components/Icon.tsx"];

function* sources(dir) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) yield* sources(path);
    else if (/\.tsx?$/.test(name) && !/\.test\.tsx?$/.test(name)) yield path;
  }
}

let failures = 0;
for (const file of sources("src")) {
  const fileRules = file.replaceAll("\\", "/").endsWith("src/components/Icon.tsx") ? rules : [...rules, iconRule];
  readFileSync(file, "utf8")
    .split("\n")
    .forEach((line, index) => {
      for (const [rule, reason] of fileRules) {
        if (rule.test(line)) {
          console.error(`${file}:${index + 1}: ${reason}: ${line.trim()}`);
          failures += 1;
        }
      }
    });
}
if (failures > 0) process.exit(1);
console.log("Tailwind: todas las clases respetan el sistema visual.");
