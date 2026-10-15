param([Parameter(Mandatory)][string]$Source, [Parameter(Mandatory)][string]$Destination)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$sourcePath = (Resolve-Path $Source).Path
$destinationPath = [IO.Path]::GetFullPath($Destination)
[IO.Compression.ZipFile]::CreateFromDirectory($sourcePath, $destinationPath)
$archive = [IO.Compression.ZipFile]::OpenRead($destinationPath)
try {
    if (-not ($archive.Entries | Where-Object FullName -eq 'snip2pin.exe')) {
        throw 'Package is missing snip2pin.exe'
    }
} finally { $archive.Dispose() }
