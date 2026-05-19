# Single-host snapshot of UE installs, RenderStream plugin version, default RHI,
# GPU/Driver, and project paths on common drives. JSON to stdout.
param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [string]$Username,
    [string]$Password
)
$ErrorActionPreference = 'Stop'

$script = {
    # UE installs: read registry
    $ueInstalls = @()
    $keyPaths = @('HKLM:\SOFTWARE\EpicGames\Unreal Engine', 'HKLM:\SOFTWARE\WOW6432Node\EpicGames\Unreal Engine')
    foreach ($p in $keyPaths) {
        if (Test-Path $p) {
            $versions = Get-ChildItem $p -ErrorAction SilentlyContinue
            foreach ($v in $versions) {
                $installed = (Get-ItemProperty -Path $v.PSPath -Name 'InstalledDirectory' -ErrorAction SilentlyContinue).InstalledDirectory
                if ($installed) {
                    $ueInstalls += [pscustomobject]@{
                        Version = $v.PSChildName
                        Path = $installed
                    }
                }
            }
        }
    }

    # GPU/Driver from Win32_VideoController
    $gpu = Get-CimInstance Win32_VideoController -ErrorAction SilentlyContinue | Select-Object -First 1
    $gpuInfo = if ($gpu) {
        [pscustomobject]@{
            Name = $gpu.Name; Driver = $gpu.DriverVersion; DriverDate = "$($gpu.DriverDate)"
        }
    } else { $null }

    # Default RHI from CurrentUser preference (best effort)
    $rhi = $null
    try {
        $defaultGraphicsRHI = Get-ItemProperty -Path 'HKCU:\Software\Epic Games\Unreal Engine\Settings' -Name 'DefaultGraphicsRHI' -ErrorAction SilentlyContinue
        if ($defaultGraphicsRHI) { $rhi = $defaultGraphicsRHI.DefaultGraphicsRHI }
    } catch {}

    # Project root candidates
    $projectDirs = @()
    foreach ($drive in @('C:', 'D:', 'E:', 'F:')) {
        $candidates = @("$drive\Projects", "$drive\RenderStream Projects", "$drive\Unreal Projects")
        foreach ($c in $candidates) {
            if (Test-Path -LiteralPath $c) {
                $children = Get-ChildItem -LiteralPath $c -Directory -ErrorAction SilentlyContinue | Select-Object -First 50
                foreach ($child in $children) {
                    $uproject = Get-ChildItem -LiteralPath $child.FullName -Filter '*.uproject' -ErrorAction SilentlyContinue | Select-Object -First 1
                    if ($uproject) {
                        $projectDirs += [pscustomobject]@{ Path = $child.FullName; UProject = $uproject.Name }
                    }
                }
            }
        }
    }

    # RenderStream plugin version (look for d3 install)
    $rsVersion = $null
    try {
        $d3 = Get-ItemProperty -Path 'HKLM:\SOFTWARE\d3 Technologies\d3 Production Suite' -ErrorAction SilentlyContinue
        if ($d3 -and $d3.Version) { $rsVersion = $d3.Version }
    } catch {}

    @{
        ue_installs = $ueInstalls
        gpu = $gpuInfo
        rhi = $rhi
        projects = $projectDirs
        renderstream_version = $rsVersion
        host = $env:COMPUTERNAME
    }
}

try {
    $result = if ($Username) {
        $pass = ConvertTo-SecureString $Password -AsPlainText -Force
        $cred = New-Object System.Management.Automation.PSCredential($Username, $pass)
        Invoke-Command -ComputerName $HostName -Credential $cred -Authentication Default -ScriptBlock $script
    } else {
        Invoke-Command -ComputerName $HostName -ScriptBlock $script
    }
    @{ ok = $true; data = $result } | ConvertTo-Json -Compress -Depth 6
} catch {
    @{ ok = $false; message = $_.Exception.Message } | ConvertTo-Json -Compress
}
