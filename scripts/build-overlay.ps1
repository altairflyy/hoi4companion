param([switch]$NoInstall,[string]$OutputDirectory)
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
}else{
    $cargo=(Get-Command cargo -ErrorAction Stop).Source
}
$env:CARGO_TARGET_DIR=Join-Path $projectRoot '.local\rust-backend-target'
& $cargo build --release --locked --manifest-path (Join-Path $projectRoot 'native\overlay\Cargo.toml')
if($LASTEXITCODE -ne 0){throw 'Rust overlay build failed'}
$executable=Join-Path $env:CARGO_TARGET_DIR 'release\hoi4-overlay.exe'
if(-not (Test-Path -LiteralPath $executable)){throw 'Rust overlay executable was not produced'}
if($NoInstall){Write-Output $executable;return}
$destination=if($OutputDirectory){[IO.Path]::GetFullPath($OutputDirectory)}else{Join-Path $projectRoot 'src\hoi4companion\_native\overlay'}
New-Item -ItemType Directory -Path $destination -Force | Out-Null
Copy-Item -LiteralPath $executable -Destination (Join-Path $destination 'hoi4-overlay.exe') -Force
$overlayHash=Get-Sha256 (Join-Path $destination 'hoi4-overlay.exe')
@{schema_version=1;engine='rust-native-hud';native_hud_protocol=1;files=@{'hoi4-overlay.exe'=$overlayHash}} |
    ConvertTo-Json -Depth 3 | Set-Content -LiteralPath (Join-Path $destination 'provenance.json') -Encoding utf8
Write-Output (Join-Path $destination 'hoi4-overlay.exe')
