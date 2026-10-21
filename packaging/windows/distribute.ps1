param(
    [Parameter(Mandatory)][string]$Candidate,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [ValidateSet('signed', 'unverified')][string]$Distribution = 'signed',
    [string]$CertificateThumbprint,
    [string]$TimestampUrl
)
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
cargo xtask verify-candidate $Candidate x86_64-pc-windows-msvc
if ($LASTEXITCODE -ne 0) { throw 'Candidate validation failed' }
$manifest = Get-Content (Join-Path $Candidate 'x86_64-pc-windows-msvc.json') -Raw | ConvertFrom-Json
$candidateArchive = @($manifest.artifacts | Where-Object format -eq 'zip')
if ($candidateArchive.Count -ne 1 -or $manifest.version -notmatch '^\d+\.\d+\.\d+$') { throw 'Invalid Windows candidate' }
if ($Distribution -eq 'signed' -and ($CertificateThumbprint -notmatch '^[a-fA-F0-9]{40}$' -or $TimestampUrl -notmatch '^https://')) {
    throw 'Signed mode requires a certificate fingerprint and HTTPS timestamp URL'
}
$temporary = Join-Path ([IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString())
$previousCertificate = $env:SNIP2PIN_SIGN_CERTIFICATE
$previousTimestamp = $env:SNIP2PIN_SIGN_TIMESTAMP
New-Item -ItemType Directory -Path $temporary | Out-Null
try {
    Expand-Archive -LiteralPath (Join-Path $Candidate $candidateArchive[0].file) -DestinationPath $temporary
    $expectedFiles = @('LICENSE', 'README.txt', 'snip2pin.exe', 'THIRD_PARTY_NOTICES.txt') | Sort-Object
    $actualFiles = @(Get-ChildItem -LiteralPath $temporary -Recurse -File | ForEach-Object { [IO.Path]::GetRelativePath($temporary, $_.FullName) }) | Sort-Object
    if (Compare-Object $expectedFiles $actualFiles) { throw 'Unexpected Windows candidate payload' }
    $binary = Join-Path $temporary 'snip2pin.exe'
    if ((Get-AuthenticodeSignature -LiteralPath $binary).Status -ne 'NotSigned') { throw 'Candidate must contain an unsigned executable' }
    if ($Distribution -eq 'signed') {
        $env:SNIP2PIN_SIGN_CERTIFICATE = $CertificateThumbprint
        $env:SNIP2PIN_SIGN_TIMESTAMP = $TimestampUrl
        & "$PSScriptRoot/sign-file.ps1" -Path $binary
    }
    New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
    $suffix = if ($Distribution -eq 'unverified') { '-unverified' } else { '' }
    $stem = "Snip2Pin-$($manifest.version)-x86_64-pc-windows-msvc$suffix"
    $archive = Join-Path ([IO.Path]::GetFullPath($OutputDirectory)) "$stem.zip"
    $installer = Join-Path ([IO.Path]::GetFullPath($OutputDirectory)) "$stem-Setup.exe"
    if ((Test-Path -LiteralPath $archive) -or (Test-Path -LiteralPath $installer)) { throw 'Refusing to overwrite final packages' }
    & "$PSScriptRoot/archive.ps1" $temporary $archive
    $definitions = @("/DPAYLOAD=$temporary", "/DOUTPUT=$installer", "/DVERSION=$($manifest.version)")
    if ($Distribution -eq 'signed') { $definitions += "/DSIGN_SCRIPT=$PSScriptRoot/sign-file.ps1" }
    & makensis /V2 @definitions "$PSScriptRoot/installer.nsi"
    if ($LASTEXITCODE -ne 0) { throw 'NSIS installer build failed' }
    if ($Distribution -eq 'signed') { & "$PSScriptRoot/sign-file.ps1" -Path $installer }
    & "$PSScriptRoot/verify.ps1" -Archive $archive -Installer $installer -Destination (Join-Path $temporary 'verify') -Distribution $Distribution
    cargo xtask finalize $Candidate $OutputDirectory $Distribution
    if ($LASTEXITCODE -ne 0) { throw 'Final manifest failed' }
} finally {
    $env:SNIP2PIN_SIGN_CERTIFICATE = $previousCertificate
    $env:SNIP2PIN_SIGN_TIMESTAMP = $previousTimestamp
    Remove-Item -LiteralPath $temporary -Recurse -Force
}
