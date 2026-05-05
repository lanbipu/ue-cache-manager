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
    if ($User -notmatch '[\\@]') { $User = ".\$User" }
    $secure = ConvertTo-SecureString -String $Pass -AsPlainText -Force
    return New-Object System.Management.Automation.PSCredential($User, $secure)
}

try {
    $script = {
        param($FilePath, $Section, $Name, $Value, $Remove)
        if (-not (Test-Path $FilePath)) { throw "file not found: $FilePath" }
        $backup = "$FilePath.bak.$(Get-Date -UFormat '%Y%m%d-%H%M%S')"
        Copy-Item -Path $FilePath -Destination $backup -Force
        $lines = Get-Content -Path $FilePath -Encoding UTF8
        $out = New-Object System.Collections.ArrayList
        $inSection = $false
        $written = $false
        $bracket = "[$Section]"
        foreach ($line in $lines) {
            $trim = $line.Trim()
            if ($trim -eq $bracket) { $inSection = $true; [void]$out.Add($line); continue }
            if ($inSection -and $trim.StartsWith('[') -and $trim.EndsWith(']')) {
                if (-not $Remove -and -not $written) {
                    [void]$out.Add("$Name=$Value"); $written = $true
                }
                $inSection = $false
                [void]$out.Add($line)
                continue
            }
            if ($inSection -and $trim -match "^\s*$([regex]::Escape($Name))\s*=") {
                if ($Remove) { continue }
                [void]$out.Add("$Name=$Value"); $written = $true; continue
            }
            [void]$out.Add($line)
        }
        if (-not $Remove -and -not $written -and $inSection) {
            [void]$out.Add("$Name=$Value")
        }
        Set-Content -Path $FilePath -Value $out -Encoding UTF8
        return "$backup"
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
    @{ ok = $true; backup_path = "$remoteResult"; message = "wrote $Name in [$Section]" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; backup_path = ""; message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
