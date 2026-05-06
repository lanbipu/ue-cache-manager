# Reads a system-level environment variable on a remote host via WinRM.
# Parameters: -HostName <string> -Name <string>
#             [-Username <string>] [-Password <string>]
# Output: JSON { ok: bool, value: string|null, message: string }

param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$Name,
    [string]$Username,
    [string]$Password
)

[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; chcp 65001 | Out-Null

$ErrorActionPreference = 'Stop'

function Build-CredentialOrNull {
    param([string]$User, [string]$Pass)
    if ([string]::IsNullOrEmpty($User) -or [string]::IsNullOrEmpty($Pass)) { return $null }
    $User = $User.Trim()
    if ([string]::IsNullOrEmpty($User)) { return $null }
    if ($User.StartsWith(".\") -or $User.StartsWith("./")) { $User = $User.Substring(2) }
    $secure = ConvertTo-SecureString -String $Pass -AsPlainText -Force
    return New-Object System.Management.Automation.PSCredential($User, $secure)
}

try {
    $script = {
        param($Name)
        [System.Environment]::GetEnvironmentVariable($Name, 'Machine')
    }
    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock  = $script
        ArgumentList = @($Name)
        ErrorAction  = 'Stop'
        Authentication = 'Negotiate'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $remoteValue = Invoke-Command @invokeArgs

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
