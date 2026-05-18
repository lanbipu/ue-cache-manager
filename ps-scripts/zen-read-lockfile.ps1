# Plan 7 T1.8 sidecar - CB binary (base64) with exclusive-lock workaround.
#
# Purpose:
#   Read <DataDir>\.lock while the zen daemon holds an exclusive
#   (no-share) lock on it. Normal [IO.File]::Open with any FileShare flag
#   fails - see docs/research/zen-launch-mechanism.md §8 + fact-find T0.4b /
#   T1.3. The workaround is Win32 CreateFile with FILE_FLAG_BACKUP_SEMANTICS,
#   which bypasses share-mode enforcement when SeBackupPrivilege is held by
#   the calling process token. The caller must therefore be running as an
#   Administrator (or member of the Backup Operators group on a domain box).
#
#   The raw bytes are returned base64-encoded; the Rust core
#   (core::zen::lockfile::parse_lockfile_bytes) does the CB decode.
#
# Parameters:
#   -DataDir <string>            absolute path to the zen data directory
#                                (the parent of .lock), e.g. F:\Epic\DDC\Zen
#
# Output (success):
#   {
#     "ok": true,
#     "data_dir": "F:\\Epic\\DDC\\Zen",
#     "lockfile_cb_b64": "<base64>",
#     "lockfile_size": 163,
#     "lockfile_sha256": "<lowercase hex>",
#     "note": "read via Win32 BackupRead (FILE_FLAG_BACKUP_SEMANTICS)"
#   }
#
# Output (failure - e.g. SeBackupPrivilege missing, GetLastError=1314):
#   {
#     "ok": false,
#     "data_dir": "F:\\Epic\\DDC\\Zen",
#     "message": "BackupRead failed: GetLastError=1314 (privilege not held); zen daemon holds an exclusive lock on .lock - re-run as Administrator or wait until zen exits"
#   }
#
# TODO (future work, out of scope for T1.8):
#   - Volume Shadow Copy fallback (`vssadmin create shadow /for=<volume>`)
#     for environments where the operator can't easily elevate.
#   - Auto-retry after the next zen exit by integrating with the
#     observed-PID telemetry from T1.4.
#
# Usage:
#   powershell.exe -NoProfile -ExecutionPolicy Bypass -File zen-read-lockfile.ps1 -DataDir F:\Epic\DDC\Zen

param(
    [Parameter(Mandatory=$true)] [string]$DataDir
)

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
chcp 65001 | Out-Null

$ErrorActionPreference = 'Stop'

# Win32 API surface. Kept minimal: just CreateFile / ReadFile / CloseHandle.
# We don't actually need the BackupRead() call - FILE_FLAG_BACKUP_SEMANTICS
# during CreateFile is the bit that lets us open a file already opened with
# zero share mode. Once the handle is open the normal ReadFile path works.
if (-not ('UecmZen.LockfileReader' -as [type])) {
    Add-Type -Namespace UecmZen -Name LockfileReader -MemberDefinition @"
[System.Runtime.InteropServices.DllImport("kernel32.dll", SetLastError=true, CharSet=System.Runtime.InteropServices.CharSet.Unicode)]
public static extern System.IntPtr CreateFileW(
    string lpFileName,
    uint dwDesiredAccess,
    uint dwShareMode,
    System.IntPtr lpSecurityAttributes,
    uint dwCreationDisposition,
    uint dwFlagsAndAttributes,
    System.IntPtr hTemplateFile);

[System.Runtime.InteropServices.DllImport("kernel32.dll", SetLastError=true)]
public static extern bool ReadFile(
    System.IntPtr hFile,
    byte[] lpBuffer,
    uint nNumberOfBytesToRead,
    out uint lpNumberOfBytesRead,
    System.IntPtr lpOverlapped);

[System.Runtime.InteropServices.DllImport("kernel32.dll", SetLastError=true)]
public static extern bool CloseHandle(System.IntPtr hObject);

[System.Runtime.InteropServices.DllImport("kernel32.dll", SetLastError=true)]
public static extern uint GetFileSize(System.IntPtr hFile, System.IntPtr lpFileSizeHigh);
"@
}

function Read-LockedFileBytes {
    param([string]$Path)

    # CreateFile constants. Cast via [System.Convert] to avoid PS 5.1's
    # signed-int overflow when literals like 0x80000000 / 0x02000000 are
    # parsed as [int] first.
    $GENERIC_READ              = [System.Convert]::ToUInt32('80000000', 16)
    $FILE_SHARE_READ_WRITE_DEL = [uint32]7   # READ(1) | WRITE(2) | DELETE(4)
    $OPEN_EXISTING             = [uint32]3
    $FILE_FLAG_BACKUP_SEMANTICS = [System.Convert]::ToUInt32('02000000', 16)

    $handle = [UecmZen.LockfileReader]::CreateFileW(
        $Path,
        $GENERIC_READ,
        $FILE_SHARE_READ_WRITE_DEL,
        [System.IntPtr]::Zero,
        $OPEN_EXISTING,
        $FILE_FLAG_BACKUP_SEMANTICS,
        [System.IntPtr]::Zero
    )
    # INVALID_HANDLE_VALUE is (HANDLE)(-1). Compare via ToInt64 so we work on
    # both x86 and x64 PowerShell hosts.
    if ($handle.ToInt64() -eq -1) {
        $err = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()
        # 1314 = ERROR_PRIVILEGE_NOT_HELD. Surface a hint about elevation.
        $hint = ''
        if ($err -eq 1314) { $hint = ' (privilege not held)' }
        elseif ($err -eq 5) { $hint = ' (access denied)' }
        elseif ($err -eq 2) { $hint = ' (file not found)' }
        elseif ($err -eq 32) { $hint = ' (sharing violation - exclusive lock without backup semantics)' }
        throw "CreateFileW failed: GetLastError=$err$hint"
    }

    try {
        # File size first so we know how much to read in one shot. .lock is
        # tiny (~150 bytes) but allocating per-actual-size keeps the script
        # honest if zen ever grows the format.
        $sizeLow = [UecmZen.LockfileReader]::GetFileSize($handle, [System.IntPtr]::Zero)
        if ($sizeLow -eq [uint32]::MaxValue) {
            # INVALID_FILE_SIZE: per MSDN, ALSO check GetLastError - a 4 GB
            # file legitimately returns 0xFFFFFFFF in the low DWORD with
            # GetLastError() == NO_ERROR. .lock will never be that large but
            # the docs are documented to behave this way.
            $err = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()
            if ($err -ne 0) {
                throw "GetFileSize failed: GetLastError=$err"
            }
        }
        if ($sizeLow -gt 16777216) {
            # 16 MiB sanity cap. .lock is ~150 bytes today; if it ever balloons
            # past 16 MiB something else is wrong and we'd rather bail than
            # OOM the runner.
            throw "lockfile too large ($sizeLow bytes) - refusing to read"
        }

        $buffer = New-Object byte[] $sizeLow
        $bytesRead = [uint32]0
        $ok = [UecmZen.LockfileReader]::ReadFile($handle, $buffer, $sizeLow, [ref]$bytesRead, [System.IntPtr]::Zero)
        if (-not $ok) {
            $err = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()
            throw "ReadFile failed: GetLastError=$err"
        }
        if ($bytesRead -lt $sizeLow) {
            # Short read - resize the buffer rather than ship trailing zero
            # bytes that would confuse the CB parser.
            $trimmed = New-Object byte[] $bytesRead
            [System.Array]::Copy($buffer, 0, $trimmed, 0, [int]$bytesRead)
            return ,$trimmed
        }
        return ,$buffer
    }
    finally {
        [void][UecmZen.LockfileReader]::CloseHandle($handle)
    }
}

function Get-Sha256OfBytes {
    param([byte[]]$Bytes)
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        $hash = $sha.ComputeHash($Bytes)
        return ([System.BitConverter]::ToString($hash) -replace '-', '').ToLowerInvariant()
    }
    finally {
        $sha.Dispose()
    }
}

try {
    if ([string]::IsNullOrWhiteSpace($DataDir)) {
        throw "DataDir is empty"
    }
    if (-not (Test-Path -LiteralPath $DataDir)) {
        throw "DataDir does not exist: $DataDir"
    }
    $lockPath = Join-Path -Path $DataDir -ChildPath '.lock'
    if (-not (Test-Path -LiteralPath $lockPath)) {
        @{
            ok = $false
            data_dir = $DataDir
            message = "lockfile not present (zen never wrote it, or data dir is wrong): $lockPath"
            locked_by_running_zen = $false
        } | ConvertTo-Json -Compress
        exit 0
    }

    $bytes = Read-LockedFileBytes -Path $lockPath
    if ($null -eq $bytes -or $bytes.Length -eq 0) {
        # Empty .lock is suspicious but not impossible (zen mid-write). Surface
        # it as ok=true with size=0; Rust will see a CB parse error and treat
        # it as "lock exists but unreadable yet".
        @{
            ok = $true
            data_dir = $DataDir
            lockfile_cb_b64 = ""
            lockfile_size = 0
            lockfile_sha256 = $null
            note = "read via Win32 BackupRead (FILE_FLAG_BACKUP_SEMANTICS); file is empty"
        } | ConvertTo-Json -Compress
        exit 0
    }

    $b64 = [Convert]::ToBase64String($bytes)
    $sha = Get-Sha256OfBytes -Bytes $bytes

    @{
        ok = $true
        data_dir = $DataDir
        lockfile_cb_b64 = $b64
        lockfile_size = $bytes.Length
        lockfile_sha256 = $sha
        note = "read via Win32 BackupRead (FILE_FLAG_BACKUP_SEMANTICS)"
    } | ConvertTo-Json -Compress
}
catch {
    # IMPORTANT: do NOT `exit 1`. The Rust caller (winrm::invoke_json) treats
    # non-zero exit as a hard error and never parses stdout, so the structured
    # `{ ok:false, ... }` envelope would be discarded and the caller would see
    # nothing. Keep exit code 0 for all expected failure paths; the `ok` flag
    # in JSON tells the caller what actually happened.
    $msg = "$($_.Exception.Message)"
    $isLockedByZen = $false
    if ($msg -match 'GetLastError=1314') {
        # ERROR_PRIVILEGE_NOT_HELD — Backup semantics requires SeBackupPrivilege.
        $msg = "$msg; re-run as Administrator (FILE_FLAG_BACKUP_SEMANTICS needs SeBackupPrivilege)"
    }
    elseif ($msg -match 'GetLastError=32') {
        # ERROR_SHARING_VIOLATION. Windows share-mode enforcement applies even
        # to BackupRead handles: if zen opened .lock with FileShare::None
        # (the default UE 5.4+ lockfile behaviour), no flag combination can
        # bypass it. Caller should fall back to /health/info CB which carries
        # the same EffectivePort / Pid / Executable fields while zen is alive.
        $isLockedByZen = $true
        $msg = "zen holds exclusive lock on .lock (ERROR_SHARING_VIOLATION); use /health/info via probe.rs while zen is running — this script only succeeds when zen has exited"
    }
    @{
        ok = $false
        data_dir = $DataDir
        message = $msg
        locked_by_running_zen = $isLockedByZen
    } | ConvertTo-Json -Compress
    exit 0
}
