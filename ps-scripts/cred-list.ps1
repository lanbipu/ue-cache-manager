# Lists all credentials matching prefix UECM:* (we never list system creds).
# Output: JSON array of { alias, target_type } — note: passwords are NOT exposed.

[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; chcp 65001 | Out-Null

$ErrorActionPreference = 'SilentlyContinue'

$raw = & cmdkey.exe /list:UECM:*
# cmdkey output is human-readable, not JSON. Parse line-by-line for "Target: <alias>".
$results = @()
foreach ($line in $raw) {
    if ($line -match '^\s*Target:\s*(.+)$') {
        $alias = $Matches[1].Trim()
        $results += [PSCustomObject]@{ alias = $alias }
    }
}
ConvertTo-Json -InputObject @($results) -Compress
