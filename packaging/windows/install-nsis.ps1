param([string]$Destination = (Join-Path $env:RUNNER_TEMP 'snip2pin-nsis'))
$ErrorActionPreference = 'Stop'
# Official NSIS 3.13 binary distribution, verified before executing its compiler.
$url = 'https://downloads.sourceforge.net/project/nsis/NSIS%203/3.13/nsis-3.13.zip'
$sha256 = 'ba63dffc4410ee89193e1cb5a41989991bd77c61068da17e3156d136b7b0b3d8'
if (Test-Path -LiteralPath $Destination) { throw 'NSIS destination must not exist' }
$archive = Join-Path ([IO.Path]::GetTempPath()) "$([guid]::NewGuid()).zip"
try {
    Invoke-WebRequest -Uri $url -OutFile $archive
    if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $sha256) { throw 'NSIS download hash mismatch' }
    Expand-Archive -LiteralPath $archive -DestinationPath $Destination
    $compilerDirectory = Join-Path $Destination 'nsis-3.13'
    if (-not (Test-Path -LiteralPath (Join-Path $compilerDirectory 'makensis.exe'))) { throw 'NSIS compiler missing' }
    $env:PATH = "$compilerDirectory;$env:PATH"
    if ($env:GITHUB_PATH) { Add-Content -LiteralPath $env:GITHUB_PATH -Value $compilerDirectory }
} finally { Remove-Item -LiteralPath $archive -Force -ErrorAction SilentlyContinue }
