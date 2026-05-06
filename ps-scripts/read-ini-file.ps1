# Reads an entire INI file and returns its sections + keys with line numbers.
# Parameters: -HostName <string> -FilePath <string>
#             [-Username <string>] [-Password <string>]
# Output: JSON { ok, sections: [{ name, keys: [{ name, value, line_number }] }], message }

param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$FilePath,
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
    if ($User.StartsWith("./")) { $User = ".\" + $User.Substring(2) }
    elseif ($User -notmatch '[\\@]') { $User = ".\$User" }
    $secure = ConvertTo-SecureString -String $Pass -AsPlainText -Force
    return New-Object System.Management.Automation.PSCredential($User, $secure)
}

try {
    $script = {
        param($FilePath)
        if (-not (Test-Path $FilePath)) {
            return @{ found = $false; sections = @() }
        }
        $lines = Get-Content -Path $FilePath -Encoding UTF8
        $sections = New-Object System.Collections.ArrayList
        $current = $null
        $lineNo = 0
        foreach ($line in $lines) {
            $lineNo++
            $trim = $line.Trim()
            if ($trim.StartsWith('[') -and $trim.EndsWith(']') -and $trim.Length -gt 2) {
                if ($current -ne $null) { [void]$sections.Add($current) }
                $current = @{
                    name = $trim.Substring(1, $trim.Length - 2)
                    keys = New-Object System.Collections.ArrayList
                }
                continue
            }
            if ($current -eq $null) { continue }
            if ([string]::IsNullOrEmpty($trim)) { continue }
            if ($trim.StartsWith(';') -or $trim.StartsWith('#') -or $trim.StartsWith('//')) { continue }
            $eq = $trim.IndexOf('=')
            if ($eq -gt 0) {
                $name = $trim.Substring(0, $eq).Trim()
                $value = $trim.Substring($eq + 1).Trim()
                [void]$current.keys.Add([PSCustomObject]@{
                    name = $name
                    value = $value
                    line_number = $lineNo
                })
            }
        }
        if ($current -ne $null) { [void]$sections.Add($current) }
        return @{ found = $true; sections = $sections }
    }
    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock  = $script
        ArgumentList = @($FilePath)
        ErrorAction  = 'Stop'
        Authentication = 'Negotiate'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $result = Invoke-Command @invokeArgs

    @{
        ok = $true
        found = [bool]$result.found
        sections = @($result.sections)
        message = ""
    } | ConvertTo-Json -Compress -Depth 6
}
catch {
    @{ ok = $false; found = $false; sections = @(); message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
