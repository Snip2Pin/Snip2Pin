$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
Get-ChildItem "$PSScriptRoot/*.ps1" | ForEach-Object {
    $errors = $null
    $tokens = $null
    [System.Management.Automation.Language.Parser]::ParseFile($_.FullName, [ref]$tokens, [ref]$errors) | Out-Null
    if ($errors.Count -gt 0) { throw "PowerShell syntax error: $($_.Name)" }
}
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'Installer fixture tests require a GitHub-hosted runner'
}
$temporary = Join-Path $env:RUNNER_TEMP ([guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $temporary | Out-Null
try {
    $payload = Join-Path $temporary 'payload'
    New-Item -ItemType Directory -Path $payload | Out-Null
    # Generated inert PE payload: the installer never launches this executable.
    $source = Join-Path $temporary 'fixture.rs'
    Set-Content -LiteralPath $source -Value 'fn main() {}'
    & rustc --edition=2024 $source -o (Join-Path $payload 'snip2pin.exe')
    if ($LASTEXITCODE -ne 0) { throw 'Fixture executable compile failed' }
    foreach ($name in @('LICENSE', 'README.txt', 'THIRD_PARTY_NOTICES.txt')) {
        Set-Content -LiteralPath (Join-Path $payload $name) -Value "fixture $name"
    }
    $installer = Join-Path $temporary 'fixture-Setup.exe'
    $archive = Join-Path $temporary 'fixture.zip'
    & "$PSScriptRoot/archive.ps1" $payload $archive
    & makensis /V2 "/DPAYLOAD=$payload" "/DOUTPUT=$installer" /DVERSION=0.0.0 "$PSScriptRoot/installer.nsi"
    if ($LASTEXITCODE -ne 0) { throw 'Fixture NSIS compile failed' }
    & "$PSScriptRoot/verify.ps1" -Archive $archive -Installer $installer -Destination (Join-Path $temporary 'verify') -Distribution unverified
    try {
        & "$PSScriptRoot/verify.ps1" -Archive $archive -Installer $installer -Destination (Join-Path $temporary 'reject-unsigned') -Distribution signed
        throw 'Signed verification accepted an unsigned fixture'
    } catch {
        if ($_.Exception.Message -notlike 'A valid Authenticode signature and timestamp are required:*') { throw }
    }

    # The compiler must reject a failed embedded-uninstaller finalization hook.
    $failingHook = Join-Path $temporary 'reject-signature.ps1'
    Set-Content -LiteralPath $failingHook -Value 'exit 27'
    $PSNativeCommandUseErrorActionPreference = $false
    & makensis /V2 "/DPAYLOAD=$payload" "/DOUTPUT=$temporary/rejected-Setup.exe" /DVERSION=0.0.0 "/DSIGN_SCRIPT=$failingHook" "$PSScriptRoot/installer.nsi"
    if ($LASTEXITCODE -eq 0) { throw 'NSIS accepted a failed uninstaller signing hook' }
} finally { Remove-Item -LiteralPath $temporary -Recurse -Force }
