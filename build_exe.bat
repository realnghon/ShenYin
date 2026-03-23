@echo off
setlocal

if not exist ".venv\Scripts\python.exe" (
  echo Please create the venv first.
  exit /b 1
)

tasklist /FI "IMAGENAME eq local-workspace.exe" | find /I "local-workspace.exe" >nul
if not errorlevel 1 (
  echo dist\local-workspace.exe is running. Close it before rebuilding.
  exit /b 1
)

.\.venv\Scripts\python -m pip install pyinstaller
if errorlevel 1 exit /b 1

.\.venv\Scripts\pyinstaller ^
  --noconfirm ^
  --clean ^
  local-workspace.spec
