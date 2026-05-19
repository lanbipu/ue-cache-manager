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
    [Parameter(Mandatory = $true)][string]$DataDir,
    # Optional: account the service runs as. Applied via `sc.exe config
    # <SvcName> obj= <user>` AFTER `zen service install` succeeds. Default
    # (parameter unset / empty) leaves zen's hardcoded NT AUTHORITY\LocalService
    # in place.
    #
    # NOTE: Zen's Windows installer (zenutil/service.cpp:441-453) hardcodes
    # CreateService with `NT AUTHORITY\LocalService` and ignores its own
    # `-u` install option. So we can't change the account via zen and must
    # patch the SCM record after the install lands. Acceptance on lanPC
    # 2026-05-19 showed LocalService can't read zen.exe under a user
    # profile (`%LOCALAPPDATA%\...\Common\Zen\Install\`), motivating this
    # post-install patch.
    #
    # Common values:
    #   "LocalSystem"               — full machine privileges, can read user profile
    #   ".\\uecm-test"              — local account (must exist on the host)
    #   "DOMAIN\\renderfarm-svc"    — domain account
    #   "NT AUTHORITY\\NetworkService" — narrower than LocalSystem
    [string]$ServiceUser = '',
    # Optional password for ServiceUser. Required for non-built-in accounts
    # (built-in ones like LocalSystem / LocalService / NetworkService have
    # no password). Note: ends up in sc.exe argv, visible via Process
    # Explorer for the duration of the post-install patch call — acceptable
    # for an admin-driven one-shot but documented here for transparency.
    [string]$ServicePassword = ''
)

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
chcp 65001 | Out-Null

$ErrorActionPreference = 'Stop'

# ----------------------------------------------------------------------------
# Helpers (script scope so both the idempotency path and the post-install
# `sc.exe config` path can reuse them).
# ----------------------------------------------------------------------------

# Built-in account name normalization: SCM stores LocalSystem as
# `LocalSystem`, LocalService as `NT AUTHORITY\LocalService`, NetworkService
# as `NT AUTHORITY\NetworkService`. Operator input may use either short or
# long form; normalize both sides for equality checks.
function Normalize-Account([string]$a) {
    if ([string]::IsNullOrWhiteSpace($a)) { return '' }
    $t = $a.Trim().ToLowerInvariant()
    switch -Regex ($t) {
        '^(localsystem|nt authority\\system|nt authority\\localsystem|\.\\localsystem)$' { 'localsystem' }
        '^(localservice|nt authority\\localservice|\.\\localservice)$' { 'localservice' }
        '^(networkservice|nt authority\\networkservice|\.\\networkservice)$' { 'networkservice' }
        default { $t -replace '^\.\\', '' }
    }
}

# Read a service's `StartName` (account it runs as) with CIM first and a
# registry fallback. Hosts that have WMI/CIM disabled by policy still need
# to be supported for both idempotency checks AND post-`sc.exe config`
# verification — without the fallback we'd misread null and trip rollback
# even when sc.exe succeeded.
function Get-ServiceAccount([string]$Name) {
    try {
        $cim = Get-CimInstance -ClassName Win32_Service `
            -Filter "Name='$Name'" -ErrorAction Stop
        if ($null -ne $cim -and $null -ne $cim.StartName) {
            return $cim.StartName
        }
    } catch { }
    try {
        $regPath = "HKLM:\SYSTEM\CurrentControlSet\Services\$Name"
        return (Get-ItemProperty -LiteralPath $regPath `
            -Name 'ObjectName' -ErrorAction Stop).ObjectName
    } catch { }
    return $null
}

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

    # --- Validate ServiceUser / ServicePassword ------------------------------
    # Codex P2: move this check BEFORE `zen service install` so a missing
    # password on a non-built-in account doesn't leave a half-installed
    # service behind. Without this, retries see the orphan LocalService
    # install and refuse on account mismatch until manual uninstall.
    if (-not [string]::IsNullOrWhiteSpace($ServiceUser)) {
        $normalizedUserUpfront = Normalize-Account $ServiceUser
        $isBuiltinUpfront = @('localsystem', 'localservice', 'networkservice') -contains $normalizedUserUpfront
        if (-not $isBuiltinUpfront -and [string]::IsNullOrEmpty($ServicePassword)) {
            throw ("ServiceUser '{0}' is not a built-in account; ServicePassword is required " +
                   "(built-in accounts: LocalSystem / LocalService / NetworkService).") `
                  -f $ServiceUser
        }
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
        $existingStartName = $null
        try {
            $cim = Get-CimInstance -ClassName Win32_Service `
                -Filter "Name='$ServiceName'" -ErrorAction Stop
            if ($null -ne $cim) {
                $existingPathName = $cim.PathName
                $existingStartName = $cim.StartName
            }
        } catch {
            # Fallback to registry if CIM is unavailable (rare).
            # Codex P3: read both ImagePath AND ObjectName — without
            # ObjectName the account comparison sees `null` vs the
            # requested account and flags drift even when the service is
            # already in the desired state.
            try {
                $regPath = "HKLM:\SYSTEM\CurrentControlSet\Services\$ServiceName"
                $regProps = Get-ItemProperty -LiteralPath $regPath -ErrorAction Stop
                $existingPathName = $regProps.ImagePath
                $existingStartName = $regProps.ObjectName
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

        # Codex P2: ServiceUser must match too. Without this an
        # idempotent-no-op path would return ok=true on existing service
        # while the requested account (e.g. LocalSystem) doesn't get
        # applied — the entire point of the new --service-user flag.
        # Uses the script-scope `Normalize-Account` helper above.
        $requestedAccount = if ([string]::IsNullOrWhiteSpace($ServiceUser)) {
            # zen's default — what the install would land as when no -u
            # supplied. Defaulted here so the comparison is meaningful.
            'localservice'
        } else {
            Normalize-Account $ServiceUser
        }
        $existingAccount = Normalize-Account $existingStartName
        $userMatches = ($requestedAccount -eq $existingAccount)

        if ($matchesExpected -and $userMatches) {
            @{
                ok = $true
                service_name = $ServiceName
                already_installed = $true
                existing_status = "$($existingSvc.Status)"
                existing_path_name = $existingPathName
                existing_service_account = $existingStartName
                message = "service '$ServiceName' already installed with matching config (no-op)"
            } | ConvertTo-Json -Compress -Depth 4
            exit 0
        }

        $reason = if (-not $matchesExpected) {
            'different ZenExePath / DataDir'
        } elseif (-not $userMatches) {
            "different service account (existing: '$existingStartName', requested: '$ServiceUser')"
        } else {
            'unknown drift'
        }
        @{
            ok = $false
            message = ("Service '{0}' is already installed (status: {1}) with {2}. " +
                       "Refusing to re-install without --full (Plan 7 §12 red line). " +
                       "Run zen-service-uninstall.ps1 first to change DataDir / zen.exe path / service account.") `
                      -f $ServiceName, $existingSvc.Status, $reason
            existing_service_account = $existingStartName
            service_name = $ServiceName
            existing_status = "$($existingSvc.Status)"
            existing_path_name = $existingPathName
        } | ConvertTo-Json -Compress -Depth 4
        exit 0
    }

    # --- Run zen.exe service install ----------------------------------------
    # `zen service install` only knows install-time options (--name,
    # --install-path, etc.); zenserver runtime flags MUST appear after the
    # bare `--` separator so zen records them in the installed service's
    # command line (see /Users/bip.lan/AIWorkspace/vp/zen/src/zen/cmds/
    # service_cmd.h:37 and service_cmd.cpp PassthroughCommandLine).
    # Therefore `--data-dir` goes after `--`, NOT as an install option.
    #
    # NOTE: Zen's `-u` install option is defined in service_cmd.cpp but the
    # Windows InstallService implementation in zenutil/service.cpp:441-453
    # hardcodes `CreateService(..., "NT AUTHORITY\LocalService", NULL)` and
    # ignores Spec.UserName. There is also no `-p` install option. So we
    # cannot set the service account via zen on Windows — we patch the SCM
    # record with `sc.exe config obj= ... password= ...` after a clean
    # install lands, below.
    #
    # Use the call operator (`&`) with separate string args so PowerShell's
    # native argument quoting handles space-containing paths like
    # `D:\UE Cache\Zen` correctly. Start-Process -ArgumentList flattens the
    # array using its own quoting rules which historically drops boundaries
    # for space-containing args.
    $installArgs = New-Object System.Collections.ArrayList
    [void]$installArgs.Add('service')
    [void]$installArgs.Add('install')
    [void]$installArgs.Add('--name')
    [void]$installArgs.Add($ServiceName)
    [void]$installArgs.Add('--')
    [void]$installArgs.Add('--data-dir')
    [void]$installArgs.Add($normalizedDataDir)
    $combined = (& $ZenExePath @installArgs 2>&1 | Out-String)
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

    # --- Patch service account via sc.exe config ----------------------------
    # Only when -ServiceUser was supplied. zen has left the service at
    # LocalService at this point; we move it to the requested account.
    # Built-in accounts (LocalSystem / LocalService / NetworkService) take
    # no password; sc requires `password= ""` (empty) for them. Other
    # accounts require a real password — the caller is responsible for
    # ensuring the account exists and has SeServiceLogonRight.
    $scOutput = $null
    $scExit = $null
    $scApplied = $false
    $scAccount = $null
    if (-not [string]::IsNullOrWhiteSpace($ServiceUser)) {
        $normalizedUser = Normalize-Account $ServiceUser
        $isBuiltin = @('localsystem', 'localservice', 'networkservice') -contains $normalizedUser
        # Credential / password coherency was validated above, before
        # `zen service install` ran. Reaching this point with a non-built-in
        # account guarantees a password is present.
        # Codex P2: validation accepts built-in aliases like `LocalService`,
        # `NT AUTHORITY\System`, `.\LocalSystem` but `sc.exe config obj=`
        # only accepts the canonical SCM names — `LocalSystem`,
        # `NT AUTHORITY\LocalService`, `NT AUTHORITY\NetworkService`.
        # Forwarding the raw alias breaks the patch and forces a rollback
        # the operator can't recover from. Canonicalize before sc.exe.
        $effectiveUser = switch ($normalizedUser) {
            'localsystem'     { 'LocalSystem' }
            'localservice'    { 'NT AUTHORITY\LocalService' }
            'networkservice'  { 'NT AUTHORITY\NetworkService' }
            default           { $ServiceUser }
        }
        # sc.exe's bizarre `option= value` syntax: the `=` is attached to the
        # option name and value is a separate token. Effective password for
        # built-in accounts is empty (sc accepts `password= ""`).
        $effectivePassword = if ($isBuiltin) { '' } else { $ServicePassword }
        $scArgs = @('config', $ServiceName, 'obj=', $effectiveUser, 'password=', $effectivePassword)
        $scOutput = (& sc.exe @scArgs 2>&1 | Out-String)
        $scExit = [int]$LASTEXITCODE

        # Codex P2: roll back the freshly-created LocalService install when
        # the account patch fails. Without this, a retry with corrected
        # credentials hits the existing-service drift refusal and forces
        # the operator to run zen-service-uninstall.ps1 by hand. We use
        # `zen.exe service uninstall` (not raw sc delete) so the rollback
        # mirrors the regular uninstall path and removes any zen-side
        # state it might track. The rollback is best-effort: we report
        # the original sc failure regardless.
        function Invoke-ServiceRollback {
            $uninstallArgs = @('service', 'uninstall', '--name', $ServiceName)
            $uOut = (& $ZenExePath @uninstallArgs 2>&1 | Out-String)
            $uExit = [int]$LASTEXITCODE
            return @{ exit_code = $uExit; output = $uOut }
        }

        # Codex P2: the rollback is best-effort; if `zen service uninstall`
        # also fails, the message must say "rollback FAILED" so the caller
        # knows an orphan service still exists. Without this they'd retry
        # and hit the existing-service drift refusal believing the
        # environment was clean.
        function Format-RollbackTag {
            param([int]$ExitCode)
            if ($ExitCode -eq 0) { 'service rolled back' }
            else { "rollback FAILED (exit $ExitCode) — orphan service may remain, manual uninstall required" }
        }

        if ($scExit -ne 0) {
            $rollback = Invoke-ServiceRollback
            $tag = Format-RollbackTag -ExitCode $rollback.exit_code
            @{
                ok = $false
                message = "sc.exe config (set service account) failed (exit $scExit); $tag"
                service_name = $ServiceName
                zen_exit_code = $exitCode
                sc_exit_code = $scExit
                sc_output = $scOutput
                rollback_exit_code = $rollback.exit_code
                rollback_output = $rollback.output
            } | ConvertTo-Json -Compress -Depth 4
            exit 0
        }
        # Verify the change landed — sc.exe occasionally returns 0 on
        # permission failures, so re-read the SCM record. Codex P2: use
        # the CIM-or-registry helper so hosts with WMI/CIM disabled by
        # policy don't read null and trip a false rollback.
        $actualStartName = Get-ServiceAccount $ServiceName
        $verifyAccount = if ($null -ne $actualStartName) { Normalize-Account $actualStartName } else { '' }
        if ($verifyAccount -ne $normalizedUser) {
            $rollback = Invoke-ServiceRollback
            $tag = Format-RollbackTag -ExitCode $rollback.exit_code
            @{
                ok = $false
                message = ("sc.exe config returned 0 but service account did not change " +
                           "(expected '{0}', got '{1}'); {2}.") `
                          -f $ServiceUser, $actualStartName, $tag
                service_name = $ServiceName
                zen_exit_code = $exitCode
                sc_exit_code = $scExit
                sc_output = $scOutput
                actual_service_account = $actualStartName
                rollback_exit_code = $rollback.exit_code
                rollback_output = $rollback.output
            } | ConvertTo-Json -Compress -Depth 4
            exit 0
        }
        $scApplied = $true
        $scAccount = $actualStartName
    }

    $payload = @{
        ok = $true
        service_name = $ServiceName
        zen_exit_code = $exitCode
        service_account_applied = $scApplied
        service_account = $scAccount
        sc_exit_code = $scExit
        message = $combined.Trim()
    }
    $payload | ConvertTo-Json -Compress -Depth 4
}
catch {
    @{ ok = $false; message = "$($_.Exception.Message)" } | ConvertTo-Json -Compress
    exit 0
}
