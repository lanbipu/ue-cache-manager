param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$SearchRoots,
    [int]$MaxDepth = 6,
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
        param($Roots, $MaxDepth)
        $found = @()
        foreach ($root in $Roots) {
            if (-not (Test-Path -LiteralPath $root)) { continue }
            try {
                $uprojects = Get-ChildItem -LiteralPath $root -Filter '*.uproject' -Recurse -Depth $MaxDepth -File -ErrorAction SilentlyContinue
                foreach ($u in $uprojects) {
                    $abs = Split-Path -LiteralPath $u.FullName -Parent
                    $engineAssociation = $null
                    try {
                        $json = Get-Content -LiteralPath $u.FullName -Raw -ErrorAction SilentlyContinue | ConvertFrom-Json -ErrorAction SilentlyContinue
                        if ($json -and $json.EngineAssociation) { $engineAssociation = "$($json.EngineAssociation)" }
                    } catch {}
                    $found += @{
                        uproject_filename = "$($u.Name)"
                        uproject_path = "$($u.FullName)"
                        abs_path = "$abs"
                        engine_association = $engineAssociation
                    }
                }
            } catch {}
        }
        return ,$found
    }

    $rootsArr = $SearchRoots -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ -ne '' }
    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock = $script
        ArgumentList = @($rootsArr, $MaxDepth)
        ErrorAction = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $remoteResult = Invoke-Command @invokeArgs

    $list = @($remoteResult)
    @{ ok = $true; items = $list; count = $list.Count } | ConvertTo-Json -Depth 6 -Compress
}
catch {
    @{ ok = $false; items = @(); count = 0; message = "$($_.Exception.Message)" } | ConvertTo-Json -Depth 6 -Compress
    exit 1
}
