# Reads all sections/keys from an INI file on a remote host.
# Output: JSON { ok, sections: [{ name, keys: [{ name, value, line_number, raw }] }], message }

param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$FilePath,
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
        param($FilePath)
        if (-not (Test-Path $FilePath)) {
            throw "file not found: $FilePath"
        }

        $sections = [ordered]@{}
        $current = ""
        $lineNo = 0
        foreach ($line in (Get-Content -Path $FilePath -Encoding UTF8)) {
            $lineNo += 1
            $trim = $line.Trim()
            if ([string]::IsNullOrWhiteSpace($trim) -or $trim.StartsWith(';') -or $trim.StartsWith('#') -or $trim.StartsWith('//')) {
                continue
            }
            if ($trim.StartsWith('[') -and $trim.EndsWith(']')) {
                $current = $trim.Substring(1, $trim.Length - 2)
                if (-not $sections.Contains($current)) {
                    $sections[$current] = New-Object System.Collections.Generic.List[object]
                }
                continue
            }
            $eq = $trim.IndexOf('=')
            if ($eq -gt 0) {
                if (-not $sections.Contains($current)) {
                    $sections[$current] = New-Object System.Collections.Generic.List[object]
                }
                $name = $trim.Substring(0, $eq).Trim()
                $value = $trim.Substring($eq + 1).Trim()
                $sections[$current].Add([PSCustomObject]@{
                    name = "$name"
                    value = "$value"
                    line_number = $lineNo
                    raw = "$line"
                })
            }
        }

        $out = @()
        foreach ($key in $sections.Keys) {
            $out += [PSCustomObject]@{ name = "$key"; keys = @($sections[$key]) }
        }
        return ,$out
    }

    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock  = $script
        ArgumentList = @($FilePath)
        ErrorAction  = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $sections = Invoke-Command @invokeArgs
    @{ ok = $true; sections = @($sections); message = "" } | ConvertTo-Json -Compress -Depth 8
}
catch {
    @{ ok = $false; sections = @(); message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
