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
    echo Startup failed. Check the preceding message for Python, dependencies, or the Rust parser.
    pause
)
