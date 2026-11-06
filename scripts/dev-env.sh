#!/usr/bin/env bash
# Prepara el entorno de compilación en Windows con Git Bash: `source scripts/dev-env.sh`.
# whisper-rs necesita CMake (incluido en Build Tools) y libclang (ver README).
export PATH="$PATH:$HOME/.cargo/bin"
for cmake_dir in "/c/Program Files (x86)/Microsoft Visual Studio/"*/*/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin; do
  [ -x "$cmake_dir/cmake.exe" ] && export PATH="$PATH:$cmake_dir"
done
for clang_dir in "$HOME"/.nexium-tools/llvm-*/bin; do
  [ -f "$clang_dir/libclang.dll" ] && LIBCLANG_PATH="$(cygpath -w "$clang_dir")" && export LIBCLANG_PATH
done
