param(
    [Parameter(Mandatory)][string]$Archive,
    [Parameter(Mandatory)][string]$Destination,
    [Parameter(Mandatory)][ValidateSet('signed', 'unverified')][string]$Distribution
)
$ErrorActionPreference = 'Stop'
Expand-Archive -LiteralPath $Archive -DestinationPath $Destination
$signature = Get-AuthenticodeSignature -LiteralPath (Join-Path $Destination 'snip2pin.exe')
if ($Distribution -eq 'unverified') {
    if ($signature.Status -ne 'NotSigned') { throw 'Expected an unsigned Windows executable' }
} elseif ($signature.Status -ne 'Valid' -or $null -eq $signature.TimeStamperCertificate) {
    throw 'A valid Authenticode signature and timestamp are required'
}
