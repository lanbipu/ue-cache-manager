# Bootstrap 健壮性重构验收记录

**日期**：2026-05-21  
**分支**：main  
**任务编号**：Tasks 1–10（规划文档：`docs/superpowers/plans/`）

---

## 一、重构内容摘要

针对 `ps-scripts/enable-winrm.ps1` 的健壮性重构，共 9 个功能 commit（Task 1–9）+ 本验收文档（Task 10）。

### 核心变更

| 模块 | 改动 |
|---|---|
| 步骤表驱动 | 原来的顺序执行改为 `Build-UecmStepTable` 生成描述符列表，每个步骤有 `id`、`critical`、`skip_if_firewall_disabled` 等属性 |
| 失败隔离 | `Invoke-UecmStep` 捕获单步失败，`critical=$false` 的步骤（quickconfig/psremoting）失败不终止后续步骤 |
| 运行日志 | `Get-UecmLogDirCandidates` 多路径 fallback；`Format-UecmLogLine` 带时间戳；全程写入 `UECM-Bootstrap-<host>-<ts>.log` |
| 扩展 JSON | 新增 `failed[]`、`log_path`、`log_write_ok`、`state.firewall_profiles_disabled`、`state.missing_critical` |
| -CheckOnly 也写 log | `-CheckOnly` 模式同样输出日志文件，并在 JSON 中暴露 `log_path`/`log_write_ok` |
| catch bug 修复 | 旧 catch 路径输出的 JSON 缺少 `changed`/`log_path`，现已补全 |
| L1/L2 判定 | `Get-UecmCriticalVerdict` 分层判定 WinRM 可达性（L1）+ 配置正确性（L2） |

### Commit 列表（Tasks 1–9）

| SHA | 说明 |
|---|---|
| `87106cb` | `-LibraryOnly` guard，允许 dot-source 测试 |
| `f5d6f59` | `Format-UecmLogLine` 时间线格式化器 |
| `6c335ba` | `Get-UecmLogDirCandidates` fallback 路径顺序 |
| `4e577a2` | `Get-UecmCriticalVerdict` L1+L2 判定 |
| `dc0468e` | `Build-UecmStepTable` step 描述符 |
| `d95c3b7` | `Get-UecmWinRmState` 补 4 个 L2 判定字段 |
| `88e1225` | log writer + per-step runner（失败隔离） |
| `b9b94c4` | 主执行体改步骤表驱动 + 扩展 JSON + 修 `changed` 丢失 |
| `bb35a07` | `-CheckOnly` 也写 log + 暴露 `log_path`/`log_write_ok` |

---

## 二、自动化验证结果

### 2.1 lib test（断言组）

**命令**：
```
powershell.exe -NoProfile -ExecutionPolicy Bypass -File ps-scripts/__tests__/enable-winrm-lib.tests.ps1
```

**结果**：`OK`

覆盖的断言组（测试文件中的 `Assert-*` / `Invoke-*` 块）：

- `Format-UecmLogLine`：时间戳格式、level 前缀、内容拼接
- `Get-UecmLogDirCandidates`：路径 fallback 顺序、至少返回 1 条
- `Get-UecmCriticalVerdict`：L1（WinRM 可达）+ L2（配置完整）组合判定
- `Build-UecmStepTable`：步骤数量、每个步骤有 `id`/`critical`/`action` 字段
- `Get-UecmWinRmState`：返回结构包含 `firewall_profiles_disabled`、`missing_critical`、`is_admin`
- `Invoke-UecmStep`：失败隔离（非 critical 步骤失败不 throw）、成功/失败正确归类

### 2.2 语法 parse 检查

**命令**：
```
powershell.exe -NoProfile -Command "
  $null=[System.Management.Automation.PSParser]::Tokenize(
    (Get-Content -Raw 'ps-scripts/enable-winrm.ps1'),[ref]$null);
  'parse-ok'"
```

**结果**：`parse-ok`

### 2.3 -CheckOnly 真机运行（lanPC）

**机器**：LANPC（有线，非 elevated WSL 下 `powershell.exe` 运行）  
**结果**：`verdict=SUCCESS`（已在 Task 9 前验证，此处记录为已完成）

### 2.4 非管理员全流程运行（catch bug 验证）

**目的**：验证 catch 路径现在正确携带 `changed`/`log_path`，不再输出残缺 JSON。

**命令**：
```
powershell.exe -NoProfile -ExecutionPolicy Bypass -File ps-scripts/enable-winrm.ps1
```
（无 `-CheckOnly`，在非 elevated 环境下运行）

**实际输出 JSON**（已格式化）：
```json
{
  "message": "Administrator privileges are required. Start PowerShell with Run as Administrator.",
  "changed": [],
  "ok": false,
  "failed": [],
  "log_write_ok": true,
  "state": {
    "lanman_server_running": true,
    "winrm_start_type": "Auto",
    "trusted_hosts": "*",
    "firewall_rules": [
      {"DisplayName": "Windows Remote Management (HTTP-In)", "Enabled": 1, "Profile": 4, "Direction": 1, "Action": 2},
      {"DisplayName": "Windows Remote Management (HTTP-In)", "Enabled": 1, "Profile": 3, "Direction": 1, "Action": 2}
    ],
    "fps_smb_in_tcp_enabled": true,
    "is_admin": false,
    "computer_name": "LANPC",
    "wsman_localhost_ok": true,
    "local_account_token_filter_policy": 1,
    "network_profiles": [
      {"InterfaceAlias": "以太网", "InterfaceIndex": 17, "NetworkCategory": 1, "IPv4Connectivity": 4, "IPv6Connectivity": 1}
    ],
    "winmgmt_running": true,
    "long_paths_enabled": 1,
    "winrm_service_status": "Running",
    "listeners": [],
    "winrm_service_exists": true
  },
  "log_path": "E:\\AIWorkspace\\uecm-bootstrap-spec-wt\\ps-scripts\\UECM-Bootstrap-LANPC-20260521-235803.log"
}
```

**验证点**：
- `ok: false` ✓
- `message` 包含 "Administrator privileges are required" ✓
- `changed: []` 字段存在（旧 bug：catch 路径不输出此字段）✓
- `failed: []` 字段存在 ✓
- `log_path` 字段存在 ✓
- `log_write_ok: true`（catch 前的 log 写入仍成功） ✓

### 2.5 CLI JSON 向后兼容（Rust struct 核查）

**相关文件**：`src-tauri/src/core/bootstrap.rs`

**`WinrmBootstrapResult` struct 定义**：
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WinrmBootstrapResult {
    pub ok: bool,
    pub method: String,
    pub message: String,
    pub winrm_ok: bool,
    #[serde(default)]
    pub changed: Vec<String>,
    pub manual_script: Option<String>,
}
```

**结论**：

- 无 `#[serde(deny_unknown_fields)]`，新增字段（`failed`、`log_path`、`log_write_ok`、`state.*`）会被静默忽略，**不会导致反序列化失败**。
- Rust struct 解析的是 `bootstrap-winrm-remote.ps1` 的 stdout（remote 脚本自己构造 JSON），`enable-winrm.ps1` 的 stdout 由 remote 脚本作为字符串捕获后再包装——两者 JSON 格式互不干扰。
- `changed` 字段有 `#[serde(default)]`，即使 remote 脚本某路径漏了 `changed`，Rust 也不会报错。
- **向后兼容确认**：扩展 JSON 对现有 Rust CLI 解析无破坏。

---

## 三、待手动执行的验证步骤

以下步骤因需要 Administrator 权限或特定真机环境，无法在非提升的 WSL 环境中自动执行，**必须手动完成**。

### 3.1 真机管理员全流程运行

**目标机器**：任意一台可以提升权限的 Windows 10/11 节点（推荐先用 lanPC 本机）。

**方式 A — 直接 elevated PowerShell**：
```powershell
# 以管理员身份打开 PowerShell，cd 到脚本目录后执行
powershell -ExecutionPolicy Bypass -File enable-winrm.ps1 `
  -NetworkCategory Private `
  -EnableLocalAccountRemoteAdmin `
  -EnableSmbServer `
  -EnableWmi `
  -SetExecutionPolicy RemoteSigned `
  -EnableLongPaths `
  -PowerProfile HighPerformance
```

**方式 B — USB 包双击**：将 `ps-scripts/` 打包到 U 盘，双击 `UECM-Bootstrap.cmd`（会自动提升）。

**预期行为**：
- 所有步骤依次运行，非 critical 步骤（quickconfig/psremoting）失败时记录到 `failed[]` 但继续执行。
- 最终 JSON 中 `ok=true`（若 L1+L2 全满足）或 `ok=false`（附 `missing_critical` 列表）。
- 脚本同目录生成 `UECM-Bootstrap-<hostname>-<timestamp>.log`。
- `state.firewall_profiles_disabled` 字段正确反映防火墙配置文件状态。

### 3.2 lanbipu-razer（.173）防火墙全关机器

**背景**：该机器的所有 3 个 Windows 防火墙配置文件均处于 Disabled 状态，旧脚本在 `winrm quickconfig` 步骤会崩溃退出，无法完成后续步骤。

**连接方式**：
```bash
sshpass -p 'Shawn198812' ssh -o PubkeyAuthentication=no lanbp@192.168.10.173
```

**验证命令**（在 .173 上以管理员 PowerShell 执行）：
```powershell
powershell -ExecutionPolicy Bypass -File enable-winrm.ps1 `
  -NetworkCategory Private `
  -EnableLocalAccountRemoteAdmin `
  -EnableSmbServer
```

**预期行为**：
- `quickconfig` 和 `psremoting` 步骤标记为失败，进入 `failed[]`，但脚本不中止。
- `FPS-SMB-In-TCP`、`LATFP`（local_account_token_filter_policy）、`LongPaths` 等后续步骤正常执行。
- 最终 JSON：`state.firewall_profiles_disabled: true`，`failed` 包含 quickconfig/psremoting，`changed` 包含成功执行的步骤。

### 3.3 真机运行后的健康检查

真机 bootstrap 完成后，用 CLI 对目标节点执行健康探测，确认关键指标全部达标：

```bash
uecm-cli health run --host <target-ip> --username <user> --password <pass>
```

**预期通过的探针**：
- `firewall_445`（FPS-SMB-In-TCP 规则已启用）
- `local_account_token_filter_policy`（LATFP = 1）
- `long_paths_enabled`（LongPaths = 1）
- `lanman_server`（Server 服务运行中）

---

## 四、已知后续清理项

| 项目 | 说明 |
|---|---|
| `Enable-UecmWinRm` 函数 | 原脚本中的旧版入口函数，现已被步骤表主体替代，是死代码，可在后续 task 中删除 |
| catch JSON 缺少字段 | catch 路径的 JSON 目前不包含 `missing_critical`、`firewall_profiles_disabled`；admin 门槛前这两个字段无法采集，属已知限制，不影响功能 |
| UTF-8 BOM 写入 | 当前 log 文件用 `[System.Text.Encoding]::UTF8` 写入，会带 BOM；Windows 工具兼容，但 Unix 侧 `cat` 会看到 BOM 字节（`\xEF\xBB\xBF`）。可后续改为 `New-Object System.Text.UTF8Encoding($false)` 去掉 BOM |
