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
function Invoke-Fixture([string]$Path, [string]$Arguments) {
    $process = Start-Process -FilePath $Path -ArgumentList $Arguments -PassThru
    try {
        if (-not $process.WaitForExit(60000)) { throw 'Fixture installer/uninstaller timed out' }
        return $process.ExitCode
    } finally { $process.Dispose() }
}
$temporary = Join-Path $env:RUNNER_TEMP ([guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $temporary | Out-Null
try {
    $payload = Join-Path $temporary 'payload'
    New-Item -ItemType Directory -Path $payload | Out-Null
    # Generated inert PE payload: the installer never launches this executable.
    $source = Join-Path $temporary 'fixture.rs'
    Set-Content -LiteralPath $source -Value 'fn main() { std::fs::write(std::env::current_exe().unwrap().with_extension("launched"), b"unexpected launch").unwrap(); }'
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
    & "$PSScriptRoot/verify.ps1" -Archive $archive -Installer $installer -Destination (Join-Path $temporary 'verify-pure') -Distribution unverified
    & "$PSScriptRoot/verify.ps1" -Archive $archive -Installer $installer -Destination (Join-Path $temporary 'verify') -Distribution unverified -InstallSmoke
    try {
        & "$PSScriptRoot/verify.ps1" -Archive $archive -Installer $installer -Destination (Join-Path $temporary 'reject-unsigned') -Distribution signed
        throw 'Signed verification accepted an unsigned fixture'
    } catch {
        if ($_.Exception.Message -notlike 'A valid Authenticode signature and timestamp are required:*') { throw }
    }

    # Refuse an upgrade while the old application cannot be written, before
    # changing any payload, registration, uninstaller or Start Menu shortcut.
    $installPath = Join-Path $temporary 'upgrade with spaces'
    $registration = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Snip2Pin'
    $shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'Snip2Pin/Snip2Pin.lnk'
    $uninstaller = Join-Path $installPath 'Uninstall.exe'
    try {
        if ((Invoke-Fixture $installer "/S /D=$installPath") -ne 0) { throw 'Initial fixture installation failed' }
        Set-Content -LiteralPath (Join-Path $installPath 'unrelated.txt') -Value 'preserve'
        $installedFiles = @('snip2pin.exe', 'LICENSE', 'README.txt', 'THIRD_PARTY_NOTICES.txt', 'Uninstall.exe')
        $hashes = @{}
        foreach ($file in $installedFiles) { $hashes[$file] = (Get-FileHash -LiteralPath (Join-Path $installPath $file)).Hash }
        $shortcutHash = (Get-FileHash -LiteralPath $shortcut).Hash
        $metadata = Get-ItemProperty -LiteralPath $registration | ConvertTo-Json -Compress

        Set-Content -LiteralPath $source -Value 'fn main() { std::fs::write(std::env::current_exe().unwrap().with_extension("launched"), b"unexpected upgrade launch").unwrap(); }'
        & rustc --edition=2024 $source -o (Join-Path $payload 'snip2pin.exe')
        if ($LASTEXITCODE -ne 0) { throw 'Upgrade fixture executable compile failed' }
        Set-Content -LiteralPath (Join-Path $payload 'README.txt') -Value 'upgraded fixture'
        $upgrade = Join-Path $temporary 'upgrade-Setup.exe'
        & makensis /V2 "/DPAYLOAD=$payload" "/DOUTPUT=$upgrade" /DVERSION=0.0.1 "$PSScriptRoot/installer.nsi"
        if ($LASTEXITCODE -ne 0) { throw 'Upgrade fixture NSIS compile failed' }

        foreach ($blocked in @('snip2pin.exe', 'README.txt')) {
            $held = [IO.File]::Open((Join-Path $installPath $blocked), [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
            try {
                if ((Invoke-Fixture $upgrade "/S /D=$installPath") -eq 0) { throw "Upgrade accepted a non-writable file: $blocked" }
                foreach ($file in $installedFiles) {
                    if ((Get-FileHash -LiteralPath (Join-Path $installPath $file)).Hash -ne $hashes[$file]) { throw "Rejected upgrade changed $file" }
                }
                if ((Get-FileHash -LiteralPath $shortcut).Hash -ne $shortcutHash -or
                    (Get-ItemProperty -LiteralPath $registration | ConvertTo-Json -Compress) -ne $metadata) {
                    throw 'Rejected upgrade changed registration or shortcut'
                }
            } finally { $held.Dispose() }
        }

        if ((Invoke-Fixture $upgrade "/S /D=$installPath") -ne 0) { throw 'Normal fixture upgrade failed' }
        foreach ($file in $installedFiles | Where-Object { $_ -ne 'Uninstall.exe' }) {
            if ((Get-FileHash -LiteralPath (Join-Path $installPath $file)).Hash -ne
                (Get-FileHash -LiteralPath (Join-Path $payload $file)).Hash) { throw "Upgrade payload mismatch: $file" }
        }
        if ((Get-ItemProperty -LiteralPath $registration).DisplayVersion -ne '0.0.1' -or
            (Test-Path -LiteralPath (Join-Path $installPath 'snip2pin.launched'))) { throw 'Upgrade metadata or launch behavior is incorrect' }
        if ((Invoke-Fixture $uninstaller "/S _?=$installPath") -ne 0) { throw 'Upgraded fixture uninstall failed' }
        if ((Test-Path -LiteralPath $registration) -or (Test-Path -LiteralPath $shortcut) -or
            (Test-Path -LiteralPath (Join-Path $installPath 'snip2pin.exe')) -or
            -not (Test-Path -LiteralPath (Join-Path $installPath 'unrelated.txt'))) { throw 'Upgrade/uninstall boundaries failed' }
    } finally {
        if ((Test-Path -LiteralPath $registration) -and (Test-Path -LiteralPath $uninstaller)) {
            if ((Invoke-Fixture $uninstaller "/S _?=$installPath") -ne 0) { throw 'Fixture cleanup uninstall failed' }
        }
    }

    # The compiler must reject a failed embedded-uninstaller finalization hook.
    $failingHook = Join-Path $temporary 'reject-signature.ps1'
    Set-Content -LiteralPath $failingHook -Value 'exit 27'
    $PSNativeCommandUseErrorActionPreference = $false
    & makensis /V2 "/DPAYLOAD=$payload" "/DOUTPUT=$temporary/rejected-Setup.exe" /DVERSION=0.0.0 "/DSIGN_SCRIPT=$failingHook" "$PSScriptRoot/installer.nsi"
    if ($LASTEXITCODE -eq 0) { throw 'NSIS accepted a failed uninstaller signing hook' }
} finally { Remove-Item -LiteralPath $temporary -Recurse -Force }
Write-Output 'Windows installation, upgrade, uninstall and signing-failure fixtures passed'
# The final native command intentionally fails; do not propagate its expected
# exit code through the GitHub PowerShell wrapper after all assertions passed.
exit 0
