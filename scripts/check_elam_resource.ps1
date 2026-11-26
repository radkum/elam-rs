# check_elam_resource.ps1 - guard against the MicrosoftElamCertificateInfo
# resource-format regression.
#
# Windows looks up a resource of TYPE "MSElamCertInfoID" named
# "MicrosoftElamCertificateInfo". Emitting `1 MicrosoftElamCertificateInfo`
# swaps name/type, so the cert is never registered as an ELAM anchor and PPL
# launch fails with 577 (CodeIntegrity event 3033, "Custom 3 / Antimalware").
#
# This check reads generate_cert.ps1 as text so it needs no certificate and is
# deterministic. Exit code 1 on failure.

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$src = Get-Content (Join-Path $root 'generate_cert.ps1') -Raw

if ($src -notmatch 'MicrosoftElamCertificateInfo\s+MSElamCertInfoID') {
    throw "generate_cert.ps1 does not emit 'MicrosoftElamCertificateInfo MSElamCertInfoID' (required ELAM resource form)."
}
if ($src -match '(?m)^\s*\d+\s+MicrosoftElamCertificateInfo') {
    throw "generate_cert.ps1 uses the wrong resource form ('<id> MicrosoftElamCertificateInfo'); name/type are swapped and Windows will not read the ELAM certificate."
}

Write-Host "ELAM certificate resource format OK"
