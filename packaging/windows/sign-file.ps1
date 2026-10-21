param([Parameter(Mandatory)][string]$Path)
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
if ($env:SNIP2PIN_SIGN_CERTIFICATE -notmatch '^[a-fA-F0-9]{40}$' -or
    $env:SNIP2PIN_SIGN_TIMESTAMP -notmatch '^https://') { throw 'Invalid signing configuration' }
signtool sign /sha1 $env:SNIP2PIN_SIGN_CERTIFICATE /fd SHA256 /tr $env:SNIP2PIN_SIGN_TIMESTAMP /td SHA256 $Path
if ($LASTEXITCODE -ne 0) { throw 'Authenticode signing failed' }
signtool verify /pa /all /v $Path
if ($LASTEXITCODE -ne 0) { throw 'Authenticode verification failed' }
$signature = Get-AuthenticodeSignature -LiteralPath $Path
if ($signature.Status -ne 'Valid' -or $null -eq $signature.TimeStamperCertificate -or
    $signature.SignerCertificate.Thumbprint -ne $env:SNIP2PIN_SIGN_CERTIFICATE) {
    throw 'Configured publisher signature and timestamp are required'
}
