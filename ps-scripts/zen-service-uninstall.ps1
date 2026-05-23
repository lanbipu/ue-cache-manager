# Plan 7 T2.4 sidecar - uninstall the zen Windows service.
#
# Purpose:
#   Wrap `zen.exe service uninstall --name <ServiceName>`. Idempotent: if the
#   service was never installed, the script reports was_present=false rather
#   than failing.
#
# Parameters:
#   -ZenExePath  <string>  absolute path to zen.exe.
#   -ServiceName <string>  Windows service name. Default "ZenServer".
#
# Output (single JSON object on stdout):
#   {
#     "ok": true,
#     "was_present": true,
#     "service_name": "ZenServer",
#     "zen_exit_code": 0
#   }
#
# Rust parser: core::zen::service::parse_uninstall_response (T2.5).
#
# Usage:
#   powershell.exe -NoProfile -ExecutionPolicy Bypass -File zen-service-uninstall.ps1 `
#       -ZenExePath "C:\Users\me\AppData\Local\UnrealEngine\Common\Zen\Install\zen.exe" `
#       -ServiceName "ZenServer"

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
chcp 65001 | Out-Null

$ErrorActionPreference = 'Stop'

try {
    $p = [Console]::In.ReadToEnd() | ConvertFrom-Json
    # ZenExePath is only required when the service is actually installed; the
    # body short-circuits "service not installed" before touching it, so we
    # bind it here without a hard null-guard and let the body's conditional
    # IsNullOrWhiteSpace check (below) enforce it on the install-present path.
    $ZenExePath = $p.ZenExePath
    $ServiceName = if ($p.ServiceName) { $p.ServiceName } else { 'ZenServer' }
    # Validate ServiceName FIRST so the "service not installed" idempotent
    # short-circuit works even on hosts where zen.exe has been cleaned up
    # (a fresh box or after a partial uninstall).
    if ([string]::IsNullOrWhiteSpace($ServiceName)) {
        throw "ServiceName must be non-empty"
    }
    if ($ServiceName -match '[\*\?\[\]]') {
        throw "ServiceName must be a literal name (no wildcards `*` `?` `[` `]`), got: $ServiceName"
    }

    # Pre-check: is the service even installed? Run this BEFORE requiring
    # ZenExePath so cleanup of a host that no longer has zen.exe still
    # reports `was_present=false` cleanly (e.g. after a tool wipe).
    $existing = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
    if ($null -eq $existing) {
        $payload = @{
            ok = $true
            was_present = $false
            service_name = $ServiceName
            zen_exit_code = $null
        }
        $payload | ConvertTo-Json -Compress -Depth 4
        exit 0
    }

    # Service exists: now zen.exe is required to actually uninstall it.
    if ([string]::IsNullOrWhiteSpace($ZenExePath)) {
        throw "ZenExePath must be non-empty (service '$ServiceName' is installed and needs zen.exe to uninstall)"
    }
    if (-not (Test-Path -LiteralPath $ZenExePath -PathType Leaf)) {
        throw "ZenExePath not found or not a file: $ZenExePath (service '$ServiceName' is installed and needs zen.exe to uninstall)"
    }

    # Use the call operator so PowerShell preserves space-containing args.
    $combined = (& $ZenExePath service uninstall --name $ServiceName 2>&1 | Out-String)
    $exitCode = [int]$LASTEXITCODE
    if ($null -eq $combined) { $combined = '' }

    if ($exitCode -ne 0) {
        @{
            ok = $false
            message = "zen service uninstall failed (exit $exitCode)"
            service_name = $ServiceName
            zen_exit_code = $exitCode
            zen_combined = $combined
        } | ConvertTo-Json -Compress -Depth 4
        exit 0
    }

    $payload = @{
        ok = $true
        was_present = $true
        service_name = $ServiceName
        zen_exit_code = $exitCode
    }
    $payload | ConvertTo-Json -Compress -Depth 4
}
catch {
    @{ ok = $false; message = "$($_.Exception.Message)" } | ConvertTo-Json -Compress
    exit 0
}
