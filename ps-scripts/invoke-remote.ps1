# Runs an arbitrary PowerShell scriptblock on a remote host via WinRM.
# Stdin: the scriptblock body (UTF-8, no BOM).
# Returns: stdout from the remote scriptblock (the script is responsible for
# producing its own JSON if structured output is needed). On failure, prints
# JSON { error: string } to stderr and exits non-zero.
# Usage:
#   echo '<script body>' | powershell.exe -NoProfile -ExecutionPolicy Bypass -File invoke-remote.ps1 -Host RENDER-01

param(
    [Parameter(Mandatory=$true)]
    [string]$HostName
)

$ErrorActionPreference = 'Stop'

# Read entire stdin as the scriptblock body
$scriptText = [Console]::In.ReadToEnd()
if ([string]::IsNullOrWhiteSpace($scriptText)) {
    [Console]::Error.WriteLine((@{ error = "empty script body on stdin" } | ConvertTo-Json -Compress))
    exit 2
}

$scriptBlock = [scriptblock]::Create($scriptText)

try {
    Invoke-Command -ComputerName $HostName -ScriptBlock $scriptBlock -ErrorAction Stop
    exit 0
}
catch {
    [Console]::Error.WriteLine((@{ error = $_.Exception.Message } | ConvertTo-Json -Compress))
    exit 1
}
