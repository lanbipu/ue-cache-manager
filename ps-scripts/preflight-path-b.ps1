# Preflight check whether Path B (remote PsExec bootstrap) is viable for a target host.
#
# Steps (executed in order, with early-exit on hard failures):
#   1. tcp_135           — DCE/RPC Endpoint Mapper port reachability
#   2. tcp_445           — SMB port reachability
#   3. admin_share_mount — actually mount \\<HostName>\ADMIN$ with given credential
#   4. admin_share_write — copy + delete a small probe file under ADMIN$\Temp
#   5. psexec_probe      — (opt-in via -WithPsExec) run `cmd /c exit 0` via PsExec64
#                          to verify SCM service registration is not blocked by
#                          UAC remote token filter. Writes one service install
#                          + one service remove entry to target machine's Event Log.
#
# Output: JSON with `ok`, `verdict`, `reason`, and per-step `results[]`.
# Verdict values: viable | likely_viable | blocked | uncertain

param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$Username,
    [Parameter(Mandatory=$true)] [string]$Password,
    [string]$PsExecPath,
    [switch]$WithPsExec,
    [int]$TimeoutMs = 3000
)

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
try { chcp 65001 | Out-Null } catch {}
$ErrorActionPreference = 'Stop'

# Normalize username ONCE for the whole preflight. For workgroup/local-admin
# auth, prefix bare usernames with <host>\ so both SMB ADMIN$ and PsExec treat
# the credential as a local account on the TARGET (not operator's domain).
# If the caller already qualified the name with a backslash or @ form, respect it.
$UserResolved = if ($Username -match '[\\@]') { $Username } else { "$HostName\$Username" }

# Operator-side PsExec64 must exist regardless of -WithPsExec, because every
# UECM Path B path (this preflight's deep probe AND the actual `uecm-cli winrm
# bootstrap` later) depends on it. Detecting it missing here lets us fail fast
# instead of returning likely_viable and letting automation proceed.
$PsExecAvailable = (-not [string]::IsNullOrWhiteSpace($PsExecPath)) -and (Test-Path $PsExecPath)

$results = New-Object 'System.Collections.Generic.List[hashtable]'

function Add-Result {
    param([string]$Name, [string]$Status, [string]$Message)
    $results.Add(@{ name = $Name; status = $Status; message = $Message }) | Out-Null
}

function Test-TcpPort {
    param([string]$RemoteHost, [int]$Port, [int]$TimeoutMs)
    $client = New-Object System.Net.Sockets.TcpClient
    try {
        $async = $client.BeginConnect($RemoteHost, $Port, $null, $null)
        $wait = $async.AsyncWaitHandle.WaitOne($TimeoutMs, $false)
        if (-not $wait) { return $false }
        $client.EndConnect($async) | Out-Null
        return $true
    } catch {
        return $false
    } finally {
        $client.Close()
    }
}

# ----- Step 1: TCP 135 (RPC EPM) -----
try {
    $ok = Test-TcpPort -RemoteHost $HostName -Port 135 -TimeoutMs $TimeoutMs
    if ($ok) {
        Add-Result 'tcp_135' 'ok' 'RPC Endpoint Mapper (135) reachable'
    } else {
        Add-Result 'tcp_135' 'fail' "RPC Endpoint Mapper (135) unreachable within ${TimeoutMs}ms"
    }
} catch {
    Add-Result 'tcp_135' 'fail' "TCP 135 probe threw: $($_.Exception.Message)"
}

# ----- Step 2: TCP 445 (SMB) -----
try {
    $ok = Test-TcpPort -RemoteHost $HostName -Port 445 -TimeoutMs $TimeoutMs
    if ($ok) {
        Add-Result 'tcp_445' 'ok' 'SMB (445) reachable'
    } else {
        Add-Result 'tcp_445' 'fail' "SMB (445) unreachable within ${TimeoutMs}ms"
    }
} catch {
    Add-Result 'tcp_445' 'fail' "TCP 445 probe threw: $($_.Exception.Message)"
}

# ----- Step 3: Mount \\<host>\ADMIN$ with credential -----
$adminShare = "\\$HostName\ADMIN$"
$mounted = $false
$driveName = "UECMPF$(Get-Random -Minimum 1000 -Maximum 9999)"

try {
    $tcp445Status = ($results | Where-Object { $_.name -eq 'tcp_445' }).status
    if ($tcp445Status -ne 'ok') {
        Add-Result 'admin_share_mount' 'skipped' 'tcp_445 not reachable; cannot mount ADMIN$'
    } else {
        $secure = ConvertTo-SecureString -String $Password -AsPlainText -Force
        $cred = New-Object System.Management.Automation.PSCredential($UserResolved, $secure)

        $null = New-PSDrive -Name $driveName -PSProvider FileSystem -Root $adminShare `
            -Credential $cred -Scope Script -ErrorAction Stop
        $mounted = $true
        Add-Result 'admin_share_mount' 'ok' "mounted $adminShare as $UserResolved"
    }
} catch {
    Add-Result 'admin_share_mount' 'fail' "cannot mount $adminShare : $($_.Exception.Message)"
}

# ----- Step 4: Write + delete a probe file under ADMIN$\Temp -----
if ($mounted) {
    $probeName = "uecm-preflight-$(Get-Random).probe"
    $tempDir = "${driveName}:\Temp"
    $probePath = "$tempDir\$probeName"
    $remoteProbePath = "$adminShare\Temp\$probeName"
    try {
        if (-not (Test-Path $tempDir)) {
            New-Item -Path $tempDir -ItemType Directory -Force | Out-Null
        }
        Set-Content -Path $probePath -Value 'uecm preflight probe — safe to delete' -ErrorAction Stop
        # Write succeeded — that's what makes ADMIN$ "writable" from preflight's
        # perspective. Cleanup is best-effort; if Remove-Item fails (AV scan, ACL,
        # file lock) we surface the residual file path in the message so the
        # operator can clean up manually. Keep status=ok because the writability
        # claim is still true.
        try {
            Remove-Item -Path $probePath -Force -ErrorAction Stop
            Add-Result 'admin_share_write' 'ok' "wrote + removed probe under $adminShare\Temp"
        } catch {
            Add-Result 'admin_share_write' 'ok' "wrote probe under $adminShare\Temp but FAILED to clean up (please delete manually): $remoteProbePath — $($_.Exception.Message)"
        }
    } catch {
        Add-Result 'admin_share_write' 'fail' "could not write probe under $adminShare\Temp : $($_.Exception.Message)"
    }
} else {
    Add-Result 'admin_share_write' 'skipped' 'admin_share_mount did not succeed; write probe skipped'
}

# ----- Step 5: PsExec noop probe (opt-in) -----
# Note: $PsExecAvailable was already checked at script init. If it's false, the
# verdict logic short-circuits to 'uncertain' regardless of -WithPsExec.
if ($WithPsExec) {
    if (-not $PsExecAvailable) {
        # Should not reach verdict 'blocked' from here — script-init handles the
        # operator-side case. Still record it for transparency in the results list.
        Add-Result 'psexec_probe' 'fail' "operator-side: PsExec64.exe not found at '$PsExecPath'"
    } else {
        try {
            # Use the SAME normalized username as the SMB mount above so the
            # two probes converge on identical credentials. Otherwise the same
            # credential could pass mount but fail PsExec, yielding a misleading
            # verdict=blocked.
            $psExecArgs = @(
                "\\$HostName",
                '-u', $UserResolved,
                '-p', $Password,
                '-accepteula',
                '-nobanner',
                '-h',
                'cmd.exe',
                '/c',
                'exit 0'
            )
            $stdoutAndErr = & $PsExecPath @psExecArgs 2>&1
            $exitCode = $LASTEXITCODE
            if ($exitCode -eq 0) {
                Add-Result 'psexec_probe' 'ok' 'PsExec registered SCM service + ran cmd /c exit 0 successfully'
            } else {
                $combined = ($stdoutAndErr -join ' | ')
                Add-Result 'psexec_probe' 'fail' "PsExec exit=$exitCode (likely UAC remote token filter blocking SCM): $combined"
            }
        } catch {
            Add-Result 'psexec_probe' 'fail' "PsExec invocation threw: $($_.Exception.Message)"
        }
    }
} else {
    Add-Result 'psexec_probe' 'skipped' 'use -WithPsExec to actually probe SCM service registration (writes 2 entries to target Event Log)'
}

# ----- Cleanup -----
if ($mounted) {
    try { Remove-PSDrive -Name $driveName -Force -ErrorAction SilentlyContinue } catch {}
}

# ----- Verdict -----
$tcp135Result = ($results | Where-Object { $_.name -eq 'tcp_135' })
$tcp445Result = ($results | Where-Object { $_.name -eq 'tcp_445' })
$mountResult  = ($results | Where-Object { $_.name -eq 'admin_share_mount' })
$writeResult  = ($results | Where-Object { $_.name -eq 'admin_share_write' })
$probeResult  = ($results | Where-Object { $_.name -eq 'psexec_probe' })

$verdict = 'uncertain'
$reason  = ''

if (-not $PsExecAvailable) {
    # operator-side prerequisite missing → Path B cannot run regardless of target state.
    # Returning 'uncertain' (not 'blocked') because the issue is on the operator side,
    # not the target. CLI handler maps both blocked + uncertain to non-zero exit.
    $verdict = 'uncertain'
    $reason  = "operator-side: PsExec64.exe missing (expected at '$PsExecPath'). Path B cannot run from this operator install — reinstall UECM or pass a valid --PsExecPath."
} elseif ($tcp135Result.status -ne 'ok') {
    $verdict = 'blocked'
    $reason  = 'TCP 135 (RPC EPM) unreachable — PsExec cannot reach Service Control Manager'
} elseif ($tcp445Result.status -ne 'ok') {
    $verdict = 'blocked'
    $reason  = 'TCP 445 (SMB) unreachable — PsExec cannot push its service binary'
} elseif ($mountResult.status -ne 'ok') {
    $verdict = 'blocked'
    $reason  = 'cannot mount ADMIN$ — credential rejected or share access denied'
} elseif ($writeResult.status -ne 'ok') {
    $verdict = 'blocked'
    $reason  = 'mounted ADMIN$ but cannot write Temp file — permission issue under ADMIN$'
} elseif ($probeResult.status -eq 'ok') {
    $verdict = 'viable'
    $reason  = 'all checks passed including PsExec SCM probe — Path B is fully viable'
} elseif ($probeResult.status -eq 'fail') {
    # Distinguish operator-side failure (PsExec64 missing on this machine) from
    # target-side blocker (UAC remote token filter / SCM access denied).
    if ($probeResult.message -like 'operator-side:*') {
        $verdict = 'uncertain'
        $reason  = "SCM probe could not run: $($probeResult.message). Target appears reachable but Path B viability cannot be confirmed without a working PsExec64."
    } else {
        $verdict = 'blocked'
        $reason  = 'TCP + SMB + ADMIN$ all OK but PsExec SCM probe failed — UAC remote token filter is the most likely cause; use Path A (USB bootstrap)'
    }
} else {
    # probe was 'skipped' (user did not pass -WithPsExec). All non-probe checks ok.
    $verdict = 'likely_viable'
    $reason  = 'network + SMB + ADMIN$ all reachable; SCM probe was skipped — run again with --probe for a definitive verdict'
}

@{
    ok       = ($verdict -eq 'viable' -or $verdict -eq 'likely_viable')
    verdict  = $verdict
    reason   = $reason
    results  = @($results)
} | ConvertTo-Json -Depth 6 -Compress
exit 0
