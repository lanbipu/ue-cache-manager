param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$LogPath,
    [Parameter(Mandatory=$true)] [long]$LastReadOffset,
    [int]$MaxBytes = 65536,
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
        param($LogPath, [long]$LastReadOffset, [int]$MaxBytes)
        if (-not (Test-Path -LiteralPath $LogPath)) {
            return @{ exists = $false; new_offset = "0"; new_text = "" }
        }
        $size = (Get-Item -LiteralPath $LogPath).Length
        if ($size -le $LastReadOffset) {
            return @{ exists = $true; new_offset = "$size"; new_text = "" }
        }

        $start = $LastReadOffset
        $count = [Math]::Min($MaxBytes, ($size - $start))
        $stream = [System.IO.File]::Open($LogPath, 'Open', 'Read', 'ReadWrite')
        try {
            $stream.Seek($start, 'Begin') | Out-Null
            $buf = New-Object byte[] $count
            $read = $stream.Read($buf, 0, $count)
            $text = [System.Text.Encoding]::UTF8.GetString($buf, 0, $read)
        } finally {
            $stream.Dispose()
        }
        return @{ exists = $true; new_offset = "$($start + $read)"; new_text = "$text" }
    }

    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock = $script
        ArgumentList = @($LogPath, $LastReadOffset, $MaxBytes)
        ErrorAction = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $r = Invoke-Command @invokeArgs

    @{ ok = $true; exists = $r.exists; new_offset = "$($r.new_offset)"; new_text = "$($r.new_text)" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; message = "$($_.Exception.Message)" } | ConvertTo-Json -Compress
    exit 1
}
