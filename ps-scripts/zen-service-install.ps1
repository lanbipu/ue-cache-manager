# Plan 7 T2.4 sidecar - install zen as a Windows service.
#
# Purpose:
#   Wrap `zen.exe service install --name <ServiceName> --datadir <DataDir>`
#   to register the zenserver service. The install binary itself is treated
#   read-only (we only invoke it; we never copy or modify zen.exe here).
#
# Parameters:
#   -ZenExePath  <string>  absolute path to zen.exe (e.g.
#                          %LOCALAPPDATA%\UnrealEngine\Common\Zen\Install\zen.exe).
#                          Must exist and be a file, not a folder.
#   -ServiceName <string>  Windows service name. Default "ZenServer".
#   -DataDir     <string>  absolute path to the zen data directory. Rejected
#                          if under C:\Windows / C:\Program Files / C:\Program
#                          Files (x86).
#
# ============================================================================
# RED LINE (Plan 7 §12): the `--full` flag is FORBIDDEN.
# `zen service install --full` instructs zen to take over as the host's
# primary zenserver, including stealing the install copy under
# %LOCALAPPDATA%\UnrealEngine\Common\Zen\Install\ from UE. UECM never wants
# that ownership transfer to happen via this sidecar - if an operator needs
# `--full`, they run zen.exe by hand outside UECM.
#
# This script hard-blocks any attempt to smuggle `--full` in via $args. The
# script's own param block intentionally does NOT expose a -Full switch.
# ============================================================================
#
# Output (single JSON object on stdout):
#   {
#     "ok": true,
#     "service_name": "ZenServer",
#     "zen_exit_code": 0,
#     "message": "...zen stdout..."
#   }
#
# Rust parser: core::zen::service::parse_install_response (T2.5).
#
# Usage:
#   powershell.exe -NoProfile -ExecutionPolicy Bypass -File zen-service-install.ps1 `
#       -ZenExePath "C:\Users\me\AppData\Local\UnrealEngine\Common\Zen\Install\zen.exe" `
#       -ServiceName "ZenServer" `
#       -DataDir "D:\ZenData"

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$ZenExePath,
    [string]$ServiceName = 'ZenServer',
    [Parameter(Mandatory = $true)][string]$DataDir
)

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
chcp 65001 | Out-Null

$ErrorActionPreference = 'Stop'

# ----------------------------------------------------------------------------
# HARD-BLOCK --full BEFORE the try/catch. We want a deterministic refusal even
# if a downstream code path throws.
# $args holds positional arguments passed after the named params; if a caller
# slips an extra token like "--full" through there, refuse outright.
# ----------------------------------------------------------------------------
foreach ($a in $args) {
    if ($null -eq $a) { continue }
    $sa = [string]$a
    if ($sa -imatch '(^|\s)--full(\s|=|$)') {
        @{
            ok = $false
            message = "RED LINE: --full flag refused (Plan 7 §12)"
        } | ConvertTo-Json -Compress
        exit 0
    }
}
# Also belt-and-braces check the bound parameter values just in case a caller
# tries to smuggle "--full" inside another argument.
foreach ($v in @($ZenExePath, $ServiceName, $DataDir)) {
    if ($null -eq $v) { continue }
    if ([string]$v -imatch '(^|\s)--full(\s|=|$)') {
        @{
            ok = $false
            message = "RED LINE: --full flag refused (Plan 7 §12)"
        } | ConvertTo-Json -Compress
        exit 0
    }
}

try {
    # --- Validate ZenExePath -------------------------------------------------
    if ([string]::IsNullOrWhiteSpace($ZenExePath)) {
        throw "ZenExePath must be non-empty"
    }
    if (-not (Test-Path -LiteralPath $ZenExePath -PathType Leaf)) {
        throw "ZenExePath not found or not a file: $ZenExePath"
    }

    # --- Validate ServiceName ------------------------------------------------
    if ([string]::IsNullOrWhiteSpace($ServiceName)) {
        throw "ServiceName must be non-empty"
    }
    # Reject wildcards in the service identifier — defense in depth even
    # though zen.exe itself would likely refuse `*` / `?`.
    if ($ServiceName -match '[\*\?\[\]]') {
        throw "ServiceName must be a literal name (no wildcards `*` `?` `[` `]`), got: $ServiceName"
    }

    # --- Validate DataDir ----------------------------------------------------
    if ([string]::IsNullOrWhiteSpace($DataDir)) {
        throw "DataDir must be non-empty"
    }
    # `IsPathRooted` accepts drive-relative paths like `D:ZenCache` (relative
    # to D:'s current directory) and root-relative paths like `\ZenCache`
    # (relative to the current drive). Both would be resolved by
    # `GetFullPath` against whatever the remote PowerShell session happens
    # to have as its current location — non-deterministic and likely
    # outside the operator's intent. Require a fully-qualified path:
    # either `<letter>:\...` (drive-absolute) or `\\<host>\...` (UNC).
    $dataDirTrim = $DataDir.Trim()
    # Reject Win32 device-namespace prefixes (`\\?\C:\...`, `\\.\X:\...`)
    # outright — GetFullPath preserves the prefix, which would let a
    # caller bypass the forbidden-system-root guard below (the
    # `\\?\C:\Windows\...` form would not match `c:\windows\...`). Allow
    # only "normal" drive-absolute or UNC paths.
    if ($dataDirTrim -match '^\\\\[\?\.]\\' -or $dataDirTrim -match '^//[\?\.]/') {
        throw ("DataDir must not use the Win32 device namespace prefix " +
               "(\\?\ / \\.\); got: $DataDir")
    }
    $isDriveAbsolute = $dataDirTrim -match '^[A-Za-z]:[\\/]'
    $isUnc = ($dataDirTrim.StartsWith('\\') -or $dataDirTrim.StartsWith('//')) -and
             -not ($dataDirTrim -match '^\\\\[\?\.]\\') -and
             -not ($dataDirTrim -match '^//[\?\.]/')
    if (-not ($isDriveAbsolute -or $isUnc)) {
        throw ("DataDir must be a fully-qualified absolute path " +
               "(e.g. 'D:\ZenCache' or '\\host\share\Zen'); " +
               "drive-relative or root-relative paths are not accepted. Got: $DataDir")
    }
    $normalizedDataDir = [System.IO.Path]::GetFullPath($dataDirTrim)
    # Compare with and without trailing slash so the exact system root
    # itself is rejected too, not just child paths. Without this, passing
    # `C:\Windows` (no trailing backslash) would slip past a StartsWith
    # check that only matched `c:\windows\`.
    $lowerDataDir = $normalizedDataDir.TrimEnd('\').ToLowerInvariant()
    $forbiddenRoots = @(
        'c:\windows',
        'c:\program files',
        'c:\program files (x86)'
    )
    foreach ($root in $forbiddenRoots) {
        if ($lowerDataDir -eq $root -or $lowerDataDir.StartsWith($root + '\')) {
            throw "DataDir '$normalizedDataDir' is under a forbidden system location ($root)"
        }
    }

    # --- Handle an already-installed service ----------------------------------
    # `zen service install` (without --full, which Plan §12 forbids) is a
    # no-op when the service is already registered: it exits 0 without
    # changing the existing service's binary path / command line / data-dir.
    # We split the cases:
    #
    # - Existing service with the SAME ZenExePath + same `--data-dir` →
    #   idempotent no-op, ok=true with `already_installed=true`. Lets
    #   `zen enable` retries succeed when the service is already in the
    #   desired state.
    # - Existing service with a DIFFERENT path or data-dir → refuse with
    #   a clear error pointing the operator at `service uninstall`. Telling
    #   the caller ok=true here would silently leave UECM thinking the
    #   desired config is live when actually the prior config is.
    $existingSvc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
    if ($null -ne $existingSvc) {
        $existingPathName = $null
        try {
            $cim = Get-CimInstance -ClassName Win32_Service `
                -Filter "Name='$ServiceName'" -ErrorAction Stop
            if ($null -ne $cim) { $existingPathName = $cim.PathName }
        } catch {
            # Fallback to registry if CIM is unavailable (rare).
            try {
                $regPath = "HKLM:\SYSTEM\CurrentControlSet\Services\$ServiceName"
                $existingPathName = (Get-ItemProperty -LiteralPath $regPath `
                    -Name 'ImagePath' -ErrorAction Stop).ImagePath
            } catch {
                # We'll fall through with $null; mismatch defaults to refuse.
            }
        }

        # Token-parse the existing PathName and compare the recorded
        # `--data-dir <value>` and exe path against the requested config.
        # Substring matching is unsafe because `D:\ZenCache` is a substring
        # of `D:\ZenCache2`, which would falsely report idempotent no-op
        # while the SCM actually points at a different data dir.
        $matchesExpected = $false
        if ($null -ne $existingPathName -and $existingPathName.Length -gt 0) {
            $expectedExe = [System.IO.Path]::GetFullPath($ZenExePath).TrimEnd('\').ToLowerInvariant()
            $expectedDir = $normalizedDataDir.TrimEnd('\').ToLowerInvariant()

            # Naive token split honoring "..." quoted args.
            $tokens = New-Object System.Collections.ArrayList
            $current = ''
            $inQuote = $false
            foreach ($ch in $existingPathName.ToCharArray()) {
                if ($ch -eq '"') {
                    $inQuote = -not $inQuote
                    continue
                }
                if ((-not $inQuote) -and ($ch -eq ' ' -or $ch -eq "`t")) {
                    if ($current.Length -gt 0) {
                        [void]$tokens.Add($current)
                        $current = ''
                    }
                } else {
                    $current += $ch
                }
            }
            if ($current.Length -gt 0) { [void]$tokens.Add($current) }

            # Token 0 should be the exe path (PathName form: "exe" args...).
            $existingExe = $null
            if ($tokens.Count -gt 0) {
                try {
                    $existingExe = [System.IO.Path]::GetFullPath(
                        $tokens[0].ToString()).TrimEnd('\').ToLowerInvariant()
                } catch {
                    $existingExe = $tokens[0].ToString().TrimEnd('\').ToLowerInvariant()
                }
            }

            # Find `--data-dir <value>` or `--data-dir=<value>`.
            $existingDir = $null
            for ($i = 0; $i -lt $tokens.Count; $i++) {
                $t = $tokens[$i].ToString()
                if ($t -ieq '--data-dir' -and ($i + 1) -lt $tokens.Count) {
                    $existingDir = $tokens[$i + 1].ToString()
                    break
                }
                if ($t -match '^--data-dir=(.*)$') {
                    $existingDir = $Matches[1]
                    break
                }
            }
            if ($null -ne $existingDir) {
                try {
                    $existingDir = [System.IO.Path]::GetFullPath(
                        $existingDir).TrimEnd('\').ToLowerInvariant()
                } catch {
                    $existingDir = $existingDir.TrimEnd('\').ToLowerInvariant()
                }
            }

            $matchesExpected = ($existingExe -eq $expectedExe) -and
                               ($null -ne $existingDir) -and
                               ($existingDir -eq $expectedDir)
        }

        if ($matchesExpected) {
            @{
                ok = $true
                service_name = $ServiceName
                already_installed = $true
                existing_status = "$($existingSvc.Status)"
                existing_path_name = $existingPathName
                message = "service '$ServiceName' already installed with matching config (no-op)"
            } | ConvertTo-Json -Compress -Depth 4
            exit 0
        }

        @{
            ok = $false
            message = ("Service '{0}' is already installed (status: {1}) with a different " +
                       "configuration. Refusing to re-install without --full (Plan 7 §12 red line). " +
                       "Run zen-service-uninstall.ps1 first to change DataDir or zen.exe path.") `
                      -f $ServiceName, $existingSvc.Status
            service_name = $ServiceName
            existing_status = "$($existingSvc.Status)"
            existing_path_name = $existingPathName
        } | ConvertTo-Json -Compress -Depth 4
        exit 0
    }

    # --- Run zen.exe service install ----------------------------------------
    # `zen service install` only knows install-time options (--name, --user,
    # --install-path, etc.); zenserver runtime flags MUST appear after the
    # bare `--` separator so zen records them in the installed service's
    # command line (see /Users/bip.lan/AIWorkspace/vp/zen/src/zen/cmds/
    # service_cmd.h:37 and service_cmd.cpp PassthroughCommandLine).
    # Therefore `--data-dir` goes after `--`, NOT as an install option.
    #
    # Use the call operator (`&`) with separate string args so PowerShell's
    # native argument quoting handles space-containing paths like
    # `D:\UE Cache\Zen` correctly. Start-Process -ArgumentList flattens the
    # array using its own quoting rules which historically drops boundaries
    # for space-containing args.
    $combined = (& $ZenExePath service install --name $ServiceName -- --data-dir $normalizedDataDir 2>&1 | Out-String)
    $exitCode = [int]$LASTEXITCODE
    if ($null -eq $combined) { $combined = '' }

    if ($exitCode -ne 0) {
        @{
            ok = $false
            message = "zen service install failed (exit $exitCode)"
            zen_exit_code = $exitCode
            zen_combined = $combined
        } | ConvertTo-Json -Compress -Depth 4
        exit 0
    }

    $payload = @{
        ok = $true
        service_name = $ServiceName
        zen_exit_code = $exitCode
        message = $combined.Trim()
    }
    $payload | ConvertTo-Json -Compress -Depth 4
}
catch {
    @{ ok = $false; message = "$($_.Exception.Message)" } | ConvertTo-Json -Compress
    exit 0
}
