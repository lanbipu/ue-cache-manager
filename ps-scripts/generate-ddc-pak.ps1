param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$EnginePath,
    [Parameter(Mandatory=$true)] [string]$ProjectPath,
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
        param($EnginePath, $ProjectPath)
        $exe = Join-Path -Path $EnginePath -ChildPath 'Engine\Binaries\Win64\UnrealEditor.exe'
        $existsExe = Test-Path -LiteralPath $exe
        $existsProject = Test-Path -LiteralPath $ProjectPath
        $projectDir = Split-Path -LiteralPath $ProjectPath -Parent
        $ddcDir = Join-Path -Path $projectDir -ChildPath 'DerivedDataCache'
        $hasDdcDir = Test-Path -LiteralPath $ddcDir
        return @{
            exe_exists = "$existsExe" -eq "True"
            proj_exists = "$existsProject" -eq "True"
            ddc_dir_exists = "$hasDdcDir" -eq "True"
            ddc_dir = "$ddcDir"
        }
    }
    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock = $script
        ArgumentList = @($EnginePath, $ProjectPath)
        ErrorAction = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $r = Invoke-Command @invokeArgs

    @{
        ok = $true
        exe_exists = $r.exe_exists
        proj_exists = $r.proj_exists
        ddc_dir_exists = $r.ddc_dir_exists
        ddc_dir = "$($r.ddc_dir)"
    } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; message = "$($_.Exception.Message)" } | ConvertTo-Json -Compress
    exit 1
}
