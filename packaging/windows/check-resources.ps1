param([Parameter(Mandatory)][string]$Executable)
$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') { throw 'Windows resource inspection requires Windows' }
$path = (Resolve-Path -LiteralPath $Executable).Path
if (-not ('Snip2PinResourceProbe' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class Snip2PinResourceProbe {
    [DllImport("shell32.dll", CharSet = CharSet.Unicode, ExactSpelling = true)]
    public static extern uint ExtractIconExW(string file, int index, IntPtr large, IntPtr small, uint count);
}
'@
}
# Index -1 queries the embedded icon count without allocating icon handles.
$icons = [Snip2PinResourceProbe]::ExtractIconExW($path, -1, [IntPtr]::Zero, [IntPtr]::Zero, 0)
if ($icons -ne 2) { throw 'Expected separate application and notification-area icon groups' }
$version = [Diagnostics.FileVersionInfo]::GetVersionInfo($path)
if ($version.ProductName -ne 'Snip2Pin' -or $version.OriginalFilename -ne 'snip2pin.exe') {
    throw 'Application version resources are incomplete'
}
Write-Output 'Windows application icon and version resources verified'
