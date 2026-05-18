# Plan 7 T1.8 sidecar - CB binary (base64).
#
# Purpose:
#   Hit zen's three health endpoints (/health, /health/version, /health/info)
#   and return the raw bodies. CB-encoded /health/info bytes are base64-encoded
#   so they survive the JSON-over-stdout transport without modification; the
#   Rust core decodes them (see core::zen::probe + cb_parser).
#
# Parameters:
#   -Url <string>                base URL like "http://127.0.0.1:8558" (no trailing slash)
#   -TimeoutSeconds <int> = 5    per-request timeout; matches the default in
#                                core::zen::probe::probe_endpoint
#
# Output (single JSON object on stdout):
#   {
#     "ok": true,
#     "health_text": "OK!",
#     "health_version_text": "5.8.10",
#     "health_info_cb_b64": "<base64>",
#     "errors": [{ "endpoint": "/health/info", "status_or_error": "404" }]
#   }
#
# Error semantics (parity with Rust probe.rs):
#   - /health failure  -> ok=false, message set, sub-endpoints not attempted.
#   - /health/version or /health/info failure -> ok=true, that field is null,
#     entry appended to "errors".
#   - Bodies are returned verbatim; the Rust side does the CB parse / heartbeat
#     check.
#
# Usage:
#   powershell.exe -NoProfile -ExecutionPolicy Bypass -File zen-probe-health.ps1 -Url http://127.0.0.1:8558

param(
    [Parameter(Mandatory=$true)] [string]$Url,
    [int]$TimeoutSeconds = 5
)

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
chcp 65001 | Out-Null

$ErrorActionPreference = 'Stop'

function Get-RawResponse {
    param(
        [string]$Uri,
        [int]$Timeout
    )
    # Invoke-WebRequest throws on non-2xx by default - that's fine, callers
    # wrap in try/catch and stash the failure in the errors array.
    $resp = Invoke-WebRequest -Uri $Uri -UseBasicParsing -TimeoutSec $Timeout -ErrorAction Stop
    # RawContentStream is a MemoryStream; .ToArray() pulls the bytes verbatim
    # without re-encoding (Content / RawContent run text through PS string
    # converters and would corrupt the CB blob).
    $bytes = $resp.RawContentStream.ToArray()
    return [pscustomobject]@{
        StatusCode = [int]$resp.StatusCode
        Bytes      = $bytes
    }
}

function Bytes-ToUtf8 {
    param([byte[]]$Bytes)
    if ($null -eq $Bytes -or $Bytes.Length -eq 0) { return "" }
    return [System.Text.Encoding]::UTF8.GetString($Bytes)
}

function Make-ErrorEntry {
    param(
        [string]$Endpoint,
        [System.Management.Automation.ErrorRecord]$Err
    )
    # Prefer HTTP status code when available; fall back to exception message.
    # PS 5.1 does NOT support hashtable + hashtable so build a single dict
    # here instead of merging two.
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

    # --- /health (mandatory; failure short-circuits the whole probe) ---------
    $healthText = $null
    try {
        $r = Get-RawResponse -Uri "$base/health" -Timeout $TimeoutSeconds
        $healthText = Bytes-ToUtf8 -Bytes $r.Bytes
    } catch {
        # Keep exit 0 so Rust winrm::invoke_json gets the JSON envelope; the
        # `ok=false` flag tells the caller /health was unreachable.
        $entry = Make-ErrorEntry -Endpoint '/health' -Err $_
        @{
            ok = $false
            message = "GET /health failed: $($_.Exception.Message)"
            errors = @($entry)
        } | ConvertTo-Json -Compress -Depth 6
        exit 0
    }

    # --- /health/version (non-fatal) -----------------------------------------
    $versionText = $null
    try {
        $r = Get-RawResponse -Uri "$base/health/version" -Timeout $TimeoutSeconds
        $versionText = Bytes-ToUtf8 -Bytes $r.Bytes
    } catch {
        [void]$errors.Add((Make-ErrorEntry -Endpoint '/health/version' -Err $_))
    }

    # --- /health/info (non-fatal; raw bytes base64-encoded) ------------------
    $infoB64 = $null
    try {
        $r = Get-RawResponse -Uri "$base/health/info" -Timeout $TimeoutSeconds
        if ($null -ne $r.Bytes -and $r.Bytes.Length -gt 0) {
            $infoB64 = [Convert]::ToBase64String($r.Bytes)
        } else {
            $infoB64 = ""
        }
    } catch {
        [void]$errors.Add((Make-ErrorEntry -Endpoint '/health/info' -Err $_))
    }

    $payload = @{
        ok = $true
        health_text = $healthText
        health_version_text = $versionText
        health_info_cb_b64 = $infoB64
        errors = @($errors)
    }
    $payload | ConvertTo-Json -Compress -Depth 6
}
catch {
    @{ ok = $false; message = "$($_.Exception.Message)" } | ConvertTo-Json -Compress
    exit 0
}
