param([switch]$NoInstall)
$ErrorActionPreference='Stop'
function Get-Sha256([string]$Path){
    $stream=[System.IO.File]::OpenRead($Path)
    try{
        $hash=[System.Security.Cryptography.SHA256]::Create().ComputeHash($stream)
        return [System.BitConverter]::ToString($hash).Replace("-", "").ToLowerInvariant()
    }finally{
        $stream.Close()
    }
}
$projectRoot=Split-Path -Parent $PSScriptRoot
$localCargo=Join-Path $projectRoot '.local\toolchains\cargo\bin\cargo.exe'
if(Test-Path -LiteralPath $localCargo){
    $cargo=$localCargo
    $env:CARGO_HOME=Join-Path $projectRoot '.local\toolchains\cargo'
    $env:RUSTUP_HOME=Join-Path $projectRoot '.local\toolchains\rustup'
}else{$cargo=(Get-Command cargo -ErrorAction Stop).Source}
$env:CARGO_TARGET_DIR=Join-Path $projectRoot '.local\rust-backend-target'
& $cargo build --release --locked --manifest-path (Join-Path $projectRoot 'native\backend\Cargo.toml')
if($LASTEXITCODE -ne 0){throw 'Rust backend build failed'}
$library=Join-Path $env:CARGO_TARGET_DIR 'release\hoi4companion_backend.dll'
$hostExecutable=Join-Path $env:CARGO_TARGET_DIR 'release\hoi4-backend.exe'
$desktopExecutable=Join-Path $env:CARGO_TARGET_DIR 'release\hoi4-desktop.exe'
if($NoInstall){Write-Output $library;Write-Output $hostExecutable;Write-Output $desktopExecutable;return}
$destination=Join-Path $projectRoot 'src\hoi4companion\_native\hoi4companion_backend.dll'
New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
if(-not (Test-Path -LiteralPath $destination) -or (Get-Sha256 $library) -ne (Get-Sha256 $destination)){Copy-Item -LiteralPath $library -Destination $destination -Force}
$hostDestination=Join-Path (Split-Path -Parent $destination) 'hoi4-backend.exe'
if(-not (Test-Path -LiteralPath $hostDestination) -or (Get-Sha256 $hostExecutable) -ne (Get-Sha256 $hostDestination)){Copy-Item -LiteralPath $hostExecutable -Destination $hostDestination -Force}
Write-Output $destination
Write-Output $hostDestination
$desktopDestination=Join-Path (Split-Path -Parent $destination) 'hoi4-desktop.exe'
if(-not (Test-Path -LiteralPath $desktopDestination) -or (Get-Sha256 $desktopExecutable) -ne (Get-Sha256 $desktopDestination)){Copy-Item -LiteralPath $desktopExecutable -Destination $desktopDestination -Force}
Write-Output $desktopDestination
