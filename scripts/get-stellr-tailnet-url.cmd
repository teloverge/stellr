@echo off
setlocal EnableExtensions

where powershell.exe >nul 2>nul
if errorlevel 1 (
  echo Windows PowerShell is required but powershell.exe was not found on PATH. 1>&2
  exit /b 1
)

powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0get-stellr-tailnet-url.ps1" %*
exit /b %errorlevel%
