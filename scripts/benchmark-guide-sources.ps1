param(
    [Parameter(Mandatory=$true)][string]$Requests,
    [string]$OutputPath
)

# Read source documents directly, without invoking the importer or extracting files.
# Coordinates are one-based and refer to the original Word table, row, cell and
# direct paragraph within the cell. Paragraphs may contain embedded newlines.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem

function Read-BoundedEntry($entry, [int]$limit) {
    if ($entry.Length -gt $limit) { throw "Entry exceeds $limit bytes: $($entry.FullName)" }
    $inputStream = $entry.Open()
    $outputStream = New-Object System.IO.MemoryStream
    try {
        $buffer = New-Object byte[] 65536
        while (($read = $inputStream.Read($buffer, 0, $buffer.Length)) -gt 0) {
            if ($outputStream.Length + $read -gt $limit) { throw "Entry exceeds $limit bytes: $($entry.FullName)" }
            $outputStream.Write($buffer, 0, $read)
        }
        return ,$outputStream.ToArray()
    } finally { $inputStream.Dispose(); $outputStream.Dispose() }
}

function Find-UniqueEntry($zip, [string]$name) {
    $matches = @($zip.Entries | Where-Object { $_.FullName -ceq $name })
    if ($matches.Count -ne 1) { throw "Expected exactly one ZIP entry '$name'; found $($matches.Count)" }
    return $matches[0]
}

function Get-BytesHash([byte[]]$bytes) {
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try { return ([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-', '').ToLowerInvariant() }
    finally { $sha.Dispose() }
}

function Get-ParagraphText($paragraph, $ns) {
    $builder = New-Object System.Text.StringBuilder
    foreach ($node in $paragraph.SelectNodes('.//*[self::w:t or self::w:tab or self::w:br or self::w:cr]', $ns)) {
        switch ($node.LocalName) {
            't' { [void]$builder.Append($node.InnerText) }
            'tab' { [void]$builder.Append("`t") }
            'br' { [void]$builder.Append("`n") }
            'cr' { [void]$builder.Append("`n") }
        }
    }
    return $builder.ToString()
}

function Get-CellText($cell, $ns) {
    $parts = @($cell.SelectNodes('./w:p', $ns) | ForEach-Object { Get-ParagraphText $_ $ns })
    return ($parts -join "`n")
}

function Normalize-Header([string]$text) {
    return (($text -replace '\s+', ' ').Trim().TrimEnd('*').Trim()).ToUpperInvariant()
}

function Read-Guide($request) {
    if (-not $request.guide_id -or -not $request.archive -or -not $request.entry) { throw 'Request requires guide_id, archive and entry' }
    if (-not [System.IO.Path]::IsPathRooted([string]$request.archive)) { throw 'Archive path must be absolute' }
    $file = [System.IO.File]::OpenRead([string]$request.archive)
    try {
        $archive = New-Object System.IO.Compression.ZipArchive($file, [System.IO.Compression.ZipArchiveMode]::Read, $false)
        try { $docxBytes = Read-BoundedEntry (Find-UniqueEntry $archive ([string]$request.entry)) (12 * 1024 * 1024) }
        finally { $archive.Dispose() }
    } finally { $file.Dispose() }

    $docxStream = New-Object System.IO.MemoryStream(,$docxBytes)
    try {
        $docx = New-Object System.IO.Compression.ZipArchive($docxStream, [System.IO.Compression.ZipArchiveMode]::Read, $false)
        try { $xmlBytes = Read-BoundedEntry (Find-UniqueEntry $docx 'word/document.xml') (8 * 1024 * 1024) }
        finally { $docx.Dispose() }
    } finally { $docxStream.Dispose() }

    $settings = New-Object System.Xml.XmlReaderSettings
    $settings.DtdProcessing = [System.Xml.DtdProcessing]::Prohibit
    $settings.XmlResolver = $null
    $xmlStream = New-Object System.IO.MemoryStream(,$xmlBytes)
    try {
        $reader = [System.Xml.XmlReader]::Create($xmlStream, $settings)
        try {
            $document = New-Object System.Xml.XmlDocument
            $document.XmlResolver = $null
            $document.PreserveWhitespace = $true
            $document.Load($reader)
        } finally { $reader.Dispose() }
    } finally { $xmlStream.Dispose() }
    $ns = New-Object System.Xml.XmlNamespaceManager($document.NameTable)
    $ns.AddNamespace('w', 'http://schemas.openxmlformats.org/wordprocessingml/2006/main')
    $paragraphs = New-Object 'System.Collections.Generic.List[object]'
    $priorityCount = 0
    $tables = @($document.SelectNodes('//w:tbl', $ns))
    for ($ti = 0; $ti -lt $tables.Count; $ti++) {
        $rows = @($tables[$ti].SelectNodes('./w:tr', $ns))
        $actionColumn = -1
        for ($ri = 0; $ri -lt $rows.Count; $ri++) {
            $cells = @($rows[$ri].SelectNodes('./w:tc', $ns))
            if ($actionColumn -lt 0) {
                $headers = @($cells | ForEach-Object { Normalize-Header (Get-CellText $_ $ns) })
                $candidate = [Array]::IndexOf($headers, 'ACTIONS')
                if ($candidate -ge 0 -and $headers -contains 'RESEARCH PRIORITY' -and $headers -contains 'FOCUS PRIORITY') {
                    $actionColumn = $candidate
                    $priorityCount++
                }
                continue
            }
            if ($actionColumn -ge $cells.Count) { continue }
            $nodes = @($cells[$actionColumn].SelectNodes('./w:p', $ns))
            for ($pi = 0; $pi -lt $nodes.Count; $pi++) {
                $value = Get-ParagraphText $nodes[$pi] $ns
                if (-not [string]::IsNullOrWhiteSpace($value)) {
                    $paragraphs.Add([pscustomobject]@{table_index=$ti+1; row_index=$ri+1; column_index=$actionColumn+1; paragraph_index=$pi+1; text=$value})
                }
            }
        }
    }
    return [pscustomobject]@{guide_id=[string]$request.guide_id; archive_entry=[string]$request.entry; document_sha256=(Get-BytesHash $docxBytes); document_xml_sha256=(Get-BytesHash $xmlBytes); paragraphs=@($paragraphs.ToArray()); priority_table_count=$priorityCount}
}

$requestsList = Get-Content -Raw -LiteralPath $Requests -Encoding UTF8 | ConvertFrom-Json
$results = foreach ($request in $requestsList) {
    try { Read-Guide $request }
    catch { [pscustomobject]@{guide_id=[string]$request.guide_id; archive_entry=[string]$request.entry; error=$_.Exception.Message} }
}
$json = ConvertTo-Json -InputObject @($results) -Depth 10
if ($OutputPath) { [System.IO.File]::WriteAllText([System.IO.Path]::GetFullPath($OutputPath), $json, (New-Object System.Text.UTF8Encoding($false))) }
else { [Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false); [Console]::Out.WriteLine($json) }
