# Queries GPU model + driver version via WMI.
# Output: JSON array of { gpu_model, driver_version, vendor, vram_mb }
# Designed to run via `invoke-remote.ps1`.

$ErrorActionPreference = 'SilentlyContinue'

$controllers = Get-CimInstance -ClassName Win32_VideoController

$results = @()
foreach ($c in $controllers) {
    $name = $c.Name
    $vendor = 'unknown'
    if ($name -match 'NVIDIA')   { $vendor = 'nvidia' }
    elseif ($name -match 'AMD' -or $name -match 'Radeon') { $vendor = 'amd' }
    elseif ($name -match 'Intel') { $vendor = 'intel' }

    $vramBytes = [int64]$c.AdapterRAM
    # AdapterRAM is unsigned 32-bit and may report negative for >4GB; recompute via DXGI fallback if available.
    # For now, accept the WMI value; null if we can't compute.
    $vramMb = $null
    if ($vramBytes -gt 0) {
        $vramMb = [int64]([math]::Round($vramBytes / 1MB))
    }

    $results += [PSCustomObject]@{
        gpu_model = $name
        driver_version = $c.DriverVersion
        vendor = $vendor
        vram_mb = $vramMb
    }
}

ConvertTo-Json -InputObject @($results) -Compress
