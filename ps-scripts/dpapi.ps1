# Wraps .NET System.Security.Cryptography.ProtectedData (DPAPI, CurrentUser
# scope) so the Rust side can encrypt + decrypt without a windows-rs FFI dep.
# Parameters:
#   -Mode <protect|unprotect>
#   -DataB64 <string>   base64 of plaintext (protect) or ciphertext (unprotect)
# Output: JSON { ok: bool, data: string, message: string }
#   `data` is base64 of ciphertext (protect) or plaintext (unprotect).
# Notes:
#   Argv exposes the base64 to Process Explorer — same risk profile as
#   cred-set.ps1's -Password argv. The plaintext window is one Invoke call.

param(
    [Parameter(Mandatory=$true)] [ValidateSet('protect','unprotect')] [string]$Mode,
    [Parameter(Mandatory=$true)] [string]$DataB64
)

[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; chcp 65001 | Out-Null

$ErrorActionPreference = 'Stop'

try {
    Add-Type -AssemblyName System.Security | Out-Null
    $bytes = [Convert]::FromBase64String($DataB64)
    if ($Mode -eq 'protect') {
        $out = [System.Security.Cryptography.ProtectedData]::Protect(
            $bytes, $null, [System.Security.Cryptography.DataProtectionScope]::CurrentUser
        )
    } else {
        $out = [System.Security.Cryptography.ProtectedData]::Unprotect(
            $bytes, $null, [System.Security.Cryptography.DataProtectionScope]::CurrentUser
        )
    }
    $b64 = [Convert]::ToBase64String($out)
    @{ ok = $true; data = $b64; message = "" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; data = ""; message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
