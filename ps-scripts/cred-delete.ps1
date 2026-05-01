# Deletes a stored credential by alias.
# Parameters: -Alias <string>
# Output: JSON { ok: bool, message: string }

param(
    [Parameter(Mandatory=$true)] [string]$Alias
)

$ErrorActionPreference = 'Stop'

try {
    $p = Start-Process -FilePath 'cmdkey.exe' `
        -ArgumentList @("/delete:$Alias") `
        -NoNewWindow -Wait -PassThru -RedirectStandardOutput 'NUL'
    if ($p.ExitCode -ne 0) {
        @{ ok = $false; message = "cmdkey exited $($p.ExitCode)" } | ConvertTo-Json -Compress
        exit 1
    }
    @{ ok = $true; message = "credential deleted" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
