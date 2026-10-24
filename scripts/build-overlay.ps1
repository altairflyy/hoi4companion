param([switch]$NoInstall,[string]$OutputDirectory)
$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
$localCargo=Join-Path $projectRoot '.local\toolchains\cargo\bin\cargo.exe'
if(Test-Path -LiteralPath $localCargo){
    $cargo=$localCargo
    $env:CARGO_HOME=Join-Path $projectRoot '.local\toolchains\cargo'
    $env:RUSTUP_HOME=Join-Path $projectRoot '.local\toolchains\rustup'
}else{
    $cargo=(Get-Command cargo -ErrorAction Stop).Source
}
$env:CARGO_TARGET_DIR=Join-Path $projectRoot '.local\rust-overlay-target'
& $cargo build --release --locked --manifest-path (Join-Path $projectRoot 'native\overlay\Cargo.toml')
if($LASTEXITCODE -ne 0){throw 'Compilazione overlay Rust fallita'}
$executable=Join-Path $env:CARGO_TARGET_DIR 'release\hoi4-overlay.exe'
if(-not (Test-Path -LiteralPath $executable)){throw 'Eseguibile overlay Rust non prodotto'}
if($NoInstall){Write-Output $executable;return}
$destination=if($OutputDirectory){[IO.Path]::GetFullPath($OutputDirectory)}else{Join-Path $projectRoot 'src\hoi4companion\_native\overlay'}
New-Item -ItemType Directory -Path $destination -Force | Out-Null
Copy-Item -LiteralPath $executable -Destination (Join-Path $destination 'hoi4-overlay.exe') -Force
Write-Output (Join-Path $destination 'hoi4-overlay.exe')
