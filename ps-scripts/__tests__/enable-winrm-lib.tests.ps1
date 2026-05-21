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
# Get-UecmLogDirCandidates: 顺序 = scriptRoot -> ProgramData\UECM\Logs -> Temp
$cand = Get-UecmLogDirCandidates -ScriptRoot 'C:\Pkg' -ProgramData 'C:\PD' -Temp 'C:\T'
if ($cand.Count -ne 3) { throw "expected 3 candidates, got $($cand.Count)" }
if ($cand[0] -ne 'C:\Pkg')            { throw "candidate[0] should be scriptRoot, got $($cand[0])" }
if ($cand[1] -ne 'C:\PD\UECM\Logs')   { throw "candidate[1] should be ProgramData\UECM\Logs, got $($cand[1])" }
if ($cand[2] -ne 'C:\T')              { throw "candidate[2] should be Temp, got $($cand[2])" }
# 空 scriptRoot 被跳过
$cand2 = Get-UecmLogDirCandidates -ScriptRoot '' -ProgramData 'C:\PD' -Temp 'C:\T'
if ($cand2[0] -ne 'C:\PD\UECM\Logs') { throw "empty scriptRoot should be skipped" }

# Get-UecmCriticalVerdict: 全绿 -> SUCCESS / 缺项 -> FAILED + missing
$green = @{
    winrm_service_status = 'Running'; wsman_localhost_ok = $true
    local_account_token_filter_policy = 1; long_paths_enabled = 1
    lanman_server_running = $true; winmgmt_running = $true; fps_smb_in_tcp_enabled = $true
}
$v1 = Get-UecmCriticalVerdict -State $green
if ($v1.verdict -ne 'SUCCESS') { throw "all-green should be SUCCESS, got $($v1.verdict)" }
if ($v1.missing.Count -ne 0)   { throw "all-green missing should be empty, got $($v1.missing -join ',')" }

$noLatfp = $green.Clone(); $noLatfp.local_account_token_filter_policy = $null
$v2 = Get-UecmCriticalVerdict -State $noLatfp
if ($v2.verdict -ne 'FAILED')        { throw "missing latfp should be FAILED" }
if ($v2.missing -notcontains 'latfp'){ throw "missing should contain latfp, got $($v2.missing -join ',')" }

$noFw = $green.Clone(); $noFw.fps_smb_in_tcp_enabled = $false
$v3 = Get-UecmCriticalVerdict -State $noFw
if ($v3.missing -notcontains 'firewall_445') { throw "missing should contain firewall_445" }
"OK"
