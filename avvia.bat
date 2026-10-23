@echo off
setlocal
cd /d "%~dp0"
set "PYTHONPATH=%~dp0src"
where py >nul 2>nul
if %errorlevel% equ 0 (
    py -3 -m hoi4companion %*
) else (
    python -m hoi4companion %*
)
if errorlevel 1 (
    echo.
    echo Avvio fallito. Controlla il messaggio precedente per Python, dipendenze o parser Rust.
    pause
)
