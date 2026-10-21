# Cross-platform policy fixture: signatures are mocked; no PE is executed.
$ErrorActionPreference = 'Stop'
function Start-Process { throw 'Pure verification must never execute a package' }
$signatureReads = [System.Collections.Generic.List[string]]::new()
function Get-AuthenticodeSignature {
    param([string]$LiteralPath)
    if (-not (Test-Path -LiteralPath $LiteralPath)) { throw 'Signature fixture file missing' }
    $signatureReads.Add($LiteralPath)
    return [pscustomobject]@{ Status = 'NotSigned'; TimeStamperCertificate = $null }
}
$temporary = Join-Path ([IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $temporary | Out-Null
$previousActions = $env:GITHUB_ACTIONS
$env:GITHUB_ACTIONS = 'false'
try {
    $payload = Join-Path $temporary 'payload'
    New-Item -ItemType Directory -Path $payload | Out-Null
    Set-Content -LiteralPath (Join-Path $payload 'snip2pin.exe') -Value 'inert payload'
    $installer = Join-Path $temporary 'inert-Setup.exe'
    Set-Content -LiteralPath $installer -Value 'inert setup'
    $archive = Join-Path $temporary 'fixture.zip'
    & "$PSScriptRoot/archive.ps1" $payload $archive
    & "$PSScriptRoot/verify.ps1" -Archive $archive -Installer $installer -Destination (Join-Path $temporary 'pure') -Distribution unverified
    if ($signatureReads.Count -ne 2) { throw 'Pure verification must inspect the app and installer signatures' }
    try {
        & "$PSScriptRoot/verify.ps1" -Archive $archive -Installer $installer -Destination (Join-Path $temporary 'signed') -Distribution signed
        throw 'Signed verification accepted unsigned fixture'
    } catch {
        if ($_.Exception.Message -notlike 'A valid Authenticode signature and timestamp are required:*') { throw }
    }
    try {
        & "$PSScriptRoot/verify.ps1" -Archive $archive -Installer $installer -Destination (Join-Path $temporary 'smoke') -Distribution unverified -InstallSmoke
        throw 'Install smoke accepted local execution'
    } catch {
        if ($_.Exception.Message -ne 'Windows install verification requires a GitHub-hosted runner') { throw }
    }
    Write-Output 'Pure verification fixtures passed without executing packages'
    function cargo {
        if (($args -join ' ') -ne 'xtask verify-candidate fixture x86_64-pc-windows-msvc') {
            throw 'Candidate source validation was not the first packaging operation'
        }
        throw 'Source validation fixture rejection'
    }
    try {
        & "$PSScriptRoot/distribute.ps1" -Candidate fixture -OutputDirectory (Join-Path $temporary 'never-built') -Distribution unverified
        throw 'Packaging accepted a rejected candidate'
    } catch {
        if ($_.Exception.Message -ne 'Source validation fixture rejection') { throw }
    }
} finally {
    $env:GITHUB_ACTIONS = $previousActions
    Remove-Item -LiteralPath $temporary -Recurse -Force
}
