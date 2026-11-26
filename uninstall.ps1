# uninstall.ps1 - run as Administrator on the test VM
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File .\uninstall.ps1
#
# Stops/removes the ppl-test service, uninstalls the ELAM driver, removes the cert.
# A running PPL service cannot be stopped/deleted even by an administrator
# (that is the point of PPL), so if ElamPplTest is running this script asks you
# to reboot (the service is demand-start and will not restart) and run it again.

#Requires -RunAsAdministrator

$ErrorActionPreference = 'Continue'
$dir = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }

# 1. Stop and remove ppl-test service
Write-Host "Removing ppl-test service..."
$pplKey = 'HKLM:\SYSTEM\CurrentControlSet\Services\ElamPplTest'
if (Test-Path $pplKey) {
    # Clear the PPL flag first: while it is set (and until the next boot) SCM
    # denies deletion even to administrators. After a reboot the service can be
    # deleted by re-running this script.
    try { Set-ItemProperty -Path $pplKey -Name LaunchProtected -Value 0 -Type DWord } catch {}
}
sc.exe stop   ElamPplTest *> $null
sc.exe delete ElamPplTest *> $null
Start-Sleep -Milliseconds 800
$null = sc.exe qc ElamPplTest 2>&1
$pplStillThere = ($LASTEXITCODE -eq 0)

# 2. Uninstall ELAM driver via INF (falls back to sc delete without the INF)
$inf = Join-Path $dir 'elam_rs.inf'
if (Test-Path $inf) {
    Write-Host "Uninstalling ELAM driver via INF..."
    & rundll32.exe setupapi.dll,InstallHinfSection DefaultUninstall 128 $inf
} else {
    Write-Warning "elam_rs.inf not found - removing service manually..."
}
$null = sc.exe qc Elam 2>&1
if ($LASTEXITCODE -eq 0) {
    sc.exe stop Elam *> $null
    sc.exe delete Elam *> $null
}

# 3. Remove the test certificate from system stores (by thumbprint, so it works
#    regardless of how the cert was originally loaded)
$pfxPath = Join-Path $dir 'elam_rs.pfx'
if (Test-Path $pfxPath) {
    $pfxPass = ConvertTo-SecureString -String 'password' -Force -AsPlainText
    $cert = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2(
                $pfxPath, $pfxPass,
                [System.Security.Cryptography.X509Certificates.X509KeyStorageFlags]::DefaultKeySet)
    Write-Host "Removing certificate $($cert.Thumbprint) from Root and TrustedPublisher stores..."
    foreach ($storeName in 'Root', 'TrustedPublisher') {
        $store = New-Object System.Security.Cryptography.X509Certificates.X509Store($storeName, 'LocalMachine')
        $store.Open('ReadWrite')
        foreach ($found in @($store.Certificates | Where-Object { $_.Thumbprint -eq $cert.Thumbprint })) {
            $store.Remove($found)
        }
        $remaining = @($store.Certificates | Where-Object { $_.Thumbprint -eq $cert.Thumbprint }).Count
        $store.Close()
        Write-Host "    ${storeName}: ${remaining} remaining"
    }
} else {
    Write-Warning "elam_rs.pfx not found - certificate not removed from stores"
}

Remove-Item -ErrorAction SilentlyContinue 'C:\Windows\Temp\ppl_test.log'

Write-Host ""
if ($pplStillThere) {
    Write-Warning "ElamPplTest is still registered (protected service). LaunchProtected was cleared; reboot the VM, then run uninstall.ps1 again to finish removal."
} else {
    Write-Host "Done. Reboot to complete driver removal."
}
