@echo off
setlocal
cd /d "%~dp0"

if "%~1"=="" (
    start "" powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "%~dp0launcher.ps1" -Mode gui
    exit /b %ERRORLEVEL%
) else (
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0launcher.ps1" -Mode %*
    exit /b %ERRORLEVEL%
)
