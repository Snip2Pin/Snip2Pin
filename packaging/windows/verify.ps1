param([Parameter(Mandatory)][string]$Archive, [Parameter(Mandatory)][string]$Destination)
$ErrorActionPreference = 'Stop'
Expand-Archive -LiteralPath $Archive -DestinationPath $Destination
$signature = Get-AuthenticodeSignature -LiteralPath (Join-Path $Destination 'snip2pin.exe')
if ($signature.Status -ne 'Valid' -or $null -eq $signature.TimeStamperCertificate) {
    throw 'A valid Authenticode signature and timestamp are required'
}
