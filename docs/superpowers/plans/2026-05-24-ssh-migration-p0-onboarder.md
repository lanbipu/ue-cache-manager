# SSH 迁移 P0 — enable-ssh.ps1 完整 onboarder + 重 onboard lanPC 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: 用 superpowers:subagent-driven-development(推荐)或 superpowers:executing-plans 逐 task 执行。步骤用 `- [ ]` 复选框跟踪。
> 本计划对应 spec `docs/superpowers/specs/2026-05-23-ssh-transport-migration-design.md` 的 **P0**(前置,解锁后续所有真机验证)。
> ⚠️ **本 phase 主要是 PowerShell 脚本 + 运维,不是 Rust TDD**:验证靠 lanPC 真机跑(`cargo` 不覆盖 .ps1)。提交用 `git -c commit.gpgsign=false`,只新增 commit。

**Goal:** 把 `enable-winrm.ps1` 里的非 WinRM 节点 prep(建 `uecm-svc` 账号 + SMB/LongPaths/HighPerformance/ExecutionPolicy)折进 `enable-ssh.ps1`,使其成为完整独立 onboarder;并用纯 SSH 路径重 onboard lanPC,打通 Mac→lanPC `from_config()` SSH 认证,解锁后续 phase 的端到端验证。

**Architecture:** `enable-ssh.ps1` 当前只开 OpenSSH + 授权 pubkey + 铺脚本 + 装 PsExec。P0 把 6 个节点 prep 函数(逐字从 `enable-winrm.ps1` 复制,使 enable-ssh 自洽——P5 会删 enable-winrm)+ 对应 param 接进它的执行流;`UECM-Bootstrap.cmd` 把已有的 admin 凭据/prep 参数也传给 enable-ssh。WinRM bootstrap 此 phase 仍并存(P5 才删),prep 幂等跑两遍无害。

**Tech Stack:** Windows PowerShell 5.1(节点脚本)、cmd.exe(.cmd 双击入口)、系统 ssh/scp、lanPC 真节点。

---

## 文件结构

- **Modify** `ps-scripts/enable-ssh.ps1`:加 param 块(prep 开关)+ 在 library 区粘入 6 个 prep 函数 + 3 个辅助函数(逐字来自 `enable-winrm.ps1`)+ 在主流程末尾(现有 step 5/6 之后)按开关调用它们 + JSON envelope 增报 prep 结果。
- **Modify** `ps-scripts/UECM-Bootstrap.cmd`:给现有 `enable-ssh.ps1` 调用行追加 prep 参数(账号名/密码取自已有 `UECM_LOCAL_ADMIN*` 变量)。**必须 CRLF + 纯 ASCII**(per CLAUDE.md `bootstrap_cmd_crlf_trap`)。
- **验证(无源码改动)**:lanPC 真机重 onboard + 连通性断言。

---

## Task 1:enable-ssh.ps1 加 prep 参数 + 粘入 prep 函数

**Files:**
- Modify: `ps-scripts/enable-ssh.ps1`(param 块在文件头 `param(...)`;函数粘在 `try {` 主体之前)

- [ ] **Step 1:扩展 param 块**

把 `enable-ssh.ps1` 现有 param 块(当前是 `PublicKeyPath/UecmPublicKey/StagingSourceDir/CheckOnly`)替换为下面这版(新增 6 个 prep 开关,默认全 off,保持「只传 pubkey 就只开 SSH」的老行为不变):

```powershell
param(
    [string]$PublicKeyPath = '',
    [string]$UecmPublicKey = '',
    [string]$StagingSourceDir = '',
    [switch]$CheckOnly,
    # --- node prep (folded from enable-winrm.ps1; off by default) ---
    [switch]$CreateLocalAdmin,
    [string]$LocalAdminName = 'uecm-svc',
    [string]$LocalAdminPassword = '',
    [switch]$EnableSmbServer,
    [switch]$EnableLongPaths,
    [ValidateSet('HighPerformance', 'Balanced', 'Skip')]
    [string]$PowerProfile = 'Skip',
    [ValidateSet('RemoteSigned', 'Bypass', 'Skip')]
    [string]$SetExecutionPolicy = 'Skip'
)
```

- [ ] **Step 2:粘入辅助函数**

在 `enable-ssh.ps1` 的 `try {`(当前主流程开始)**之前**,粘入以下函数,**逐字复制**自 `ps-scripts/enable-winrm.ps1`(行号为该文件中的位置,复制时连函数体一起):
- `Test-UecmAdministrator`(enable-winrm.ps1:203-207)
- `Add-UecmChange`(enable-winrm.ps1:209-217)
- `Get-UecmRegistryDword`(enable-winrm.ps1:219-231)

> enable-ssh.ps1 现用 `$changes = New-Object System.Collections.ArrayList` + `Note($m)`。这些 prep 函数用 `[System.Collections.Generic.List[string]]$Changes` + `Add-UecmChange`。**桥接**:在调用 prep 函数处传一个 `Generic.List[string]`,跑完把它的元素 `Note` 进现有 `$changes`(见 Task 2 Step 2),避免改这些函数内部。

- [ ] **Step 3:粘入 6 个 prep 函数**

同样粘在 `try {` 之前,逐字复制自 `enable-winrm.ps1`:
- `Enable-UecmLocalAdmin`(414-465)— 建/重置 `uecm-svc` + 加 Administrators(用 SID S-1-5-32-544,本地化安全)
- `Enable-UecmSmbServer`(467-538)— LanmanServer Auto+Running + FPS-SMB-In-TCP
- `Enable-UecmWmi`(540-563)— Winmgmt Auto+Running
- `Enable-UecmLongPaths`(583-595)— LongPathsEnabled=1
- `Set-UecmLocalExecutionPolicy`(565-581)— LocalMachine 执行策略
- `Set-UecmPowerPlan`(597-656)— 电源计划(含 duplicatescheme 幂等)

> 这些函数读 `$CreateLocalAdmin`/`$LocalAdminName`/`$LocalAdminPassword`/`$EnableSmbServer`/`$EnableLongPaths`/`$SetExecutionPolicy`/`$PowerProfile`——Step 1 的 param 已提供同名变量,逐字复制即可直接工作。`Enable-UecmWmi` 读 `$EnableWmi`(enable-ssh 没这个 param)→ **改一处**:把 `Enable-UecmWmi` 函数体第一行 `if (-not $EnableWmi) { return }` 改为 `if (-not $EnableSmbServer) { return }`(WMI 与 SMB 同属"远程管理可达"前置,用 SmbServer 开关一起带;或在 Step 1 param 另加 `[switch]$EnableWmi` 并由 .cmd 传——二选一,推荐复用 EnableSmbServer 减少 .cmd 改动)。**决策点:Task 1 实现时定,在本步注释写明选了哪个。**

- [ ] **Step 4:语法自检(mac 本地,无需 PS 运行时)**

Run: `grep -c 'function Enable-UecmLocalAdmin\|function Enable-UecmSmbServer\|function Enable-UecmLongPaths\|function Set-UecmPowerPlan\|function Set-UecmLocalExecutionPolicy\|function Enable-UecmWmi' ps-scripts/enable-ssh.ps1`
Expected: `6`(6 个 prep 函数都粘进来了)。
Run: `grep -c 'CreateLocalAdmin\|EnableSmbServer\|EnableLongPaths' ps-scripts/enable-ssh.ps1`
Expected: `>= 3`。

- [ ] **Step 5:Commit**

```bash
git add ps-scripts/enable-ssh.ps1
git -c commit.gpgsign=false commit -m "feat(ssh): add node-prep params + functions to enable-ssh.ps1 (P0 part 1)

Fold enable-winrm.ps1's non-WinRM node prep (uecm-svc account, SMB, LongPaths,
power, exec policy) into enable-ssh.ps1 so it becomes a complete standalone
onboarder before WinRM bootstrap is retired in P5. Functions copied verbatim;
wired into the flow in part 2.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 2:把 prep 步骤接进 enable-ssh.ps1 主流程

**Files:**
- Modify: `ps-scripts/enable-ssh.ps1`(主 `try {}` 流程,现有 step 6「install PsExec64」之后)

- [ ] **Step 1:在 PsExec 安装步骤后插入 prep 调用**

在 enable-ssh.ps1 现有「# 6. install PsExec64 ...」块**之后**、readiness 计算(`$sshd = Get-Service sshd ...`)**之前**,插入:

```powershell
    # 7. node prep (folded from enable-winrm.ps1). Idempotent; off unless the
    #    matching switch is passed. Run as the SSH user (already elevated when
    #    invoked from UECM-Bootstrap.cmd). Bridges the prep functions'
    #    Generic.List[string] $Changes into this script's $changes ArrayList.
    if (-not $CheckOnly) {
        $prep = New-Object 'System.Collections.Generic.List[string]'
        try { Enable-UecmLocalAdmin -Changes $prep }            catch { Note "WARNING: local admin prep: $($_.Exception.Message)" }
        try { Enable-UecmSmbServer  -Changes $prep }            catch { Note "WARNING: smb prep: $($_.Exception.Message)" }
        try { Enable-UecmWmi        -Changes $prep }            catch { Note "WARNING: wmi prep: $($_.Exception.Message)" }
        try { Enable-UecmLongPaths  -Changes $prep }            catch { Note "WARNING: longpaths prep: $($_.Exception.Message)" }
        try { Set-UecmLocalExecutionPolicy -Changes $prep }     catch { Note "WARNING: execpolicy prep: $($_.Exception.Message)" }
        try { Set-UecmPowerPlan     -Changes $prep }            catch { Note "WARNING: power prep: $($_.Exception.Message)" }
        foreach ($m in $prep) { Note $m }
    }
```

> 用 per-step try/catch + `Note WARNING`,而非让 prep 异常炸掉整个 onboarding——SSH 本身(前 6 步)已成功,prep 失败应降级记录不阻断(与 enable-winrm 的 non-critical 语义一致;真正的 readiness 仍由下面 sshd/key 判定)。

- [ ] **Step 2:readiness 增报 uecm-svc 存在性(不改 $ok 判定)**

把 enable-ssh.ps1 末尾 readiness 块里的 `$keyAuthorized = ...` 之后、`$ok = ...` 之前,加一行只读探测(供 JSON 输出诊断,不进 `$ok`,因为 prep 是 opt-in):

```powershell
    $svcAccountExists = $false
    if ($CreateLocalAdmin) {
        try { $svcAccountExists = [bool](Get-LocalUser -Name $LocalAdminName -ErrorAction SilentlyContinue) } catch {}
    }
```

并把最终 JSON envelope(`@{ ok = $ok; changes = $changes; message = $msg }`)改为带上诊断:

```powershell
    @{ ok = $ok; changes = $changes; message = $msg; svc_account_exists = $svcAccountExists } | ConvertTo-Json -Depth 6 -Compress
```

- [ ] **Step 3:语法自检**

Run: `grep -n 'Enable-UecmLocalAdmin -Changes\|svc_account_exists' ps-scripts/enable-ssh.ps1`
Expected: 两处都命中(prep 接线 + 诊断字段)。

- [ ] **Step 4:Commit**

```bash
git add ps-scripts/enable-ssh.ps1
git -c commit.gpgsign=false commit -m "feat(ssh): wire node-prep into enable-ssh.ps1 flow (P0 part 2)

Prep runs after SSH setup, per-step try/catch (non-fatal), bridges into the
existing \$changes log; JSON envelope reports svc_account_exists for diagnosis.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 3:UECM-Bootstrap.cmd 把 prep 参数传给 enable-ssh.ps1

**Files:**
- Modify: `ps-scripts/UECM-Bootstrap.cmd`(SSH onboarding 调用行,当前约在「SSH transport onboarding」段)

- [ ] **Step 1:读现状,定位 enable-ssh 调用行**

Run: `grep -n 'SSH_PS1\|enable-ssh\|UECM_PUB' ps-scripts/UECM-Bootstrap.cmd`
当前该行:`...powershell.exe ... -File "%SSH_PS1%" -PublicKeyPath "%UECM_PUB%" -StagingSourceDir "%SCRIPT_DIR%"`

- [ ] **Step 2:追加 prep 参数(复用已有的 ADMIN_ARGS 变量值)**

把 enable-ssh 调用行改为(在现有参数后追加 prep 开关;账号名/密码用 .cmd 已定义的 `%UECM_LOCAL_ADMIN%`/`%UECM_LOCAL_ADMIN_PASSWORD%`):

```bat
if exist "%SSH_PS1%" if exist "%UECM_PUB%" powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%SSH_PS1%" -PublicKeyPath "%UECM_PUB%" -StagingSourceDir "%SCRIPT_DIR%" -EnableSmbServer -EnableLongPaths -PowerProfile HighPerformance -SetExecutionPolicy RemoteSigned %SSH_ADMIN_ARGS%
```

并在 `set ADMIN_ARGS=...`(WinRM 用)附近,新增一行构造 enable-ssh 用的账号参数(若密码非空才建账号):

```bat
set "SSH_ADMIN_ARGS="
if not "%UECM_LOCAL_ADMIN_PASSWORD%"=="" set SSH_ADMIN_ARGS=-CreateLocalAdmin -LocalAdminName "%UECM_LOCAL_ADMIN%" -LocalAdminPassword "%UECM_LOCAL_ADMIN_PASSWORD%"
```

> 必须用纯 ASCII;中文注释只能进 README.txt。`%SSH_ADMIN_ARGS%` 不加引号(它本身含多个 token)。

- [ ] **Step 3:验证 CRLF + 纯 ASCII(CLAUDE.md 硬要求)**

Run: `file ps-scripts/UECM-Bootstrap.cmd`
Expected: 含 `CRLF line terminators`。
Run: `LC_ALL=C grep -nP '[^\x00-\x7F]' ps-scripts/UECM-Bootstrap.cmd; echo "exit=$?"`
Expected: 无输出 + `exit=1`(无非 ASCII 字节)。
若 CRLF 丢失:`perl -i -pe 's/\n/\r\n/ unless /\r\n/' ps-scripts/UECM-Bootstrap.cmd` 后重验。

- [ ] **Step 4:Commit**

```bash
git add ps-scripts/UECM-Bootstrap.cmd
git -c commit.gpgsign=false commit -m "feat(ssh): UECM-Bootstrap.cmd passes node-prep args to enable-ssh.ps1 (P0 part 3)

The double-click bootstrap now drives full node prep through the SSH path too
(account + SMB + LongPaths + power + exec policy), so retiring the WinRM
bootstrap in P5 loses no node setup. CRLF + ASCII verified.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 4:lanPC 真机验证 + 重 onboard(解锁后续 E2E)

> 这是 P0 的核心交付。无源码改动,纯运维 + 断言。用 `ssh lanpc`(lanpc/1Password key)推文件 + 跑脚本(Mac→lanPC 的 uecm-svc SSH 当前不通,正是本 task 要修的)。

- [ ] **Step 1:取 Mac UECM 公钥**

Run: `cat "$(.worktrees/ssh-transport-migration/src-tauri/target/release/uecm system db-path 2>/dev/null | xargs dirname 2>/dev/null)/uecm_ed25519.pub" 2>/dev/null || find ~ -name 'uecm_ed25519.pub' 2>/dev/null | head -1 | xargs cat`
> 目的:拿到当前 Mac keystore 的 `uecm_ed25519.pub`(memory `mac_keystore_key_mismatch` 记其指纹 ...OrVf254)。记下完整公钥串供下一步。若路径不确定,读 `core::keystore`/`startup::resolve_config_dir` 确认 keystore 位置。

- [ ] **Step 2:把当前 enable-ssh.ps1 + Mac uecm.pub 推到 lanPC 测试目录**

```bash
ssh lanpc 'powershell -NoProfile -Command "New-Item -ItemType Directory -Force C:\Users\lanpc\uecm-p0 | Out-Null"'
scp ps-scripts/enable-ssh.ps1 lanpc:uecm-p0/enable-ssh.ps1
# 把 Step 1 拿到的公钥写成 uecm.pub 推过去：
printf '%s\n' '<MAC_UECM_PUB_FROM_STEP_1>' > /tmp/uecm.pub && scp /tmp/uecm.pub lanpc:uecm-p0/uecm.pub && rm /tmp/uecm.pub
```

- [ ] **Step 3:在 lanPC 跑完整 onboarding(SSH session 已 elevated)**

```bash
ssh lanpc 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\Users\lanpc\uecm-p0\enable-ssh.ps1 -PublicKeyPath C:\Users\lanpc\uecm-p0\uecm.pub -StagingSourceDir C:\Users\lanpc\uecm-p0 -CreateLocalAdmin -LocalAdminName uecm-svc -LocalAdminPassword UecmRender@2026 -EnableSmbServer -EnableLongPaths -PowerProfile HighPerformance -SetExecutionPolicy RemoteSigned'
```
Expected JSON: `"ok":true` + `"svc_account_exists":true` + changes 含 "created/reset ... uecm-svc"、"added 'uecm-svc' to ... Administrators"、"LongPathsEnabled"、"LanmanServer"、"authorized UECM key"。

- [ ] **Step 4:断言节点 prep 真生效**

```bash
ssh lanpc 'powershell -NoProfile -Command "Write-Output (\"svc=\" + [bool](Get-LocalUser uecm-svc -EA SilentlyContinue)); Write-Output (\"inAdmins=\" + [bool](@(Get-LocalGroupMember -SID S-1-5-32-544) | ? { $_.Name -match \"uecm-svc\" })); Write-Output (\"longpaths=\" + (Get-ItemProperty HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem LongPathsEnabled -EA SilentlyContinue).LongPathsEnabled); Write-Output (\"lanman=\" + (Get-Service LanmanServer).Status)"'
```
Expected: `svc=True` / `inAdmins=True` / `longpaths=1` / `lanman=Running`。

- [ ] **Step 5:验证 Mac→lanPC 以 uecm-svc + Mac key 登录(核心解锁点)**

```bash
KEY="$(find ~ -name 'uecm_ed25519' -not -name '*.pub' 2>/dev/null | head -1)"
KH="$(dirname "$KEY")/known_hosts"
ssh -i "$KEY" -o IdentitiesOnly=yes -o UserKnownHostsFile="$KH" -o StrictHostKeyChecking=accept-new -o BatchMode=yes uecm-svc@192.168.10.20 'powershell -NoProfile -Command "Write-Output ok-uecm-svc"'
```
Expected: `ok-uecm-svc`(Mac UECM key 以 uecm-svc 登录成功 → `from_config()` 路径打通)。
> 若失败:检查 administrators_authorized_keys 是否含 Mac uecm.pub(`ssh lanpc 'powershell -NoProfile -Command "Get-Content C:\ProgramData\ssh\administrators_authorized_keys"'`)+ ACL(enable-ssh.ps1 已 enforce SYSTEM+Administrators)。

- [ ] **Step 6:更新 memory(解除 mac_keystore_key_mismatch 的阻塞)**

编辑 `~/.claude/projects/.../memory/mac_keystore_key_mismatch.md`:记录 lanPC 已用 P0 enable-ssh 重 onboard、Mac key 现已授权、uecm-svc 已建,Mac→lanPC from_config() 现已通(注明日期)。同步更新 MEMORY.md 那行的 hook。

- [ ] **Step 7:清理 lanPC 测试目录(prep 出的账号/配置保留——那是真 onboarding)**

```bash
ssh lanpc 'powershell -NoProfile -Command "Remove-Item C:\Users\lanpc\uecm-p0 -Recurse -Force -EA SilentlyContinue; Write-Output cleaned"'
```
> 只删推送的临时文件;uecm-svc 账号 / SMB / LongPaths / 授权 key 是本次 onboarding 的成果,**保留**。

---

## Task 5:P0 收口 + codex review

- [ ] **Step 1:cargo 基线未受影响(P0 不改 Rust,确认没误伤)**

Run: `cd src-tauri && cargo check --all-targets 2>&1 | tail -3`
Expected: 编译通过(P0 只改 .ps1/.cmd)。

- [ ] **Step 2:codex review 本 phase diff**

Run: `node "$(ls -t ~/.claude/plugins/cache/openai-codex/codex/*/scripts/codex-companion.mjs 2>/dev/null | head -1)" review --base 0e39fb6 --wait`
原样贴输出;报问题先修完再继续。

- [ ] **Step 3:P0 完成,回到 spec 选 P1**

P0 解锁了真机 E2E。下一步按 spec §7 顺序进 P1(探测迁移),P1 详细计划在执行前就近编写(基于 P0 后真实代码)。

---

## Self-Review(对照 spec P0)

- **spec 覆盖**:P0 = ① enable-ssh 完整 onboarder(账号 + SMB/LongPaths/power/execpolicy)→ Task 1-3;② 重 onboard lanPC 解锁 E2E → Task 4。✓
- **无占位**:复制的函数指向 enable-winrm.ps1 确切行号(既有具体代码,非 TBD);新增的 param/接线/.cmd 改动给出完整代码;`Enable-UecmWmi` 的 `$EnableWmi`→`$EnableSmbServer` 改动明确标为实现时定的决策点。
- **类型一致**:prep 函数用 `Generic.List[string]`,enable-ssh 用 `ArrayList` + `Note`——Task 2 Step 1 显式桥接(`$prep` List → `foreach Note`),不混用。
- **风险**:`Enable-UecmLocalAdmin` 依赖 `Microsoft.PowerShell.LocalAccounts` 模块(域控/32-bit PS 上可能缺)——home lab 渲染节点是普通 Win10/11 x64,有该模块;真机 Task 4 会暴露任何缺失。
