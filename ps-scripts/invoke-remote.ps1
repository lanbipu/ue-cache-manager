# Runs an arbitrary PowerShell scriptblock on a remote host via WinRM.
# Stdin: the scriptblock body (UTF-8, no BOM).
# Returns: stdout from the remote scriptblock (the script is responsible for
# producing its own JSON if structured output is needed). On failure, prints
# JSON { error: string } to stderr and exits non-zero.
# Usage:
#   echo '<script body>' | powershell.exe -NoProfile -ExecutionPolicy Bypass -File invoke-remote.ps1 -Host RENDER-01
# Optional: -Username <string> -Password <string> for explicit credential auth.

param(
    [Parameter(Mandatory=$true)]
    [string]$HostName,
    [string]$Username,
    [string]$Password,
    [ValidateSet('Negotiate','Basic','Kerberos','CredSSP','Digest')]
    [string]$AuthMethod = 'Negotiate'
)

[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; chcp 65001 | Out-Null

$ErrorActionPreference = 'Stop'

function Build-CredentialOrNull {
    param([string]$User, [string]$Pass)
    if ([string]::IsNullOrEmpty($User) -or [string]::IsNullOrEmpty($Pass)) { return $null }
    $User = $User.Trim()
    if ([string]::IsNullOrEmpty($User)) { return $null }
    if ($User.StartsWith("./")) { $User = ".\" + $User.Substring(2) }
    elseif ($User -notmatch '[\\@]') { $User = ".\$User" }
    $secure = ConvertTo-SecureString -String $Pass -AsPlainText -Force
    return New-Object System.Management.Automation.PSCredential($User, $secure)
}

# Read entire stdin as the scriptblock body
$scriptText = [Console]::In.ReadToEnd()
if ([string]::IsNullOrWhiteSpace($scriptText)) {
    [Console]::Error.WriteLine((@{ error = "empty script body on stdin" } | ConvertTo-Json -Compress))
    exit 2
}

$scriptBlock = [scriptblock]::Create($scriptText)

try {
    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock  = $scriptBlock
        ErrorAction  = 'Stop'
        Authentication = $AuthMethod
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    Invoke-Command @invokeArgs
    exit 0
}
catch {
    [Console]::Error.WriteLine((@{ error = $_.Exception.Message } | ConvertTo-Json -Compress))
    exit 1
}
