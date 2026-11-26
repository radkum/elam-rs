# diagnose.ps1 - run as Administrator on the test VM
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File .\diagnose.ps1

$dir = Split-Path -Parent $MyInvocation.MyCommand.Path

function Section($title) {
    Write-Host ""
    Write-Host "=== $title ===" -ForegroundColor Cyan
}

Section "Elam service status"
sc.exe query Elam

Section "Elam service config"
sc.exe qc Elam

Section "Elam registry"
$reg = Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Services\Elam' -ErrorAction SilentlyContinue
if ($reg) {
    Write-Host "ImagePath    : $($reg.ImagePath)"
    Write-Host "Start        : $($reg.Start)"
    Write-Host "Type         : $($reg.Type)"
    Write-Host "Group        : $($reg.Group)"
    Write-Host "ErrorControl : $($reg.ErrorControl)"
} else {
    Write-Warning "Elam service not found in registry"
}

Section "Installed driver binary"
$sysDest = 'C:\Windows\System32\drivers\elam_rs.sys'
if (Test-Path $sysDest) {
    $info = Get-Item $sysDest
    Write-Host "Path     : $($info.FullName)"
    Write-Host "Size     : $($info.Length) bytes"
    Write-Host "Modified : $($info.LastWriteTime)"
} else {
    Write-Warning "elam_rs.sys NOT found in System32\drivers"
}

Section "Staged driver binary"
$sysSrc = Join-Path $dir 'elam_rs.sys'
if (Test-Path $sysSrc) {
    $info = Get-Item $sysSrc
    Write-Host "Size     : $($info.Length) bytes"
    Write-Host "Modified : $($info.LastWriteTime)"
    if ((Test-Path $sysDest) -and ((Get-Item $sysDest).Length -eq $info.Length)) {
        Write-Host "Size match: YES" -ForegroundColor Green
    } else {
        Write-Host "Size match: NO - driver in System32\drivers differs from staged file" -ForegroundColor Red
    }
} else {
    Write-Warning "elam_rs.sys not found in $dir"
}

Section "Certificate stores"
$pfxPath = Join-Path $dir 'elam_rs.pfx'
if (Test-Path $pfxPath) {
    $pfxPass = ConvertTo-SecureString -String 'password' -Force -AsPlainText
    $cert = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2(
        $pfxPath, $pfxPass,
        [System.Security.Cryptography.X509Certificates.X509KeyStorageFlags]::DefaultKeySet)
    Write-Host "PFX thumbprint  : $($cert.Thumbprint)"
    Write-Host "PFX valid until : $($cert.NotAfter)"
    foreach ($storeName in 'Root', 'TrustedPublisher') {
        $store = New-Object System.Security.Cryptography.X509Certificates.X509Store($storeName, 'LocalMachine')
        $store.Open('ReadOnly')
        $found = $store.Certificates | Where-Object { $_.Thumbprint -eq $cert.Thumbprint }
        $store.Close()
        if ($found) {
            Write-Host "${storeName}: PRESENT" -ForegroundColor Green
        } else {
            Write-Host "${storeName}: MISSING" -ForegroundColor Red
        }
    }
} else {
    Write-Warning "elam_rs.pfx not found in $dir"
}

Section "ElamPplTest service config"
sc.exe qc ElamPplTest

Section "ElamPplTest registry"
$reg2 = Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Services\ElamPplTest' -ErrorAction SilentlyContinue
if ($reg2) {
    Write-Host "LaunchProtected : $($reg2.LaunchProtected)"
    Write-Host "ImagePath       : $($reg2.ImagePath)"
} else {
    Write-Warning "ElamPplTest not found in registry"
}

Section "PPL test log (last 5 lines)"
$log = 'C:\Windows\Temp\ppl_test.log'
if (Test-Path $log) {
    Get-Content $log -Tail 5
} else {
    Write-Host "(no log yet)"
}
