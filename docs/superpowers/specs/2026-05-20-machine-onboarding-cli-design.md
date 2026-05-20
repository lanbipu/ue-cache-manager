# 机器纳管编排命令 — 设计规格

- 日期：2026-05-20
- 状态：已与用户确认（待 spec 复审）
- 范围：**功能 + CLI 实现，不含 UI**（UI 留待后续，按项目 Figma-first 规则单独走）

## 1. 背景与动机

目标是把"4 步机器纳管流程"做成 CLI 可端到端驱动：

1. **发现扫描** — 扫局域网发现机器。
2. **选择加入** — 用户挑选要纳入 UECM 的机器，入库。
3. **深度扫描** — 对已入库机器拉取并更新完整状态。
4. **开通授权** — 为每台机器开通远程管理权限。

核对现有 CLI 后，4 步的后端能力基本都已存在，但**步骤 3、4 各自是 3 条原子命令拼出来的，没有单一入口**：

| 步骤 | 现有命令 | 缺口 |
|---|---|---|
| 1 发现扫描 | `machine scan <CIDR>` | 无（纯报告、不入库） |
| 2 选择加入 | `machine add --ip X`（逐台） | 无（逐台即可） |
| 3 深度扫描 | `machine refresh <id>` + `ini scan` + `health run` | **3 条命令，无单一"深扫"入口** |
| 4 开通授权 | `winrm preflight` + `winrm bootstrap` + `cred save` | **3 条命令，无单一"授权"入口** |

本设计补这两个缺口：新增两条**编排命令**，把现有原子能力各串成一个动作。

## 2. 锁定的决策

- 采用方案 **B**：新增两条编排命令，**不写新后端逻辑**，只调用现有域函数 + 聚合输出。
- 流程顺序保持用户原序（发现→加入→深扫→授权），**不为依赖关系调序**。
- 深度扫描 = **全套**（refresh + INI scan + health）。
- 深扫遇到目标机 **WinRM 未开通** → 标记该机、跳过其后续步骤、提示先授权，**继续扫整批不中断**。
- 开通授权 = `winrm preflight`（Path B 可行性）→ `winrm bootstrap`（远程 PsExec 开通）；UECM 是 WinRM pull 模式、**无常驻 agent**，"安装"即指此开通配置。Path B 不可达时回落 USB 脚本（人工到机器跟前跑）。
- 凭据走**共享 alias**（DPAPI），一次输入、整批复用。
- authorize 带 `--save-as`：首次可现输 `--user/--pass-stdin` 并存成 alias，省掉单独 `cred save`。
- 命令名定为 `machine deep-scan` / `machine authorize`。
- 二者均为 **Windows-only**（同现有 WinRM/PowerShell/DPAPI 约束），非 Windows 按现有 stub 报错。

## 3. 命令 1 ·『深度扫描』`machine deep-scan`

### Synopsis
```
uecm-cli machine deep-scan --machine-ids 3,4,5 --cred-alias prod
uecm-cli machine deep-scan --all --cred-alias prod
```

### 参数
- `--machine-ids <M1,M2,...>` / `--all`：目标选择，二选一，语义与 `health run` 一致。
- `CredentialArgs`（`--cred-alias` / `--user` + `--pass-stdin`）：复用现有共享参数集。

### 行为
对每台目标机器**依次**执行，结果全部持久化进现有表（`machine_ue_installs` / `machine_gpus` / `ini_findings` / `health_check_runs`）：

1. `refresh`：WinRM 探活 + UE 版本（注册表）+ GPU（WMI）。
2. `ini scan`：R001–R025 base + 机器已注册 zen 端点时追加 zen 规则。
3. `health run`：24 项矩阵（L1 端口 + L2 bootstrap + L3 业务 + derived + zen）。

### 错误处理（按用户决策）
- 第 1 步 `refresh` 的 WinRM 探活失败 → 该机标 `winrm_unreachable`，**跳过其 ini/health**，输出提示"先运行 `machine authorize`"，**继续处理下一台**。
- 单台其他步骤出错记录到该机摘要，不中断整批。

### 输出
每台一段聚合摘要：UE 安装数/版本、GPU、INI findings 计数（按 severity）、health 的 healthy/warning/critical/skipped 计数 + `scan_run_id`。批末给整批汇总。

## 4. 命令 2 ·『开通授权』`machine authorize`

### Synopsis
```
# 复用已存 alias
uecm-cli machine authorize --machine-ids 3,4 --cred-alias prod
# 首次现输现存
uecm-cli machine authorize --machine-ids 3,4 --user Administrator --pass-stdin --save-as prod
```

### 参数
- `--machine-ids <M1,M2,...>` / `--all`：目标选择。
- `CredentialArgs`：本地管理员凭据（PsExec 登录用，也是日后 WinRM 复用的同一凭据）。
- `--save-as <alias>`：把本次 `--user/--pass-stdin` 存成 DPAPI alias，供后续 `deep-scan` 复用。

### 行为
对每台目标机器：
1. `winrm preflight`（Path B 可行性：TCP 135/445 + ADMIN$ 挂载 + 写探针）。
2. `verdict=viable` → `winrm bootstrap` **全量 provision**（用户确认）：除 WinRM core（PSRemoting + WinRM 服务 + WRM 防火墙 + Network=Private + TrustedHosts + `LocalAccountTokenFilterPolicy=1`）外，再开 **SMB Server + 445 / WMI / LongPaths / ExecutionPolicy=RemoteSigned / PowerProfile=HighPerformance**，一次把 render node 配到 DDC-ready。

   **需要的后端扩展**（不再是纯编排）：
   - `core::bootstrap::enable_winrm_with_psexec` 增 `full_provision: bool` 参数；为 true 时往 PsExec args 追加 `-EnableSmbServer -EnableWmi -EnableLongPaths -SetExecutionPolicy RemoteSigned -PowerProfile HighPerformance`。
   - `ps-scripts/bootstrap-winrm-remote.ps1` 增对应 `[switch]`/`[string]` 参数并转发给远程 `enable-winrm.ps1`（脚本本身已支持这些开关）。
   - 现有 3 个调用方（`core/mod.rs` 示例、`domain_winrm.rs` 的 `winrm bootstrap` CLI、`commands/bootstrap.rs` UI）同步传 `full_provision=false`，行为不变。
   - `machine authorize` 以 `enable_local_account_remote_admin=true, full_provision=true` 调用。

### 错误处理 / 回落
- preflight 判 445/135 不通或不可行 → 该机标 `path_b_unavailable`，输出 USB 脚本获取提示（指向 `get-winrm-bootstrap-script` / `winrm bootstrap-script`），**继续下一台**。
- 凭据校验失败（alias 不存在、flag 组合不一致）按现有 `CredentialArgs::preflight` 报错。

### 输出
每台一段：preflight verdict + bootstrap 结果（成功开通的 7 项 / 回落原因）。整批汇总成功/回落/失败计数。授权成功的机器提示可接着跑 `machine deep-scan`。

## 5. 实现形态

- `args.rs`：`MachineAction` 增 `DeepScan { ... }` 与 `Authorize { ... }` 两个变体。
- handler：`src-tauri/src/cli/domain_machine.rs` 增对应处理函数。
- **复用而非复制**：handler 内部调用现有域逻辑（`domain_machine::refresh`、`domain_ini::scan`、`domain_health::run`、`winrm preflight/bootstrap` 的 core 函数），编排命令本身只负责"选择展开 → 逐机调用 → 错误归类 → 聚合输出"。
- 若某个现有 handler 与 `ctx`/emitter 耦合过紧、不便直接复用，则做**定向抽取**：把其纯逻辑提成一个可复用的 `fn`（输入 db + 参数 + 凭据，返回结构化结果），原 handler 与新编排命令都调它——不复制实现、不顺手改无关代码。
- 凭据解析复用 `CredentialArgs`；`--save-as` 复用现有 `cred save` 写入逻辑（DPAPI + cmdkey + SQLite 元数据）。
- `bootstrap` 现有 CLI 签名收内联 `--user/--pass-stdin`（非 `--cred-alias`）；编排命令负责把 alias 解析成 user/pass 再喂进 bootstrap core 函数。

## 6. 非目标（Non-goals）

- 不做任何 UI（含 Figma）；UI 后续单独立项。
- 不做一条总命令 `machine onboard` 串完 4 步（选机器、输凭据需人介入，刻意不全自动）。
- 不自动化 Path A（USB）——仅在 Path B 不可达时给出脚本提示。
- 不改 `machine scan`（仍不入库）、不新增批量 `add`（步骤 1、2 用现有命令）。
- 不重写 refresh/ini/health 的底层逻辑（纯复用）。bootstrap 仅做**加法扩展**（新增 `full_provision` 开关透传，不改其既有默认行为）。

## 7. 测试

- 编排逻辑用 in-memory DB + 桩化域函数：
  - 选择器（`--machine-ids` / `--all`）展开正确。
  - deep-scan：WinRM 失败时跳过该机后续步骤但继续整批；聚合计数正确。
  - authorize：viable → bootstrap；not-viable → 回落分支输出；`--save-as` 写入 alias。
  - 凭据 flag 组合校验沿用 `CredentialArgs` 既有测试。
- 不测真实 WinRM（需 Windows + 真机；在 lanPC 上人工验收）。

## 8. 后续 / 待办

- UI（4 步纳管界面）后续按 Figma-first 在 UI worktree 单独设计实现，届时直接调这两条命令。
- 可选：把"扫描发现结果"持久化为"候选"状态，支撑步骤 2 的有状态多选（当前 CLI 用逐台 `add` 即可）。
