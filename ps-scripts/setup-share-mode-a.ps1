# Creates an open SMB share on a remote host with Everyone:Full access (Mode A).
# Used in closed on-set environments where the network is trusted.
# Parameters:
#   -HostName <string>             remote host (Mode A operates against host only)
#   -ShareName <string>            e.g. "DDC"
#   -LocalPath <string>            e.g. "D:\DDC" (will be created if missing)
#   [-Username <string>] [-Password <string>]    operator's admin cred for the host
# Output: JSON { ok, unc_path, message }
# Requires: WinRM session user (or supplied credential) is admin on the remote host.

param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$ShareName,
    [Parameter(Mandatory=$true)] [string]$LocalPath,
    [string]$Username,
    [string]$Password
)

$ErrorActionPreference = 'Stop'

function Build-CredentialOrNull {
    param([string]$User, [string]$Pass)
    if ([string]::IsNullOrEmpty($User) -or [string]::IsNullOrEmpty($Pass)) { return $null }
    if ($User -notmatch '[\\@]') {
        $User = ".\$User"
    }
    $secure = ConvertTo-SecureString -String $Pass -AsPlainText -Force
    return New-Object System.Management.Automation.PSCredential($User, $secure)
}

try {
    $script = {
        param($ShareName, $LocalPath)
        if (-not (Test-Path $LocalPath)) {
            New-Item -ItemType Directory -Path $LocalPath -Force | Out-Null
        }
        $guest = Get-LocalUser -Name 'Guest' -ErrorAction Stop
        if (-not $guest.Enabled) {
            Enable-LocalUser -Name 'Guest'
        }
        $regPath = 'HKLM:\SYSTEM\CurrentControlSet\Services\LanmanServer\Parameters'
        Set-ItemProperty -Path $regPath -Name 'AutoShareWks' -Value 1 -Type DWord -ErrorAction SilentlyContinue
        Set-ItemProperty -Path $regPath -Name 'RestrictNullSessAccess' -Value 0 -Type DWord -ErrorAction SilentlyContinue
        $existing = Get-SmbShare -Name $ShareName -ErrorAction SilentlyContinue
        if ($existing) {
            Remove-SmbShare -Name $ShareName -Force
        }
        New-SmbShare -Name $ShareName -Path $LocalPath -FullAccess 'Everyone' -Description 'UECM open DDC share (Mode A)' | Out-Null
        icacls $LocalPath /grant 'Everyone:(OI)(CI)F' | Out-Null
        return "\\$($env:COMPUTERNAME)\$ShareName"
    }
    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock  = $script
        ArgumentList = @($ShareName, $LocalPath)
        ErrorAction  = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $remoteResult = Invoke-Command @invokeArgs
    # Force-cast Deserialized.System.String to plain string before JSON
    # (same wrap-pattern fix as f6b40ff / 57c75d5).
    $unc = "$remoteResult"
    @{ ok = $true; unc_path = $unc; message = "Mode A share created: $unc" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; unc_path = ""; message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
