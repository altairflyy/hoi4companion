param([Parameter(Mandatory=$true)][ValidateSet('Create','Extract')][string]$Mode,[Parameter(Mandatory=$true)][string]$Source,[Parameter(Mandatory=$true)][string]$Destination)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$sourcePath=(Resolve-Path -LiteralPath $Source).Path
$destinationPath=[IO.Path]::GetFullPath($Destination)
if($Mode -eq 'Create'){
    if(Test-Path -LiteralPath $destinationPath){throw 'ZIP output must be a new file'}
    [IO.Compression.ZipFile]::CreateFromDirectory($sourcePath,$destinationPath,[IO.Compression.CompressionLevel]::Optimal,$true)
}else{
    if(Test-Path -LiteralPath $destinationPath){throw 'Extraction destination must be new'}
    $zip=[IO.Compression.ZipFile]::OpenRead($sourcePath)
    try{
        $names=New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::OrdinalIgnoreCase)
        [long]$decoded=0
        if($zip.Entries.Count -gt 20000){throw 'ZIP contains too many entries'}
        foreach($entry in $zip.Entries){
            $name=$entry.FullName.Replace('\','/')
            if($name.StartsWith('/') -or $name.Contains(':') -or (($name -split '/') -contains '..') -or !$names.Add($name)){throw 'Unsafe or duplicate ZIP entry'}
            $decoded+=$entry.Length
            if($decoded -gt 2147483648){throw 'ZIP exceeds decoded size limit'}
            $target=[IO.Path]::GetFullPath((Join-Path $destinationPath $name))
            if(!$target.StartsWith($destinationPath+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'ZIP entry escapes destination'}
        }
    }finally{$zip.Dispose()}
    [IO.Compression.ZipFile]::ExtractToDirectory($sourcePath,$destinationPath)
}
