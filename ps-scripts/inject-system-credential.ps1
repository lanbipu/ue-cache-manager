# Injects an SMB credential into THIS node's SYSTEM-account Credential Manager
# (and the current SSH user's), so LocalSystem services (UE engine /
# RenderStream Service) can transparently authenticate to a Mode B share.
#
# Node-pure: runs locally on the target (shipped + executed via SSH -File).
# Writing SYSTEM's credential vault requires running cmdkey AS SYSTEM, which we
# do via PsExec64 -s. PsExec64.exe is installed at onboarding (enable-ssh.ps1)
# to C:\ProgramData\UECM\PsExec64.exe.
#
# stdin: JSON { "TargetHost", "SvcUsername", "SvcPassword" }
#   TargetHost  = the SMB host the credential authenticates to (cmdkey /add target)
#   SvcUsername = the share service account, e.g. "ddc-svc"
#   SvcPassword = its password
# Output: JSON { ok, message }

[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; chcp 65001 | Out-Null

$ErrorActionPreference = 'Stop'

try {
    $p = [Console]::In.ReadToEnd() | ConvertFrom-Json
    $TargetHost  = $p.TargetHost
    $SvcUsername = $p.SvcUsername
    $SvcPassword = $p.SvcPassword
    if ([string]::IsNullOrWhiteSpace($TargetHost) -or
        [string]::IsNullOrWhiteSpace($SvcUsername) -or
        [string]::IsNullOrEmpty($SvcPassword)) {
        throw "TargetHost, SvcUsername, SvcPassword are required"
    }

    $psexec = Join-Path $env:ProgramData 'UECM\PsExec64.exe'
    if (-not (Test-Path -LiteralPath $psexec)) {
        @{ ok = $false; message = "PsExec64.exe not found at $psexec; re-run UECM-Bootstrap.cmd on this node to install it" } | ConvertTo-Json -Compress
        exit 1
    }

    # PsExec writes "Connecting to local system..." status lines to stderr, and
    # cmdkey also writes to stderr on success. With $ErrorActionPreference='Stop',
    # PowerShell turns a native command's stderr (merged via 2>&1) into a
    # terminating error, so drop to 'Continue' around these calls and rely on the
    # explicit cmdkey /list verification below to decide success.
    $prevPref = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        cmdkey.exe "/add:$TargetHost" "/user:$SvcUsername" "/pass:$SvcPassword" 2>&1 | Out-Null
        & $psexec -accepteula -nobanner -s -i 0 cmdkey.exe "/add:$TargetHost" "/user:$SvcUsername" "/pass:$SvcPassword" 2>&1 | Out-Null
        $listOut = (& $psexec -accepteula -nobanner -s -i 0 cmdkey.exe /list:$TargetHost 2>&1) -join "`n"
    }
    finally {
        $ErrorActionPreference = $prevPref
    }

    if ($listOut -notmatch [regex]::Escape($SvcUsername)) {
        throw "SYSTEM cred verify failed; cmdkey /list under SYSTEM did not show '$SvcUsername'. Got: $listOut"
    }
    @{ ok = $true; message = "user + SYSTEM creds injected for $TargetHost" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
