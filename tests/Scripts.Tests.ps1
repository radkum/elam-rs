# Pester tests for the PowerShell helper scripts.
#
# These are contract tests: they do not run a real install (that needs a VM and
# admin/service/cert side effects, covered by TESTING.md). Instead they parse the
# scripts and assert the behaviours that were fixed after real-world failures:
# service detection via $LASTEXITCODE, LaunchProtected handling, idempotent cert
# import and protected-service uninstall.

BeforeAll {
    $script:Root = Split-Path -Parent $PSScriptRoot

    function Get-ScriptText([string] $name) {
        Get-Content -LiteralPath (Join-Path $script:Root $name) -Raw
    }
}

Describe 'PowerShell scripts parse' {
    It '<name> has no parse errors' -TestCases @(
        @{ name = 'install.ps1' }
        @{ name = 'uninstall.ps1' }
        @{ name = 'diagnose.ps1' }
        @{ name = 'scripts/check_elam_resource.ps1' }
        @{ name = 'scripts/check_driver_resource.ps1' }
        @{ name = 'scripts/run-tests.ps1' }
    ) {
        param($name)

        $errors = $null
        [void][System.Management.Automation.Language.Parser]::ParseFile(
            (Join-Path $script:Root $name), [ref]$null, [ref]$errors)
        $errors.Count | Should -Be 0
    }
}

Describe 'install.ps1' {
    BeforeAll {
        $script:Install = Get-ScriptText 'install.ps1'
    }

    It 'requires administrator' {
        $script:Install | Should -Match '#Requires -RunAsAdministrator'
    }

    It 'detects existing services via $LASTEXITCODE, not output matching' {
        $script:Install | Should -Match 'sc\.exe qc Elam \*>\s*\$null'
        $script:Install | Should -Match '\$LASTEXITCODE -eq 0'
        $script:Install | Should -Not -Match "-notmatch 'FAILED'"
    }

    It 'sets LaunchProtected as a registry DWORD (3 = Antimalware-Light)' {
        $script:Install | Should -Match '-Name LaunchProtected -Value 3 -Type DWord'
    }

    It 'imports the certificate only when missing (idempotent)' {
        $script:Install | Should -Match '\$store\.Certificates \| Where-Object \{ \$_\.Thumbprint -eq \$cert\.Thumbprint \}'
        $script:Install | Should -Match '\$store\.Add\(\$cert\)'
    }

    It 'stops ElamPplTest before overwriting ppl_test.exe' {
        $stop = $script:Install.IndexOf('sc.exe stop ElamPplTest')
        $copy = $script:Install.IndexOf('Copy-Item $pplExe $pplDest')
        ($stop -ge 0) | Should -Be $true
        ($copy -ge 0) | Should -Be $true
        ($stop -lt $copy) | Should -Be $true
    }
}

Describe 'uninstall.ps1' {
    BeforeAll {
        $script:Uninstall = Get-ScriptText 'uninstall.ps1'
    }

    It 'resolves its directory via $PSScriptRoot' {
        $script:Uninstall | Should -Match '\$PSScriptRoot'
    }

    It 'clears LaunchProtected before deleting the service' {
        $clear = $script:Uninstall.IndexOf('-Name LaunchProtected -Value 0')
        $delete = $script:Uninstall.IndexOf('sc.exe delete ElamPplTest')
        ($clear -ge 0) | Should -Be $true
        ($delete -ge 0) | Should -Be $true
        ($clear -lt $delete) | Should -Be $true
    }

    It 'removes the certificate by thumbprint from both stores' {
        $script:Uninstall | Should -Match "'Root', 'TrustedPublisher'"
        $script:Uninstall | Should -Match '\$_\.Thumbprint -eq \$cert\.Thumbprint'
    }
}

Describe 'diagnose.ps1' {
    BeforeAll {
        $script:Diagnose = Get-ScriptText 'diagnose.ps1'
    }

    It 'reports the Elam service and the PPL test log' {
        $script:Diagnose | Should -Match 'sc\.exe query Elam'
        $script:Diagnose | Should -Match 'ppl_test\.log'
        $script:Diagnose | Should -Match 'LaunchProtected'
    }
}
