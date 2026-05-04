param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [int]$Pid,
    [string]$Username,
    [string]$Password
)

$ErrorActionPreference = 'Stop'

function Build-CredentialOrNull {
    param([string]$User, [string]$Pass)
    if ([string]::IsNullOrEmpty($User) -or [string]::IsNullOrEmpty($Pass)) { return $null }
    $secure = ConvertTo-SecureString -String $Pass -AsPlainText -Force
    return New-Object System.Management.Automation.PSCredential($User, $secure)
}

try {
    $script = {
        param($TargetPid)
        try {
            Stop-Process -Id $TargetPid -Force -ErrorAction Stop
            return @{ killed = $true; message = "stopped pid $TargetPid" }
        } catch {
            return @{ killed = $false; message = "$($_.Exception.Message)" }
        }
    }

    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock = $script
        ArgumentList = @($Pid)
        ErrorAction = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $r = Invoke-Command @invokeArgs

    @{ ok = $true; killed = "$($r.killed)" -eq "True"; message = "$($r.message)" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; killed = $false; message = "$($_.Exception.Message)" } | ConvertTo-Json -Compress
    exit 1
}
