# uninstall.ps1 — run as Administrator on the test VM
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File .\uninstall.ps1
#
# Stops/removes the ppl-test service, uninstalls the ELAM driver, removes the cert.
# Reboot after this script to complete driver removal.

#Requires -RunAsAdministrator

$ErrorActionPreference = 'Continue'
$dir = Split-Path -Parent $MyInvocation.MyCommand.Path

# 1. Stop and remove ppl-test service
Write-Host "Removing ppl-test service..."
sc.exe stop   ElamPplTest 2>$null
sc.exe delete ElamPplTest 2>$null

# 2. Uninstall ELAM driver via INF
$inf = Join-Path $dir 'elam_rs.inf'
if (Test-Path $inf) {
    Write-Host "Uninstalling ELAM driver via INF..."
    rundll32.exe setupapi.dll,InstallHinfSection DefaultUninstall 128 $inf
} else {
    Write-Warning "elam_rs.inf not found — removing service manually..."
    sc.exe delete Elam 2>$null
}

# 3. Remove the test certificate from system stores
$pfxPath = Join-Path $dir 'elam_rs.pfx'
if (Test-Path $pfxPath) {
    Write-Host "Removing certificate from Root and TrustedPublisher stores..."
    $pfxPass = ConvertTo-SecureString -String 'password' -Force -AsPlainText
    $cert    = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2(
                   $pfxPath, $pfxPass,
                   [System.Security.Cryptography.X509Certificates.X509KeyStorageFlags]::DefaultKeySet)

    foreach ($storeName in 'Root', 'TrustedPublisher') {
        $store = New-Object System.Security.Cryptography.X509Certificates.X509Store($storeName, 'LocalMachine')
        $store.Open('ReadWrite')
        $store.Remove($cert)
        $store.Close()
    }
} else {
    Write-Warning "elam_rs.pfx not found — certificate not removed from stores"
}

Remove-Item -ErrorAction SilentlyContinue 'C:\Windows\Temp\ppl_test.log'

Write-Host ""
Write-Host "Done. Reboot to complete driver removal."
