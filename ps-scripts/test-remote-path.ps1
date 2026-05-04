# Tests whether a remote machine can resolve a path.
# Output: JSON { ok, exists, path, message }

param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$Path,
    [string]$Username,
    [string]$Password
)

$ErrorActionPreference = 'Stop'

function Build-CredentialOrNull {
    param([string]$User, [string]$Pass)
    if ([string]::IsNullOrEmpty($User) -or [string]::IsNullOrEmpty($Pass)) { return $null }
    if ($User -notmatch '[\\@]') { $User = ".\$User" }
    $secure = ConvertTo-SecureString -String $Pass -AsPlainText -Force
    return New-Object System.Management.Automation.PSCredential($User, $secure)
}

try {
    $script = {
        param($Path)
        return [bool](Test-Path -LiteralPath $Path)
    }
    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock  = $script
        ArgumentList = @($Path)
        ErrorAction  = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $remoteResult = Invoke-Command @invokeArgs
    $exists = [System.Convert]::ToBoolean("$remoteResult")
    @{ ok = $true; exists = $exists; path = $Path; message = "" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; exists = $false; path = $Path; message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
