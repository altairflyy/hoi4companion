param([switch]$NoBrowser,[switch]$Stop)
$ErrorActionPreference='Stop'
$launchRoot=Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $launchRoot
$launchNative=Join-Path $launchRoot 'src\hoi4companion\_native\hoi4-desktop.exe'
if(-not (Test-Path -LiteralPath $launchNative)){
    Write-Error 'Native launcher missing. Build with scripts/build-backend.ps1 and scripts/build-overlay.ps1, or extract the complete Windows ZIP.'
    exit 1
}
$launchLocal=Join-Path $launchRoot '.local'
New-Item -ItemType Directory -Path $launchLocal -Force | Out-Null
$launchArgs=@('--port','8766')
if($NoBrowser){$launchArgs+='--no-browser'}
if($Stop){$launchArgs+='--stop'}
$launchProcess=Start-Process -FilePath $launchNative -ArgumentList $launchArgs -WorkingDirectory $launchRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $launchLocal 'launcher.log') -RedirectStandardError (Join-Path $launchLocal 'launcher-error.log')
if($launchProcess.ExitCode -ne 0){
    Get-Content -LiteralPath (Join-Path $launchLocal 'launcher-error.log') -ErrorAction SilentlyContinue
    Write-Error 'Companion did not complete the requested operation. See .local\launcher-error.log.'
    exit $launchProcess.ExitCode
}
if($Stop){Write-Host 'Companion stopped. HOI4 was not changed.'}
else{Write-Host 'Companion and HUD are ready. You can close this launch window.'}
