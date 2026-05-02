# Runs bundled machine health probes in one remote round-trip.
# Output: JSON { ok, results: { check_id: { status, message, sample, remediation } }, message }

param(
    [Parameter(Mandatory=$true)] [string]$HostName,
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
        function Outcome($Status, $Message, $Sample, $Fix) {
            [PSCustomObject]@{
                status = "$Status"
                message = "$Message"
                sample = "$Sample"
                remediation = "$Fix"
            }
        }

        $results = [ordered]@{}

        $smb = Get-Service -Name LanmanServer -ErrorAction SilentlyContinue
        if ($smb -and $smb.Status -eq 'Running') {
            $results.smb = Outcome "healthy" "LanmanServer service is running." "$($smb.Status)" "Start the Server service."
        } else {
            $results.smb = Outcome "critical" "LanmanServer service is not running." "$($smb.Status)" "Start LanmanServer and rerun."
        }

        $fw = Get-NetFirewallRule -DisplayGroup "File and Printer Sharing" -ErrorAction SilentlyContinue |
            Where-Object { $_.Enabled -eq 'True' } |
            Select-Object -First 1
        if ($fw) {
            $results.firewall_445 = Outcome "healthy" "File and Printer Sharing firewall group has enabled rules." "$($fw.DisplayName)" "Enable inbound SMB firewall rules."
        } else {
            $results.firewall_445 = Outcome "warning" "No enabled File and Printer Sharing firewall rule was found." "" "Enable TCP 445 inbound rule for the cluster network."
        }

        $shares = Get-SmbShare -ErrorAction SilentlyContinue | Where-Object { $_.Name -notmatch '^\w\$' }
        if ($shares) {
            $results.share_reachable = Outcome "healthy" "At least one non-admin SMB share exists." "$(($shares | Select-Object -First 1).Name)" "Create or validate the DDC share."
        } else {
            $results.share_reachable = Outcome "warning" "No non-admin SMB shares found." "" "Create the managed DDC share."
        }

        $drive = Get-PSDrive -PSProvider FileSystem | Select-Object -First 1
        $results.ntfs = Outcome "healthy" "Filesystem provider is available." "$($drive.Root)" "Verify NTFS ACLs on cache root if writes fail."

        $cmdkey = cmdkey /list 2>$null
        if ($LASTEXITCODE -eq 0) {
            $results.cred_user = Outcome "healthy" "User credential store is readable." "$(($cmdkey | Select-Object -First 1) -join '')" "Store WinRM/SMB credentials through UECM."
        } else {
            $results.cred_user = Outcome "warning" "cmdkey failed for current user." "" "Check user Credential Manager access."
        }

        $psexec = Join-Path $env:LOCALAPPDATA "UECM\PsExec64.exe"
        if (Test-Path $psexec) {
            $results.cred_system = Outcome "healthy" "PsExec64.exe is staged for SYSTEM credential checks." $psexec "Inject SYSTEM credentials if share writes fail under services."
            $temp = Join-Path $env:LOCALAPPDATA "UECM\system-write-test.tmp"
            "uecm" | Set-Content -Path $temp -Encoding UTF8
            Remove-Item $temp -Force
            $results.system_write = Outcome "healthy" "Local SYSTEM-write probe prerequisites are staged." $temp "Run the full SYSTEM write probe against the DDC share."
        } else {
            $results.cred_system = Outcome "warning" "PsExec64.exe is not staged." $psexec "Stage PsExec64.exe at %LOCALAPPDATA%\UECM for SYSTEM checks."
            $results.system_write = Outcome "warning" "SYSTEM write test skipped because PsExec64.exe is missing." "" "Stage PsExec64.exe and rerun."
        }

        $envVar = [Environment]::GetEnvironmentVariable("UE-SharedDataCachePath", "Machine")
        if ([string]::IsNullOrWhiteSpace($envVar)) {
            $results.env_vars = Outcome "warning" "UE-SharedDataCachePath is not set at machine scope." "" "Set the machine-level env var through UECM."
        } else {
            $results.env_vars = Outcome "healthy" "UE-SharedDataCachePath is set." "$envVar" "No action required."
        }

        return $results
    }

    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock  = $script
        ErrorAction  = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $results = Invoke-Command @invokeArgs
    @{ ok = $true; results = $results; message = "" } | ConvertTo-Json -Compress -Depth 8
}
catch {
    @{ ok = $false; results = @{}; message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
