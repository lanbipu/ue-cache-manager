# Enables Windows Remote Management for one-time UECM target onboarding.
# Run locally on the target machine as Administrator.
#
# Default changes:
# - Start WinRM and set it to Automatic startup.
# - Enable PowerShell Remoting and the WinRM HTTP listener on port 5985.
# - Enable Windows Remote Management firewall rules.
# - Change active Public network profiles to Private unless -NetworkCategory Skip is used.
#
# Optional changes:
# - -AllowedRemoteAddress restricts WinRM firewall rules to specific source IPs/CIDRs.
# - -TrustedHosts sets WSMan Client TrustedHosts on this machine.
# - -EnableLocalAccountRemoteAdmin enables remote admin tokens for local admin accounts.
#
# This script does not enable SSH, Basic auth, CredSSP, AllowUnencrypted, or WinRM HTTPS.

param(
    [ValidateSet('Private', 'Skip')]
    [string]$NetworkCategory = 'Private',

    [string[]]$AllowedRemoteAddress = @(),

    [string[]]$TrustedHosts = @(),

    [switch]$EnableLocalAccountRemoteAdmin,

    [switch]$CheckOnly
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
