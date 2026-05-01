# Tests WinRM connectivity to a remote host.
# Returns JSON: { ok: bool, message: string, latency_ms: int }
# Usage: powershell.exe -NoProfile -ExecutionPolicy Bypass -File test-winrm.ps1 -Host RENDER-01

param(
    [Parameter(Mandatory=$true)]
    [string]$HostName
)

$ErrorActionPreference = 'Stop'
$started = Get-Date

try {
    $null = Test-WSMan -ComputerName $HostName -ErrorAction Stop
    $elapsed = [int]((Get-Date) - $started).TotalMilliseconds
    @{
        ok = $true
        message = "WinRM reachable"
        latency_ms = $elapsed
    } | ConvertTo-Json -Compress
}
catch {
    $elapsed = [int]((Get-Date) - $started).TotalMilliseconds
    @{
        ok = $false
        message = $_.Exception.Message
        latency_ms = $elapsed
    } | ConvertTo-Json -Compress
}
