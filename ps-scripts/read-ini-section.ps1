# Reads a single [section] from an INI file on a remote host.
# Parameters: -HostName <string> -FilePath <string> -Section <string>
# Output: JSON { ok: bool, keys: [{ name, value }], message: string }

param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$FilePath,
    [Parameter(Mandatory=$true)] [string]$Section
)

$ErrorActionPreference = 'Stop'

try {
    $keys = Invoke-Command -ComputerName $HostName -ScriptBlock {
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
    } -ArgumentList $FilePath, $Section -ErrorAction Stop

    @{ ok = $true; keys = @($keys); message = "" } | ConvertTo-Json -Compress -Depth 4
}
catch {
    @{ ok = $false; keys = @(); message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
