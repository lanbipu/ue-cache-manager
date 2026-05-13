# Creates a USB-friendly UECM WinRM bootstrap package.
# The package contains:
# - UECM-Bootstrap-WinRM.ps1
# - README.txt

param(
    [string]$OutputDirectory = (Join-Path (Get-Location) 'UECM-WinRM-Bootstrap')
)

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
try { chcp 65001 | Out-Null } catch {}

$ErrorActionPreference = 'Stop'

try {
    $scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
    $source = Join-Path $scriptRoot 'enable-winrm.ps1'
    if (-not (Test-Path $source)) {
        throw "enable-winrm.ps1 not found at $source"
    }

    New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null

    $targetScript = Join-Path $OutputDirectory 'UECM-Bootstrap-WinRM.ps1'
    Copy-Item -Path $source -Destination $targetScript -Force

    $readme = @'
UECM WinRM Bootstrap Package
============================

Purpose
-------
Run this package once on each Windows target machine that has no remote
management channel yet. After it completes, UECM can manage the machine through
WinRM instead of SSH.

Basic local target bootstrap
----------------------------
1. Open PowerShell as Administrator in this folder.
2. Run:

powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\UECM-Bootstrap-WinRM.ps1

Workgroup/local Administrator variant
-------------------------------------
Use this only when UECM will authenticate with local Administrator accounts
instead of domain accounts:

powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\UECM-Bootstrap-WinRM.ps1 -EnableLocalAccountRemoteAdmin

Restrict WinRM to the UECM operator machine
-------------------------------------------
Replace 192.168.10.20 with the operator machine IP:

powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\UECM-Bootstrap-WinRM.ps1 -AllowedRemoteAddress 192.168.10.20

Validation
----------
On the target machine:

powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\UECM-Bootstrap-WinRM.ps1 -CheckOnly

On the UECM operator machine:

Test-WSMan <target-host-or-ip>

Security notes
--------------
This script enables WinRM HTTP on port 5985 and Windows Remote Management
firewall rules. It does not enable SSH, Basic auth, CredSSP, AllowUnencrypted,
or global ExecutionPolicy changes.
'@

    Set-Content -Path (Join-Path $OutputDirectory 'README.txt') -Value $readme -Encoding UTF8

    @{
        ok = $true
        message = 'UECM WinRM bootstrap package created'
        output_directory = (Resolve-Path $OutputDirectory).Path
        files = @('UECM-Bootstrap-WinRM.ps1', 'README.txt')
    } | ConvertTo-Json -Compress
    exit 0
}
catch {
    @{
        ok = $false
        message = $_.Exception.Message
        output_directory = $OutputDirectory
        files = @()
    } | ConvertTo-Json -Compress
    exit 1
}
