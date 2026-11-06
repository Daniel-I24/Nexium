# Prepara el entorno de compilación en PowerShell: . .\scripts\dev-env.ps1
# whisper-rs necesita CMake (incluido en Build Tools) y libclang (ver README).
$env:PATH += ";$env:USERPROFILE\.cargo\bin"
$cmake = Get-ChildItem "${env:ProgramFiles(x86)}\Microsoft Visual Studio\*\*\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
if ($cmake) { $env:PATH += ";$($cmake.DirectoryName)" }
$clang = Get-ChildItem "$env:USERPROFILE\.nexium-tools\llvm-*\bin\libclang.dll" -ErrorAction SilentlyContinue | Select-Object -First 1
if ($clang) { $env:LIBCLANG_PATH = $clang.DirectoryName }
