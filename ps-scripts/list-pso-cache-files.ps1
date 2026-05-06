param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$ProjectDir,
    [string]$Username,
    [string]$Password
)

$ErrorActionPreference = 'Stop'

function Build-CredentialOrNull {
    param([string]$User, [string]$Pass)
    if ([string]::IsNullOrEmpty($User) -or [string]::IsNullOrEmpty($Pass)) { return $null }
    if ($User -notmatch '[\\@]') { $User = ".\$User" }
    $secure = ConvertTo-SecureString -String $Pass -AsPlainText -Force
    return New-Object System.Management.Automation.PSCredential($User, $secure)
}

try {
    $script = {
        param($ProjectDir)
        $dir = Join-Path -Path $ProjectDir -ChildPath 'Saved\CollectedPSOs'
        if (-not (Test-Path -LiteralPath $dir)) { return ,@() }

        $files = Get-ChildItem -LiteralPath $dir -File -ErrorAction SilentlyContinue | Where-Object {
            $_.Extension -eq '.upipelinecache' -or $_.Extension -eq '.csv'
        }
        $out = @()
        foreach ($f in $files) {
            $out += @{
                file_path = "$($f.FullName)"
                file_name = "$($f.Name)"
                size = "$($f.Length)"
                last_write = "$($f.LastWriteTimeUtc.ToString('o'))"
            }
        }
        return ,$out
    }

    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock = $script
        ArgumentList = @($ProjectDir)
        ErrorAction = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $items = Invoke-Command @invokeArgs

    @{
        ok = $true
        items = @($items)
        count = (@($items)).Count
    } | ConvertTo-Json -Depth 6 -Compress
}
catch {
    @{ ok = $false; items = @(); count = 0; message = "$($_.Exception.Message)" } | ConvertTo-Json -Compress
    exit 1
}
