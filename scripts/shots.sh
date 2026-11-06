#!/usr/bin/env bash
# Fotografía la interfaz en varios tamaños y temas. Requiere `npm run dev` en marcha.
# Uso: scripts/shots.sh <carpeta de salida>
out="${1:?carpeta de salida}"
mkdir -p "$out"
edge="/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"
base="http://localhost:1420/tests/visual/preview.html"
shot() { # nombre ancho alto consulta [oscuro]
  local flags=(--blink-settings=preferredColorScheme=1)
  [ "${5:-}" = dark ] && flags=(--blink-settings=preferredColorScheme=0)
  timeout 60 "$edge" --headless=new --disable-gpu --no-first-run --hide-scrollbars --user-data-dir="$(cygpath -w "$out/.edge")" "${flags[@]}" \
    --window-size="$2,$3" --timeout=12000 --screenshot="$(cygpath -w "$out/$1.png")" "$base?$4" >/dev/null 2>&1
}
shot ram-1100 1100 720 "body=ram&bar=idle"
shot ram-dark-1100 1100 720 "body=ram&bar=idle" dark
shot empty-1100 1100 720 "body=empty&bar=idle"
shot listening-1100 1100 720 "body=ram&bar=listening"
shot listening-dark-1100 1100 720 "body=ram&bar=listening" dark
shot capture-1100 1100 720 "body=ram&bar=capture"
shot capture-dark-1100 1100 720 "body=ram&bar=capture" dark
shot clarify-1100 1100 720 "body=draft&bar=clarify"
shot clarify-dark-1100 1100 720 "body=draft&bar=clarify" dark
shot responding-1100 1100 720 "body=screen&bar=responding"
shot error-dark-1100 1100 720 "body=quota&bar=error" dark
shot files-1100 1100 720 "body=files&bar=idle"
shot weather-1100 1100 720 "body=weather&bar=idle"
shot ram-720 720 520 "body=ram&bar=idle"
shot capture-720 720 520 "body=ram&bar=capture"
shot files-720 720 520 "body=files&bar=idle"
shot ram-1920 1920 1080 "body=ram&bar=idle"
rm -rf "$out/.edge"
