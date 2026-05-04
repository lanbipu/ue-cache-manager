param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$EnginePath,
    [Parameter(Mandatory=$true)] [string]$ProjectPath,
    [Parameter(Mandatory=$true)] [string[]]$ExtraArgs,
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
        param($EnginePath, $ProjectPath, [string[]]$ExtraArgs)
        $exe = Join-Path -Path $EnginePath -ChildPath 'Engine\Binaries\Win64\UnrealEditor.exe'
        if (-not (Test-Path -LiteralPath $exe)) { throw "UnrealEditor.exe not found at $exe" }
        if (-not (Test-Path -LiteralPath $ProjectPath)) { throw "uproject not found at $ProjectPath" }

        $argList = @("`"$ProjectPath`"") + $ExtraArgs
        $proc = Start-Process -FilePath $exe -ArgumentList $argList -PassThru -WindowStyle Hidden
        $projDir = Split-Path -LiteralPath $ProjectPath -Parent
        $projName = [System.IO.Path]::GetFileNameWithoutExtension($ProjectPath)
        $logPath = Join-Path -Path $projDir -ChildPath ("Saved\Logs\$projName.log")
        return @{
            pid = "$($proc.Id)"
            log_path = "$logPath"
            project_dir = "$projDir"
            project_name = "$projName"
        }
    }

    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock = $script
        ArgumentList = @($EnginePath, $ProjectPath, ,$ExtraArgs)
        ErrorAction = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $r = Invoke-Command @invokeArgs

    @{
        ok = $true
        pid = "$($r.pid)"
        log_path = "$($r.log_path)"
        project_dir = "$($r.project_dir)"
        project_name = "$($r.project_name)"
    } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; pid = ""; log_path = ""; message = "$($_.Exception.Message)" } | ConvertTo-Json -Compress
    exit 1
}
