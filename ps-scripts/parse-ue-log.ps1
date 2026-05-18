# Runs UnrealEditor.exe in nullrhi mode with DDC verbose logging, captures the
# log file path, and returns the parsed log contents up to a configurable size
# cap. Designed to be called over WinRM via run_json.
param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$EditorExe,
    [Parameter(Mandatory=$true)] [string]$ProjectPath,
    [int]$TimeoutSeconds = 180,
    [int]$MaxLogBytes = 2097152,
    [string]$Username,
    [string]$Password
)
$ErrorActionPreference = 'Stop'

$script = {
    param($EditorExe, $ProjectPath, $TimeoutSeconds, $MaxLogBytes)
    if (-not (Test-Path -LiteralPath $EditorExe)) { throw "editor not found: $EditorExe" }
    if (-not (Test-Path -LiteralPath $ProjectPath)) { throw "project not found: $ProjectPath" }

    $logDir = Join-Path $env:TEMP "uecm-log-verify-$(Get-Random)"
    New-Item -ItemType Directory -Path $logDir -Force | Out-Null
    $logFile = Join-Path $logDir 'verify.log'

    $args = @(
        $ProjectPath,
        '-nullrhi',
        '-nosound',
        '-unattended',
        '-nopause',
        '-ExecCmds=quit',
        '-logcmds=LogDerivedDataCache Verbose',
        "-abslog=$logFile"
    )
    $proc = Start-Process -FilePath $EditorExe -ArgumentList $args -PassThru -WindowStyle Hidden
    if (-not $proc.WaitForExit($TimeoutSeconds * 1000)) {
        try { $proc.Kill() } catch {}
        throw "editor did not exit within $TimeoutSeconds s"
    }
    if (-not (Test-Path -LiteralPath $logFile)) { throw "log not produced at $logFile" }
    $size = (Get-Item $logFile).Length
    $content = if ($size -le $MaxLogBytes) {
        Get-Content -LiteralPath $logFile -Raw -Encoding UTF8
    } else {
        $bytes = [System.IO.File]::ReadAllBytes($logFile)
        $tail = $bytes[($bytes.Length - $MaxLogBytes)..($bytes.Length - 1)]
        [System.Text.Encoding]::UTF8.GetString($tail)
    }
    @{
        log_path = $logFile
        size = $size
        truncated = ($size -gt $MaxLogBytes)
        content = $content
        exit_code = $proc.ExitCode
    }
}

try {
    $result = if ($Username) {
        $pass = ConvertTo-SecureString $Password -AsPlainText -Force
        $cred = New-Object System.Management.Automation.PSCredential($Username, $pass)
        Invoke-Command -ComputerName $HostName -Credential $cred -Authentication Default `
            -ScriptBlock $script -ArgumentList $EditorExe, $ProjectPath, $TimeoutSeconds, $MaxLogBytes
    } else {
        Invoke-Command -ComputerName $HostName -ScriptBlock $script `
            -ArgumentList $EditorExe, $ProjectPath, $TimeoutSeconds, $MaxLogBytes
    }
    @{
        ok = $true
        log_path = $result.log_path
        size = $result.size
        truncated = $result.truncated
        content = $result.content
        exit_code = $result.exit_code
    } | ConvertTo-Json -Compress -Depth 4
} catch {
    @{ ok = $false; message = $_.Exception.Message } | ConvertTo-Json -Compress
}
