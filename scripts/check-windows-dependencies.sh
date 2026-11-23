#!/usr/bin/env bash
# Falla si el ejecutable de Windows necesita DLL de Visual C++ (MSVCP140, VCRUNTIME140…), que no vienen con
# Windows: sin el «Visual C++ Redistributable», Nexium no abriría. Uso: scripts/check-windows-dependencies.sh <exe>
set -euo pipefail
vswhere="/c/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe"
dumpbin=$("$vswhere" -latest -products '*' -find 'VC/Tools/MSVC/**/bin/Hostx64/x64/dumpbin.exe' | head -1 | tr -d '\r')
dependencies=$(MSYS_NO_PATHCONV=1 "$dumpbin" /dependents "$(cygpath -w "$1")" | grep -iE '^\s+[a-z0-9_.-]+\.dll' | tr -d ' \r')
echo "$dependencies"
test -n "$dependencies" || { echo "No se pudieron leer las dependencias." >&2; exit 1; }
if echo "$dependencies" | grep -iqE '^(msvcp|vcruntime|concrt|vcomp)[0-9_]*\.dll$'; then
  echo "El ejecutable depende de DLL de Visual C++ que no vienen con Windows." >&2
  exit 1
fi
