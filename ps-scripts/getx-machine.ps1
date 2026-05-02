# Reads a system-level environment variable on a remote host via WinRM.
# Parameters: -HostName <string> -Name <string>
# Output: JSON { ok: bool, value: string|null, message: string }

param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$Name
)

$ErrorActionPreference = 'Stop'

try {
    $remoteValue = Invoke-Command -ComputerName $HostName -ScriptBlock {
        param($Name)
        [System.Environment]::GetEnvironmentVariable($Name, 'Machine')
    } -ArgumentList $Name -ErrorAction Stop

    # Invoke-Command wraps the returned string in a Deserialized.System.String
    # PSObject (carrying PSComputerName/RunspaceId/PSShowComputerName metadata).
    # ConvertTo-Json would emit `value` as a nested object, which Rust's
    # Option<String> cannot deserialize. Force-cast to plain string (or $null
    # when the env var does not exist) before JSON serialization.
    if ($null -eq $remoteValue) {
        $value = $null
    } else {
        $value = "$remoteValue"
    }

    @{ ok = $true; value = $value; message = "" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; value = $null; message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
