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

# Build-UecmStepTable: 结构完整 / Key 唯一 / 关键步 Critical 标记正确
$steps = Build-UecmStepTable
if ($steps.Count -lt 10) { throw "expected >=10 steps, got $($steps.Count)" }
$keys = $steps | ForEach-Object { $_.Key }
$dups = $keys | Group-Object | Where-Object { $_.Count -gt 1 }
if ($dups) { throw "duplicate step keys: $($dups.Name -join ',')" }
foreach ($s in $steps) {
    foreach ($f in 'Key','Critical','Label','Action') {
        if (-not $s.ContainsKey($f)) { throw "step $($s.Key) missing field $f" }
    }
    if ($s.Action -isnot [scriptblock]) { throw "step $($s.Key) Action not a scriptblock" }
}
$byKey = @{}; $steps | ForEach-Object { $byKey[$_.Key] = $_ }
if ($byKey['winrm_service'].Critical -ne $true)     { throw "winrm_service must be Critical" }
if ($byKey['winrm_psremoting'].Critical -ne $false) { throw "winrm_psremoting must be non-critical (firewall 异常时不拖垮整体)" }
if ($byKey['quickconfig'].Critical -ne $false)      { throw "quickconfig must be non-critical" }
if (-not $byKey.ContainsKey('smb_firewall'))        { throw "missing smb_firewall step" }
if ($byKey['smb_firewall'].Critical -ne $true)      { throw "smb_firewall must be Critical" }
if ($byKey['latfp'].Critical -ne $true)             { throw "latfp must be Critical" }
if ($byKey['winmgmt'].Critical -ne $true)           { throw "winmgmt must be Critical" }
if ($byKey['long_paths'].Critical -ne $true)        { throw "long_paths must be Critical" }

# Get-UecmWinRmState: 含 4 个新判定字段(只读查询, dot-source 下可安全调用)
$state = Get-UecmWinRmState
foreach ($k in 'long_paths_enabled','lanman_server_running','winmgmt_running','fps_smb_in_tcp_enabled') {
    if (-not $state.ContainsKey($k)) { throw "Get-UecmWinRmState missing key $k" }
}

# Write-UecmLogLine: 写到第一个可写目录, 返回最终文件路径
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("uecmlogtest-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $tmp -Force | Out-Null
# ScriptRoot 故意指向一个永远无法写入的路径（kernel32.dll 是文件, 其子路径 AppendAllText 必然失败）
$logState = New-UecmLogState -ScriptRoot 'C:\Windows\System32\kernel32.dll\bad-subdir' -ProgramData $tmp -Temp $tmp
if (-not $logState.Path) { throw "New-UecmLogState should resolve a writable path" }
if ($logState.Path -notlike "$tmp*") { throw "should fall back to writable dir (ProgramData or Temp), got $($logState.Path)" }
Write-UecmLogLine -LogState $logState -Status 'ok' -Key 'k1' -Message 'hello'
if (-not (Test-Path $logState.Path)) { throw "log file not created" }
if ((Get-Content $logState.Path -Raw) -notmatch 'k1.*hello') { throw "log line not written" }

# Invoke-UecmStep: 失败步骤不抛, 返回 fail 结果
$okStep   = @{ Key='s_ok';   Critical=$true;  Label='ok step';   Action={ param($c) Add-UecmChange $c 'did ok' } }
$failStep = @{ Key='s_fail'; Critical=$false; Label='fail step'; Action={ param($c) throw 'boom' } }
$changes = New-Object 'System.Collections.Generic.List[string]'
$r1 = Invoke-UecmStep -Step $okStep   -Changes $changes -LogState $logState
$r2 = Invoke-UecmStep -Step $failStep -Changes $changes -LogState $logState
if ($r1.status -ne 'ok')   { throw "ok step should be ok, got $($r1.status)" }
if ($r2.status -ne 'fail') { throw "fail step should be fail, got $($r2.status)" }
if ($r2.message -notmatch 'boom') { throw "fail step should capture exception message" }
Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue

# Enable-UecmLocalAccountRemoteAdmin: 只能设值, 绝不能 New-Item -Force 删键重建.
# 真实目标是 HKLM\...\Policies\System (装着 EnableLUA 等系统策略), New-Item -Force
# 会把整个键清空 -- 这里用 HKCU 临时键 + 预置一个兄弟值, 跑完断言兄弟值还在 + LATFP=1.
$EnableLocalAccountRemoteAdmin = $true   # dot-source 下默认 $false 会 early-return
$latfpKey = "HKCU:\Software\UECM_LIBTEST_$([guid]::NewGuid().ToString('N'))"
try {
    New-Item -Path $latfpKey -Force | Out-Null
    New-ItemProperty -Path $latfpKey -Name 'SiblingValue' -PropertyType DWord -Value 42 -Force | Out-Null
    $latfpChanges = New-Object 'System.Collections.Generic.List[string]'
    Enable-UecmLocalAccountRemoteAdmin -Changes $latfpChanges -RegistryPath $latfpKey
    $sib = (Get-ItemProperty -Path $latfpKey -Name 'SiblingValue' -ErrorAction SilentlyContinue).SiblingValue
    if ($sib -ne 42) { throw "Enable-UecmLocalAccountRemoteAdmin clobbered sibling values (got '$sib', expected 42) -- New-Item -Force wiped the key" }
    $latfpVal = (Get-ItemProperty -Path $latfpKey -Name 'LocalAccountTokenFilterPolicy' -ErrorAction SilentlyContinue).LocalAccountTokenFilterPolicy
    if ($latfpVal -ne 1) { throw "LocalAccountTokenFilterPolicy should be 1, got '$latfpVal'" }
} finally {
    Remove-Item -Path $latfpKey -Recurse -Force -ErrorAction SilentlyContinue
    $EnableLocalAccountRemoteAdmin = $false
}

"OK"
