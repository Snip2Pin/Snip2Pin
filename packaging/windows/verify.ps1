param(
    [Parameter(Mandatory)][string]$Archive,
    [Parameter(Mandatory)][string]$Installer,
    [Parameter(Mandatory)][string]$Destination,
    [Parameter(Mandatory)][ValidateSet('signed', 'unverified')][string]$Distribution,
    [switch]$InstallSmoke
)
$ErrorActionPreference = 'Stop'
function Assert-Signature([string]$Path) {
    $signature = Get-AuthenticodeSignature -LiteralPath $Path
    if ($Distribution -eq 'unverified') {
        if ($signature.Status -ne 'NotSigned') { throw "Expected an unsigned Windows executable: $Path" }
    } elseif ($signature.Status -ne 'Valid' -or $null -eq $signature.TimeStamperCertificate) {
        throw "A valid Authenticode signature and timestamp are required: $Path"
    }
    return $signature
}
function Invoke-Silent([string]$Path, [string]$Arguments) {
    $process = Start-Process -FilePath $Path -ArgumentList $Arguments -PassThru
    try {
        if (-not $process.WaitForExit(60000)) { throw 'Silent installer/uninstaller timed out' }
        if ($process.ExitCode -ne 0) { throw "Silent installer/uninstaller failed: $($process.ExitCode)" }
    } finally { $process.Dispose() }
}
# Embedded uninstallers are verified after a silent, temporary per-user install.
# Never alter a developer's installation or execute this check on a self-hosted runner.
if ($InstallSmoke -and ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted')) {
    throw 'Windows install verification requires a GitHub-hosted runner'
}
if (Test-Path -LiteralPath $Destination) { throw 'Verification destination must not exist' }
Expand-Archive -LiteralPath $Archive -DestinationPath $Destination
$appSignature = Assert-Signature (Join-Path $Destination 'snip2pin.exe')
$setupSignature = Assert-Signature $Installer
if ($Distribution -eq 'signed' -and $appSignature.SignerCertificate.Thumbprint -ne $setupSignature.SignerCertificate.Thumbprint) {
    throw 'Application and installer publishers differ'
}
if (-not $InstallSmoke) { return }
$registration = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Snip2Pin'
$startMenu = Join-Path ([Environment]::GetFolderPath('Programs')) 'Snip2Pin'
$desktopShortcut = Join-Path ([Environment]::GetFolderPath('DesktopDirectory')) 'Snip2Pin.lnk'
if ((Test-Path -LiteralPath $registration) -or (Test-Path -LiteralPath $startMenu) -or
    (Test-Path -LiteralPath $desktopShortcut)) { throw 'Refusing to modify an existing installation' }
$installed = Join-Path ([IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString())
$uninstaller = Join-Path $installed 'Uninstall.exe'
try {
    Invoke-Silent ([IO.Path]::GetFullPath($Installer)) "/S /D=$installed"
    foreach ($file in @('snip2pin.exe', 'LICENSE', 'README.txt', 'THIRD_PARTY_NOTICES.txt')) {
        $sourceHash = (Get-FileHash -LiteralPath (Join-Path $Destination $file) -Algorithm SHA256).Hash
        if ((Get-FileHash -LiteralPath (Join-Path $installed $file) -Algorithm SHA256).Hash -ne $sourceHash) {
            throw "Installer payload differs from portable archive: $file"
        }
    }
    $uninstallSignature = Assert-Signature $uninstaller
    if ($Distribution -eq 'signed' -and $appSignature.SignerCertificate.Thumbprint -ne $uninstallSignature.SignerCertificate.Thumbprint) {
        throw 'Application, installer and uninstaller publishers differ'
    }
    if ((Get-ItemProperty -LiteralPath $registration).InstallLocation -ne $installed -or
        -not (Test-Path -LiteralPath $desktopShortcut)) { throw 'Per-user installation metadata is incomplete' }
    $shell = New-Object -ComObject WScript.Shell
    foreach ($link in @((Join-Path $startMenu 'Snip2Pin.lnk'), $desktopShortcut)) {
        if (-not (Test-Path -LiteralPath $link) -or
            $shell.CreateShortcut($link).TargetPath -ne (Join-Path $installed 'snip2pin.exe')) {
            throw 'Application shortcut is missing or has an incorrect target'
        }
    }
    # An unrelated file must survive uninstall; settings outside this directory are untouched.
    Set-Content -LiteralPath (Join-Path $installed 'unrelated.txt') -Value 'preserve'
    Invoke-Silent $uninstaller "/S _?=$installed"
    if ((Test-Path -LiteralPath $registration) -or (Test-Path -LiteralPath (Join-Path $startMenu 'Snip2Pin.lnk')) -or
        (Test-Path -LiteralPath $desktopShortcut) -or
        (Test-Path -LiteralPath (Join-Path $installed 'snip2pin.exe')) -or
        -not (Test-Path -LiteralPath (Join-Path $installed 'unrelated.txt'))) { throw 'Per-user uninstall did not preserve the expected boundaries' }
} finally {
    if (Test-Path -LiteralPath $registration) {
        if (Test-Path -LiteralPath $uninstaller) { Invoke-Silent $uninstaller "/S _?=$installed" }
    }
    if (Test-Path -LiteralPath $installed) { Remove-Item -LiteralPath $installed -Recurse -Force }
}
