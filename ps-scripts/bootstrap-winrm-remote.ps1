# Attempts first-contact WinRM bootstrap through Windows admin channels.
#
# Requirements:
# - Operator machine can reach \\<HostName>\ADMIN$ and Service Control Manager/RPC.
# - Username/Password is an administrator on the target machine.
# - PsExec64.exe is available on the operator machine.
#
# If this fails, UECM should show enable-winrm.ps1 as the manual fallback.

param(
    [Parameter(Mandatory=$true)]
    [string]$HostName,

    [Parameter(Mandatory=$true)]
    [string]$Username,

    [Parameter(Mandatory=$true)]
    [string]$Password,

    [Parameter(Mandatory=$true)]
    [string]$PsExecPath,

    [Parameter(Mandatory=$true)]
    [string]$LocalScriptPath,

    [switch]$EnableLocalAccountRemoteAdmin,

    # Full render-node provisioning switches, forwarded to the remote enable-winrm.ps1.
    [switch]$EnableSmbServer,
    [switch]$EnableWmi,
    [switch]$EnableLongPaths,
    [string]$SetExecutionPolicy,
    [string]$PowerProfile
)

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
try { chcp 65001 | Out-Null } catch {}

$ErrorActionPreference = 'Stop'

function Test-UecmRemoteWsMan {
    param([string]$ComputerName)
    try {
        $null = Test-WSMan -ComputerName $ComputerName -ErrorAction Stop
        return $true
    }
    catch {
        return $false
    }
}

function New-UecmCredential {
    param([string]$User, [string]$Pass)
    $secure = ConvertTo-SecureString -String $Pass -AsPlainText -Force
    return New-Object System.Management.Automation.PSCredential($User, $secure)
}

function Set-UecmOperatorTrustedHost {
    param([string]$Target)

    $path = 'WSMan:\localhost\Client\TrustedHosts'
    try {
        $current = (Get-Item -Path $path -ErrorAction Stop).Value
        if ($current -eq '*') {
            return 'operator TrustedHosts already allows all hosts'
        }

        $entries = @()
        if (-not [string]::IsNullOrWhiteSpace($current)) {
            $entries = @(
                $current -split ',' |
                    ForEach-Object { $_.Trim() } |
                    Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
            )
        }

        if ($entries -contains $Target) {
            return "operator TrustedHosts already contains $Target"
        }

        $entries += $Target
        Set-Item -Path $path -Value ($entries -join ',') -Force -ErrorAction Stop
        return "operator TrustedHosts added $Target"
    }
    catch {
        throw "operator TrustedHosts update failed for $Target. Run UECM as Administrator or add $Target to WSMan TrustedHosts manually. $($_.Exception.Message)"
    }
}

$driveName = "UECMBootstrap$([System.Guid]::NewGuid().ToString('N').Substring(0, 8))"
$remoteScriptName = 'UECM-Bootstrap-WinRM.ps1'
$remoteWindowsPath = "%SystemRoot%\Temp\$remoteScriptName"
$remoteDrivePath = "${driveName}:\Temp\$remoteScriptName"

try {
    if (-not (Test-Path $PsExecPath)) {
        throw "PsExec64.exe not found at $PsExecPath"
    }
    if (-not (Test-Path $LocalScriptPath)) {
        throw "local bootstrap script not found at $LocalScriptPath"
    }

    $credential = New-UecmCredential -User $Username -Pass $Password
    New-PSDrive `
        -Name $driveName `
        -PSProvider FileSystem `
        -Root "\\$HostName\ADMIN$" `
        -Credential $credential `
        -ErrorAction Stop | Out-Null

    Copy-Item -Path $LocalScriptPath -Destination $remoteDrivePath -Force -ErrorAction Stop

    $remoteArgs = @(
        "\\$HostName",
        '-u', $Username,
        '-p', $Password,
        '-accepteula',
        '-nobanner',
        '-h',
        'cmd.exe',
        '/c',
        'powershell.exe',
        '-NoProfile',
        '-ExecutionPolicy',
        'Bypass',
        '-File',
        $remoteWindowsPath
    )
    if ($EnableLocalAccountRemoteAdmin) {
        $remoteArgs += '-EnableLocalAccountRemoteAdmin'
    }
    if ($EnableSmbServer) { $remoteArgs += '-EnableSmbServer' }
    if ($EnableWmi) { $remoteArgs += '-EnableWmi' }
    if ($EnableLongPaths) { $remoteArgs += '-EnableLongPaths' }
    if ($SetExecutionPolicy) { $remoteArgs += @('-SetExecutionPolicy', $SetExecutionPolicy) }
    if ($PowerProfile) { $remoteArgs += @('-PowerProfile', $PowerProfile) }

    $output = (& $PsExecPath @remoteArgs 2>&1) -join "`n"
    $trustedHostsChange = Set-UecmOperatorTrustedHost -Target $HostName
    $winrmOk = Test-UecmRemoteWsMan -ComputerName $HostName

    if (-not $winrmOk) {
        throw "remote bootstrap command ran, but Test-WSMan $HostName still failed. Output: $output"
    }

    @{
        ok = $true
        method = 'psexec'
        message = 'WinRM enabled through ADMIN$ + PsExec bootstrap'
        winrm_ok = $true
        changed = @('copied enable-winrm.ps1 to target', 'ran target bootstrap through PsExec', $trustedHostsChange)
        manual_script = $null
        raw_output = $output
    } | ConvertTo-Json -Depth 6 -Compress
    exit 0
}
catch {
    @{
        ok = $false
        method = 'psexec'
        message = $_.Exception.Message
        winrm_ok = Test-UecmRemoteWsMan -ComputerName $HostName
        changed = @()
        manual_script = $null
    } | ConvertTo-Json -Depth 6 -Compress
    exit 1
}
finally {
    try {
        if (Get-PSDrive -Name $driveName -ErrorAction SilentlyContinue) {
            Remove-Item -Path $remoteDrivePath -Force -ErrorAction SilentlyContinue
            Remove-PSDrive -Name $driveName -Force -ErrorAction SilentlyContinue
        }
    }
    catch {}
}
