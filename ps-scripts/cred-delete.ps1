# Deletes a stored credential by alias.
# Parameters: -Alias <string>
# Output: JSON { ok: bool, message: string }

param(
    [Parameter(Mandatory=$true)] [string]$Alias
)

[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; chcp 65001 | Out-Null

$ErrorActionPreference = 'Stop'

try {
    # Idempotent: deleting a credential that isn't stored is a no-op success, not
    # a failure. `cmdkey /delete` on a missing target exits non-zero, so check
    # presence first (the alias is an ASCII target name, so the match is
    # locale-independent). This lets callers treat a non-zero exit as a GENUINE
    # delete failure (perms/cmdkey) rather than confusing it with "no such entry"
    # (e.g. a SecretStore-backed alias that never had a Credential Manager entry).
    $list = (& cmdkey.exe "/list:$Alias" 2>$null) -join "`n"
    if ($list -notmatch [regex]::Escape($Alias)) {
        @{ ok = $true; message = "no credential for '$Alias' (nothing to delete)" } | ConvertTo-Json -Compress
        return
    }
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
