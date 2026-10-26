param([switch]$NoBrowser)
$ErrorActionPreference = 'Stop'
$launchRoot = Split-Path -Parent $PSScriptRoot
$launchLocal = Join-Path $launchRoot '.local'
$launchBase = 'http://127.0.0.1:8766'
$launchLock = New-Object System.Threading.Mutex($false, 'Local\HOI4CompanionLaunch8766')
$launchOwned = $false
try {
    try { $launchOwned = $launchLock.WaitOne(0) }
    catch [System.Threading.AbandonedMutexException] { $launchOwned = $true }
    if (-not $launchOwned) {
        Write-Host 'Startup is already in progress. Wait for the first window.'
        exit 0
    }
    Set-Location -LiteralPath $launchRoot
    New-Item -ItemType Directory -Path $launchLocal -Force | Out-Null
    $env:PYTHONPATH = Join-Path $launchRoot 'src'
    $launchPython = Get-Command py.exe -ErrorAction SilentlyContinue
    $launchPrefix = @('-3')
    if (-not $launchPython) {
        $launchPython = Get-Command python.exe -ErrorAction Stop
        $launchPrefix = @()
    }
    Write-Host '1/3 - Starting HOI4 Companion...'
    $launchReady = $false
    try {
        $launchReply = Invoke-RestMethod ($launchBase + '/api/guides') -TimeoutSec 3
        $launchReady = $null -ne $launchReply.guides
    } catch {}
    if (-not $launchReady) {
        if (Get-NetTCPConnection -LocalPort 8766 -State Listen -ErrorAction SilentlyContinue) {
            throw 'Port 8766 is occupied but Companion is not responding. No process was stopped.'
        }
        $launchServer = Start-Process -FilePath $launchPython.Source -ArgumentList ($launchPrefix + @('-m', 'hoi4companion', '--no-browser', '--port', '8766')) -WorkingDirectory $launchRoot -WindowStyle Hidden -RedirectStandardOutput (Join-Path $launchLocal 'avvio-server.out.log') -RedirectStandardError (Join-Path $launchLocal 'avvio-server.err.log') -PassThru
        $launchDeadline = (Get-Date).AddMinutes(3)
        while ((Get-Date) -lt $launchDeadline) {
            try {
                $launchReply = Invoke-RestMethod ($launchBase + '/api/guides') -TimeoutSec 3
                if ($null -ne $launchReply.guides) { $launchReady = $true; break }
            } catch {}
            if ($launchServer.HasExited) { throw 'Il server si e fermato. Vedi .local\avvio-server.err.log.' }
            Start-Sleep -Milliseconds 500
        }
        if (-not $launchReady) { throw 'The server is not ready after 3 minutes. See .local\avvio-server.err.log; it may still be loading.' }
    }
    Write-Host '2/3 - Starting native HUD (button highlights)...'
    $launchOverlayPath = Join-Path $launchRoot 'src\hoi4companion\_native\overlay\hoi4-overlay.exe'
    if ($env:HOI4_OVERLAY_NATIVE) { $launchOverlayPath = $env:HOI4_OVERLAY_NATIVE }
    $launchOverlayPath = (Resolve-Path -LiteralPath $launchOverlayPath).Path
    $launchExisting = Get-CimInstance Win32_Process -Filter "Name='hoi4-overlay.exe'" | Where-Object {
        $_.ExecutablePath -eq $launchOverlayPath -and $_.CommandLine -match '(?:^|\s)--port\s+8766(?:\s|$)'
    }
    if (-not $launchExisting) {
        $launchOverlay = Start-Process -FilePath $launchPython.Source -ArgumentList ($launchPrefix + @('-m', 'hoi4companion.overlay_native', '--port', '8766', '--start')) -WorkingDirectory $launchRoot -WindowStyle Hidden -RedirectStandardOutput (Join-Path $launchLocal 'avvio-hud.out.log') -RedirectStandardError (Join-Path $launchLocal 'avvio-hud.err.log') -PassThru
        Start-Sleep -Seconds 2
        if ($launchOverlay.HasExited) { throw 'HUD did not start. See .local\avvio-hud.err.log.' }
    }
    Write-Host '3/3 - Opening the walkthrough...'
    if (-not $NoBrowser) { Start-Process ($launchBase + '/walkthrough') }
    Write-Host ''
    Write-Host 'Ready. Select the guide you want to follow in the HUD control window.' -ForegroundColor Green
    Write-Host 'Highlights appear with HOI4 in the foreground and a compatible campaign loaded.'
    Write-Host 'You can close this window: the application and HUD will keep running.'
    Start-Sleep -Seconds 3
} catch {
    Write-Host ('ERROR: ' + $_.Exception.Message) -ForegroundColor Red
    Write-Host ('Logs in: ' + $launchLocal)
    exit 1
} finally {
    if ($launchOwned) { $launchLock.ReleaseMutex() }
    $launchLock.Dispose()
}
