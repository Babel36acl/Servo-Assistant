@echo off
rem Run a command inside the Visual Studio developer environment.
rem Why: a plain terminal has no MSVC/Windows SDK INCLUDE/LIB/PATH, so cl.exe
rem fails on native/bridge.c (winsock2.h not found) and cargo/tauri tasks in
rem VSCode can not run.
rem Toolset notes (this machine):
rem  - prefer Visual Studio 2022 (17.x); VS 18 preview Build Tools fails with
rem    cl error D8050 on large C files.
rem  - cl.exe/c1.dll still fails with D8050 when cargo runs it, so use clang-cl
rem    (MSVC ABI compatible, bundled with LLVM) for C/C++ compilation.
rem Usage: scripts\with-msvc.cmd <command> [args...]
rem   scripts\with-msvc.cmd cargo test --manifest-path src-tauri/Cargo.toml
rem   scripts\with-msvc.cmd pnpm tauri dev
setlocal EnableExtensions EnableDelayedExpansion

set "PF=%ProgramFiles%"
set "PF86=%ProgramFiles(x86)%"
set "VSPATH="

if not defined VSPATH if exist "!PF!\Microsoft Visual Studio\2022\Community\Common7\Tools\VsDevCmd.bat" set "VSPATH=!PF!\Microsoft Visual Studio\2022\Community"
if not defined VSPATH if exist "!PF!\Microsoft Visual Studio\2022\Professional\Common7\Tools\VsDevCmd.bat" set "VSPATH=!PF!\Microsoft Visual Studio\2022\Professional"
if not defined VSPATH if exist "!PF!\Microsoft Visual Studio\2022\Enterprise\Common7\Tools\VsDevCmd.bat" set "VSPATH=!PF!\Microsoft Visual Studio\2022\Enterprise"
if not defined VSPATH if exist "!PF86!\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat" set "VSPATH=!PF86!\Microsoft Visual Studio\2022\BuildTools"

if not defined VSPATH (
  set "VSW=!PF86!\Microsoft Visual Studio\Installer\vswhere.exe"
  if exist "!VSW!" for /f "usebackq tokens=*" %%i in (`"!VSW!" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "VSPATH=%%i"
)

if not defined VSPATH (
  echo [with-msvc] Visual Studio 2022 / Build Tools not found. Install "Desktop development with C++".
  exit /b 1
)

@call "!VSPATH!\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64 >nul
@echo off
if errorlevel 1 (
  echo [with-msvc] failed to init VsDevCmd: !VSPATH!
  exit /b 1
)

where clang-cl >nul 2>nul
if not errorlevel 1 (
  set "CC=clang-cl"
  set "CXX=clang-cl"
)

rem Keep scratch files inside the repo: some sandboxed/agent shells deny writes
rem to %%TEMP%%, which makes cargo tests fail with "Permission denied (os error 5)".
if not defined SA_KEEP_TMP (
  if not exist "%CD%\.tmp" mkdir "%CD%\.tmp" >nul 2>nul
  set "TMP=%CD%\.tmp"
  set "TEMP=%CD%\.tmp"
)

%*
exit /b %errorlevel%

