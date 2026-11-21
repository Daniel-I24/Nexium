// Comprueba con qué banderas se compiló de verdad whisper.cpp en la compilación de distribución.
// Regresión: en release llegó a compilarse sin optimizar (/Od) y cada transcripción tardaba unos 13 s en lugar de 1 s.
//
//   node scripts/check-whisper-build.mjs [carpeta target]   (por defecto, src-tauri/target)
//
// Lee las órdenes reales del compilador: el registro de MSBuild (CL.command.*.tlog) con MSVC, o flags.make con
// Makefiles. Falla si no encuentra ninguna, si alguna no optimiza o si ninguna lleva /O2 o -O2/-O3.
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

const target = process.argv[2] ?? "src-tauri/target";
const buildRoot = join(target, "release", "build");

function* walk(dir) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) yield* walk(path);
    else yield path;
  }
}

const sources = [];
for (const name of readdirSync(buildRoot)) {
  if (!name.startsWith("whisper-rs-sys-")) continue;
  const out = join(buildRoot, name, "out", "build");
  try {
    statSync(out);
  } catch {
    continue;
  }
  for (const file of walk(out)) {
    const normalized = file.replaceAll("\\", "/");
    if (/\/Release\/[^/]+\.tlog\/CL\.command\.\d+\.tlog$/.test(normalized)) {
      sources.push({ file, text: readFileSync(file, "utf16le"), kind: "msvc" });
    } else if (normalized.endsWith("/flags.make") && /CMakeFiles\/(ggml|whisper)/.test(normalized)) {
      sources.push({ file, text: readFileSync(file, "utf8"), kind: "make" });
    }
  }
}

if (sources.length === 0) {
  console.error(`No se encontraron las órdenes de compilación de whisper.cpp en ${buildRoot}. ¿Se recompiló whisper-rs-sys?`);
  process.exit(1);
}

let optimized = 0;
const problems = [];
for (const { file, text, kind } of sources) {
  if (kind === "msvc") {
    if (/\s\/Od(\s|$)/.test(text)) problems.push(`${file}: compila con /Od, sin optimizar`);
    if (/\s\/O2(\s|$)/.test(text)) optimized += 1;
  } else {
    for (const line of text.split("\n").filter((line) => /^(C|CXX)_FLAGS\s*=/.test(line))) {
      if (/\s-O0(\s|$)/.test(` ${line} `)) problems.push(`${file}: compila con -O0, sin optimizar`);
      if (/\s-O[23s](\s|$)/.test(` ${line} `)) optimized += 1;
    }
  }
}
if (optimized === 0) problems.push("Ninguna orden de compilación de whisper.cpp lleva /O2 ni -O2/-O3.");
if (problems.length > 0) {
  console.error(problems.join("\n"));
  process.exit(1);
}
console.log(`whisper.cpp optimizado: ${optimized} órdenes de compilación con optimización en ${sources.length} archivos revisados.`);
