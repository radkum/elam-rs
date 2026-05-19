# install.ps1 - run as Administrator on the test VM
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File .\install.ps1
#
# Safe to run multiple times.
# REBOOT after this script, then: sc start ElamPplTest

#Requires -RunAsAdministrator

$ErrorActionPreference = 'Stop'
$dir    = Split-Path -Parent $MyInvocation.MyCommand.Path
$sysSrc = Join-Path $dir 'elam_rs.sys'
$pfxPath= Join-Path $dir 'elam_rs.pfx'
$pplExe = Join-Path $dir 'ppl_test.exe'

foreach ($f in $sysSrc, $pfxPath, $pplExe) {
    if (-not (Test-Path $f)) { throw "Missing: $f" }
}

# 1. Import cert to Root and TrustedPublisher
Write-Host "[1/4] Importing certificate..."
$pfxPass = ConvertTo-SecureString -String 'password' -Force -AsPlainText
$cert    = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2(
               $pfxPath, $pfxPass,
               [System.Security.Cryptography.X509Certificates.X509KeyStorageFlags]::DefaultKeySet)
foreach ($storeName in 'Root', 'TrustedPublisher') {
    $store = New-Object System.Security.Cryptography.X509Certificates.X509Store($storeName, 'LocalMachine')
    $store.Open('ReadWrite')
    $store.Add($cert)
    $store.Close()
}
Write-Host "    Thumbprint: $($cert.Thumbprint)"

# 2. Copy driver binary
$sysDest = 'C:\Windows\System32\drivers\elam_rs.sys'
Write-Host "[2/4] Copying driver to $sysDest ..."
Copy-Item $sysSrc $sysDest -Force
Write-Host "    OK ($((Get-Item $sysDest).Length) bytes)"

# 3. Register Elam as boot driver in Early-Launch group
Write-Host "[3/4] Registering Elam boot driver..."
$exists = (sc.exe qc Elam 2>&1) -notmatch 'FAILED'
if ($exists) {
    sc.exe config Elam binPath= $sysDest start= boot error= normal group= "Early-Launch" | Out-Null
} else {
    sc.exe create Elam binPath= $sysDest type= kernel start= boot error= normal group= "Early-Launch" | Out-Null
}
$cfg = sc.exe qc Elam 2>&1
if ($cfg -match 'FAILED') { throw "Failed to configure Elam service" }
($cfg | Where-Object { $_ -match 'START_TYPE|LOAD_ORDER_GROUP|ERROR_CONTROL' }) |
    ForEach-Object { Write-Host "    $_" }

# 4. Register ElamPplTest with Antimalware-Light PPL (launchProtected= 3)
Write-Host "[4/4] Registering ElamPplTest (PPL Antimalware-Light)..."
$pplDest = 'C:\Windows\Temp\ppl_test.exe'
Copy-Item $pplExe $pplDest -Force
$exists2 = (sc.exe qc ElamPplTest 2>&1) -notmatch 'FAILED'
if ($exists2) {
    sc.exe config ElamPplTest binPath= "`"$pplDest`"" | Out-Null
} else {
    sc.exe create ElamPplTest binPath= "`"$pplDest`"" type= own start= demand | Out-Null
}
sc.exe config ElamPplTest launchProtected= 3 | Out-Null
$cfg2 = sc.exe qc ElamPplTest 2>&1
($cfg2 | Where-Object { $_ -match 'BINARY_PATH|LAUNCH_PROTECTED' }) |
    ForEach-Object { Write-Host "    $_" }

Write-Host ""
Write-Host "Done." -ForegroundColor Green
Write-Host ""
Write-Host "*** REBOOT REQUIRED ***" -ForegroundColor Yellow
Write-Host "After reboot, run:"
Write-Host "  sc query Elam                   # should show STATE: 4 RUNNING (or 1 STOPPED - both ok)"
Write-Host "  Get-WmiObject Win32_SystemDriver -Filter ""Name='Elam'"" | Select Name, Started"
Write-Host "  sc start ElamPplTest"
Write-Host "  type C:\Windows\Temp\ppl_test.log"
