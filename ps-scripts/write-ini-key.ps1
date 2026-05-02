# Sets a single key in an INI section on a remote host with auto-backup.
# Parameters: -HostName <string> -FilePath <string> -Section <string>
#             -Name <string> -Value <string>
# Output: JSON { ok: bool, backup_path: string, message: string }

param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$FilePath,
    [Parameter(Mandatory=$true)] [string]$Section,
    [Parameter(Mandatory=$true)] [string]$Name,
    [Parameter(Mandatory=$true)] [string]$Value
)

$ErrorActionPreference = 'Stop'

try {
    $remoteResult = Invoke-Command -ComputerName $HostName -ScriptBlock {
        param($FilePath, $Section, $Name, $Value)

        if (-not (Test-Path $FilePath)) {
            throw "file not found: $FilePath"
        }

        # Backup
        $ts = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
        $backup = "$FilePath.uecm-bak-$ts"
        Copy-Item -Path $FilePath -Destination $backup -Force

        $lines = Get-Content -Path $FilePath -Encoding UTF8
        $sectionPattern = "[$Section]"
        $inSection = $false
        $found = $false
        $newLines = New-Object System.Collections.Generic.List[string]
        $sectionIndex = -1
        $i = 0

        foreach ($line in $lines) {
            $trim = $line.Trim()
            if ($trim -eq $sectionPattern) {
                $inSection = $true
                $sectionIndex = $i
                $newLines.Add($line)
            }
            elseif ($inSection -and $trim.StartsWith('[') -and $trim.EndsWith(']')) {
                if (-not $found) {
                    # Insert key at the end of this section
                    $newLines.Add("$Name=$Value")
                    $found = $true
                }
                $inSection = $false
                $newLines.Add($line)
            }
            elseif ($inSection -and ($trim -match "^\s*$([regex]::Escape($Name))\s*=")) {
                $newLines.Add("$Name=$Value")
                $found = $true
            }
            else {
                $newLines.Add($line)
            }
            $i++
        }

        if ($inSection -and -not $found) {
            $newLines.Add("$Name=$Value")
            $found = $true
        }

        if (-not $found -and $sectionIndex -lt 0) {
            # Section did not exist; append section + key.
            $newLines.Add("")
            $newLines.Add("[$Section]")
            $newLines.Add("$Name=$Value")
        }

        Set-Content -Path $FilePath -Value $newLines -Encoding UTF8
        return $backup
    } -ArgumentList $FilePath, $Section, $Name, $Value -ErrorAction Stop

    # Invoke-Command wraps the returned string in a PSObject with
    # PSComputerName/RunspaceId metadata; force-cast to plain string so
    # ConvertTo-Json emits a flat field instead of a nested object.
    $backupPath = "$remoteResult"

    @{ ok = $true; backup_path = $backupPath; message = "" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; backup_path = ""; message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
