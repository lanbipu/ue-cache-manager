# Reads installed Unreal Engine versions from registry.
# Designed to run via `invoke-remote.ps1` (passed as scriptblock body via stdin),
# but also runnable standalone for local testing.
# Output: JSON array of { version, install_path }, e.g.
#   [{"version":"5.4","install_path":"C:\\Program Files\\Epic Games\\UE_5.4"}]

$ErrorActionPreference = 'SilentlyContinue'

$results = @()

$keys = @(
    'HKLM:\SOFTWARE\EpicGames\Unreal Engine',
    'HKLM:\SOFTWARE\WOW6432Node\EpicGames\Unreal Engine'
)

foreach ($keyPath in $keys) {
    if (Test-Path $keyPath) {
        Get-ChildItem $keyPath | ForEach-Object {
            $version = $_.PSChildName
            $installedDir = (Get-ItemProperty $_.PSPath -Name 'InstalledDirectory' -ErrorAction SilentlyContinue).InstalledDirectory
            if ($installedDir) {
                $results += [PSCustomObject]@{
                    version = $version
                    install_path = $installedDir
                }
            }
        }
    }
}

# Deduplicate by (version, install_path)
$results = $results | Sort-Object version, install_path -Unique

# Always emit valid JSON, even for empty
ConvertTo-Json -InputObject @($results) -Compress
