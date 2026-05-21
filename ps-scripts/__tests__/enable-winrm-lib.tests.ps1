# Manual: run on a Windows box. dot-sources enable-winrm.ps1 with -LibraryOnly
# so only function definitions load (no admin check, no system changes), then
# asserts the pure helpers behave. Prints OK on success, throws on failure.
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\..\enable-winrm.ps1" -LibraryOnly

if (-not (Get-Command Format-UecmLogLine -ErrorAction SilentlyContinue)) {
    throw 'Format-UecmLogLine not defined after -LibraryOnly dot-source'
}

# Format-UecmLogLine: 固定时间戳 + 状态 + key + message
$line = Format-UecmLogLine -Timestamp ([datetime]'2026-05-21 21:40:03') -Status 'ok' -Key 'latfp' -Message 'LATFP=1'
if ($line -ne '[2026-05-21 21:40:03] [OK]    latfp            | LATFP=1') {
    throw "Format-UecmLogLine wrong output: <$line>"
}
$fail = Format-UecmLogLine -Timestamp ([datetime]'2026-05-21 21:40:05') -Status 'fail' -Key 'quickconfig' -Message 'firewall check failed'
if ($fail -notmatch '^\[2026-05-21 21:40:05\] \[FAIL\]  quickconfig      \| firewall check failed$') {
    throw "Format-UecmLogLine FAIL line wrong: <$fail>"
}
"OK"
