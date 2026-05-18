# Plan 7 T1.8 sidecar - CB binary (base64).
#
# Purpose:
#   Hit zen's /stats and /stats/<cache-provider> endpoints and return the raw
#   CB blobs base64-encoded. Rust core (core::zen::cache_stats + cb_parser)
#   does the CB decode, provider extraction, and persistence.
#
# Parameters:
#   -Url <string>                base URL like "http://127.0.0.1:8558"
#   -TimeoutSeconds <int> = 5    per-request timeout
#   -CacheProvider <string> = "z$"
#                                provider name to fetch from /stats/<name>.
#                                The $ character is URL-encoded before use
#                                (browsers tolerate raw $, but documenting
#                                the dollar-sign quirk: see plan §3 HC4).
#
# Output (single JSON object on stdout):
#   {
#     "ok": true,
#     "stats_cb_b64": "<base64>",
#     "z_cache_cb_b64": "<base64>" | null,
#     "errors": [{ "endpoint": "/stats/z%24", "status": 404 }]
#   }
#
# Note: this script does NOT decode CB itself - the "providers" field is
# intentionally omitted. Rust decodes stats_cb_b64 and pulls provider names
# (parity contract: PS owns IO + base64, Rust owns parsing).
#
# Error semantics:
#   - /stats failure  -> ok=false, message set, /stats/<provider> not attempted.
#   - /stats/<provider> failure -> ok=true, z_cache_cb_b64 null, entry in
#     errors[]. Caller decides whether that's actionable.
#
# Usage:
#   powershell.exe -NoProfile -ExecutionPolicy Bypass -File zen-probe-cache-stats.ps1 -Url http://127.0.0.1:8558

param(
    [Parameter(Mandatory=$true)] [string]$Url,
    [int]$TimeoutSeconds = 5,
    [string]$CacheProvider = 'z$'
)

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
chcp 65001 | Out-Null

$ErrorActionPreference = 'Stop'

function Get-RawResponse {
    param(
        [string]$Uri,
        [int]$Timeout
    )
    $resp = Invoke-WebRequest -Uri $Uri -UseBasicParsing -TimeoutSec $Timeout -ErrorAction Stop
    $bytes = $resp.RawContentStream.ToArray()
    return [pscustomobject]@{
        StatusCode = [int]$resp.StatusCode
        Bytes      = $bytes
    }
}

function Make-ErrorEntry {
    param(
        [string]$Endpoint,
        [System.Management.Automation.ErrorRecord]$Err
    )
    $entry = @{ endpoint = $Endpoint }
    $statusCode = $null
    try {
        $resp = $Err.Exception.Response
        if ($resp -and $resp.StatusCode) { $statusCode = [int]$resp.StatusCode }
    } catch { $statusCode = $null }
    if ($null -ne $statusCode) {
        $entry['status'] = $statusCode
    } else {
        $entry['error'] = "$($Err.Exception.Message)"
    }
    return $entry
}

try {
    $base = $Url.TrimEnd('/')
    $errors = New-Object System.Collections.ArrayList

    # --- /stats (mandatory) ---------------------------------------------------
    $statsB64 = $null
    try {
        $r = Get-RawResponse -Uri "$base/stats" -Timeout $TimeoutSeconds
        if ($null -ne $r.Bytes -and $r.Bytes.Length -gt 0) {
            $statsB64 = [Convert]::ToBase64String($r.Bytes)
        } else {
            $statsB64 = ""
        }
    } catch {
        # Keep exit 0 so Rust winrm::invoke_json gets the JSON envelope.
        $entry = Make-ErrorEntry -Endpoint '/stats' -Err $_
        @{
            ok = $false
            message = "GET /stats failed: $($_.Exception.Message)"
            errors = @($entry)
        } | ConvertTo-Json -Compress -Depth 6
        exit 0
    }

    # --- /stats/<provider> (non-fatal) ---------------------------------------
    # URL-encode the provider name. The "$" in "z$" is reserved per RFC 3986
    # sub-delims; zen's HTTP server happens to accept raw "$" but encoding it
    # is the safe path and matches what the dashboard uses.
    $encoded = [uri]::EscapeDataString($CacheProvider)
    $providerUrl = "$base/stats/$encoded"
    $zCacheB64 = $null
    try {
        $r = Get-RawResponse -Uri $providerUrl -Timeout $TimeoutSeconds
        if ($null -ne $r.Bytes -and $r.Bytes.Length -gt 0) {
            $zCacheB64 = [Convert]::ToBase64String($r.Bytes)
        } else {
            $zCacheB64 = ""
        }
    } catch {
        [void]$errors.Add((Make-ErrorEntry -Endpoint "/stats/$encoded" -Err $_))
    }

    $payload = @{
        ok = $true
        stats_cb_b64 = $statsB64
        z_cache_cb_b64 = $zCacheB64
        errors = @($errors)
    }
    $payload | ConvertTo-Json -Compress -Depth 6
}
catch {
    @{ ok = $false; message = "$($_.Exception.Message)" } | ConvertTo-Json -Compress
    exit 0
}
