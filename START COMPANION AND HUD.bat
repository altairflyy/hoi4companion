@echo off
setlocal
cd /d "%~dp0"
set "launchNative=%~dp0src\hoi4companion\_native\hoi4-desktop.exe"
if not exist "%launchNative%" (
    echo Native launcher missing. Extract the complete application or build the native components.
    pause
    exit /b 1
)
rem Keep the two legacy PowerShell switches, including their combination.
set "launchArgs="
:arguments
if "%~1"=="" goto launch
if /I "%~1"=="-NoBrowser" (
    set "launchArgs=%launchArgs% --no-browser"
) else if /I "%~1"=="-Stop" (
    set "launchArgs=%launchArgs% --stop"
) else (
    set "launchArgs=%launchArgs% "%~1""
)
shift /1
goto arguments
:launch
rem The native GUI launcher reports errors itself. Do not keep a console waiting.
start "" /D "%~dp0" "%launchNative%" %launchArgs%
exit /b
