# Sets a system-level environment variable on a remote host via WinRM.
# Parameters: -HostName <string> -Name <string> -Value <string>
# Output: JSON { ok: bool, message: string }
# Uses [System.Environment]::SetEnvironmentVariable with "Machine" target.
# Requires the WinRM session user to be admin on the remote host.

param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$Name,
    [Parameter(Mandatory=$true)] [string]$Value
)

$ErrorActionPreference = 'Stop'

try {
    $script = {
        param($Name, $Value)
        [System.Environment]::SetEnvironmentVariable($Name, $Value, 'Machine')
        # Verify by reading back
        $readback = [System.Environment]::GetEnvironmentVariable($Name, 'Machine')
        if ($readback -ne $Value) {
            throw "verify failed: read '$readback', expected '$Value'"
        }
        return $true
    }
    Invoke-Command -ComputerName $HostName -ScriptBlock $script -ArgumentList $Name, $Value -ErrorAction Stop | Out-Null
    @{ ok = $true; message = "set $Name on $HostName" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
