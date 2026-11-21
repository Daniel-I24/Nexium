// Comprueba con qué banderas se compiló de verdad whisper.cpp en la compilación de distribución.
// Regresión: en release llegó a compilarse sin optimizar (/Od) y cada transcripción tardaba unos 13 s en lugar de 1 s.
//
//   node scripts/check-whisper-build.mjs [carpeta target]   (por defecto, src-tauri/target)
//
// Lee las órdenes reales del compilador: el registro de MSBuild (CL.command.*.tlog) con MSVC, o flags.make con
// Makefiles. Falla si no encuentra ninguna, si alguna no optimiza o si ninguna lleva /O2 o -O2/-O3.
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

const target = process.argv[2] ?? "src-tauri/target";

/** Carpetas build de los perfiles release: target/release/build y, con --target, target/<triple>/release/build. */
function releaseBuildRoots(dir) {
  const roots = [];
  const direct = join(dir, "release", "build");
  if (existsSync(direct)) roots.push(direct);
  for (const name of existsSync(dir) ? readdirSync(dir) : []) {
    const nested = join(dir, name, "release", "build");
    if (existsSync(nested)) roots.push(nested);
  }
  return roots;
}
const buildRoots = releaseBuildRoots(target);

function* walk(dir) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) yield* walk(path);
    else yield path;
  }
}

const sources = [];
const whisperDirs = buildRoots.flatMap((root) =>
  readdirSync(root)
    .filter((name) => name.startsWith("whisper-rs-sys-"))
    .map((name) => ({ root, name })),
);
for (const { root: buildRoot, name } of whisperDirs) {
  const out = join(buildRoot, name, "out");
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
    } else if (normalized.endsWith("/build.ninja")) {
      // Con Ninja, las banderas de cada objetivo van en líneas "FLAGS = ...".
      const flags = readFileSync(file, "utf8")
        .split("\n")
        .filter((line) => /^\s*FLAGS\s*=/.test(line))
        .map((line) => line.replace(/^\s*FLAGS\s*=/, "C_FLAGS ="));
      if (flags.length > 0) sources.push({ file, text: flags.join("\n"), kind: "make" });
    }
  }
}

if (sources.length === 0) {
  console.error(`No se encontraron las órdenes de compilación de whisper.cpp en ${target}. ¿Se recompiló whisper-rs-sys?`);
  // Lo que sí hay, para poder diagnosticarlo desde el registro de la CI.
  console.error(`Carpetas en ${target}: ${existsSync(target) ? readdirSync(target).join(", ") : "(no existe)"}`);
  console.error(`Carpetas build de release: ${buildRoots.join(", ") || "ninguna"}`);
  for (const { root, name } of whisperDirs) {
    const dir = join(root, name);
    const files = [...walk(dir)].map((file) => file.slice(dir.length + 1).replaceAll("\\", "/"));
    console.error(`${dir}: ${files.length} archivos; ejemplos: ${files.filter((file) => /tlog|flags\.make|build\.ninja|CMakeCache/.test(file)).slice(0, 8).join(", ") || files.slice(0, 8).join(", ")}`);
  }
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
