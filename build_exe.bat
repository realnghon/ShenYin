@echo off
setlocal

if not exist ".venv\Scripts\python.exe" (
  echo Please create the venv first.
  exit /b 1
)

tasklist /FI "IMAGENAME eq ShenYin.exe" | find /I "ShenYin.exe" >nul
if not errorlevel 1 (
  echo dist\ShenYin.exe is running. Close it before rebuilding.
  exit /b 1
)

.\.venv\Scripts\python -m pip install pyinstaller
if errorlevel 1 exit /b 1

.\.venv\Scripts\pyinstaller ^
  --noconfirm ^
  --clean ^
  local-workspace.spec
