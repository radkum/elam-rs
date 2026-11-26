# Verifies that a built driver image actually contains the ELAM certificate
# resource: TYPE=MSElamCertInfoID, NAME=MicrosoftElamCertificateInfo.
#
# Complements check_elam_resource.ps1 (which only inspects the .rc text): this
# catches an embed_resource step that silently dropped the resource.

param(
    [string] $Path
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
if (-not $Path) { $Path = Join-Path $root 'target\debug\elam_rs.dll' }
if (-not (Test-Path -LiteralPath $Path)) { throw "driver binary not found: $Path" }
$Path = (Resolve-Path -LiteralPath $Path).Path

$src = @'
using System; using System.Collections.Generic; using System.Runtime.InteropServices;
namespace ElamResCheck {
  public static class PeResources {
    [DllImport("kernel32", CharSet=CharSet.Unicode, SetLastError=true)] static extern IntPtr LoadLibraryExW(string f, IntPtr h, uint flags);
    [DllImport("kernel32", SetLastError=true)] static extern bool FreeLibrary(IntPtr h);
    delegate bool EnumTypes(IntPtr h, IntPtr t, IntPtr l);
    delegate bool EnumNames(IntPtr h, IntPtr t, IntPtr n, IntPtr l);
    [DllImport("kernel32", CharSet=CharSet.Unicode, SetLastError=true)] static extern bool EnumResourceTypesW(IntPtr h, EnumTypes cb, IntPtr l);
    [DllImport("kernel32", CharSet=CharSet.Unicode, SetLastError=true)] static extern bool EnumResourceNamesW(IntPtr h, IntPtr t, EnumNames cb, IntPtr l);
    static string S(IntPtr p){ return ((long)p & 0xFFFF0000L)==0 ? ("#"+((long)p & 0xFFFF)) : Marshal.PtrToStringUni(p); }
    public static List<string> Dump(string file){
      var r = new List<string>();
      IntPtr h = LoadLibraryExW(file, IntPtr.Zero, 0x2);
      if (h == IntPtr.Zero) { r.Add("LOAD_FAILED;" + Marshal.GetLastWin32Error()); return r; }
      EnumResourceTypesW(h, (hm, t, l) => {
        var ts = S(t);
        EnumResourceNamesW(hm, t, (hm2, t2, n, l2) => { r.Add(ts + ";" + S(n)); return true; }, IntPtr.Zero);
        return true;
      }, IntPtr.Zero);
      FreeLibrary(h);
      return r;
    }
  }
}
'@

if (-not ('ElamResCheck.PeResources' -as [type])) {
    Add-Type -TypeDefinition $src
}

$entries = [ElamResCheck.PeResources]::Dump($Path)
$entries | ForEach-Object { Write-Host "  resource: $_" }

if (-not ($entries | Where-Object { $_ -match '(?i)^MSELAMCERTINFOID;MICROSOFTELAMCERTIFICATEINFO$' })) {
    throw "missing ELAM certificate resource (want TYPE=MSElamCertInfoID NAME=MicrosoftElamCertificateInfo) in $Path"
}

Write-Host "ELAM certificate resource present in $Path"
