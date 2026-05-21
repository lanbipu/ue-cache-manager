# enable-winrm.ps1 步骤表驱动重构 + 运行 log 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 `ps-scripts/enable-winrm.ps1` 改成步骤表驱动 + 单步失败隔离 + 写一份人读 log,让脚本在任何环境(含防火墙被禁用)都能跑完并如实报告,达成 L1+L2 可纳管。

**Architecture:** 单文件脚本内抽出 4 个纯函数(log 行格式化 / log 路径候选 / 末尾判定 / 步骤表构建)+ 1 个 runner;主执行体用 `-LibraryOnly` switch 做 dot-source guard,使纯函数可被测试脚本加载而不触发系统副作用。测试沿用现有手写断言风格(`throw` 失败 / 结尾 `OK`)。

**Tech Stack:** Windows PowerShell 5.1(目标机自带),从 WSL 用 `powershell.exe` 调用;无 Pester,纯 `.ps1` 断言脚本。

**关键约束:** `enable-winrm.ps1` 被 `src-tauri/src/core/bootstrap.rs` 用 `include_str!` 单文件嵌入,且 `package-winrm-bootstrap.ps1` 只拷它一个文件 → **所有逻辑必须留在 enable-winrm.ps1 单文件内,不得拆分外部依赖**。

---

## File Structure

- **Modify:** `ps-scripts/enable-winrm.ps1`(498 行)
  - `param` 增加 `[switch]$LibraryOnly`
  - 新增纯函数:`Format-UecmLogLine` / `Get-UecmLogDirCandidates` / `Get-UecmCriticalVerdict` / `Build-UecmStepTable`
  - 新增副作用函数:`Write-UecmLogLine`(解析可写目录 + 追加)、`Invoke-UecmStep`(单步 try/catch)
  - 扩展 `Get-UecmWinRmState`:补 `long_paths_enabled` / `lanman_server_running` / `winmgmt_running` / `fps_smb_in_tcp_enabled`
  - 主执行 try/catch 体包进 `if (-not $LibraryOnly)`,改用步骤表 + runner + 末尾判定 + 扩展 JSON
- **Create:** `ps-scripts/__tests__/enable-winrm-lib.tests.ps1`(纯函数断言测试)

运行测试(在 repo 根):
```bash
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$(wslpath -w ps-scripts/__tests__/enable-winrm-lib.tests.ps1)"
```
全过 → 末行打印 `OK`;任一断言失败 → 抛异常、非零退出。

---

## Task 1: 加 `-LibraryOnly` guard,让脚本可被 dot-source

**Files:**
- Modify: `ps-scripts/enable-winrm.ps1`(param 段 26-49、主执行段 443-498)
- Test: `ps-scripts/__tests__/enable-winrm-lib.tests.ps1`(新建)

- [ ] **Step 1: 写失败测试** — 新建 `ps-scripts/__tests__/enable-winrm-lib.tests.ps1`:

```powershell
# Manual: run on a Windows box. dot-sources enable-winrm.ps1 with -LibraryOnly
# so only function definitions load (no admin check, no system changes), then
# asserts the pure helpers behave. Prints OK on success, throws on failure.
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\..\enable-winrm.ps1" -LibraryOnly

if (-not (Get-Command Format-UecmLogLine -ErrorAction SilentlyContinue)) {
    throw 'Format-UecmLogLine not defined after -LibraryOnly dot-source'
}
"OK"
```

- [ ] **Step 2: 跑测试确认失败**

Run: `powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$(wslpath -w ps-scripts/__tests__/enable-winrm-lib.tests.ps1)"`
Expected: FAIL — 报 `A parameter cannot be found that matches parameter name 'LibraryOnly'`(param 还没加)。

- [ ] **Step 3: 加 param + guard** — 在 `enable-winrm.ps1` 的 `param(...)` 末尾(`[switch]$CheckOnly` 之后)加一行:

```powershell
    [switch]$CheckOnly,

    [switch]$LibraryOnly
)
```

把主执行体(当前 `try { if ($CheckOnly) {...} ... } catch {...}`,约 443-498 行整段)包进 guard:

```powershell
if (-not $LibraryOnly) {
try {
    # ... 现有主执行体原样保留 ...
}
catch {
    # ... 现有 catch 原样保留 ...
}
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$(wslpath -w ps-scripts/__tests__/enable-winrm-lib.tests.ps1)"`
Expected: 仍 FAIL，但报错变成 `Format-UecmLogLine not defined`(说明 dot-source 成功了,只是函数还没写)。这验证了 guard 生效。

- [ ] **Step 5: 提交**

```bash
git add ps-scripts/enable-winrm.ps1 ps-scripts/__tests__/enable-winrm-lib.tests.ps1
git -c user.name='lanpc' -c user.email='lanbipu@gmail.com' commit -m 'refactor(bootstrap): add -LibraryOnly guard for dot-source testing'
```

---

## Task 2: `Format-UecmLogLine` — 时间线行格式化(纯函数)

**Files:**
- Modify: `ps-scripts/enable-winrm.ps1`(函数定义区,Test-UecmAdministrator 之前)
- Test: `ps-scripts/__tests__/enable-winrm-lib.tests.ps1`

- [ ] **Step 1: 加失败测试** — 在测试文件 `"OK"` 之前插入:

```powershell
# Format-UecmLogLine: 固定时间戳 + 状态 + key + message
$line = Format-UecmLogLine -Timestamp ([datetime]'2026-05-21 21:40:03') -Status 'ok' -Key 'latfp' -Message 'LATFP=1'
if ($line -ne '[2026-05-21 21:40:03] [OK]    latfp            | LATFP=1') {
    throw "Format-UecmLogLine wrong output: <$line>"
}
$fail = Format-UecmLogLine -Timestamp ([datetime]'2026-05-21 21:40:05') -Status 'fail' -Key 'quickconfig' -Message 'firewall check failed'
if ($fail -notmatch '^\[2026-05-21 21:40:05\] \[FAIL\]  quickconfig      \| firewall check failed$') {
    throw "Format-UecmLogLine FAIL line wrong: <$fail>"
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$(wslpath -w ps-scripts/__tests__/enable-winrm-lib.tests.ps1)"`
Expected: FAIL — `Format-UecmLogLine not defined` 或输出不匹配。

- [ ] **Step 3: 实现** — 在 `enable-winrm.ps1` 第 55 行(`$ErrorActionPreference = 'Stop'` 之后)插入:

```powershell
function Format-UecmLogLine {
    param(
        [datetime]$Timestamp,
        [string]$Status,
        [string]$Key,
        [string]$Message
    )
    $ts = $Timestamp.ToString('yyyy-MM-dd HH:mm:ss')
    $st = $Status.ToUpper().PadRight(4)
    $k  = $Key.PadRight(16)
    return "[$ts] [$st] $k | $Message"
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$(wslpath -w ps-scripts/__tests__/enable-winrm-lib.tests.ps1)"`
Expected: PASS(暂到 `Get-UecmLogDirCandidates` 那条新断言前;本步只看 Format 两条过)。

- [ ] **Step 5: 提交**

```bash
git add ps-scripts/enable-winrm.ps1 ps-scripts/__tests__/enable-winrm-lib.tests.ps1
git -c user.name='lanpc' -c user.email='lanbipu@gmail.com' commit -m 'feat(bootstrap): Format-UecmLogLine timeline formatter'
```

---

## Task 3: `Get-UecmLogDirCandidates` — log 目录候选顺序(纯函数)

**Files:**
- Modify: `ps-scripts/enable-winrm.ps1`
- Test: `ps-scripts/__tests__/enable-winrm-lib.tests.ps1`

- [ ] **Step 1: 加失败测试** — 在 `"OK"` 之前插入:

```powershell
# Get-UecmLogDirCandidates: 顺序 = scriptRoot -> ProgramData\UECM\Logs -> Temp
$cand = Get-UecmLogDirCandidates -ScriptRoot 'C:\Pkg' -ProgramData 'C:\PD' -Temp 'C:\T'
if ($cand.Count -ne 3) { throw "expected 3 candidates, got $($cand.Count)" }
if ($cand[0] -ne 'C:\Pkg')            { throw "candidate[0] should be scriptRoot, got $($cand[0])" }
if ($cand[1] -ne 'C:\PD\UECM\Logs')   { throw "candidate[1] should be ProgramData\UECM\Logs, got $($cand[1])" }
if ($cand[2] -ne 'C:\T')              { throw "candidate[2] should be Temp, got $($cand[2])" }
# 空 scriptRoot 被跳过
$cand2 = Get-UecmLogDirCandidates -ScriptRoot '' -ProgramData 'C:\PD' -Temp 'C:\T'
if ($cand2[0] -ne 'C:\PD\UECM\Logs') { throw "empty scriptRoot should be skipped" }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$(wslpath -w ps-scripts/__tests__/enable-winrm-lib.tests.ps1)"`
Expected: FAIL — `Get-UecmLogDirCandidates not defined`。

- [ ] **Step 3: 实现** — 在 `Format-UecmLogLine` 之后插入:

```powershell
function Get-UecmLogDirCandidates {
    param(
        [string]$ScriptRoot,
        [string]$ProgramData,
        [string]$Temp
    )
    $dirs = New-Object 'System.Collections.Generic.List[string]'
    if (-not [string]::IsNullOrWhiteSpace($ScriptRoot))  { $dirs.Add($ScriptRoot) | Out-Null }
    if (-not [string]::IsNullOrWhiteSpace($ProgramData)) { $dirs.Add((Join-Path $ProgramData 'UECM\Logs')) | Out-Null }
    if (-not [string]::IsNullOrWhiteSpace($Temp))        { $dirs.Add($Temp) | Out-Null }
    return $dirs.ToArray()
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$(wslpath -w ps-scripts/__tests__/enable-winrm-lib.tests.ps1)"`
Expected: PASS(到此 Format + Candidates 全过)。

- [ ] **Step 5: 提交**

```bash
git add ps-scripts/enable-winrm.ps1 ps-scripts/__tests__/enable-winrm-lib.tests.ps1
git -c user.name='lanpc' -c user.email='lanbipu@gmail.com' commit -m 'feat(bootstrap): Get-UecmLogDirCandidates fallback order'
```

---

## Task 4: `Get-UecmCriticalVerdict` — 末尾关键项判定(纯函数)

**Files:**
- Modify: `ps-scripts/enable-winrm.ps1`
- Test: `ps-scripts/__tests__/enable-winrm-lib.tests.ps1`

- [ ] **Step 1: 加失败测试** — 在 `"OK"` 之前插入:

```powershell
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
```

- [ ] **Step 2: 跑测试确认失败**

Run: `powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$(wslpath -w ps-scripts/__tests__/enable-winrm-lib.tests.ps1)"`
Expected: FAIL — `Get-UecmCriticalVerdict not defined`。

- [ ] **Step 3: 实现** — 在 `Get-UecmLogDirCandidates` 之后插入:

```powershell
function Get-UecmCriticalVerdict {
    param([hashtable]$State)
    $missing = New-Object 'System.Collections.Generic.List[string]'
    if ($State.winrm_service_status -ne 'Running')         { $missing.Add('winrm_service')  | Out-Null }
    if (-not $State.wsman_localhost_ok)                    { $missing.Add('wsman_localhost')| Out-Null }
    if ([int]($State.local_account_token_filter_policy) -ne 1) { $missing.Add('latfp')      | Out-Null }
    if ([int]($State.long_paths_enabled) -ne 1)            { $missing.Add('long_paths')     | Out-Null }
    if ($State.lanman_server_running -ne $true)            { $missing.Add('lanman_server')  | Out-Null }
    if ($State.winmgmt_running -ne $true)                  { $missing.Add('winmgmt')        | Out-Null }
    if ($State.fps_smb_in_tcp_enabled -ne $true)           { $missing.Add('firewall_445')   | Out-Null }
    return @{
        verdict = $(if ($missing.Count -eq 0) { 'SUCCESS' } else { 'FAILED' })
        missing = $missing.ToArray()
    }
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$(wslpath -w ps-scripts/__tests__/enable-winrm-lib.tests.ps1)"`
Expected: PASS。

- [ ] **Step 5: 提交**

```bash
git add ps-scripts/enable-winrm.ps1 ps-scripts/__tests__/enable-winrm-lib.tests.ps1
git -c user.name='lanpc' -c user.email='lanbipu@gmail.com' commit -m 'feat(bootstrap): Get-UecmCriticalVerdict L1+L2 readiness判定'
```

---

## Task 5: `Build-UecmStepTable` — 步骤表构建(纯函数)

**Files:**
- Modify: `ps-scripts/enable-winrm.ps1`
- Test: `ps-scripts/__tests__/enable-winrm-lib.tests.ps1`

步骤表把现有 10 个 `Enable-Uecm*` / `Set-Uecm*` 动作拆成原子步骤。每步 `@{ Key; Critical; Label; Action }`,`Action` 是 `[scriptblock]`(此 task 只校验结构,不执行)。

- [ ] **Step 1: 加失败测试** — 在 `"OK"` 之前插入:

```powershell
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
```

- [ ] **Step 2: 跑测试确认失败**

Run: `powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$(wslpath -w ps-scripts/__tests__/enable-winrm-lib.tests.ps1)"`
Expected: FAIL — `Build-UecmStepTable not defined`。

- [ ] **Step 3: 实现** — 在 `Get-UecmCriticalVerdict` 之后插入。`Action` 直接复用脚本里**已存在**的 `Enable-Uecm*` / `Set-Uecm*` 函数(它们已定义,接收 `$changes` 列表;此处包一层把它们接进步骤模型):

```powershell
function Build-UecmStepTable {
    # Critical = 参与 L1+L2 末尾判定的步骤。Action 接收一个 [List[string]]$Changes。
    # WinRM 5 连击拆成独立步骤:服务启停 critical;PSRemoting/quickconfig/防火墙规则都
    # non-critical —— 它们正是防火墙被禁用时抛 "Unable to check the status of the
    # firewall" 的地方,但 listener 通常在报错前已建好,末尾判定看 wsman_localhost_ok 实际状态。
    return @(
        @{ Key='network_profile'; Critical=$false; Label='Public 网络改 Private';
           Action={ param($c) Set-UecmNetworkProfile -Changes $c } }
        @{ Key='winrm_service';   Critical=$true;  Label='WinRM 设 Automatic 并启动';
           Action={ param($c)
               Set-Service -Name WinRM -StartupType Automatic -ErrorAction Stop; Add-UecmChange $c 'WinRM 设为 Automatic'
               Start-Service -Name WinRM -ErrorAction Stop;                       Add-UecmChange $c 'WinRM 已启动' } }
        @{ Key='winrm_psremoting'; Critical=$false; Label='Enable-PSRemoting(防火墙异常时可失败,listener 多已建)';
           Action={ param($c) Enable-PSRemoting -Force -SkipNetworkProfileCheck | Out-Null; Add-UecmChange $c 'PSRemoting 已启用' } }
        @{ Key='quickconfig';     Critical=$false; Label='winrm quickconfig 便捷封装(可失败)';
           Action={ param($c) & winrm quickconfig -q | Out-Null; Add-UecmChange $c 'winrm quickconfig 完成' } }
        @{ Key='winrm_fw_rule';   Critical=$false; Label='WinRM HTTP-In 防火墙规则就位';
           Action={ param($c) Enable-NetFirewallRule -DisplayGroup 'Windows Remote Management' -ErrorAction Stop | Out-Null; Add-UecmChange $c 'WinRM 防火墙规则已启用' } }
        @{ Key='firewall_scope';  Critical=$false; Label='WinRM 防火墙限制源 IP(可选)';
           Action={ param($c) Set-UecmFirewallScope -Changes $c } }
        @{ Key='trusted_hosts';   Critical=$false; Label='WSMan TrustedHosts(可选)';
           Action={ param($c) Set-UecmTrustedHosts -Changes $c } }
        @{ Key='latfp';           Critical=$true;  Label='LocalAccountTokenFilterPolicy=1';
           Action={ param($c) Enable-UecmLocalAccountRemoteAdmin -Changes $c } }
        @{ Key='smb_firewall';    Critical=$true;  Label='LanmanServer + FPS-SMB-In-TCP 规则';
           Action={ param($c) Enable-UecmSmbServer -Changes $c } }
        @{ Key='winmgmt';         Critical=$true;  Label='Winmgmt 服务 Auto+Running';
           Action={ param($c) Enable-UecmWmi -Changes $c } }
        @{ Key='execution_policy'; Critical=$false; Label='LocalMachine 执行策略';
           Action={ param($c) Set-UecmLocalExecutionPolicy -Changes $c } }
        @{ Key='long_paths';      Critical=$true;  Label='LongPathsEnabled=1';
           Action={ param($c) Enable-UecmLongPaths -Changes $c } }
        @{ Key='power_plan';      Critical=$false; Label='电源计划';
           Action={ param($c) Set-UecmPowerPlan -Changes $c } }
    )
}
```

> **重要:必须同时改 `Enable-UecmWinRm`** —— 删掉它内部第 201-202 行的 `& winrm quickconfig` 调用(quickconfig 已是独立 non-critical 步骤),避免重复执行。步骤表已不再整体调用 `Enable-UecmWinRm`(改为内联拆细的 winrm_service / psremoting / fw_rule),该函数可删或留作备用。
> `smb_firewall` 仍**无条件执行**:`Enable-UecmSmbServer` 内的 `Enable-NetFirewallRule` 设的是规则 `Enabled` 标志,防火墙 profile 全禁用也能成功 —— 不再有"防火墙关就跳过规则"的旧逻辑。

- [ ] **Step 4: 跑测试确认通过**

Run: `powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$(wslpath -w ps-scripts/__tests__/enable-winrm-lib.tests.ps1)"`
Expected: PASS。

- [ ] **Step 5: 提交**

```bash
git add ps-scripts/enable-winrm.ps1 ps-scripts/__tests__/enable-winrm-lib.tests.ps1
git -c user.name='lanpc' -c user.email='lanbipu@gmail.com' commit -m 'feat(bootstrap): Build-UecmStepTable step descriptors'
```

---

## Task 6: `Get-UecmWinRmState` 扩展 4 个判定字段

**Files:**
- Modify: `ps-scripts/enable-winrm.ps1`(`Get-UecmWinRmState` 函数,当前约 140-169 行)
- Test: `ps-scripts/__tests__/enable-winrm-lib.tests.ps1`

`Get-UecmCriticalVerdict` 依赖 `long_paths_enabled` / `lanman_server_running` / `winmgmt_running` / `fps_smb_in_tcp_enabled`,现有 `Get-UecmWinRmState` 不返回它们。本 task 补上。这是真机查询函数,测试只验证"返回的 hashtable 含这 4 个 key"(用 -LibraryOnly dot-source 后调用是安全的,均为只读查询)。

- [ ] **Step 1: 加失败测试** — 在 `"OK"` 之前插入:

```powershell
# Get-UecmWinRmState: 含 4 个新判定字段(只读查询, dot-source 下可安全调用)
$state = Get-UecmWinRmState
foreach ($k in 'long_paths_enabled','lanman_server_running','winmgmt_running','fps_smb_in_tcp_enabled') {
    if (-not $state.ContainsKey($k)) { throw "Get-UecmWinRmState missing key $k" }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$(wslpath -w ps-scripts/__tests__/enable-winrm-lib.tests.ps1)"`
Expected: FAIL — `Get-UecmWinRmState missing key long_paths_enabled`。

- [ ] **Step 3: 实现** — 在 `Get-UecmWinRmState` 的返回 hashtable 里(`local_account_token_filter_policy = $localAccountTokenPolicy` 之后)追加:

```powershell
        long_paths_enabled = Get-UecmRegistryDword `
            -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem' -Name 'LongPathsEnabled'
        lanman_server_running = ((Get-Service -Name LanmanServer -ErrorAction SilentlyContinue).Status -eq 'Running')
        winmgmt_running = ((Get-Service -Name Winmgmt -ErrorAction SilentlyContinue).Status -eq 'Running')
        fps_smb_in_tcp_enabled = ((Get-NetFirewallRule -Name 'FPS-SMB-In-TCP' -ErrorAction SilentlyContinue).Enabled -eq 'True')
```

- [ ] **Step 4: 跑测试确认通过**

Run: `powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$(wslpath -w ps-scripts/__tests__/enable-winrm-lib.tests.ps1)"`
Expected: PASS。

- [ ] **Step 5: 提交**

```bash
git add ps-scripts/enable-winrm.ps1 ps-scripts/__tests__/enable-winrm-lib.tests.ps1
git -c user.name='lanpc' -c user.email='lanbipu@gmail.com' commit -m 'feat(bootstrap): Get-UecmWinRmState 补 4 个 L2 判定字段'
```

---

## Task 7: `Write-UecmLogLine` + `Invoke-UecmStep`(副作用,集成测试)

**Files:**
- Modify: `ps-scripts/enable-winrm.ps1`
- Test: `ps-scripts/__tests__/enable-winrm-lib.tests.ps1`

- [ ] **Step 1: 加失败测试** — 在 `"OK"` 之前插入(用临时目录验证写入 + runner 隔离行为):

```powershell
# Write-UecmLogLine: 写到第一个可写目录, 返回最终文件路径
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("uecmlogtest-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $tmp -Force | Out-Null
$logState = New-UecmLogState -ScriptRoot 'Z:\nonexistent-readonly' -ProgramData $tmp -Temp $tmp
if (-not $logState.Path) { throw "New-UecmLogState should resolve a writable path" }
if ($logState.Path -notlike "$tmp*") { throw "should fall back to writable dir, got $($logState.Path)" }
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
```

- [ ] **Step 2: 跑测试确认失败**

Run: `powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$(wslpath -w ps-scripts/__tests__/enable-winrm-lib.tests.ps1)"`
Expected: FAIL — `New-UecmLogState not defined`。

- [ ] **Step 3: 实现** — 在 `Build-UecmStepTable` 之后插入:

```powershell
function New-UecmLogState {
    param([string]$ScriptRoot, [string]$ProgramData = $env:ProgramData, [string]$Temp = $env:TEMP)
    $fileName = "UECM-Bootstrap-$env:COMPUTERNAME-$(Get-Date -Format 'yyyyMMdd-HHmmss').log"
    foreach ($dir in (Get-UecmLogDirCandidates -ScriptRoot $ScriptRoot -ProgramData $ProgramData -Temp $Temp)) {
        try {
            if (-not (Test-Path -LiteralPath $dir)) { New-Item -ItemType Directory -Path $dir -Force -ErrorAction Stop | Out-Null }
            $candidate = Join-Path $dir $fileName
            # 探测可写: 创建空文件
            [System.IO.File]::AppendAllText($candidate, '')
            return @{ Path = $candidate; WriteOk = $true }
        } catch { continue }
    }
    return @{ Path = $null; WriteOk = $false }
}

function Write-UecmLogLine {
    param([hashtable]$LogState, [string]$Status, [string]$Key, [string]$Message)
    if (-not $LogState.WriteOk -or -not $LogState.Path) { return }
    $line = Format-UecmLogLine -Timestamp (Get-Date) -Status $Status -Key $Key -Message $Message
    try { [System.IO.File]::AppendAllText($LogState.Path, $line + "`r`n", [System.Text.Encoding]::UTF8) } catch {}
}

function Invoke-UecmStep {
    param([hashtable]$Step, [System.Collections.Generic.List[string]]$Changes, [hashtable]$LogState)
    try {
        & $Step.Action $Changes
        Write-UecmLogLine -LogState $LogState -Status 'ok' -Key $Step.Key -Message $Step.Label
        return @{ key = $Step.Key; status = 'ok'; critical = [bool]$Step.Critical; message = $Step.Label }
    } catch {
        $msg = $_.Exception.Message
        $tag = $(if ($Step.Critical) { '' } else { '  (non-critical, 继续)' })
        Write-UecmLogLine -LogState $LogState -Status 'fail' -Key $Step.Key -Message ($msg + $tag)
        return @{ key = $Step.Key; status = 'fail'; critical = [bool]$Step.Critical; message = $msg }
    }
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$(wslpath -w ps-scripts/__tests__/enable-winrm-lib.tests.ps1)"`
Expected: PASS。

- [ ] **Step 5: 提交**

```bash
git add ps-scripts/enable-winrm.ps1 ps-scripts/__tests__/enable-winrm-lib.tests.ps1
git -c user.name='lanpc' -c user.email='lanbipu@gmail.com' commit -m 'feat(bootstrap): log writer + per-step runner (失败隔离)'
```

---

## Task 8: 改造主执行体 — 步骤表驱动 + 扩展 JSON

**Files:**
- Modify: `ps-scripts/enable-winrm.ps1`(`if (-not $LibraryOnly)` 内的主执行 try 体)

这一步把主流程从"顺序调 10 个函数"换成"步骤表 + runner + log + 末尾判定",并输出扩展 JSON。无新单测(逻辑已被前面纯函数测试覆盖),靠 Task 9 的 `-CheckOnly` 真机核对。

- [ ] **Step 1: 替换主执行 try 体** — 把 `if (-not $LibraryOnly) { try { ... 非 CheckOnly 分支 ... } }` 中 `$changes = New-Object ...` 到 `exit 0` 之间(当前 465-488 行)替换为:

```powershell
    $changes = New-Object 'System.Collections.Generic.List[string]'
    $logState = New-UecmLogState -ScriptRoot $PSScriptRoot
    $stepResults = New-Object 'System.Collections.Generic.List[hashtable]'
    $failed = New-Object 'System.Collections.Generic.List[string]'

    foreach ($step in (Build-UecmStepTable)) {
        $r = Invoke-UecmStep -Step $step -Changes $changes -LogState $logState
        $stepResults.Add($r) | Out-Null
        if ($r.status -ne 'ok') { $failed.Add($r.key) | Out-Null }
    }

    $state = Get-UecmWinRmState
    $fwProfilesDisabled = -not ((Get-NetFirewallProfile -ErrorAction SilentlyContinue | Where-Object { $_.Enabled -eq $true }).Count -gt 0)
    $verdictInfo = Get-UecmCriticalVerdict -State $state
    $ok = ($verdictInfo.verdict -eq 'SUCCESS')

    $summaryLine = "verdict=$($verdictInfo.verdict)  failed_steps=[$($failed -join ',')]  missing_critical=[$($verdictInfo.missing -join ',')]  log_write_ok=$($logState.WriteOk)"
    Write-UecmLogLine -LogState $logState -Status 'sumry' -Key 'SUMMARY' -Message $summaryLine

    @{
        ok = $ok
        message = $(if ($ok) { 'UECM WinRM bootstrap completed' } else { "bootstrap finished with unmet critical items: $($verdictInfo.missing -join ',')" })
        changed = @($changes)
        failed = @($failed)
        missing_critical = @($verdictInfo.missing)
        log_path = $logState.Path
        log_write_ok = $logState.WriteOk
        firewall_profiles_disabled = $fwProfilesDisabled
        state = $state
    } | ConvertTo-Json -Depth 8 -Compress
    exit $(if ($ok) { 0 } else { 1 })
```

- [ ] **Step 2: 修 catch 块丢失 changed 的旧 bug** — 把主 `catch` 块(当前 490-498 行)替换为:

```powershell
catch {
    @{
        ok = $false
        message = $_.Exception.Message
        changed = @($changes)
        failed = @($failed)
        log_path = $(if ($logState) { $logState.Path } else { $null })
        log_write_ok = $(if ($logState) { $logState.WriteOk } else { $false })
        state = Get-UecmWinRmState
    } | ConvertTo-Json -Depth 8 -Compress
    exit 1
}
```

> 注:`$changes`/`$failed`/`$logState` 现在在 try 外不可见 → 把这三个变量的 `New-Object` 初始化**上移到 `try {` 之前**(仍在 `if (-not $LibraryOnly)` 内),catch 才能引用。

- [ ] **Step 3: 跑纯函数测试确认没回归**

Run: `powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$(wslpath -w ps-scripts/__tests__/enable-winrm-lib.tests.ps1)"`
Expected: PASS(纯函数不受主体改动影响)。

- [ ] **Step 4: 语法自检**

Run: `powershell.exe -NoProfile -Command "$null = [System.Management.Automation.PSParser]::Tokenize((Get-Content -Raw '$(wslpath -w ps-scripts/enable-winrm.ps1)'), [ref]$null); 'parse-ok'"`
Expected: 打印 `parse-ok`(脚本能被解析,无语法错误)。

- [ ] **Step 5: 提交**

```bash
git add ps-scripts/enable-winrm.ps1
git -c user.name='lanpc' -c user.email='lanbipu@gmail.com' commit -m 'refactor(bootstrap): 主执行体改步骤表驱动 + 扩展 JSON + 修 changed 丢失'
```

---

## Task 9: `-CheckOnly` 也写 log + 真机核对

**Files:**
- Modify: `ps-scripts/enable-winrm.ps1`(`if ($CheckOnly)` 分支)

- [ ] **Step 1: `-CheckOnly` 分支补 log 写入** — 在 `if ($CheckOnly) {` 块里,`$checkState = Get-UecmWinRmState` 之后加:

```powershell
        $logState = New-UecmLogState -ScriptRoot $PSScriptRoot
        $verdictInfo = Get-UecmCriticalVerdict -State $checkState
        Write-UecmLogLine -LogState $logState -Status 'sumry' -Key 'CHECKONLY' `
            -Message "verdict=$($verdictInfo.verdict)  missing_critical=[$($verdictInfo.missing -join ',')]"
```

并把 `-CheckOnly` 输出的 JSON 加上 `log_path` / `log_write_ok`(在现有 `state = $checkState` 同级 hashtable 里追加这两个 key)。

- [ ] **Step 2: 真机核对(lanPC,本机只读检查)**

Run:
```bash
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$(wslpath -w ps-scripts/enable-winrm.ps1)" -CheckOnly
```
Expected: 输出 JSON 含 `verdict`/`state`/`log_path`/`log_write_ok`,且**不修改系统**;同目录(或 fallback)生成一个 `UECM-Bootstrap-*.log`,内含一行 `[CHECKONLY]`。

- [ ] **Step 3: 验证 log 文件**

Run: `ls -t ps-scripts/UECM-Bootstrap-*.log 2>/dev/null | head -1 | xargs cat`(或 fallback 到 `%TEMP%`)
Expected: 看到时间线 + `[CHECKONLY]` summary 行。

- [ ] **Step 4: 清理测试产物**

Run: `rm -f ps-scripts/UECM-Bootstrap-*.log`

- [ ] **Step 5: 提交**

```bash
git add ps-scripts/enable-winrm.ps1
git -c user.name='lanpc' -c user.email='lanbipu@gmail.com' commit -m 'feat(bootstrap): -CheckOnly 也写 log + 暴露 log_path/log_write_ok'
```

---

## Task 10: 端到端验证(CLI 兼容 + .173 真机)

**Files:** 无代码改动,验证为主。

- [ ] **Step 1: 验证 CLI JSON 仍可解析** — `winrm bootstrap` 经 `bootstrap-winrm-remote.ps1` 调用本脚本并解析 stdout JSON;新增字段是追加,不破坏现有 `ok`/`message`/`changed`/`state`。确认 `src-tauri/src/core/bootstrap.rs` 反序列化结构对未知字段宽容(serde 默认忽略未知字段)。

Run: `grep -n "deny_unknown_fields\|struct.*Bootstrap\|changed\|message" src-tauri/src/core/bootstrap.rs | head`
Expected: **没有** `#[serde(deny_unknown_fields)]` 标注在 bootstrap 结果结构上(若有则需在本 task 加字段,否则新 JSON 字段会让反序列化失败)。

- [ ] **Step 2: 真机全流程(.173,本地 Path A 等价)** — 把改后的 `enable-winrm.ps1` 拷到 `C:\Tools\UECM\UECM-WinRM-Bootstrap\UECM-Bootstrap-WinRM.ps1`,在 .173 本地管理员 PowerShell 跑(或本机 lanPC 测一次完整执行)。
  - 注:.173 防火墙三 profile 全禁用——验证脚本**跑完所有步骤不中断**,`firewall_profiles_disabled:true`,且 `FPS-SMB-In-TCP` 规则被设为 `Enabled=True`(`firewall_445` 达成)。

- [ ] **Step 3: 验证最终 verdict + log** — 检查 stdout JSON `ok` 与 `state`,以及同目录 log 文件每步状态。
Expected: 关键项全绿 → `ok:true`;log 含每步 `[OK]/[FAIL]/[SKIP]` + `[SUMMARY]`。

- [ ] **Step 4: 重新 health 核对(可选)** — 若 .173 已纳管,`uecm-cli health run` 看 `firewall_445`/`local_account_token_filter`/`long_paths_enabled`/`lanman_server` 是否转 healthy。

- [ ] **Step 5: 提交验证记录** — 把真机输出 JSON + log 摘要追加到 spec 或新建 `docs/research/bootstrap-resilience-acceptance-2026-05-21.md`。

```bash
git add docs/research/bootstrap-resilience-acceptance-2026-05-21.md
git -c user.name='lanpc' -c user.email='lanbipu@gmail.com' commit -m 'docs: bootstrap 健壮性重构真机验收记录'
```

---

## Self-Review

**Spec coverage:**
- 步骤表 + runner(spec 3.1)→ Task 5 + 7 + 8 ✓
- Critical 分类(spec 3.2)→ Task 5(含 firewall 修订)✓
- quickconfig 降级 + 防火墙始终配 + profile 记录(spec 3.3)→ Task 5(quickconfig non-critical / smb_firewall critical 无条件)+ Task 8(firewall_profiles_disabled)✓
- log 位置 fallback + 格式 + a/b/c(spec 3.4)→ Task 2/3/7 + Task 8(SUMMARY)✓
- 末尾判定 + JSON 新字段 + 修 changed bug(spec 3.5)→ Task 4/6/8 ✓
- CheckOnly 写 log(spec 3.5)→ Task 9 ✓
- 纯函数单测(spec 3.6)→ Task 2/3/4/5(+ 6/7 集成)✓

**Placeholder scan:** 每个 code step 均有完整 PS 代码 + 确切运行命令 + 预期输出,无 TBD/TODO。

**Type consistency:** 字段名贯穿一致 — `Get-UecmWinRmState` 产出 `long_paths_enabled`/`lanman_server_running`/`winmgmt_running`/`fps_smb_in_tcp_enabled`(Task 6),`Get-UecmCriticalVerdict` 消费同名(Task 4);`New-UecmLogState` 返回 `{Path,WriteOk}`,`Write-UecmLogLine`/`Invoke-UecmStep` 消费同名(Task 7);步骤 `@{Key;Critical;Label;Action}` 在 Task 5 定义、Task 7/8 消费一致。
