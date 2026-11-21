import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

/** Una carpeta target falsa con las órdenes que dejó MSBuild o Make al compilar whisper.cpp. */
function target(kind: "msvc" | "make", flags: string, layout: "antigua" | "nueva" = "antigua") {
  const root = mkdtempSync(join(tmpdir(), "nexium-whisper-"));
  // Cargo organiza las carpetas como whisper-rs-sys-<hash> o, en versiones recientes, whisper-rs-sys/<hash>.
  const package_ = layout === "antigua" ? ["whisper-rs-sys-abc"] : ["whisper-rs-sys", "abc"];
  const build = join(root, "release", "build", ...package_, "out", "build");
  if (kind === "msvc") {
    const dir = join(build, "ggml", "src", "ggml-cpu.dir", "Release", "ggml-cpu.tlog");
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, "CL.command.1.tlog"), Buffer.from(`^C:/GGML.C\r\n/c /nologo ${flags} /D NDEBUG ggml.c\r\n`, "utf16le"));
  } else {
    const dir = join(build, "ggml", "src", "CMakeFiles", "ggml-cpu.dir");
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, "flags.make"), `C_FLAGS = ${flags} -g -DNDEBUG\nCXX_FLAGS = ${flags}\n`);
  }
  return root;
}

const check = (root: string) => spawnSync(process.execPath, ["scripts/check-whisper-build.mjs", root], { encoding: "utf8" });

describe("comprobación de la compilación de whisper.cpp", () => {
  it("acepta whisper.cpp optimizado con MSVC y con Make", () => {
    expect(check(target("msvc", "/O2 /Ob2")).status).toBe(0);
    expect(check(target("make", "-O2")).status).toBe(0);
    expect(check(target("make", "-O3", "nueva")).status).toBe(0);
    expect(check(target("msvc", "/O2", "nueva")).status).toBe(0);
  });

  it("rechaza whisper.cpp sin optimizar, que tardaba 13 s por orden", () => {
    const msvc = check(target("msvc", "/Od"));
    expect(msvc.status).toBe(1);
    expect(msvc.stderr).toContain("sin optimizar");
    expect(check(target("make", "-O0")).status).toBe(1);
  });

  it("falla si no encuentra la compilación", () => {
    const empty = mkdtempSync(join(tmpdir(), "nexium-whisper-"));
    mkdirSync(join(empty, "release", "build"), { recursive: true });
    expect(check(empty).status).toBe(1);
  });
});
