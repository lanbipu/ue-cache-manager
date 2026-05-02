# Sets a system-level environment variable on a remote host via WinRM.
# Parameters: -HostName <string> -Name <string> -Value <string>
#             [-Username <string>] [-Password <string>]
# Output: JSON { ok: bool, message: string }

param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$Name,
    [Parameter(Mandatory=$true)] [string]$Value,
    [string]$Username,
    [string]$Password
)

$ErrorActionPreference = 'Stop'

function Build-CredentialOrNull {
    param([string]$User, [string]$Pass)
    if ([string]::IsNullOrEmpty($User) -or [string]::IsNullOrEmpty($Pass)) { return $null }
    $secure = ConvertTo-SecureString -String $Pass -AsPlainText -Force
    return New-Object System.Management.Automation.PSCredential($User, $secure)
}

try {
    $script = {
        param($Name, $Value)
        [System.Environment]::SetEnvironmentVariable($Name, $Value, 'Machine')
        $readback = [System.Environment]::GetEnvironmentVariable($Name, 'Machine')
        if ($readback -ne $Value) { throw "verify failed: read '$readback', expected '$Value'" }
        return $true
    }
    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock  = $script
        ArgumentList = @($Name, $Value)
        ErrorAction  = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    Invoke-Command @invokeArgs | Out-Null
    @{ ok = $true; message = "set $Name on $HostName" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
