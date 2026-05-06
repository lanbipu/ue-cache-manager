param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$SourceUnc,
    [Parameter(Mandatory=$true)] [string]$TargetLocal,
    [Parameter(Mandatory=$true)] [string]$FileName,
    [string]$Username,
    [string]$Password,
    [string]$SourceSmbUser,
    [string]$SourceSmbPass,
    [switch]$PreflightOnly
)

$ErrorActionPreference = 'Stop'

function Build-CredentialOrNull {
    param([string]$User, [string]$Pass)
    if ([string]::IsNullOrEmpty($User) -or [string]::IsNullOrEmpty($Pass)) { return $null }
    if ($User -notmatch '[\\@]') { $User = ".\$User" }
    $secure = ConvertTo-SecureString -String $Pass -AsPlainText -Force
    return New-Object System.Management.Automation.PSCredential($User, $secure)
}

try {
    $script = {
        param($SourceUnc, $TargetLocal, $FileName, $SmbUser, $SmbPass, $PreflightOnly)
        if (-not (Test-Path -LiteralPath $TargetLocal)) {
            New-Item -Path $TargetLocal -ItemType Directory -Force | Out-Null
        }

        $driveName = "uecmpso$([System.Diagnostics.Process]::GetCurrentProcess().Id)"
        $smbCred = $null
        if (-not [string]::IsNullOrEmpty($SmbUser) -and -not [string]::IsNullOrEmpty($SmbPass)) {
            if ($SmbUser -notmatch '[\\@]') { $SmbUser = ".\$SmbUser" }
            $secure = ConvertTo-SecureString -String $SmbPass -AsPlainText -Force
            $smbCred = New-Object System.Management.Automation.PSCredential($SmbUser, $secure)
        }

        $mounted = $false
        try {
            if ($smbCred) {
                New-PSDrive -Name $driveName -PSProvider FileSystem -Root $SourceUnc -Credential $smbCred -ErrorAction Stop | Out-Null
                $mounted = $true
            }
            if (-not (Test-Path -LiteralPath $SourceUnc)) {
                throw "source UNC unreachable from target session: $SourceUnc"
            }
            if ($PreflightOnly) {
                return @{ ok = $true; exit_code = "0"; bytes_copied = "0"; stdout_tail = "preflight ok" }
            }

            $stdoutPath = Join-Path -Path $env:TEMP -ChildPath "robocopy-pso-stdout-$PID.log"
            $stderrPath = Join-Path -Path $env:TEMP -ChildPath "robocopy-pso-stderr-$PID.log"
            $args = @(
                "$SourceUnc",
                "$TargetLocal",
                "$FileName",
                '/R:3',
                '/W:5',
                '/NP',
                '/NDL',
                '/NJH',
                '/NJS',
                '/BYTES'
            )
            $proc = Start-Process -FilePath 'robocopy.exe' -ArgumentList $args -PassThru -Wait -NoNewWindow -RedirectStandardOutput $stdoutPath -RedirectStandardError $stderrPath
            $code = $proc.ExitCode
            $stdout = Get-Content -LiteralPath $stdoutPath -Raw -ErrorAction SilentlyContinue
            Remove-Item -LiteralPath $stdoutPath -ErrorAction SilentlyContinue
            Remove-Item -LiteralPath $stderrPath -ErrorAction SilentlyContinue

            $bytesCopied = 0
            try {
                $m = [regex]::Matches($stdout, 'Bytes\s*:\s*(\d+)')
                if ($m.Count -gt 0) { $bytesCopied = [long]$m[0].Groups[1].Value }
            } catch {}

            return @{
                ok = ($code -lt 8)
                exit_code = "$code"
                bytes_copied = "$bytesCopied"
                stdout_tail = if ($stdout) { ($stdout -split "`n" | Select-Object -Last 30) -join "`n" } else { "" }
            }
        }
        finally {
            if ($mounted) {
                Remove-PSDrive -Name $driveName -Force -ErrorAction SilentlyContinue
            }
        }
    }

    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock = $script
        ArgumentList = @($SourceUnc, $TargetLocal, $FileName, $SourceSmbUser, $SourceSmbPass, [bool]$PreflightOnly)
        ErrorAction = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $r = Invoke-Command @invokeArgs

    @{
        ok = "$($r.ok)" -eq "True"
        exit_code = "$($r.exit_code)"
        bytes_copied = "$($r.bytes_copied)"
        stdout_tail = "$($r.stdout_tail)"
    } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; exit_code = "-1"; bytes_copied = "0"; stdout_tail = ""; message = "$($_.Exception.Message)" } | ConvertTo-Json -Compress
    exit 1
}
