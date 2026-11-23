#!/usr/bin/env bash
# Falla si la parte de CPU de whisper.cpp (ggml-cpu) contiene instrucciones AVX-512 (registros zmm). La 1.0.0 se
# compiló en la CI con GGML_NATIVE y se cerraba al pulsar Hablar en procesadores sin AVX-512 (0xc000001d,
# instrucción ilegal). Solo se revisa ggml-cpu: otras bibliotecas tienen rutas AVX-512 que usan únicamente si el
# procesador las tiene. La biblioteca se localiza con los mensajes de Cargo, no buscando carpetas en target.
# Uso, desde src-tauri y después de compilar en release: ../scripts/check-cpu-baseline.sh [--target <triple>]
set -euo pipefail
messages=$(cargo build --release -p whisper-rs-sys "$@" --message-format=json 2>/dev/null)
out_dir=$(printf "%s\n" "$messages" | python3 -c "
import json, sys
dirs = [m['out_dir'] for m in map(json.loads, filter(str.strip, sys.stdin)) if m.get('reason') == 'build-script-executed' and 'whisper-rs-sys' in m['package_id']]
print(dirs[-1] if dirs else '')
" 2>/dev/null || printf "%s\n" "$messages" | python -c "
import json, sys
dirs = [m['out_dir'] for m in map(json.loads, filter(str.strip, sys.stdin)) if m.get('reason') == 'build-script-executed' and 'whisper-rs-sys' in m['package_id']]
print(dirs[-1] if dirs else '')
")
if [ -z "$out_dir" ]; then
  echo "Cargo no informó de la compilación de whisper-rs-sys." >&2
  exit 1
fi
lib=""
for candidate in "$out_dir/lib/ggml-cpu.lib" "$out_dir/lib/libggml-cpu.a" "$out_dir/lib64/libggml-cpu.a"; do
  [ -f "$candidate" ] && lib="$candidate" && break
done
if [ -z "$lib" ]; then
  echo "No está la biblioteca ggml-cpu en $out_dir/lib." >&2
  exit 1
fi
if [[ "$lib" == *.lib ]]; then
  vswhere="/c/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe"
  dumpbin=$("$vswhere" -latest -products '*' -find 'VC/Tools/MSVC/**/bin/Hostx64/x64/dumpbin.exe' | head -1 | tr -d '\r')
  listing=$(MSYS_NO_PATHCONV=1 "$dumpbin" /disasm:nobytes "$(cygpath -w "$lib" 2>/dev/null || echo "$lib")")
else
  listing=$(objdump -d --no-show-raw-insn "$lib")
fi
lines=$(printf "%s\n" "$listing" | wc -l)
zmm=$(printf "%s\n" "$listing" | grep -c "zmm" || true)
echo "ggml-cpu: $lines líneas desensambladas, $zmm con registros zmm (AVX-512)."
if [ "$lines" -lt 10000 ]; then
  echo "El desensamblado parece incompleto: no se puede comprobar." >&2
  exit 1
fi
if [ "$zmm" -ne 0 ]; then
  echo "ggml-cpu usa AVX-512: no funcionaría en procesadores sin esas instrucciones." >&2
  exit 1
fi
