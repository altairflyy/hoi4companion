param([switch]$NoInstall)
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
$env:CARGO_TARGET_DIR=Join-Path $projectRoot '.local\rust-target'
& $cargo build --release --locked --manifest-path (Join-Path $projectRoot 'native\parser\Cargo.toml')
if($LASTEXITCODE -ne 0){throw 'Compilazione del parser Rust fallita'}
$library=Join-Path $env:CARGO_TARGET_DIR 'release\hoi4companion_parser.dll'
if(-not $NoInstall){
    if(Get-Command py -ErrorAction SilentlyContinue){
        $extension=& py -3 -c "import sysconfig; print(sysconfig.get_config_var('EXT_SUFFIX'))"
    }else{
        $extension=& python -c "import sysconfig; print(sysconfig.get_config_var('EXT_SUFFIX'))"
    }
    if($LASTEXITCODE -ne 0 -or -not $extension){throw 'Impossibile individuare il formato della libreria Python'}
    $nativeDirectory=Join-Path $projectRoot 'src\hoi4companion\_native'
    New-Item -ItemType Directory -Path $nativeDirectory -Force | Out-Null
    $destination=Join-Path $nativeDirectory ('hoi4companion_parser'+$extension.Trim())
    Copy-Item -LiteralPath $library -Destination $destination -Force
    Write-Output $destination
}else{Write-Output $library}
