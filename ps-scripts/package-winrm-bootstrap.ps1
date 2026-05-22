# Creates a USB-friendly UECM WinRM bootstrap package.
# The package contains:
# - UECM-Bootstrap.cmd        (双击入口，自提权)
# - UECM-Bootstrap-WinRM.ps1  (脚本本体)
# - README.txt                (中文使用说明)

param(
    [string]$OutputDirectory = (Join-Path (Get-Location) 'UECM-WinRM-Bootstrap'),
    [string]$LocalAdminName = '',
    [string]$LocalAdminPassword = ''
)

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
try { chcp 65001 | Out-Null } catch {}

$ErrorActionPreference = 'Stop'

try {
    $scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
    $sourcePs1 = Join-Path $scriptRoot 'enable-winrm.ps1'
    $sourceCmd = Join-Path $scriptRoot 'UECM-Bootstrap.cmd'
    $sourceReadme = Join-Path $scriptRoot 'uecm-bootstrap-readme.zh-CN.txt'
    if (-not (Test-Path $sourcePs1)) {
        throw "enable-winrm.ps1 not found at $sourcePs1"
    }
    if (-not (Test-Path $sourceCmd)) {
        throw "UECM-Bootstrap.cmd not found at $sourceCmd"
    }
    if (-not (Test-Path $sourceReadme)) {
        throw "uecm-bootstrap-readme.zh-CN.txt not found at $sourceReadme"
    }

    New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null

    $targetScript = Join-Path $OutputDirectory 'UECM-Bootstrap-WinRM.ps1'
    Copy-Item -Path $sourcePs1 -Destination $targetScript -Force

    $targetCmd = Join-Path $OutputDirectory 'UECM-Bootstrap.cmd'
    Copy-Item -Path $sourceCmd -Destination $targetCmd -Force

    # Optionally bake the local-admin credential into the packaged .cmd so the USB
    # package is one-double-click ready (operator presets it once at package time).
    # String.Replace (NOT -replace) so a password with regex/$ chars stays literal.
    # Write UTF-8 WITHOUT BOM - a BOM on the first line would break cmd's @echo off.
    # Bake account name and/or password into the packaged .cmd. Name and password are
    # independent: an operator may preset just the name and let on-site staff fill the
    # password into the .cmd later, so do NOT gate the name replacement on a password.
    if ((-not [string]::IsNullOrWhiteSpace($LocalAdminName)) -or (-not [string]::IsNullOrEmpty($LocalAdminPassword))) {
        # cmd.exe does percent-expansion and quote parsing on the .cmd at run time, so
        # a baked password containing % " or ^ would reach PowerShell mangled - the
        # created account password would not match what the operator recorded. Refuse
        # rather than silently bake a wrong credential.
        if ($LocalAdminPassword -match '[%"^]') {
            throw 'LocalAdminPassword contains % " or ^, which cmd.exe mangles in the packaged .cmd. Use a password without those characters, or extract the package and fill the password into UECM-Bootstrap.cmd by hand.'
        }
        $enc = New-Object System.Text.UTF8Encoding $false
        $cmdText = [System.IO.File]::ReadAllText($targetCmd, $enc)
        if (-not [string]::IsNullOrWhiteSpace($LocalAdminName)) {
            $cmdText = $cmdText.Replace('set "UECM_LOCAL_ADMIN=uecm-svc"', 'set "UECM_LOCAL_ADMIN=' + $LocalAdminName + '"')
        }
        if (-not [string]::IsNullOrEmpty($LocalAdminPassword)) {
            $cmdText = $cmdText.Replace('set "UECM_LOCAL_ADMIN_PASSWORD="', 'set "UECM_LOCAL_ADMIN_PASSWORD=' + $LocalAdminPassword + '"')
        }
        [System.IO.File]::WriteAllText($targetCmd, $cmdText, $enc)
    }

    # README is a separate UTF-8-with-BOM file (NOT inline heredoc) to avoid the
    # Windows PowerShell 5.1 mojibake trap: when the host code page is not UTF-8,
    # PS 5.1 re-encodes the .ps1 source through the ANSI code page during parse,
    # which corrupts CJK characters before they ever reach the file writer.
    # Binary Copy-Item bypasses all string parsing.
    $targetReadme = Join-Path $OutputDirectory 'README.txt'
    Copy-Item -Path $sourceReadme -Destination $targetReadme -Force

    @{
        ok = $true
        message = 'UECM WinRM bootstrap package created'
        output_directory = (Resolve-Path $OutputDirectory).Path
        files = @('UECM-Bootstrap.cmd', 'UECM-Bootstrap-WinRM.ps1', 'README.txt')
        local_admin_baked = (-not [string]::IsNullOrEmpty($LocalAdminPassword))
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
