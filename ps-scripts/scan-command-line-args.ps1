# Scans Desktop + Public Desktop + Start Menu shortcuts, common .bat folders,
# and all installed Win32_Service ImagePaths for -LocalDataCachePath= and
# -SharedDataCachePath= command-line arguments.
param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [string]$Username,
    [string]$Password
)
$ErrorActionPreference = 'Stop'
$script = {
    function MatchArgs($cmd) {
        $out = @{}
        $patterns = @{
            local  = '-LocalDataCachePath=("[^"]+"|[^\s]+)'
            shared = '-SharedDataCachePath=("[^"]+"|[^\s]+)'
        }
        foreach ($k in $patterns.Keys) {
            $m = [regex]::Match($cmd, $patterns[$k], 'IgnoreCase')
            if ($m.Success) { $out[$k] = ($m.Groups[1].Value).Trim('"') }
        }
        $out
    }

    $findings = New-Object System.Collections.Generic.List[object]

    # Shortcuts
    $shortcutRoots = @(
        [Environment]::GetFolderPath('Desktop'),
        [Environment]::GetFolderPath('CommonDesktopDirectory'),
        [Environment]::GetFolderPath('Programs'),
        [Environment]::GetFolderPath('CommonPrograms')
    )
    $shell = New-Object -ComObject WScript.Shell
    foreach ($root in $shortcutRoots) {
        if (-not $root -or -not (Test-Path -LiteralPath $root)) { continue }
        Get-ChildItem -LiteralPath $root -Recurse -Filter *.lnk -ErrorAction SilentlyContinue | ForEach-Object {
            try {
                $lnk = $shell.CreateShortcut($_.FullName)
                $cmd = "$($lnk.TargetPath) $($lnk.Arguments)"
                $hits = MatchArgs $cmd
                if ($hits.Count -gt 0) {
                    $findings.Add([pscustomobject]@{ source='shortcut'; path=$_.FullName; cmd=$cmd; matches=$hits })
                }
            } catch {}
        }
    }

    # BAT files
    $batRoots = @('C:\Tools', 'C:\Scripts', "$env:USERPROFILE\Desktop")
    foreach ($root in $batRoots) {
        if (-not (Test-Path -LiteralPath $root)) { continue }
        Get-ChildItem -LiteralPath $root -Recurse -Filter *.bat -ErrorAction SilentlyContinue | ForEach-Object {
            try {
                $body = Get-Content -LiteralPath $_.FullName -Raw -Encoding UTF8
                $hits = MatchArgs $body
                if ($hits.Count -gt 0) {
                    $findings.Add([pscustomobject]@{ source='bat'; path=$_.FullName; cmd=$body.Substring(0, [Math]::Min(400, $body.Length)); matches=$hits })
                }
            } catch {}
        }
    }

    # Services
    Get-CimInstance Win32_Service -ErrorAction SilentlyContinue | ForEach-Object {
        $cmd = $_.PathName
        $hits = MatchArgs $cmd
        if ($hits.Count -gt 0) {
            $findings.Add([pscustomobject]@{ source='service'; name=$_.Name; path=$cmd; matches=$hits })
        }
    }

    @{ findings = $findings }
}

try {
    $r = if ($Username) {
        $pass = ConvertTo-SecureString $Password -AsPlainText -Force
        $cred = New-Object System.Management.Automation.PSCredential($Username, $pass)
        Invoke-Command -ComputerName $HostName -Credential $cred -Authentication Default -ScriptBlock $script
    } else { Invoke-Command -ComputerName $HostName -ScriptBlock $script }
    @{ ok = $true; findings = @($r.findings) } | ConvertTo-Json -Compress -Depth 6
} catch {
    @{ ok = $false; message = $_.Exception.Message; findings = @() } | ConvertTo-Json -Compress
}
