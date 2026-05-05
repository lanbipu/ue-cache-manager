# Runs 8 health probes against a remote host in one round-trip.
# Parameters:
#   -HostName <string>
#   -ShareUnc <string>          e.g. "\\HOST\DDC", or "" if no share configured
#   -SvcUsername <string>       e.g. "ddc-svc", or "" if no managed share
#   -ExpectedSharedDataCachePath <string>
#   [-Username <string>] [-Password <string>]
# Output: JSON { ok, results: { smb:{status,message,sample}, firewall_445:..., ... }, message }

param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [string]$ShareUnc = "",
    [string]$SvcUsername = "",
    [string]$ExpectedSharedDataCachePath = "",
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
        param($ShareUnc, $SvcUsername, $ExpectedSharedDataCachePath)
        function Probe-SmbService {
            try {
                $svc = Get-Service -Name LanmanServer -ErrorAction Stop
                @{ status = ($(if ($svc.Status -eq 'Running') {'healthy'} else {'critical'}));
                   message = "LanmanServer = $($svc.Status)"; sample = $svc.Status.ToString() }
            } catch { @{ status='critical'; message=$_.Exception.Message; sample='' } }
        }
        function Probe-Firewall445 {
            try {
                $rule = Get-NetFirewallRule -DisplayName 'File and Printer Sharing (SMB-In)' -ErrorAction SilentlyContinue
                if (-not $rule) { return @{ status='warning'; message='no SMB-In rule found'; sample='' } }
                $enabled = ($rule | Where-Object Enabled -eq 'True').Count -gt 0
                @{ status = ($(if ($enabled) {'healthy'} else {'warning'}));
                   message = "rule enabled = $enabled"; sample = ($rule[0].DisplayName) }
            } catch { @{ status='warning'; message=$_.Exception.Message; sample='' } }
        }
        function Probe-ShareReachable {
            if ([string]::IsNullOrEmpty($ShareUnc)) {
                return @{ status='na'; message='no share configured'; sample='' }
            }
            try {
                $ok = Test-Path $ShareUnc -ErrorAction Stop
                @{ status = ($(if ($ok) {'healthy'} else {'critical'}));
                   message = "Test-Path returned $ok"; sample = $ShareUnc }
            } catch { @{ status='critical'; message=$_.Exception.Message; sample=$ShareUnc } }
        }
        function Probe-NtfsPerm {
            if ([string]::IsNullOrEmpty($ShareUnc) -or [string]::IsNullOrEmpty($SvcUsername)) {
                return @{ status='na'; message='only meaningful for managed shares with svc account'; sample='' }
            }
            try {
                $share = Get-SmbShare -Name (Split-Path $ShareUnc -Leaf) -ErrorAction SilentlyContinue
                if (-not $share) { return @{ status='na'; message='not the host'; sample='' } }
                $acl = Get-Acl $share.Path
                $hasSvc = $acl.Access | Where-Object { $_.IdentityReference -match $SvcUsername }
                @{ status = ($(if ($hasSvc) {'healthy'} else {'critical'}));
                   message = "ACL on $($share.Path) for $SvcUsername"; sample = ($acl.Owner) }
            } catch { @{ status='warning'; message=$_.Exception.Message; sample='' } }
        }
        function Probe-CredUser {
            if ([string]::IsNullOrEmpty($SvcUsername)) {
                return @{ status='na'; message='no managed share'; sample='' }
            }
            try {
                $out = & cmdkey.exe /list 2>&1 | Out-String
                $hasIt = $out -match [regex]::Escape($SvcUsername)
                @{ status = ($(if ($hasIt) {'healthy'} else {'critical'}));
                   message = "cmdkey /list contains $SvcUsername = $hasIt"; sample = '' }
            } catch { @{ status='critical'; message=$_.Exception.Message; sample='' } }
        }
        function Probe-CredSystem {
            if ([string]::IsNullOrEmpty($SvcUsername)) {
                return @{ status='na'; message='no managed share'; sample='' }
            }
            $vendor = Join-Path $env:LOCALAPPDATA 'UECM\PsExec64.exe'
            if (-not (Test-Path $vendor)) {
                return @{ status='warning'; message='PsExec64 not staged on machine; cannot verify SYSTEM cred'; sample='' }
            }
            try {
                $out = & $vendor -accepteula -nobanner -s -i 0 cmdkey.exe /list 2>&1 | Out-String
                $hasIt = $out -match [regex]::Escape($SvcUsername)
                @{ status = ($(if ($hasIt) {'healthy'} else {'critical'}));
                   message = "SYSTEM cmdkey /list contains $SvcUsername = $hasIt"; sample = '' }
            } catch { @{ status='warning'; message=$_.Exception.Message; sample='' } }
        }
        function Probe-EnvVars {
            $shared = [Environment]::GetEnvironmentVariable('UE-SharedDataCachePath', 'Machine')
            if ([string]::IsNullOrEmpty($ExpectedSharedDataCachePath)) {
                @{ status = ($(if ([string]::IsNullOrEmpty($shared)) {'warning'} else {'healthy'}));
                   message = "UE-SharedDataCachePath = $shared"; sample = "$shared" }
            } else {
                @{ status = ($(if ($shared -eq $ExpectedSharedDataCachePath) {'healthy'} else {'critical'}));
                   message = "expected $ExpectedSharedDataCachePath, got $shared"; sample = "$shared" }
            }
        }
        function Probe-SystemWrite {
            if ([string]::IsNullOrEmpty($ShareUnc)) {
                return @{ status='na'; message='no share configured'; sample='' }
            }
            $vendor = Join-Path $env:LOCALAPPDATA 'UECM\PsExec64.exe'
            if (-not (Test-Path $vendor)) {
                return @{ status='warning'; message='PsExec64 not staged; cannot SYSTEM-write probe'; sample='' }
            }
            try {
                $probe = "uecm-probe-$(Get-Random).txt"
                $cmd = "echo healthcheck > `"$ShareUnc\$probe`""
                & $vendor -accepteula -nobanner -s -i 0 cmd /c $cmd 2>&1 | Out-Null
                $exists = Test-Path "$ShareUnc\$probe"
                if ($exists) { Remove-Item "$ShareUnc\$probe" -Force -ErrorAction SilentlyContinue }
                @{ status = ($(if ($exists) {'healthy'} else {'critical'}));
                   message = "SYSTEM wrote probe file = $exists"; sample = $probe }
            } catch { @{ status='critical'; message=$_.Exception.Message; sample='' } }
        }

        $results = @{
            smb              = (Probe-SmbService)
            firewall_445     = (Probe-Firewall445)
            share_reachable  = (Probe-ShareReachable)
            ntfs_perm        = (Probe-NtfsPerm)
            cred_user        = (Probe-CredUser)
            cred_system      = (Probe-CredSystem)
            env_vars         = (Probe-EnvVars)
            system_write     = (Probe-SystemWrite)
        }
        return $results
    }
    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock  = $script
        ArgumentList = @($ShareUnc, $SvcUsername, $ExpectedSharedDataCachePath)
        ErrorAction  = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $r = Invoke-Command @invokeArgs

    @{ ok = $true; results = $r; message = '' } | ConvertTo-Json -Compress -Depth 6
}
catch {
    @{ ok = $false; results = @{}; message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
