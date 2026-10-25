@echo off
setlocal
cd /d "%~dp0"
title HOI4 Companion + HUD
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\avvia-tutto.ps1"
if errorlevel 1 (
    echo.
    echo Avvio non completato. Leggi il messaggio qui sopra.
    pause
)
