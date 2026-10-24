param([string]$Output)
$ErrorActionPreference='Stop'
$script=Join-Path $PSScriptRoot 'build-cwtools.py'
if($Output){ & py -3 $script --output $Output }else{ & py -3 $script }
if($LASTEXITCODE -ne 0){throw 'Compilazione del motore CWTools fallita'}
