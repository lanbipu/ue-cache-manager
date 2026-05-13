# Reads a single [section] from an INI file on a remote host.
# Parameters: -HostName <string> -FilePath <string> -Section <string>
#             [-Username <string>] [-Password <string>]
# Output: JSON { ok: bool, keys: [{ name, value }], message: string }

param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$FilePath,
    [Parameter(Mandatory=$true)] [string]$Section,
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
        param($FilePath, $Section)
        if (-not (Test-Path $FilePath)) {
            throw "file not found: $FilePath"
        }
        $lines = Get-Content -Path $FilePath -Encoding UTF8
        $inSection = $false
        $sectionPattern = "[$Section]"
        $result = @()
        foreach ($line in $lines) {
            $trim = $line.Trim()
            if ($trim -eq $sectionPattern) { $inSection = $true; continue }
            if ($inSection -and $trim.StartsWith('[') -and $trim.EndsWith(']')) { break }
            if ($inSection -and $trim -and -not $trim.StartsWith(';') -and -not $trim.StartsWith('#')) {
                $eq = $trim.IndexOf('=')
                if ($eq -gt 0) {
                    $name = $trim.Substring(0, $eq).Trim()
                    $value = $trim.Substring($eq + 1).Trim()
                    $result += [PSCustomObject]@{ name = $name; value = $value }
                }
            }
        }
        return ,$result
    }
    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock  = $script
        ArgumentList = @($FilePath, $Section)
        ErrorAction  = 'Stop'
        Authentication = 'Negotiate'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $keys = Invoke-Command @invokeArgs

    @{ ok = $true; keys = @($keys); message = "" } | ConvertTo-Json -Compress -Depth 4
}
catch {
    @{ ok = $false; keys = @(); message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
