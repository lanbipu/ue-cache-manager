# Creates a USB-friendly UECM WinRM bootstrap package.
# The package contains:
# - UECM-Bootstrap.cmd        (双击入口，自提权)
# - UECM-Bootstrap-WinRM.ps1  (脚本本体)
# - README.txt                (中文使用说明)

param(
    [string]$OutputDirectory = (Join-Path (Get-Location) 'UECM-WinRM-Bootstrap')
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
