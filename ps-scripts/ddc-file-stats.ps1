# Returns {file_count, total_bytes} for one or two paths (Local + Shared).
param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [string]$LocalPath = "",
    [string]$SharedPath = "",
    [string]$Username,
    [string]$Password
)
$ErrorActionPreference = 'Stop'
$script = {
    param($LocalPath, $SharedPath)
    function StatPath($p) {
        if ([string]::IsNullOrEmpty($p)) { return @{ path = ""; ok = $false; file_count = 0; total_bytes = 0; error = "empty" } }
        try {
            if (-not (Test-Path -LiteralPath $p)) { return @{ path = $p; ok = $false; file_count = 0; total_bytes = 0; error = "not found" } }
            $files = Get-ChildItem -LiteralPath $p -Recurse -Force -File -ErrorAction SilentlyContinue
            $count = ($files | Measure-Object).Count
            $bytes = ($files | Measure-Object Length -Sum).Sum
            if (-not $bytes) { $bytes = 0 }
            @{ path = $p; ok = $true; file_count = $count; total_bytes = [int64]$bytes }
        } catch {
            @{ path = $p; ok = $false; error = $_.Exception.Message; file_count = 0; total_bytes = 0 }
        }
    }
    @{
        local  = (StatPath $LocalPath)
        shared = (StatPath $SharedPath)
    }
}
try {
    $r = if ($Username) {
        $pass = ConvertTo-SecureString $Password -AsPlainText -Force
        $cred = New-Object System.Management.Automation.PSCredential($Username, $pass)
        Invoke-Command -ComputerName $HostName -Credential $cred -Authentication Default -ScriptBlock $script -ArgumentList $LocalPath, $SharedPath
    } else {
        Invoke-Command -ComputerName $HostName -ScriptBlock $script -ArgumentList $LocalPath, $SharedPath
    }
    @{ ok = $true; local = $r.local; shared = $r.shared } | ConvertTo-Json -Compress -Depth 5
} catch {
    @{ ok = $false; message = $_.Exception.Message } | ConvertTo-Json -Compress
}
