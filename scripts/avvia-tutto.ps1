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
        Write-Host 'Avvio gia in corso. Attendi la prima finestra.'
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
    Write-Host '1/3 - Apertura HOI4 Companion...'
    $launchReady = $false
    try {
        $launchReply = Invoke-RestMethod ($launchBase + '/api/guides') -TimeoutSec 3
        $launchReady = $null -ne $launchReply.guides
    } catch {}
    if (-not $launchReady) {
        if (Get-NetTCPConnection -LocalPort 8766 -State Listen -ErrorAction SilentlyContinue) {
            throw 'La porta 8766 e occupata, ma non risponde il Companion. Non e stato chiuso alcun processo.'
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
        if (-not $launchReady) { throw 'Il server non e pronto dopo 3 minuti. Vedi .local\avvio-server.err.log; potrebbe essere ancora in caricamento.' }
    }
    Write-Host '2/3 - Attivazione HUD nativo (evidenziazioni sui pulsanti)...'
    $launchOverlayPath = Join-Path $launchRoot 'src\hoi4companion\_native\overlay\hoi4-overlay.exe'
    if ($env:HOI4_OVERLAY_NATIVE) { $launchOverlayPath = $env:HOI4_OVERLAY_NATIVE }
    $launchOverlayPath = (Resolve-Path -LiteralPath $launchOverlayPath).Path
    $launchExisting = Get-CimInstance Win32_Process -Filter "Name='hoi4-overlay.exe'" | Where-Object {
        $_.ExecutablePath -eq $launchOverlayPath -and $_.CommandLine -match '(?:^|\s)--port\s+8766(?:\s|$)'
    }
    if (-not $launchExisting) {
        $launchOverlay = Start-Process -FilePath $launchPython.Source -ArgumentList ($launchPrefix + @('-m', 'hoi4companion.overlay_native', '--port', '8766', '--start')) -WorkingDirectory $launchRoot -WindowStyle Hidden -RedirectStandardOutput (Join-Path $launchLocal 'avvio-hud.out.log') -RedirectStandardError (Join-Path $launchLocal 'avvio-hud.err.log') -PassThru
        Start-Sleep -Seconds 2
        if ($launchOverlay.HasExited) { throw 'HUD non avviato. Vedi .local\avvio-hud.err.log.' }
    }
    Write-Host '3/3 - Apertura walkthrough...'
    if (-not $NoBrowser) { Start-Process ($launchBase + '/walkthrough') }
    Write-Host ''
    Write-Host 'Pronto. Nel controllo HUD seleziona la guida che vuoi seguire.' -ForegroundColor Green
    Write-Host 'Le evidenziazioni compaiono con HOI4 in primo piano e una partita compatibile.'
    Write-Host 'Puoi chiudere questa finestra: programma e HUD restano aperti.'
    Start-Sleep -Seconds 3
} catch {
    Write-Host ('ERRORE: ' + $_.Exception.Message) -ForegroundColor Red
    Write-Host ('Log nella cartella: ' + $launchLocal)
    exit 1
} finally {
    if ($launchOwned) { $launchLock.ReleaseMutex() }
    $launchLock.Dispose()
}
