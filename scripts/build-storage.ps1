param([switch]$NoInstall)
$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
$localCargo=Join-Path $projectRoot '.local\toolchains\cargo\bin\cargo.exe'
if(Test-Path -LiteralPath $localCargo){
    $cargo=$localCargo
    $env:CARGO_HOME=Join-Path $projectRoot '.local\toolchains\cargo'
    $env:RUSTUP_HOME=Join-Path $projectRoot '.local\toolchains\rustup'
}else{$cargo=(Get-Command cargo -ErrorAction Stop).Source}
$env:CARGO_TARGET_DIR=Join-Path $projectRoot '.local\rust-storage-target'
& $cargo build --release --locked --manifest-path (Join-Path $projectRoot 'native\storage\Cargo.toml')
if($LASTEXITCODE -ne 0){throw 'Rust storage build failed'}
$library=Join-Path $env:CARGO_TARGET_DIR 'release\hoi4companion_storage.dll'
if($NoInstall){Write-Output $library;return}
$destination=Join-Path $projectRoot 'src\hoi4companion\_native\hoi4companion_storage.dll'
New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
if((Test-Path -LiteralPath $destination) -and (Get-FileHash -LiteralPath $library).Hash -eq (Get-FileHash -LiteralPath $destination).Hash){Write-Output ('Already installed: '+$destination);return}
Copy-Item -LiteralPath $library -Destination $destination -Force
Write-Output $destination
