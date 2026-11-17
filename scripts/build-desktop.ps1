param([switch]$NoInstall)
$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
$localCargo=Join-Path $projectRoot '.local\toolchains\cargo\bin\cargo.exe'
if(Test-Path -LiteralPath $localCargo){
    $cargo=$localCargo
    $env:CARGO_HOME=Join-Path $projectRoot '.local\toolchains\cargo'
    $env:RUSTUP_HOME=Join-Path $projectRoot '.local\toolchains\rustup'
}else{$cargo=(Get-Command cargo -ErrorAction Stop).Source}
# Keep the lightweight desktop artifacts separate from the backend cache.
$env:CARGO_TARGET_DIR=Join-Path $projectRoot '.local\rust-desktop-target'
& $cargo build --release --locked --manifest-path (Join-Path $projectRoot 'native\desktop\Cargo.toml')
if($LASTEXITCODE -ne 0){throw 'Rust desktop build failed'}
$executable=Join-Path $env:CARGO_TARGET_DIR 'release\hoi4-desktop.exe'
if(-not (Test-Path -LiteralPath $executable)){throw 'Rust desktop executable was not produced'}
if($NoInstall){Write-Output $executable;return}
$destination=Join-Path $projectRoot 'src\hoi4companion\_native\hoi4-desktop.exe'
New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
if(-not (Test-Path -LiteralPath $destination) -or (Get-FileHash -Algorithm SHA256 -LiteralPath $executable).Hash -ne (Get-FileHash -Algorithm SHA256 -LiteralPath $destination).Hash){Copy-Item -LiteralPath $executable -Destination $destination -Force}
Write-Output $destination
