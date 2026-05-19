# Discovers any RenderStream-related Windows services on a host and reports
# their StartName (the account they run as), State, and StartMode.
param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [string]$Username,
    [string]$Password
)
$ErrorActionPreference = 'Stop'

$script = {
    $patterns = @(
        'd3service*',
        '*RenderStream*',
        '*disguise*',
        '*Cluster*Render*'
    )
    $found = New-Object System.Collections.Generic.List[object]
    foreach ($p in $patterns) {
        $svcs = Get-CimInstance Win32_Service -Filter "Name LIKE '$($p.Replace('*','%'))'" -ErrorAction SilentlyContinue
        foreach ($svc in $svcs) {
            if ($found.Where({ $_.Name -eq $svc.Name }).Count -gt 0) { continue }
            $found.Add([pscustomobject]@{
                Name = $svc.Name
                DisplayName = $svc.DisplayName
                StartName = $svc.StartName
                State = $svc.State
                StartMode = $svc.StartMode
                PathName = $svc.PathName
            }) | Out-Null
        }
    }
    @{ services = $found }
}

try {
    $result = if ($Username) {
        $pass = ConvertTo-SecureString $Password -AsPlainText -Force
        $cred = New-Object System.Management.Automation.PSCredential($Username, $pass)
        Invoke-Command -ComputerName $HostName -Credential $cred -Authentication Default -ScriptBlock $script
    } else {
        Invoke-Command -ComputerName $HostName -ScriptBlock $script
    }
    @{ ok = $true; services = @($result.services) } | ConvertTo-Json -Compress -Depth 4
} catch {
    @{ ok = $false; message = $_.Exception.Message; services = @() } | ConvertTo-Json -Compress
}
