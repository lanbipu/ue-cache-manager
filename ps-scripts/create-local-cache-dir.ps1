# Creates the local DDC directory on a remote host with permissive ACLs so
# both the operator account and SYSTEM (RenderStream Service) can read/write.
param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$LocalPath,
    [string]$ServiceAccount,
    [string]$Username,
    [string]$Password
)
$ErrorActionPreference = 'Stop'

$script = {
    param($LocalPath, $ServiceAccount)
    if (-not (Test-Path -LiteralPath $LocalPath)) {
        New-Item -ItemType Directory -Path $LocalPath -Force | Out-Null
    }
    # SYSTEM full control (RenderStream / Windows service contexts)
    icacls $LocalPath /grant 'SYSTEM:(OI)(CI)F' /T /C | Out-Null
    icacls $LocalPath /grant 'Administrators:(OI)(CI)F' /T /C | Out-Null
    if ($ServiceAccount) {
        icacls $LocalPath /grant "${ServiceAccount}:(OI)(CI)F" /T /C | Out-Null
    }
    $info = Get-Item -LiteralPath $LocalPath
    @{ path = $info.FullName; created_at = $info.CreationTime.ToString('o') }
}

try {
    $result = if ($Username) {
        $pass = ConvertTo-SecureString $Password -AsPlainText -Force
        $cred = New-Object System.Management.Automation.PSCredential($Username, $pass)
        Invoke-Command -ComputerName $HostName -Credential $cred -Authentication Default `
            -ScriptBlock $script -ArgumentList $LocalPath, $ServiceAccount
    } else {
        Invoke-Command -ComputerName $HostName -ScriptBlock $script `
            -ArgumentList $LocalPath, $ServiceAccount
    }
    @{ ok = $true; message = "created $($result.path)"; path = $result.path } | ConvertTo-Json -Compress
} catch {
    @{ ok = $false; message = $_.Exception.Message } | ConvertTo-Json -Compress
}
