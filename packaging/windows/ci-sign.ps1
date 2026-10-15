param(
    [Parameter(Mandatory)][string]$Candidate,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'CI signing requires a GitHub-hosted runner'
}
foreach ($name in @('WINDOWS_CERTIFICATE_PFX_BASE64', 'WINDOWS_CERTIFICATE_PASSWORD', 'WINDOWS_CERTIFICATE_SHA1', 'WINDOWS_TIMESTAMP_URL')) {
    if ([string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable($name))) { throw "Missing signing configuration: $name" }
}
if ($env:WINDOWS_CERTIFICATE_SHA1 -notmatch '^[a-fA-F0-9]{40}$') { throw 'Invalid certificate fingerprint' }
$certificatePath = "Cert:\CurrentUser\My\$env:WINDOWS_CERTIFICATE_SHA1"
if (Test-Path $certificatePath) { throw 'Refusing to replace an existing identity' }
$temporary = Join-Path $env:RUNNER_TEMP ([guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $temporary | Out-Null
$existing = @(Get-ChildItem Cert:\CurrentUser\My | ForEach-Object { $_.Thumbprint })
try {
    $pfx = Join-Path $temporary 'certificate.pfx'
    [IO.File]::WriteAllBytes($pfx, [Convert]::FromBase64String($env:WINDOWS_CERTIFICATE_PFX_BASE64))
    $password = ConvertTo-SecureString $env:WINDOWS_CERTIFICATE_PASSWORD -AsPlainText -Force
    $imported = @(Import-PfxCertificate -FilePath $pfx -Password $password -CertStoreLocation Cert:\CurrentUser\My)
    $identity = @($imported | Where-Object { $_.HasPrivateKey })
    if ($identity.Count -ne 1 -or $identity[0].Thumbprint -ne $env:WINDOWS_CERTIFICATE_SHA1) {
        throw 'Expected exactly the configured private-key identity'
    }
    # Use the SDK already present on the runner, without downloading executable tools.
    $sdkRoot = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
    $tool = Get-ChildItem "$sdkRoot\*\x64\signtool.exe" | Sort-Object { [version]$_.Directory.Parent.Name } -Descending | Select-Object -First 1
    if ($null -eq $tool) { throw 'Windows SDK SignTool is unavailable' }
    $env:PATH = "$($tool.Directory.FullName);$env:PATH"
    & "$PSScriptRoot/distribute.ps1" -Candidate $Candidate -OutputDirectory $OutputDirectory -CertificateThumbprint $env:WINDOWS_CERTIFICATE_SHA1 -TimestampUrl $env:WINDOWS_TIMESTAMP_URL
} finally {
    try {
        foreach ($certificate in @(Get-ChildItem Cert:\CurrentUser\My | Where-Object { $_.Thumbprint -notin $existing })) {
            if ($certificate.HasPrivateKey) {
                Remove-Item -LiteralPath "Cert:\CurrentUser\My\$($certificate.Thumbprint)" -DeleteKey -Force
            } else {
                Remove-Item -LiteralPath "Cert:\CurrentUser\My\$($certificate.Thumbprint)" -Force
            }
        }
    } finally { Remove-Item -LiteralPath $temporary -Recurse -Force }
}
