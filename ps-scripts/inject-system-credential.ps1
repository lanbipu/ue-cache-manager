# Injects a credential into both the current user's Credential Manager AND
# the SYSTEM account's Credential Manager (so that services running as
# LocalSystem can use it to authenticate to a SMB share).
# Parameters:
#   -ClientHostName <string>    the client machine that needs the credential
#   -TargetHost <string>        the SMB host the credential authenticates to
#   -SvcUsername <string>       e.g. "ddc-svc"
#   -SvcPassword <string>
#   [-Username <string>] [-Password <string>]   operator admin cred for client
#   [-PsExecPath <string>]      override path to PsExec64.exe; defaults to
#                               ../vendor/PsExec64.exe relative to this script
# Output: JSON { ok, message }
# Caveat: PsExec64.exe must exist on the CLIENT machine — Invoke-Command runs
# the scriptblock there, so the operator must pre-stage it (or rely on UECM's
# Tauri resource bundling which places it at <exe-dir>\vendor\PsExec64.exe).

param(
    [Parameter(Mandatory=$true)] [string]$ClientHostName,
    [Parameter(Mandatory=$true)] [string]$TargetHost,
    [Parameter(Mandatory=$true)] [string]$SvcUsername,
    [Parameter(Mandatory=$true)] [string]$SvcPassword,
    [string]$Username,
    [string]$Password,
    [string]$PsExecPath = ""
)

$ErrorActionPreference = 'Stop'

function Build-CredentialOrNull {
    param([string]$User, [string]$Pass)
    if ([string]::IsNullOrEmpty($User) -or [string]::IsNullOrEmpty($Pass)) { return $null }
    if ($User -notmatch '[\\@]') {
        $User = ".\$User"
    }
    $secure = ConvertTo-SecureString -String $Pass -AsPlainText -Force
    return New-Object System.Management.Automation.PSCredential($User, $secure)
}

if ([string]::IsNullOrEmpty($PsExecPath)) {
    $PsExecPath = Join-Path (Split-Path -Parent $PSScriptRoot) 'vendor\PsExec64.exe'
}
if (-not (Test-Path $PsExecPath)) {
    @{ ok = $false; message = "PsExec64.exe not found at $PsExecPath" } | ConvertTo-Json -Compress
    exit 1
}

try {
    $script = {
        param($TargetHost, $SvcUsername, $SvcPassword, $PsExecPath)
        & cmdkey.exe "/add:$TargetHost" "/user:$SvcUsername" "/pass:$SvcPassword" | Out-Null
        & $PsExecPath -accepteula -nobanner -s -i 0 cmdkey.exe "/add:$TargetHost" "/user:$SvcUsername" "/pass:$SvcPassword" | Out-Null
        $listOut = & $PsExecPath -accepteula -nobanner -s -i 0 cmdkey.exe /list:$TargetHost
        if ($listOut -notmatch [regex]::Escape($SvcUsername)) {
            throw "SYSTEM cred verify failed; cmdkey /list under SYSTEM did not show '$SvcUsername'"
        }
        return "user + SYSTEM creds injected for $TargetHost"
    }
    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $ClientHostName
        ScriptBlock  = $script
        ArgumentList = @($TargetHost, $SvcUsername, $SvcPassword, $PsExecPath)
        ErrorAction  = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $remoteResult = Invoke-Command @invokeArgs
    $msg = "$remoteResult"
    @{ ok = $true; message = $msg } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
