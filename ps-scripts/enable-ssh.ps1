# Enables Windows OpenSSH Server for UECM SSH transport onboarding.
# Run locally on the target as Administrator. Idempotent; safe to re-run.
# Emits JSON { ok, changes, message } and exits 0 (ok) / 1 (failed).
#
# SSH is the UECM transport going forward; this runs alongside enable-winrm.ps1
# during migration (WinRM is removed in a later phase).
param(
    [string]$PublicKeyPath = '',
    [string]$UecmPublicKey = '',
    [string]$StagingSourceDir = '',
    [switch]$CheckOnly
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$changes = New-Object System.Collections.ArrayList
function Note($m) { [void]$changes.Add($m) }
$adminKeys = 'C:\ProgramData\ssh\administrators_authorized_keys'
$uecmDir = 'C:\ProgramData\UECM'
$staging = 'C:\ProgramData\UECM\ps-scripts'

try {
    if (-not $StagingSourceDir) { $StagingSourceDir = Split-Path -Parent $PSCommandPath }
    if (-not $PublicKeyPath) { $PublicKeyPath = Join-Path $StagingSourceDir 'uecm.pub' }

    # 1. resolve UECM public key
    $pub = ''
    if ($UecmPublicKey) { $pub = $UecmPublicKey.Trim() }
    elseif (Test-Path $PublicKeyPath) { $pub = (Get-Content -Raw $PublicKeyPath).Trim() }
    if (-not $pub) { throw "no UECM public key (set -UecmPublicKey or place uecm.pub at $PublicKeyPath)" }
    if ($pub -notmatch '^ssh-(ed25519|rsa) ') { throw "value does not look like an OpenSSH public key" }

    # 2. OpenSSH Server capability
    $cap = Get-WindowsCapability -Online -Name 'OpenSSH.Server*' -ErrorAction SilentlyContinue
    $capInstalled = ($cap -and $cap.State -eq 'Installed')
    if ($cap -and -not $capInstalled) {
        if (-not $CheckOnly) {
            Add-WindowsCapability -Online -Name $cap.Name | Out-Null
            $capInstalled = $true
        }
        Note "installed OpenSSH.Server"
    }
    elseif (-not $cap) {
        Note "WARNING: OpenSSH.Server capability not found (older Windows: install Win32-OpenSSH manually)"
    }

    # 3. services + firewall
    if (-not $CheckOnly) {
        Set-Service -Name sshd -StartupType Automatic -ErrorAction SilentlyContinue
        Set-Service -Name ssh-agent -StartupType Automatic -ErrorAction SilentlyContinue
        Start-Service sshd -ErrorAction SilentlyContinue
        $fw = Get-NetFirewallRule -Name 'OpenSSH-Server-In-TCP' -ErrorAction SilentlyContinue
        if (-not $fw) {
            New-NetFirewallRule -Name 'OpenSSH-Server-In-TCP' -DisplayName 'OpenSSH Server (sshd)' `
                -Enabled True -Direction Inbound -Protocol TCP -Action Allow -LocalPort 22 | Out-Null
            Note "added firewall rule TCP/22"
        }
        elseif ($fw.Enabled -ne 'True') {
            Enable-NetFirewallRule -Name 'OpenSSH-Server-In-TCP' | Out-Null
            Note "enabled existing firewall rule TCP/22"
        }
    }

    # 4. authorize UECM pubkey in administrators_authorized_keys
    #    (uecm-svc is a local admin, so Windows OpenSSH uses this shared file,
    #     not per-user ~/.ssh/authorized_keys). Only enforce the strict ACL on a
    #     freshly created file; never rewrite an existing one (avoids clobbering
    #     a working file's ACL and locking out other authorized keys).
    if (-not $CheckOnly) {
        $keyFileExisted = Test-Path $adminKeys
        $existing = if ($keyFileExisted) { Get-Content $adminKeys } else { @() }
        if ($existing -notcontains $pub) {
            Add-Content -Path $adminKeys -Value $pub -Encoding ascii
            Note "authorized UECM key"
        }
        if (-not $keyFileExisted) {
            icacls $adminKeys /setowner 'BUILTIN\Administrators' | Out-Null
        }
        # Windows OpenSSH ignores admin key files with loose ACLs, so always enforce
        # the canonical secure ACL (SYSTEM + Administrators only) on both fresh and
        # existing files. Single atomic icacls (one DACL write); /grant:r governs file
        # permissions only and never invalidates keys already inside the file.
        icacls $adminKeys /inheritance:r /grant:r 'SYSTEM:(F)' 'BUILTIN\Administrators:(F)' | Out-Null
        Note "enforced authorized_keys ACL"
    }

    # 5. staging dir + copy node scripts (exclude enable-* and self)
    if (-not $CheckOnly) {
        if (-not (Test-Path $staging)) { New-Item -ItemType Directory -Path $staging -Force | Out-Null }
        Get-ChildItem -Path $StagingSourceDir -Filter '*.ps1' -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -notlike 'enable-*' -and $_.FullName -ne $PSCommandPath } |
            ForEach-Object { Copy-Item $_.FullName -Destination $staging -Force }
        Note "staged node scripts -> $staging"
    }

    # 6. install PsExec64 (required by inject-system-credential.ps1 to write the
    #    SYSTEM-account cmdkey). It ships in the bootstrap package next to this
    #    script; copy it to the machine-wide UECM dir so node-pure scripts resolve
    #    it deterministically at C:\ProgramData\UECM\PsExec64.exe. Missing PsExec
    #    does not fail onboarding (SSH itself is up); inject fails with a clear
    #    message later if it was never staged.
    if (-not $CheckOnly) {
        $psexecSrc = Join-Path $StagingSourceDir 'PsExec64.exe'
        if (Test-Path -LiteralPath $psexecSrc) {
            if (-not (Test-Path $uecmDir)) { New-Item -ItemType Directory -Path $uecmDir -Force | Out-Null }
            Copy-Item -LiteralPath $psexecSrc -Destination (Join-Path $uecmDir 'PsExec64.exe') -Force
            Note "installed PsExec64 -> $uecmDir"
        }
        else {
            Note "WARNING: PsExec64.exe not in bootstrap package; SYSTEM credential injection unavailable until staged"
        }
    }

    # Readiness reflects ACTUAL prerequisites (correct for -CheckOnly too, which
    # mutates nothing): OpenSSH installed + sshd running + UECM key authorized.
    $sshd = Get-Service sshd -ErrorAction SilentlyContinue
    $sshdRunning = ($sshd -and $sshd.Status -eq 'Running')
    $keyAuthorized = (Test-Path $adminKeys) -and ((Get-Content $adminKeys) -contains $pub)
    $ok = $capInstalled -and $sshdRunning -and $keyAuthorized
    if ($ok) {
        $msg = "SSH onboarding complete"
    }
    else {
        $missing = @()
        if (-not $capInstalled) { $missing += 'OpenSSH.Server not installed' }
        if (-not $sshdRunning) { $missing += 'sshd not running' }
        if (-not $keyAuthorized) { $missing += 'UECM key not authorized' }
        $msg = "not ready: " + ($missing -join '; ')
    }
    @{ ok = $ok; changes = $changes; message = $msg } | ConvertTo-Json -Depth 6 -Compress
    exit $(if ($ok) { 0 } else { 1 })
}
catch {
    @{ ok = $false; changes = $changes; message = $_.Exception.Message } | ConvertTo-Json -Depth 6 -Compress
    exit 1
}
