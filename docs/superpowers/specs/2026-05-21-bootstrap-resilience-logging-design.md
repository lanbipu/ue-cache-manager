# enable-winrm.ps1 步骤表驱动重构 + 运行 log

- **日期**: 2026-05-21
- **状态**: 设计待实现
- **范围对象**: `ps-scripts/enable-winrm.ps1`（Path A 本地 bootstrap 脚本本体），及其在 `UECM-Bootstrap.cmd` 入口下的运行体验
- **关联**: Path A USB bootstrap 包（`package-winrm-bootstrap.ps1`）、就绪标准 `src-tauri/src/core/probe_keys.rs`

---

## 1. 背景与问题

`enable-winrm.ps1` 的使命:在目标机本地跑一次,把这台裸机配置成"可被 UECM 远程纳管"的状态。实测一台 Razer 笔记本（192.168.10.173）暴露了三个让脚本无法达成使命的问题:

1. **一步报错整个脚本崩（核心）**。脚本全局 `$ErrorActionPreference='Stop'`,执行段是顺序调用 10 个 `Enable-Uecm*` 函数。该机的 Windows 防火墙三个 profile 全部被禁用（第三方安全软件接管的典型状态,服务 `MpsSvc`/`BFE` 仍在 Running),导致 `winrm quickconfig` 报 `Unable to check the status of the firewall` 并抛出。脚本当场进 `catch` 退出,**排在后面的 LATFP / LongPaths / Winmgmt 等步骤全部没执行**。结果:WinRM 服务起来了(崩之前的 `Start-Service` 成功),但 `LocalAccountTokenFilterPolicy` 一直没设上,机器实际不可纳管。

2. **没有运行记录**。失败时脚本只在 stdout 吐一份 JSON,而且 `catch` 块里把已完成动作的 `changed` 列表清空了。操作员看不到"跑到了哪一步、哪步失败、为什么停",只能靠猜。

3. **环境不自适应**。防火墙整个关闭时,入站本就不被拦截(端口实际可达),`quickconfig` / `Enable-NetFirewallRule` 这些动作要么无意义要么报错,但脚本没有识别这种环境、把它们降级处理的能力。

## 2. 目标与范围

**目标**:脚本跑完后,目标机进入"可被 UECM 远程操作"的状态,并在脚本同目录留下一份能看懂的运行 log;即便在 .173 那种防火墙被禁用的环境也能跑完、并如实报告每一步结果。

**就绪边界 = L1 + L2**（对照 `probe_keys.rs`）:

- **L1 端口**: `tcp_5985` / `tcp_445` / `tcp_135` 可达。
- **L2 引导配置**: `firewall_445` / `local_account_token_filter` / `long_paths_enabled` / `lanman_server`。

脚本保证这两层。**非目标(范围外)**:L3 业务层（SMB 缓存共享创建、NTFS 权限、凭据存储、UE 缓存环境变量、RenderStream 服务、工程一致性）——这些因机/因工程而异,由 UECM 各功能命令按需部署,不塞进通用 bootstrap。

## 3. 设计

### 3.1 脚本骨架:步骤表 + runner

把"顺序调 10 个函数"改成**步骤表**驱动。每步是一个描述符:

```powershell
@{ Key = 'winrm_service'; Critical = $true;  Label = '启动 WinRM 服务并设 Automatic'; Action = { ... } }
@{ Key = 'latfp';         Critical = $true;  Label = '设 LocalAccountTokenFilterPolicy=1'; Action = { ... } }
@{ Key = 'quickconfig';   Critical = $false; Label = 'winrm quickconfig (便捷封装)';      Action = { ... } }
# ...
```

一个 **runner** 循环执行每步:

```
foreach ($step in $steps) {
    try   { $detail = & $step.Action;  记 result = ok,   写 log [OK] }
    catch { 记 result = fail,$_.Message; 写 log [FAIL] (标注 Critical 与否) }
    # 永不 rethrow,永不中断
}
```

每步产出 `ok` / `fail` / `skipped` + 详情,累积进结果列表。`Critical` 标记决定该步是否参与末尾整体判定。新增配置项 = 往表里加一行。

### 3.2 步骤的 Critical 分类

**Critical（参与末尾 ok 判定,对应 L1+L2 关键项）**:

| Key | 配置动作 | 对应就绪项 |
|---|---|---|
| `winrm_service` | WinRM 设 Automatic + 启动 | tcp_5985 |
| `winrm_listener` | Enable-PSRemoting / HTTP listener (5985) | tcp_5985 |
| `latfp` | LocalAccountTokenFilterPolicy=1 | local_account_token_filter |
| `lanman_server` | LanmanServer Auto + Running | tcp_445 / lanman_server |
| `long_paths` | LongPathsEnabled=1 | long_paths_enabled |
| `winmgmt` | Winmgmt Auto + Running | winmgmt (machine refresh 依赖) |
| `smb_firewall` | Enable-NetFirewallRule `FPS-SMB-In-TCP`（设规则 `Enabled=True`，**独立于 profile 开关**） | firewall_445 |
| `winrm_firewall` | Enable-NetFirewallRule WinRM HTTP-In（规则就位，未来启用防火墙时 5985 仍通） | tcp_5985（防火墙启用时） |

**Optional（失败仅警告,不影响整体 ok）**:`network_profile`(Public→Private)、`quickconfig`、`execution_policy`、`power_plan`、`trusted_hosts`、`allowed_remote_address`。

> 防火墙规则步骤为何是 Critical 而非"防火墙关就跳过":见 3.3 + 3.5（核对 `health-probes.ps1` 后修订）。

### 3.3 容错 + 环境自适应

- **`quickconfig` 降级**: `Critical=$false`。WinRM 服务、listener、防火墙规则是各自独立的步骤,`quickconfig` 只是便捷封装,它正是 .173 上抛错的那一步,失败记警告即可。
- **防火墙规则始终配置（修订）**: 经核对 `health-probes.ps1:43-59`,`firewall_445` 探测查的是 **`FPS-SMB-In-TCP` 规则的 `Enabled` 属性**(规则不启用 → `status=critical`),与防火墙 profile 是否启用、端口是否可达**无关**。因此脚本**始终执行 `Enable-NetFirewallRule`**(SMB + WinRM 规则),让规则 `Enabled=True` 就位——`Enable-NetFirewallRule` 设的是规则启用标志,**独立于 profile 开关**,即便三个 profile 全禁用(.173 的情况)也能成功设置,且未来防火墙被重新启用时端口仍通。这消除了"防火墙现在关着→脚本跳过规则→报 ok→哪天防火墙一开 445 就被拦"的隐患。
- **防火墙全禁用 = 记录,不跳过**: 跑完后查三个 `NetFirewallProfile` 的 `Enabled`,若全 `False`,在 log + JSON(`firewall_profiles_disabled:true`)如实记录这一环境状态(规则已就位、当前靠防火墙关闭放行)。这是一个**明确且可接受**的状态,不影响 `firewall_445` 达成判定。

### 3.4 log 设计（对应需求 a/b/c）

- **位置（修订,加 fallback）**: 依次尝试 ① 脚本同目录(`$PSScriptRoot`)→ ② `%ProgramData%\UECM\Logs` → ③ `%TEMP%`,**第一个可写的**即为最终落点。文件名 `UECM-Bootstrap-<COMPUTERNAME>-<yyyyMMdd-HHmmss>.log`。理由:Path A 常从可能只读的 U 盘运行,无 fallback 会出现"脚本跑了却没留下记录",正好保留了本次要消除的可观测性盲区。
- **编码**: UTF-8 with BOM(避免中文在记事本/控制台乱码,与 readme 打包同理)。
- **格式**: 人读时间线,每步一行:
  ```
  [2026-05-21 21:40:03] [OK]    winrm_service  | WinRM 设为 Automatic 并已启动
  [2026-05-21 21:40:05] [FAIL]  quickconfig    | Unable to check the status of the firewall  (non-critical, 继续)
  [2026-05-21 21:40:06] [SKIP]  smb_firewall   | 防火墙三 profile 全禁用,入站不拦,跳过
  [2026-05-21 21:40:07] [OK]    latfp          | LocalAccountTokenFilterPolicy=1
  [2026-05-21 21:40:12] [SUMMARY] verdict=SUCCESS  critical=6/6  optional=2/4
  ```
  - **(a) 运行状况**: 每步一行带时间戳 + 状态 + 详情。
  - **(b) 成功与否**: 末尾 `[SUMMARY]` 行给整体 verdict + 关键项达成比 + 可选项达成比。
  - **(c) 失败记录**: `[FAIL]` 行带完整异常 message,并标注 critical / non-critical。
- log 写入本身要容错:任何一步写 log 失败都不能影响配置主流程。

### 3.5 末尾判定 / stdout 兼容 / CheckOnly

- **末尾判定（修订）**: 跑完所有步骤后,扩展现有 `Get-UecmWinRmState`(补 `long_paths_enabled` / `lanman_server` / `winmgmt` / `fps_smb_in_tcp_enabled` 四个状态字段),检查 Critical 关键项是否全达成:WinRM Running + `wsman_localhost_ok` + `LATFP=1` + LongPaths=1 + LanmanServer Running + Winmgmt Running + **`FPS-SMB-In-TCP` 规则 `Enabled=True`(对齐 `firewall_445`)**。
  - 这条直接对齐 `probe_keys.rs` 的 L2 键 `firewall_445`,消除"bootstrap 报 ok 但 health 报红"的契约漂移。
  - `tcp_5985` 是 operator 侧远程 TCP 探测,脚本本地不强测;本地以"WinRM Running + listener 存在 + WinRM 规则已启用"作为等价保证。
  - 全绿 → `verdict=SUCCESS` (`ok:true`);否则 `ok:false`,log + JSON 已写明哪步没成。
- **stdout JSON 兼容 + 新字段**: 继续输出现有结构(`ok` / `message` / `changed` / `state`),并**新增** `failed`(失败步骤列表)、`log_path`(最终解析到的 log 路径)、`log_write_ok`(bool)、`firewall_profiles_disabled`(bool)。`winrm bootstrap` 等 CLI 仍能解析,不破坏兼容。**顺带修掉 `catch` 块清空 `changed` 的旧 bug**——失败时也带上已完成步骤。
  - 三个 log 目标全写不出 → `log_write_ok:false`,并在 stdout summary + `[SUMMARY]` 行以 **non-critical 失败**可见(不影响配置 verdict,但明确告知本次没有持久记录)。
- **`-CheckOnly`**: 保持只读检查语义,顺带也写一份 log(记录检查结果,不改系统)。

### 3.6 测试

- 末尾判定逻辑、log 行格式化抽成纯函数,加 PowerShell 单测(放 `ps-scripts/__tests__/`,与现有 `read-ini-file.tests.ps1` 同风格)。
- 步骤表结构测试:每步有 `Key`/`Critical`/`Label`/`Action`,Key 唯一。
- 系统副作用步骤靠 `-CheckOnly` 在真机核对。

## 4. 关键决策记录（来自需求澄清）

1. **目标范围** = L1 + L2(可被纳管),不含 L3 业务部署。
2. **容错策略** = 逐步隔离 + 末尾判定(每步独立 try/catch,失败不中断,末尾按关键项判定整体成败)。
3. **log 落点** = 脚本同目录优先,写不出则 fallback `%ProgramData%\UECM\Logs` → `%TEMP%`(原定"只写同目录无 fallback",经 codex adversarial review 修订:只读 U 盘会导致无运行记录,与本次目标冲突)。
4. **log 格式** = 人读时间线为主,保留现有 stdout JSON。
5. **改造方案** = 步骤表驱动重构(非最小补丁,非模块化全重写)。

## 5. 范围外 / 后续项（记录,不在本 spec 实现）

实测 .173 还暴露了两个与 Path B（远程 PsExec bootstrap）相关、但**独立于本次 Path A 脚本改造**的问题,单列待后续处理:

- **`preflight-path-b.ps1` 的 stderr 误判 bug**: 该脚本 `$ErrorActionPreference='Stop'` + `& PsExec 2>&1`,导致 PsExec 写到 stderr 的正常进度("Connecting to...")被当成 terminating error 抛出,探测在装服务前就崩,且 verdict 把它硬编码归因为 "UAC remote token filter",产生误报。
- **PsExec 本地 logon 失败(exit 46)**: PsExec 让 PSEXESVC 用本地账户在 .173 启动进程时报"用户名或密码不正确"(去掉 `-h` 仍复现),服务能装上但启动进程的本地 logon 失败。影响 `winrm bootstrap`(Path B)对该机可用性;对已通过 Path A 开好 WinRM 的机器不构成阻塞。
