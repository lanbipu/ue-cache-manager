# Sets a single key in an INI section on a remote host with auto-backup.
# Parameters: -HostName <string> -FilePath <string> -Section <string>
#             -Name <string> -Value <string> [-RemoveKey]
#             [-Username <string>] [-Password <string>]
# Output: JSON { ok: bool, backup_path: string, message: string }

param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$FilePath,
    [Parameter(Mandatory=$true)] [string]$Section,
    [Parameter(Mandatory=$true)] [string]$Name,
    [string]$Value = "",
    [switch]$RemoveKey,
    [string]$Username,
    [string]$Password
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

try {
    $script = {
        param($FilePath, $Section, $Name, $Value, $RemoveKey)

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
                if (-not $found -and -not $RemoveKey) {
                    # Insert key at the end of this section
                    $newLines.Add("$Name=$Value")
                    $found = $true
                }
                $inSection = $false
                $newLines.Add($line)
            }
            elseif ($inSection -and ($trim -match "^\s*[\+\-\!]*$([regex]::Escape($Name))\s*=")) {
                if (-not $RemoveKey) {
                    $newLines.Add("$Name=$Value")
                }
                $found = $true
            }
            else {
                $newLines.Add($line)
            }
            $i++
        }

        if ($inSection -and -not $found -and -not $RemoveKey) {
            $newLines.Add("$Name=$Value")
            $found = $true
        }

        if (-not $found -and $sectionIndex -lt 0 -and -not $RemoveKey) {
            # Section did not exist; append section + key.
            $newLines.Add("")
            $newLines.Add("[$Section]")
            $newLines.Add("$Name=$Value")
        }

        Set-Content -Path $FilePath -Value $newLines -Encoding UTF8
        return $backup
    }
    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock  = $script
        ArgumentList = @($FilePath, $Section, $Name, $Value, [bool]$RemoveKey)
        ErrorAction  = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $remoteResult = Invoke-Command @invokeArgs

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
