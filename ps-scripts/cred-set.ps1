# Stores a generic credential in Windows Credential Manager via cmdkey.
# Parameters: -Alias <string> -Username <string> -Password <string>
# Output: JSON { ok: bool, message: string }

param(
    [Parameter(Mandatory=$true)] [string]$Alias,
    [Parameter(Mandatory=$true)] [string]$Username,
    [Parameter(Mandatory=$true)] [string]$Password
)

[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; chcp 65001 | Out-Null

$ErrorActionPreference = 'Stop'

try {
    # Use Start-Process so the password isn't visible in `Get-History` / Process Explorer
    # cmdkey expects /pass:<value> with no quoting; we pass via argument list to avoid shell quoting.
    $p = Start-Process -FilePath 'cmdkey.exe' `
        -ArgumentList @("/generic:$Alias", "/user:$Username", "/pass:$Password") `
        -NoNewWindow -Wait -PassThru -RedirectStandardOutput 'NUL'
    if ($p.ExitCode -ne 0) {
        @{ ok = $false; message = "cmdkey exited $($p.ExitCode)" } | ConvertTo-Json -Compress
        exit 1
    }
    @{ ok = $true; message = "credential stored" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
