param(
    [Parameter(Mandatory)][string]$Candidate,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [Parameter(Mandatory)][string]$CertificateThumbprint,
    [Parameter(Mandatory)][string]$TimestampUrl
)
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
cargo xtask verify $Candidate
if ($LASTEXITCODE -ne 0) { throw 'Candidate validation failed' }
$manifest = Get-Content (Join-Path $Candidate 'x86_64-pc-windows-msvc.json') -Raw | ConvertFrom-Json
$temporary = Join-Path ([IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $temporary | Out-Null
try {
    Expand-Archive -LiteralPath (Join-Path $Candidate $manifest.file) -DestinationPath $temporary
    $binary = Join-Path $temporary 'snip2pin.exe'
    # Certificate/private key remains in the local certificate store or hardware provider.
    signtool sign /sha1 $CertificateThumbprint /fd SHA256 /tr $TimestampUrl /td SHA256 $binary
    if ($LASTEXITCODE -ne 0) { throw 'Authenticode signing failed' }
    signtool verify /pa /all /v $binary
    if ($LASTEXITCODE -ne 0) { throw 'Authenticode verification failed' }
    New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
    $archive = Join-Path $OutputDirectory ($manifest.file -replace '-unsigned\.zip$', '.zip')
    & "$PSScriptRoot/archive.ps1" $temporary $archive
    cargo xtask finalize $Candidate $archive
    if ($LASTEXITCODE -ne 0) { throw 'Final manifest failed' }
} finally { Remove-Item -LiteralPath $temporary -Recurse -Force }
