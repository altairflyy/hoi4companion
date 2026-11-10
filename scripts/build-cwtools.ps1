param([string]$Output)
$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot
$dotnet=Join-Path $root '.local/toolchains/dotnet/dotnet.exe'
if(!(Test-Path -LiteralPath $dotnet)){$dotnet=(Get-Command dotnet -ErrorAction Stop).Source}
$source=Join-Path $root 'native/cwtools'
$provenance=Get-Content -LiteralPath (Join-Path $source 'vendor-provenance.json') -Raw | ConvertFrom-Json
foreach($entry in $provenance.files.PSObject.Properties){
    if((Get-FileHash -LiteralPath (Join-Path $source ('vendor/'+$entry.Name))).Hash.ToLowerInvariant() -ne $entry.Value){throw ('CWTools source hash differs: '+$entry.Name)}
}
$destination=if($Output){[IO.Path]::GetFullPath($Output)}else{Join-Path $root 'src/hoi4companion/_native/cwtools'}
$env:DOTNET_CLI_TELEMETRY_OPTOUT='1';$env:DOTNET_SKIP_FIRST_TIME_EXPERIENCE='1';$env:NUGET_PACKAGES=Join-Path $root '.local/nuget-packages'
Push-Location $source
try{& $dotnet publish Companion.Cwtools.fsproj -c Release -r win-x64 --self-contained true --nologo -p:RestoreLockedMode=true -o $destination;if($LASTEXITCODE){throw 'CWTools publish failed'}}finally{Pop-Location}
Copy-Item -LiteralPath (Join-Path $source 'vendor/LICENSE') -Destination (Join-Path $destination 'CWTools.LICENSE.txt')
$assets=Get-Content -LiteralPath (Join-Path $source 'obj/project.assets.json') -Raw | ConvertFrom-Json
$notices=@()
foreach($package in ($assets.libraries.PSObject.Properties | Sort-Object Name)){
    if($package.Value.type -ne 'package'){continue}
    $notice=@{package=$package.Name};$folder=Join-Path $env:NUGET_PACKAGES $package.Value.path
    $nuspec=Get-ChildItem -LiteralPath $folder -Filter '*.nuspec' -File | Select-Object -First 1
    if($nuspec){
        [xml]$xml=Get-Content -LiteralPath $nuspec.FullName -Raw
        foreach($element in $xml.SelectNodes('//*')){
            if($element.LocalName -in @('license','licenseUrl','projectUrl','copyright') -and $element.InnerText){$notice[$element.LocalName]=$element.InnerText}
            if($element.LocalName -eq 'license' -and $element.GetAttribute('type') -eq 'file'){
                $license=Join-Path $folder $element.InnerText
                if(Test-Path -LiteralPath $license){$name=$package.Name.Replace('/','-')+'-LICENSE.txt';Copy-Item -LiteralPath $license -Destination (Join-Path $destination $name);$notice.bundled_license=$name}
            }
        }
    }
    $notices+=,$notice
}
ConvertTo-Json -InputObject @($notices) -Depth 8 | Set-Content -LiteralPath (Join-Path $destination 'THIRD-PARTY-NOTICES.json') -Encoding utf8
$hashes=@{}
Get-ChildItem -LiteralPath $destination -Recurse -File | Where-Object Name -ne 'provenance.json' | ForEach-Object {$hashes[$_.FullName.Substring($destination.Length+1).Replace('\','/')]=(Get-FileHash -LiteralPath $_.FullName).Hash.ToLowerInvariant()}
@{schema_version=1;repository='https://github.com/cwtools/cwtools';revision=$provenance.revision;runtime='self-contained win-x64 .NET 9';parser_compatibility=1;reviewed_recovery=1;files=$hashes} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $destination 'provenance.json') -Encoding utf8
Write-Output $destination
