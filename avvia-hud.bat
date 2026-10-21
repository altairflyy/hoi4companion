@echo off
setlocal
cd /d "%~dp0"
start "" /b powershell.exe -NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -File "%~dp0scripts\hud.ps1"
