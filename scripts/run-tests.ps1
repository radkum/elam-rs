# Runs the Pester tests for the PowerShell helper scripts.
# Requires Pester >= 5 (CI installs it if missing).

#Requires -Version 5.1

$ErrorActionPreference = 'Stop'
Import-Module Pester -MinimumVersion 5.0

$tests = Join-Path (Split-Path -Parent $PSScriptRoot) 'tests'
$result = Invoke-Pester -Path $tests -PassThru -Output Detailed

if ($result.FailedCount -gt 0) {
    Write-Error "$($result.FailedCount) Pester test(s) failed"
    exit 1
}

Write-Host "All $($result.PassedCount) Pester tests passed"
