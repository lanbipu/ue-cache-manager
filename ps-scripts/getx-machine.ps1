# Reads a system-level environment variable on a remote host via WinRM.
# Parameters: -HostName <string> -Name <string>
# Output: JSON { ok: bool, value: string|null, message: string }

param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$Name
)

$ErrorActionPreference = 'Stop'

try {
    $value = Invoke-Command -ComputerName $HostName -ScriptBlock {
        param($Name)
        [System.Environment]::GetEnvironmentVariable($Name, 'Machine')
    } -ArgumentList $Name -ErrorAction Stop
    @{ ok = $true; value = $value; message = "" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; value = $null; message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
