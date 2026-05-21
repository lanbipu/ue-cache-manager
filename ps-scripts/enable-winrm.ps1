# Enables Windows Remote Management for one-time UECM target onboarding.
# Run locally on the target machine as Administrator.
#
# Default changes (always applied):
# - Start WinRM and set it to Automatic startup.
# - Enable PowerShell Remoting and the WinRM HTTP listener on port 5985.
# - Enable Windows Remote Management firewall rules.
# - Change active Public network profiles to Private unless -NetworkCategory Skip is used.
#
# Optional changes (off by default - UECM-Bootstrap.cmd wrapper enables all of these):
# - -AllowedRemoteAddress restricts WinRM firewall rules to specific source IPs/CIDRs.
# - -TrustedHosts sets WSMan Client TrustedHosts on this machine.
# - -EnableLocalAccountRemoteAdmin enables remote admin tokens for local admin accounts.
# - -EnableSmbServer ensures LanmanServer is Automatic+Running and File and Printer
#   Sharing firewall rules are enabled (required for UECM SMB share creation).
# - -EnableWmi ensures Winmgmt is Automatic+Running (required for UECM machine refresh).
# - -SetExecutionPolicy <RemoteSigned|Bypass|Skip> sets LocalMachine execution policy.
# - -EnableLongPaths sets HKLM:\...\FileSystem\LongPathsEnabled=1 (required for UE
#   asset paths > 260 chars).
# - -PowerProfile <HighPerformance|Balanced|Skip> activates a built-in power scheme;
#   HighPerformance is hidden on some Win11 builds - the script restores it via
#   powercfg /duplicatescheme before activating.
#
# This script does not enable SSH, Basic auth, CredSSP, AllowUnencrypted, or WinRM HTTPS.

param(
    [ValidateSet('Private', 'Skip')]
    [string]$NetworkCategory = 'Private',

    [string[]]$AllowedRemoteAddress = @(),

    [string[]]$TrustedHosts = @(),

    [switch]$EnableLocalAccountRemoteAdmin,

    [switch]$EnableSmbServer,

    [switch]$EnableWmi,

    [ValidateSet('RemoteSigned', 'Bypass', 'Skip')]
    [string]$SetExecutionPolicy = 'Skip',

    [switch]$EnableLongPaths,

    [ValidateSet('HighPerformance', 'Balanced', 'Skip')]
    [string]$PowerProfile = 'Skip',

    [switch]$CheckOnly,

    [switch]$LibraryOnly
)

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
try { chcp 65001 | Out-Null } catch {}

$ErrorActionPreference = 'Stop'

function Test-UecmAdministrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Add-UecmChange {
    param(
        [System.Collections.Generic.List[string]]$Changes,
        [string]$Message
    )
    if (-not [string]::IsNullOrWhiteSpace($Message)) {
        $Changes.Add($Message) | Out-Null
    }
}

function Get-UecmRegistryDword {
    param(
        [string]$Path,
        [string]$Name
    )
    try {
        $value = Get-ItemProperty -Path $Path -Name $Name -ErrorAction Stop
        return [int]$value.$Name
    }
    catch {
        return $null
    }
}

function Get-UecmTrustedHosts {
    try {
        return (Get-Item -Path WSMan:\localhost\Client\TrustedHosts -ErrorAction Stop).Value
    }
    catch {
        return ''
    }
}

function Get-UecmWinRmListeners {
    try {
        $raw = (& winrm enumerate winrm/config/listener 2>$null) -join "`n"
        if ([string]::IsNullOrWhiteSpace($raw)) { return @() }
        return @($raw -split "`r?`n" | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    }
    catch {
        return @()
    }
}

function Get-UecmFirewallRules {
    try {
        return @(
            Get-NetFirewallRule -DisplayGroup 'Windows Remote Management' -ErrorAction Stop |
                Select-Object DisplayName, Enabled, Profile, Direction, Action
        )
    }
    catch {
        return @()
    }
}

function Get-UecmNetworkProfiles {
    try {
        return @(
            Get-NetConnectionProfile -ErrorAction Stop |
                Select-Object InterfaceAlias, InterfaceIndex, NetworkCategory, IPv4Connectivity, IPv6Connectivity
        )
    }
    catch {
        return @()
    }
}

function Test-UecmLocalWsMan {
    try {
        $null = Test-WSMan localhost -ErrorAction Stop
        return $true
    }
    catch {
        return $false
    }
}

function Get-UecmWinRmState {
    $service = Get-Service WinRM -ErrorAction SilentlyContinue
    $localAccountTokenPolicy = Get-UecmRegistryDword `
        -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System' `
        -Name 'LocalAccountTokenFilterPolicy'

    $startType = $null
    if ($service) {
        try {
            $startType = (Get-CimInstance Win32_Service -Filter "Name='WinRM'" -ErrorAction Stop).StartMode
        }
        catch {
            $startType = $service.StartType.ToString()
        }
    }

    return @{
        computer_name = $env:COMPUTERNAME
        is_admin = Test-UecmAdministrator
        winrm_service_exists = [bool]$service
        winrm_service_status = if ($service) { $service.Status.ToString() } else { 'Missing' }
        winrm_start_type = $startType
        wsman_localhost_ok = Test-UecmLocalWsMan
        listeners = @(Get-UecmWinRmListeners)
        firewall_rules = @(Get-UecmFirewallRules)
        network_profiles = @(Get-UecmNetworkProfiles)
        trusted_hosts = Get-UecmTrustedHosts
        local_account_token_filter_policy = $localAccountTokenPolicy
    }
}

function Set-UecmNetworkProfile {
    param(
        [System.Collections.Generic.List[string]]$Changes
    )
    if ($NetworkCategory -eq 'Skip') {
        Add-UecmChange $Changes 'network profile change skipped'
        return
    }

    $profiles = @(Get-NetConnectionProfile -ErrorAction Stop | Where-Object { $_.NetworkCategory -eq 'Public' })
    foreach ($profile in $profiles) {
        Set-NetConnectionProfile -InterfaceIndex $profile.InterfaceIndex -NetworkCategory $NetworkCategory -ErrorAction Stop
        Add-UecmChange $Changes "network profile $($profile.InterfaceAlias) changed from Public to $NetworkCategory"
    }
}

function Enable-UecmWinRm {
    param(
        [System.Collections.Generic.List[string]]$Changes
    )

    Set-Service -Name WinRM -StartupType Automatic -ErrorAction Stop
    Add-UecmChange $Changes 'WinRM startup type set to Automatic'

    Start-Service -Name WinRM -ErrorAction Stop
    Add-UecmChange $Changes 'WinRM service started'

    Enable-PSRemoting -Force -SkipNetworkProfileCheck | Out-Null
    Add-UecmChange $Changes 'PowerShell Remoting enabled'

    & winrm quickconfig -q | Out-Null
    Add-UecmChange $Changes 'WinRM quickconfig completed'

    Enable-NetFirewallRule -DisplayGroup 'Windows Remote Management' -ErrorAction Stop | Out-Null
    Add-UecmChange $Changes 'Windows Remote Management firewall rules enabled'
}

function Set-UecmFirewallScope {
    param(
        [System.Collections.Generic.List[string]]$Changes
    )
    if ($AllowedRemoteAddress.Count -eq 0) { return }

    $rules = @(Get-NetFirewallRule -DisplayGroup 'Windows Remote Management' -ErrorAction Stop)
    foreach ($rule in $rules) {
        $rule | Get-NetFirewallAddressFilter -ErrorAction Stop |
            Set-NetFirewallAddressFilter -RemoteAddress $AllowedRemoteAddress -ErrorAction Stop
    }
    Add-UecmChange $Changes "WinRM firewall remote address restricted to: $($AllowedRemoteAddress -join ',')"
}

function Set-UecmTrustedHosts {
    param(
        [System.Collections.Generic.List[string]]$Changes
    )
    if ($TrustedHosts.Count -eq 0) { return }

    $value = ($TrustedHosts | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }) -join ','
    if ([string]::IsNullOrWhiteSpace($value)) { return }

    Set-Item -Path WSMan:\localhost\Client\TrustedHosts -Value $value -Force -ErrorAction Stop
    Add-UecmChange $Changes "WSMan TrustedHosts set to: $value"
}

function Enable-UecmLocalAccountRemoteAdmin {
    param(
        [System.Collections.Generic.List[string]]$Changes
    )
    if (-not $EnableLocalAccountRemoteAdmin) { return }

    $path = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System'
    New-Item -Path $path -Force | Out-Null
    New-ItemProperty `
        -Path $path `
        -Name 'LocalAccountTokenFilterPolicy' `
        -PropertyType DWord `
        -Value 1 `
        -Force | Out-Null
    Add-UecmChange $Changes 'LocalAccountTokenFilterPolicy set to 1'
}

function Enable-UecmSmbServer {
    param([System.Collections.Generic.List[string]]$Changes)
    if (-not $EnableSmbServer) { return }

    $svc = Get-Service -Name LanmanServer -ErrorAction SilentlyContinue
    if (-not $svc) {
        Add-UecmChange $Changes 'WARNING: LanmanServer service not present; SMB share creation will fail'
        return
    }

    try {
        $startMode = (Get-CimInstance Win32_Service -Filter "Name='LanmanServer'" -ErrorAction Stop).StartMode
    } catch {
        $startMode = $svc.StartType.ToString()
    }
    if ($startMode -ne 'Auto' -and $startMode -ne 'Automatic') {
        Set-Service -Name LanmanServer -StartupType Automatic -ErrorAction Stop
        Add-UecmChange $Changes 'LanmanServer startup type set to Automatic'
    }
    if ($svc.Status -ne 'Running') {
        Start-Service -Name LanmanServer -ErrorAction Stop
        Add-UecmChange $Changes 'LanmanServer service started'
    }

    # UECM only needs SMB-In (TCP 445). Enabling the full 'File and Printer Sharing'
    # group would also open NetBIOS 137-139, LLMNR/mDNS, Spooler RPC etc - unnecessary
    # attack surface.
    #
    # Use the stable rule Name 'FPS-SMB-In-TCP' (NOT DisplayName which is localized -
    # on zh-CN Windows the DisplayName is "文件和打印机共享(SMB-入站)" and a
    # DisplayName-based lookup would silently fail and leave SMB 445 closed).
    $smbRule = $null
    try {
        $smbRule = Get-NetFirewallRule -Name 'FPS-SMB-In-TCP' -ErrorAction Stop
    } catch {
        # Fallback: enumerate inbound TCP Allow rules with LocalPort 445.
        # MUST filter by Protocol=TCP + Action=Allow - otherwise we could pick a
        # disabled Block rule and "enable" it, which would silently block SMB
        # while reporting success.
        try {
            $candidateRules = @(
                Get-NetFirewallRule -Direction Inbound -Action Allow -ErrorAction SilentlyContinue |
                    Where-Object {
                        try {
                            $port = $_ | Get-NetFirewallPortFilter -ErrorAction Stop
                            ($port.Protocol -eq 'TCP') -and ($port.LocalPort -contains '445')
                        } catch { $false }
                    }
            )
            # Prefer a currently-disabled rule (need to enable it); otherwise any match.
            $smbRule = $candidateRules | Where-Object { $_.Enabled -eq 'False' } | Select-Object -First 1
            if (-not $smbRule) {
                $smbRule = $candidateRules | Select-Object -First 1
            }
        } catch {
            $smbRule = $null
        }
    }

    if (-not $smbRule) {
        Add-UecmChange $Changes 'WARNING: SMB-In firewall rule (FPS-SMB-In-TCP / TCP 445 inbound) not found on this machine; share creation may fail'
        return
    }

    try {
        $smbRule | Enable-NetFirewallRule -ErrorAction Stop
        $ruleId = if ($smbRule.Name) { $smbRule.Name } else { $smbRule.DisplayName }
        Add-UecmChange $Changes "SMB-In firewall rule enabled (rule: $ruleId, TCP 445 only)"
    } catch {
        Add-UecmChange $Changes "WARNING: could not enable SMB-In firewall rule: $($_.Exception.Message)"
    }
}

function Enable-UecmWmi {
    param([System.Collections.Generic.List[string]]$Changes)
    if (-not $EnableWmi) { return }

    $svc = Get-Service -Name Winmgmt -ErrorAction SilentlyContinue
    if (-not $svc) {
        Add-UecmChange $Changes 'WARNING: Winmgmt service not present; UECM machine refresh will fail'
        return
    }

    try {
        $startMode = (Get-CimInstance Win32_Service -Filter "Name='Winmgmt'" -ErrorAction Stop).StartMode
    } catch {
        $startMode = $svc.StartType.ToString()
    }
    if ($startMode -ne 'Auto' -and $startMode -ne 'Automatic') {
        Set-Service -Name Winmgmt -StartupType Automatic -ErrorAction Stop
        Add-UecmChange $Changes 'Winmgmt startup type set to Automatic'
    }
    if ($svc.Status -ne 'Running') {
        Start-Service -Name Winmgmt -ErrorAction Stop
        Add-UecmChange $Changes 'Winmgmt service started'
    }
}

function Set-UecmLocalExecutionPolicy {
    param([System.Collections.Generic.List[string]]$Changes)
    if ($SetExecutionPolicy -eq 'Skip') { return }

    $current = Get-ExecutionPolicy -Scope LocalMachine -ErrorAction SilentlyContinue
    if ("$current" -eq $SetExecutionPolicy) {
        Add-UecmChange $Changes "LocalMachine execution policy already $SetExecutionPolicy"
        return
    }
    try {
        Set-ExecutionPolicy -ExecutionPolicy $SetExecutionPolicy -Scope LocalMachine -Force -ErrorAction Stop
        Add-UecmChange $Changes "LocalMachine execution policy set to $SetExecutionPolicy (was $current)"
    } catch {
        # GPO may override LocalMachine - record but do not fail bootstrap.
        Add-UecmChange $Changes "WARNING: could not set LocalMachine execution policy (likely GPO-managed): $($_.Exception.Message)"
    }
}

function Enable-UecmLongPaths {
    param([System.Collections.Generic.List[string]]$Changes)
    if (-not $EnableLongPaths) { return }

    $path = 'HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem'
    $current = Get-UecmRegistryDword -Path $path -Name 'LongPathsEnabled'
    if ($current -eq 1) {
        Add-UecmChange $Changes 'LongPathsEnabled already 1'
        return
    }
    New-ItemProperty -Path $path -Name 'LongPathsEnabled' -PropertyType DWord -Value 1 -Force | Out-Null
    Add-UecmChange $Changes 'LongPathsEnabled set to 1 (effective after reboot for some processes)'
}

function Set-UecmPowerPlan {
    param([System.Collections.Generic.List[string]]$Changes)
    if ($PowerProfile -eq 'Skip') { return }

    $guidMap = @{
        'HighPerformance' = '8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c'
        'Balanced'        = '381b4222-f694-41f0-9685-ff5bb260df2e'
    }
    $guid = $guidMap[$PowerProfile]

    # Power plan restoration is idempotent across reruns:
    #   1. Built-in GUID present in list → use it directly.
    #   2. Built-in GUID hidden but we previously duplicated it (named "UECM-<Profile>")
    #      → reuse that GUID, do NOT duplicate again.
    #   3. Truly missing → /duplicatescheme + /changename to a stable UECM-* tag so
    #      the next run finds it via case 2.
    # This prevents each bootstrap rerun from creating yet another power scheme.
    $list = (& powercfg /list 2>&1) -join "`n"
    $activeGuid = $null
    $uecmPlanTag = "UECM-$PowerProfile"
    $guidRegex = '([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})'

    if ($list -match [regex]::Escape($guid)) {
        # Case 1: built-in scheme is still visible.
        $activeGuid = $guid
    } else {
        # Case 2: look for a prior UECM-tagged duplicate.
        foreach ($line in ($list -split "`r?`n")) {
            if ($line -match ($guidRegex + '.*\(\s*' + [regex]::Escape($uecmPlanTag) + '\s*\)')) {
                $activeGuid = $matches[1]
                Add-UecmChange $Changes "reusing existing $uecmPlanTag power plan (GUID: $activeGuid)"
                break
            }
        }
    }

    if (-not $activeGuid) {
        # Case 3: actually missing - duplicate, capture new GUID, rename for next-run match.
        $dupOutput = (& powercfg /duplicatescheme $guid 2>&1) -join "`n"
        if ($LASTEXITCODE -ne 0) {
            Add-UecmChange $Changes "WARNING: power plan $PowerProfile GUID not found and duplicatescheme failed; keeping current plan"
            return
        }
        if ($dupOutput -match $guidRegex) {
            $activeGuid = $matches[1]
            & powercfg /changename $activeGuid $uecmPlanTag "Created by UECM bootstrap" 2>&1 | Out-Null
            Add-UecmChange $Changes "$PowerProfile power plan restored via duplicatescheme, tagged $uecmPlanTag (new GUID: $activeGuid)"
        } else {
            Add-UecmChange $Changes "WARNING: duplicatescheme ran but new GUID could not be parsed; keeping current plan"
            return
        }
    }

    & powercfg /setactive $activeGuid 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Add-UecmChange $Changes "WARNING: powercfg setactive returned exit code $LASTEXITCODE; current plan unchanged"
        return
    }
    Add-UecmChange $Changes "active power plan set to $PowerProfile (active GUID: $activeGuid)"
}

if (-not $LibraryOnly) {
try {
    if ($CheckOnly) {
        $checkState = Get-UecmWinRmState
        $checkOk = [bool]$checkState.wsman_localhost_ok
        $checkMessage = if ($checkOk) {
            'UECM WinRM bootstrap check completed'
        } else {
            'UECM WinRM bootstrap check failed: Test-WSMan localhost is not reachable'
        }
        @{
            ok = $checkState.wsman_localhost_ok
            message = $checkMessage
            changed = @()
            state = $checkState
        } | ConvertTo-Json -Depth 8 -Compress
        if ($checkOk) { exit 0 } else { exit 1 }
    }

    if (-not (Test-UecmAdministrator)) {
        throw 'Administrator privileges are required. Start PowerShell with Run as Administrator.'
    }

    $changes = New-Object 'System.Collections.Generic.List[string]'

    Set-UecmNetworkProfile -Changes $changes
    Enable-UecmWinRm -Changes $changes
    Set-UecmFirewallScope -Changes $changes
    Set-UecmTrustedHosts -Changes $changes
    Enable-UecmLocalAccountRemoteAdmin -Changes $changes
    Enable-UecmSmbServer -Changes $changes
    Enable-UecmWmi -Changes $changes
    Set-UecmLocalExecutionPolicy -Changes $changes
    Enable-UecmLongPaths -Changes $changes
    Set-UecmPowerPlan -Changes $changes

    if (-not (Test-UecmLocalWsMan)) {
        throw 'WinRM was configured but Test-WSMan localhost still failed.'
    }

    @{
        ok = $true
        message = 'UECM WinRM bootstrap completed'
        changed = @($changes)
        state = Get-UecmWinRmState
    } | ConvertTo-Json -Depth 8 -Compress
    exit 0
}
catch {
    @{
        ok = $false
        message = $_.Exception.Message
        changed = @()
        state = Get-UecmWinRmState
    } | ConvertTo-Json -Depth 8 -Compress
    exit 1
}
}
