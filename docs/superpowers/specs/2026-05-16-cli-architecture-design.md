# UECM CLI 架构 — 设计文档

**日期**：2026-05-16
**状态**：设计阶段（待实施计划）
**作者**：lanbipu + Claude (brainstorming)
**关联文档**：[2026-05-01-uecm-design.md](./2026-05-01-uecm-design.md)

---

## 1. 摘要

把 UECM 从「Tauri UI 单一前端」演进为「UI + CLI 双前端 + 共享 lib」架构。新增 `uecm-cli` binary，让 AI 客户端（Claude Code、其他 LLM agent）和命令行用户直接调用 UECM 的全部底层能力，不依赖 Tauri runtime / WebView。

**核心定位**：CLI 不是验证工具，是和 UI 平级的二等公民。AI 通过 CLI 能做的事和人通过 UI 能做的事**完全对等**。

---

## 2. 问题背景

当前架构（参见 [2026-05-01 设计文档](./2026-05-01-uecm-design.md)）：

- 后端有 55 个 `tauri::command`，全部锁在 Tauri IPC 后面
- 唯一 binary（`uecm`，由 `main.rs::run()` 启动）启动时构造 `tauri::Builder`，注册 `invoke_handler!`，运行 WebView
- 没有 WebView 就调不到任何 command
- 业务逻辑虽然已经下沉到 `core::*` + `data::*`（commands 层是 thin wrapper），但缺一个**绕过 Tauri runtime 的入口**

由此带来的痛点：

- **AI 无法独立操作 UECM**：Claude Code 之类的 agent 调不到底层，只能写 PowerShell 脚本绕过，无法复用 UECM 的 SQLite 状态、规则引擎、批量并发、GPU 一致性等核心资产
- **自动化困难**：CI / 脚本化运维场景需要无人值守跑 ini-scan / health-check / pso-collect，目前必须开 GUI
- **集成困难**：未来想做 MCP server / Web 后端服务化部署，没有底层可复用入口
- **验证负担重**：开发态测一个 backend 改动要起整个 Tauri app，反馈环慢

---

## 3. 目标 / 非目标

### 目标

- 新增 `uecm-cli` binary，覆盖 UI 能做的全部业务操作（55 个 Tauri command → CLI subcommand 全集）
- CLI 和 UI **共享同一个 SQLite DB**（默认路径 `%APPDATA%\com.uecm.app\uecm.sqlite`）
- 命令以**能力域 + 动作**两层组织（git/kubectl 风格）
- 长跑任务通过 **NDJSON 流式协议**实时回传事件，AI 可逐行消费
- 不破坏现有 Tauri build pipeline，UI 那条 branch 合并不冲突
- 抽出最小 startup module，让两个 binary 共用 DB 初始化逻辑（避免双份）

### 非目标

- 不做交互式 prompt（AI 用不上，人要交互就开 UI）
- 不做 shell completion（后续 nice-to-have）
- 不做 i18n（CLI 文本走英文）
- 不做后台 daemon 模式（长跑任务通过 NDJSON 流式同步处理；`--detach` 留接口，初版不实现）
- 不做 MCP server（初版只做原生 CLI，MCP 是未来一层 wrapper）
- 不重写现有 Tauri command handler（CLI 直接调 `core::*`，commands 层保持不动）

---

## 4. 顶层架构

```
┌─────────────────────────────────┐  ┌──────────────────────────┐
│ UI frontend (Tauri webview)     │  │ AI / human (terminal)    │
│   Vue 3 + Pinia                 │  │   uecm-cli <domain> <op> │
└────────────┬────────────────────┘  └────────────┬─────────────┘
             │ Tauri IPC                          │ argv + stdin
             ▼                                    ▼
   ┌─────────────────────┐              ┌───────────────────────┐
   │ binary: uecm        │              │ binary: uecm-cli      │
   │ (main.rs)           │              │ (src/bin/uecm-cli.rs) │
   │ tauri::Builder      │              │ clap + tokio          │
   │ + invoke_handler!   │              │ + NDJSON emitter      │
   └──────┬──────────────┘              └──────┬────────────────┘
          │                                    │
          ▼                                    ▼
          ┌───────────────────────────────────────────┐
          │ lib: uecm_lib (rlib)                      │
          │   core::*  data::*  error::*              │
          │   startup::* ← 新增，DB 初始化共用       │
          └──────┬────────────────────────────────────┘
                 ▼
        ┌────────────────────────────────────┐
        │ %APPDATA%\com.uecm.app\uecm.sqlite │ ← WAL mode
        │ ps-scripts\*.ps1                   │
        │ vendor\PsExec64.exe                │
        └────────────────────────────────────┘
```

### 4.1 关键决策

- **两个 binary 一个 lib**：`uecm` 走 Tauri，`uecm-cli` 是 clap CLI。共享 `uecm_lib`
- **CLI 绕过 commands 层**：CLI subcommand handler 直接调 `core::*` + `data::*`。`commands/*.rs` 仍然是 Tauri 专属薄壳，不动它
- **DB 共享 WAL mode**：CLI 启动时打开 DB 设 `PRAGMA journal_mode=WAL`，UI 同样设置；并发读写安全
- **Schema 迁移幂等**：CLI 启动跑 `data::schema::migrate`，UI 启动也跑，谁先谁建表
- **DB 路径解析**：用 `directories` crate（不依赖 Tauri 的 `app_data_dir`，那个要 Builder 上下文）
- **PowerShell 资源解析**：现在 `powershell::script_path` 依赖 Tauri resource API；加 fallback：先查 `$UECM_PS_DIR`，再查 binary 同目录下 `ps-scripts/`，最后才走 Tauri resource

### 4.2 抽出 startup module

新增 `src-tauri/src/startup.rs`，导出：

```rust
pub fn resolve_db_path() -> PathBuf;       // 替代当前 main.rs 里的 app_data_dir 逻辑
pub fn open_and_migrate_db() -> UecmResult<Db>;   // open + WAL + migrate
pub fn resolve_ps_script_dir() -> PathBuf; // env var > binary-relative > Tauri resource
```

`main.rs::run()` 和 `bin/uecm-cli.rs::main()` 都调这三个函数初始化运行时。`core::powershell` 改为先调 `startup::resolve_ps_script_dir()`。

---

## 5. 命令空间

12 个顶层域。`--hosts a,b,c` flag 自然处理批量，不单独建 batch 域。

### 5.1 域清单

```
uecm-cli <domain> <action> [args] [flags]
```

| 域 | 职责 |
|---|---|
| **machine** | 机器生命周期（list / add / refresh / delete / rename） |
| **winrm** | 远程执行底层（probe / bootstrap） |
| **cred** | 凭据管理（save / list / delete） |
| **env** | 系统环境变量读写 |
| **ini** | INI 单键编辑 + 集群扫描 |
| **share** | SMB 共享创建 + SYSTEM 凭据注入 |
| **project** | uproject 发现 + 跨机身份映射 |
| **ddc** | DDC pak workflow（generate / verify / distribute / cancel） |
| **pso** | PSO cache workflow（verify / collect / list / distribute） |
| **health** | 集群健康检查 |
| **gpu** | GPU 一致性矩阵 |
| **system** | 诊断 / 自检（echo / db-path / migrate-db / version） |

### 5.2 完整命令清单

#### `machine`

| 命令 | 对应 Tauri command | 备注 |
|---|---|---|
| `machine list` | `list_machines` | 表格或 JSON 输出 |
| `machine scan <cidr> [--timeout-ms]` | `scan_network` | 流式打 ProbedHost |
| `machine add --ip <ip> [--hostname <h>]` | `add_discovered_machine` / `add_machine` | |
| `machine refresh <id>` | `refresh_machine` | 流式打探测进度 |
| `machine delete <id>` | `delete_machine` | 二次确认走 `--yes` flag |
| `machine rename <id> <hostname>` | `rename_machine` | |
| `machine detail <id>` | `get_machine_detail` | |

#### `winrm`

| 命令 | 对应 | 备注 |
|---|---|---|
| `winrm probe <host>` | `core::winrm::probe`（新 CLI 入口） | 返回 latency_ms |
| `winrm bootstrap <host> --user --pass [--enable-local-admin]` | `bootstrap_winrm` | |
| `winrm bootstrap-script [--output <path>]` | `get_winrm_bootstrap_script` | stdout 或落盘 |

#### `cred`

| 命令 | 对应 |
|---|---|
| `cred list` | `list_credentials` |
| `cred save --alias <a> --user <u> --pass <p>` | `save_credential` |
| `cred delete <alias>` | `delete_credential` |

`pass` 也可走 `--pass-stdin`（从 stdin 读密码，避免进 shell history）。

#### `env`

| 命令 | 对应 |
|---|---|
| `env get --host <h> --name <n> [--cred-alias <a>]` | `get_machine_env_var(_with_credential)` |
| `env set --host <h>\|--hosts <h1,h2> --name <n> --value <v> [--cred-alias <a>]` | `set_machine_env_var` 或 `batch_set_env_var` |

`--host` 和 `--hosts` 互斥。`--hosts` 时走 `core::batch::run_batch`，并发上限 8。

#### `ini`

| 命令 | 对应 |
|---|---|
| `ini read --host <h> --file <f> --section <s> [--cred-alias <a>]` | `read_ini_section(_with_credential)` |
| `ini set --host <h>\|--hosts <hs> --file <f> --section <s> --key <k> --value <v>` | `set_ini_key` 或 `batch_set_ini_key` |
| `ini remove --host <h> --file <f> --section <s> --key <k>` | `remove_key_with_credential`（新 CLI 入口） |
| `ini scan --machine-ids <ids>` | `scan_inis` |
| `ini runs [--limit N]` | `list_recent_ini_runs` / `list_scan_runs` |
| `ini findings <scan-run-id>` | `list_findings_for_run` |
| `ini get-finding <finding-id>` | `get_finding` |
| `ini apply <finding-id>` | `apply_finding` |
| `ini skip <finding-id>` | `skip_finding` |

#### `share`

| 命令 | 对应 |
|---|---|
| `share list` | `list_shares` |
| `share create --mode a\|b --host <h> --share <name> --local-path <p> [...]` | `create_share` |
| `share delete <id>` | `delete_share` |
| `share inject-system-cred --client-host <h> --target-host <h> [...]` | `inject_share_credential_to_clients` |

#### `project`

| 命令 | 对应 |
|---|---|
| `project list` | `list_projects` |
| `project locations <project-id>` | `list_project_locations` |
| `project discover --machine-id <id> --roots <r1,r2>` | `discover_projects` |
| `project create-manual --name <n> --machine-id <id> --abs-path <p>` | `create_project_manual` |
| `project set-location --project-id <id> --machine-id <id> --abs-path <p>` | `set_project_location` |
| `project delete <id>` | `delete_project` |
| `project delete-location <id>` | `delete_project_location` |

#### `ddc`

| 命令 | 对应 |
|---|---|
| `ddc generate --project-id <id> --source-machine <id> [--cred-alias <a>]` | `generate_ddc_pak` |
| `ddc verify --project-id <id> --source-machine <id>` | `verify_pak_output` |
| `ddc distribute --project-id <id> --source-machine <id> --targets <ids>` | `distribute_ddc_pak` |
| `ddc cancel <job-id>` | `cancel_ue_job` |

#### `pso`

| 命令 | 对应 |
|---|---|
| `pso verify --project-id <id>` | `verify_pso_precaching` |
| `pso collect --project-id <id> --source-machine <id> [--resolution WxH --windowed --max-minutes N]` | `start_pso_collection` |
| `pso list --project-id <id>` | `list_pso_cache_files` |
| `pso distribute --project-id <id> --source-machine <id> --targets <ids>` | `distribute_pso_cache` |

#### `health`

| 命令 | 对应 |
|---|---|
| `health run --machine-ids <ids>` | `run_health_check` |
| `health runs [--limit N]` | `list_recent_health_runs` |
| `health results <scan-run-id>` | `list_health_results_for_run` |

#### `gpu`

| 命令 | 对应 |
|---|---|
| `gpu matrix` | `get_gpu_consistency_matrix` |

#### `system`

| 命令 | 对应 |
|---|---|
| `system echo <message>` | `test_powershell_bridge` |
| `system db-path` | 打印解析后的 DB 路径（新增，调试用） |
| `system migrate-db` | 手工触发 `data::schema::migrate`（新增） |
| `system version` | 打印 binary 版本 + lib 版本（新增） |
| `system ps-dir` | 打印解析后的 ps-scripts 目录（新增，调试用） |

### 5.3 命名约定

- 域是名词，动作是动词（`machine list` / `ini scan` / `pso collect`）
- 跨机操作 `--host` / `--hosts` 互斥 flag 区分单机/批量
- 数据库行 id 引用：positional `<id>` 是主键必填；其他 id 走 flag（`--project-id` / `--machine-ids`）
- 凭据 flag 在所有需要鉴权的 subcommand 上统一为 `--cred-alias <a>` 或 `--user <u> --pass <p>`（二选一）

---

## 6. 输入输出契约

### 6.1 输入

- **Positional 参数**：用于主键 id / hostname / CIDR 等位置明确的强制参数
- **Flag**：用于可选参数、列表参数、互斥选择
- **Stdin JSON**：长 payload（例如 `--from-stdin` 模式）走 stdin 喂 JSON object，方便 AI 拼请求；初版只有 `cred save --pass-stdin` 用到，其他 subcommand 保留这条通道，未来扩展

### 6.2 输出模式

CLI 有两种输出模式，通过 `--json` global flag 切换：

#### 默认：人类可读

- 表格 / 缩进 / 颜色（`atty` 检测 TTY 才上色）
- 进度类操作打人类可读 progress bar（基于 NDJSON 事件内部转换）
- 终态打成功 / 失败摘要

#### `--json`：结构化

- **单次返回**（list / get / read 类）：单个 JSON object/array 到 stdout，一次性
- **长跑任务**（scan / collect / batch / health 类）：**NDJSON 流式**，每行一个事件 JSON object

### 6.3 退出码

| code | 含义 |
|---|---|
| 0 | 成功 |
| 1 | 通用业务失败（操作完成但结果不达预期，例如远端脚本返回 `ok: false`） |
| 2 | 参数错误（clap 拦截 / 互斥 flag 冲突 / 参数格式非法） |
| 3 | 环境错误（DB 不可写 / ps-scripts 找不到 / Tauri resource 不可访问） |
| 4 | 远端不可达（WinRM probe 失败 / SMB 不通 / 超时） |
| 5 | 鉴权失败（cred-alias 不存在 / DPAPI 解密失败 / 凭据被拒） |
| 10 | NDJSON 流中已发出 `error` 事件后的最终退出码（区别于 0/1 让流式消费者快速判定） |
| 130 | SIGINT / Ctrl-C |

### 6.4 错误结构（`--json` 模式）

```json
{
  "kind": "error",
  "code": "winrm_unreachable",
  "message": "human-readable explanation",
  "details": { "host": "RENDER-01", "latency_ms": null },
  "rust_error": "UecmError::PowerShell(\"...\")"
}
```

`code` 是稳定的 string enum，AI 可以按 code 做决策。`rust_error` 仅 debug 用。

### 6.5 日志去向

- **stdout** 只放业务输出（JSON 或表格），AI 解析它
- **stderr** 放：
  - 结构化日志（`tracing` 输出，层级走 `RUST_LOG` env var 或 `--log-level info|debug|trace` flag）
  - 人类模式下的 progress bar / spinner（通过 `eprintln!` 或 `indicatif` 写到 stderr，不走 tracing）
- `--json` 模式下 stderr 默认安静（只发 `RUST_LOG` 触发的日志），不打 progress bar，避免污染 NDJSON 消费者的视野

---

## 7. 凭据传递与认证

### 7.1 三种鉴权来源

任何需要远端凭据的 subcommand（env / ini / share / discover / ddc / pso / health 等）都接受：

1. **`--cred-alias <alias>`** — 走 UECM 已有的 DPAPI 通道（`%LOCALAPPDATA%\UECM\creds.bin`）。Rust 侧 `credentials::resolve_password` 解出 → 注入 `Invoke-Command -Credential`
2. **`--user <u> --pass <p>`** — inline，CI / 一次性场景
3. **`--user <u> --pass-stdin`** — inline 但密码从 stdin 读（不进 shell history / process argv）

互斥规则：1 和 (2 或 3) 互斥。三者都不给时，按 subcommand 的需求决定 fallback：
- 操作 loopback target（本机） → 不需要凭据，跳过
- 其他 → 试图复用 caller 当前的 Kerberos/NTLM 上下文（即 `core::winrm::invoke` 不带 credential 的分支）

### 7.2 DPAPI 上下文约束

UECM 现有 DPAPI 加密用 `CurrentUser` scope。意味着：

- CLI 必须以**和 UECM UI 同一个 Windows 用户**身份运行，才能解密 alias 密码
- 跨用户调用（例如 SYSTEM 服务调用 CLI）会失败 → 走 `--user/--pass` inline 路径
- 设计文档记录此约束。`cred save` 命令的 help text 也说明

### 7.3 SYSTEM 上下文注入

`share inject-system-cred` 子命令保留现有 `core::psexec::inject_system_credential` 行为：在客户端通过 WinRM 推 PsExec64 进 SYSTEM 上下文写一条 host-specific cmdkey。CLI 不需要自己提权运行。

---

## 8. 长跑任务事件协议（NDJSON）

### 8.1 协议定义

所有长跑 subcommand（`machine scan`、`machine refresh`、`ini scan`、`health run`、`ddc generate`、`ddc distribute`、`pso collect`、`pso distribute`、`project discover`、批量 `env set` / `ini set`）在 `--json` 模式下产出 NDJSON 流：

- 每行一个 UTF-8 编码、单行 JSON object（无内部换行）
- 行尾 `\n`，不要 `\r\n`
- 流必然以一个**终态事件**结尾：`completed` / `cancelled` / `error`
- 收到终态事件后 stdout 关闭

### 8.2 事件字典

| `kind` | 触发时机 | 关键字段 |
|---|---|---|
| `started` | 任务接受、开始执行 | `task_type`, `task_id?`, `metadata` |
| `host_probe` | 扫描中某个 host 探测完成（`machine scan`） | `ip`, `winrm_open`, `smb_open` |
| `spawned` | UE 子进程启动（`ddc generate` / `pso collect`） | `pid`, `log_path` |
| `log_line` | 子进程 stdout/log 一行 | `text`, `parsed_kind?` |
| `progress` | 显式进度 | `pct?`, `label`, `current?`, `total?` |
| `item_started` | 批量任务里某个 item 启动（`batch_set_env_var` 等） | `item_id`, `index`, `total` |
| `item_completed` | 某个 item 完成 | `item_id`, `index`, `ok`, `message?` |
| `finding` | INI scan 命中一条规则（流式 emit，可选） | `rule_id`, `severity`, `file_path`, `section?`, `key?` |
| `cancelled` | 用户 Ctrl-C 或显式 cancel | `reason` |
| `error` | 不可恢复错误 | `code`, `message`, `details` |
| `completed` | 任务正常结束 | `summary` (任务专属 payload) |

### 8.3 与 `core::ue_runner::UeRunnerEvent` 的关系

`UeRunnerEvent` 是这套协议的子集——CLI 把 runner 的 enum 直接序列化为对应 NDJSON 事件，零信息损失：

```
UeRunnerEvent::Spawned   → kind=spawned
UeRunnerEvent::LogLine   → kind=log_line
UeRunnerEvent::Progress  → kind=progress
UeRunnerEvent::Completed → kind=completed
UeRunnerEvent::Cancelled → kind=cancelled
UeRunnerEvent::Error     → kind=error
```

批量 `core::batch::run_batch` 的 mpsc 进度也走同一通道（映射到 `item_started` / `item_completed`）。

### 8.4 中断处理

- SIGINT (Ctrl-C) 触发 graceful cancel：
  - 远端 UE 任务通过 `core::ue_runner::RunnerCancel` 走 `stop-ue-process.ps1` kill PID
  - 批量任务停止派发新 item，等已 in-flight 的完成
  - 最终 emit `cancelled` 事件后退 130

### 8.5 `--detach` 接口（占位，初版不实现）

为未来后台 daemon 模式预留 flag：
```
uecm-cli pso collect ... --detach    # 返回 {"job_id": "..."} 立即退出
uecm-cli system jobs                  # 列出后台 job
uecm-cli system tail <job-id> [--follow]  # 流式跟流
```

初版 `--detach` 给 clap 接受但返回 "not implemented yet"，留下兼容接口。

---

## 9. 实现拓扑

### 9.1 文件变更清单

#### 新建

```
src-tauri/src/bin/uecm-cli.rs           ← clap CLI entry
src-tauri/src/cli/                       ← CLI handler module
  mod.rs
  args.rs                                ← clap derive struct
  output.rs                              ← NDJSON / human-readable emitter
  domain_machine.rs
  domain_winrm.rs
  domain_cred.rs
  domain_env.rs
  domain_ini.rs
  domain_share.rs
  domain_project.rs
  domain_ddc.rs
  domain_pso.rs
  domain_health.rs
  domain_gpu.rs
  domain_system.rs
src-tauri/src/startup.rs                 ← 共享初始化（DB / ps-scripts 路径）
```

#### 修改

```
src-tauri/Cargo.toml                     ← 加 [[bin]] uecm-cli; 加 clap / directories / atty 依赖
src-tauri/src/lib.rs                     ← run() 改用 startup::open_and_migrate_db
src-tauri/src/main.rs                    ← 无变更
src-tauri/src/core/powershell.rs         ← script_path 加 env var + binary-relative fallback
src-tauri/src/core/mod.rs                ← 无变更
```

#### 不动

- `src-tauri/src/commands/*.rs` 全部不动（Tauri 专属）
- 现有所有 `core::*` 业务逻辑模块不动
- `src/` 前端代码不动

### 9.2 新增依赖（Cargo.toml）

```toml
clap = { version = "4.5", features = ["derive", "env"] }
directories = "5.0"          # 跨平台 app_data_dir 解析（不依赖 Tauri）
atty = "0.2"                 # TTY 检测（决定要不要上色）
```

`clap` 用 derive 风格，每个域一个 enum 嵌套。

### 9.3 Cargo bin 注册

```toml
[[bin]]
name = "uecm"               # 现有，Tauri main binary
path = "src/main.rs"

[[bin]]
name = "uecm-cli"           # 新增
path = "src/bin/uecm-cli.rs"
```

Tauri build 默认只打 `uecm`；CLI 单独 `cargo build --release --bin uecm-cli`。

### 9.4 模块依赖图

```
bin/uecm-cli.rs
   ↓
cli::args (clap derive)
   ↓
cli::domain_*  ─── 调 ──→  uecm_lib::core::*
   ↓                       uecm_lib::data::*
cli::output (NDJSON / human)
   ↓
stdout / stderr
```

### 9.5 输出层抽象

`cli::output` 模块导出：

```rust
pub trait Emitter {
    fn emit_event(&mut self, event: Event);    // 流式事件
    fn emit_result<T: Serialize>(&mut self, value: &T);  // 单次结果
    fn emit_error(&mut self, error: &UecmError);
}

pub struct NdjsonEmitter { ... }    // --json 模式
pub struct HumanEmitter { ... }     // 默认
```

`Event` 是 enum，对应 §8.2 事件字典。所有 domain handler 拿到 emitter 后写事件，不关心 JSON / 人类输出差异。

---

## 10. 兼容性 / 版本演进

### 10.1 schema 演进

- CLI 启动时跑 `data::schema::migrate`，与 UI 共享同一套迁移
- 迁移幂等。新版本（CLI 或 UI）启动会把 DB 迁到自己认识的最高版本；旧版本读新版 DB **当前不做主动检测**，假设 CLI 与 UI 同 release tag 同步发布
- 未来要做严格版本检测时，引入 `PRAGMA user_version` 或新增 `schema_meta` 表，CLI 启动检查 `schema_version > expected` 时 `exit 3`。v1 不实现

### 10.2 命令稳定性

- 顶层域名 + subcommand 名进入"稳定"集合后不改名；增不破坏
- Flag 同理。废弃 flag 保留至少一个 minor 版本，期间 stderr 打 deprecation warning
- NDJSON `kind` 值同理：增不破坏；旧 `kind` 保留至少一个 minor 版本
- 退出码语义稳定

### 10.3 与 UI release 的关系

- CLI 和 UI 共用 `uecm_lib`，绑定 lib version
- 发布物：`uecm-cli.exe` 单独 ship，文件大小 ~10 MB（rusqlite bundled + tokio）
- UI installer 可选打包 CLI 进同一目录（`C:\Tools\UECM\uecm-cli.exe`），用户 PATH 加这一项即可

---

## 11. 不做的事（YAGNI）

- 不做交互式 prompt（`-i` / 确认对话）
- 不做 shell completion（bash / zsh / pwsh）
- 不做 CLI 内的 dry-run mode（设计太复杂；用户用 `--json` + 看 `started` 事件 + Ctrl-C 等价）
- 不做 watch mode（`--watch` 自动 re-run）
- 不做 plugin / extension 系统
- 不做 daemon / IPC server 模式（未来 MCP 是更好的形态，不重复造轮子）
- 不做远程 CLI（CLI 操作本机的 UECM，UI / CLI 都在同一台机器上运行）

---

## 12. 验证标准

CLI 验收通过的条件：

1. `cargo build --release --bin uecm-cli` 在 macOS 和 Windows 都能成功
2. Tauri main binary build 不受影响（`pnpm tauri build --no-bundle` 仍正常）
3. `uecm-cli machine scan 192.168.10.0/24 --json` 在 lanPC 上跑通，NDJSON 流正确，能看到 lanPC 自己 + 局域网内其他 host
4. `uecm-cli machine refresh <id> --json` 拿到 UE 安装列表 + GPU 信息（含正确 VRAM）
5. `uecm-cli env get/set` 单机和 `--hosts` 批量都通
6. `uecm-cli ini scan --json` 流式输出 finding 事件，DB 里能查到 scan_runs 行
7. `uecm-cli health run --json` 三类检查（active probe + ini_consistency + gpu_consistency）都跑出结果
8. UI 启动后能看到 CLI 写入的 machines / scans / findings，无 SQL 锁冲突
9. 退出码语义和 §6.3 表格一致
10. 所有 `--cred-alias` 路径在 Windows 上能解出 DPAPI 加密的密码

详细测试矩阵见后续 implementation plan。

---

## 13. 后续工作

- **实施计划**：本设计完成后，调用 `writing-plans` skill 产出 step-by-step 实施计划
- **MCP server**（未来）：在 CLI 之上包一层 MCP server，把每个 subcommand 转成 MCP tool。CLI 是底层，MCP 是上层 wrapper
- **shell completion**（未来）：`clap_complete` 自动生成 bash / pwsh completion
- **后台 job 模式**（未来）：`--detach` + `jobs` 表 + `system tail` 实装
- **远程 CLI**（未来，可能不做）：通过 SSH + 远端 `uecm-cli` 二进制操作远端 UECM 状态

---

## 附录 A：典型用法示例

### A1：AI 客户端跑一次完整发现

```bash
# 扫局域网
uecm-cli machine scan 192.168.10.0/24 --json > scan.ndjson

# 加发现的 host 入库（AI 解析 scan.ndjson 后批量）
for ip in $(jq -r 'select(.kind=="host_probe" and .winrm_open==true) | .ip' scan.ndjson); do
  uecm-cli machine add --ip $ip --json
done

# 刺探 + 入库 UE / GPU 信息
for id in $(uecm-cli machine list --json | jq -r '.[].id'); do
  uecm-cli machine refresh $id --json
done

# 全集群 INI 扫描
uecm-cli ini scan --machine-ids $(uecm-cli machine list --json | jq -r 'map(.id) | join(",")') --json
```

### A2：人类一键修复某条 finding

```bash
uecm-cli ini findings 7              # 看 scan run 7 的所有 findings（人类表格）
uecm-cli ini get-finding 42          # 看一条详情
uecm-cli ini apply 42                # 一键修复
```

### A3：PSO 收集 + 分发

```bash
# 验证前置 CVar 在
uecm-cli pso verify --project-id 3

# 收集（流式跟 10 分钟）
uecm-cli pso collect --project-id 3 --source-machine 5 --max-minutes 10 --json | \
  tee pso-collect.ndjson

# 分发到目标机
uecm-cli pso distribute --project-id 3 --source-machine 5 --targets 6,7,8 --json
```

---

## 附录 B：和 [2026-05-01 UECM 设计文档](./2026-05-01-uecm-design.md) 的关系

- 本文档不修改原设计的业务模型、能力范围、UI 形态
- 本文档只增加一个新的前端入口（CLI），底层共用 `core::*` + `data::*`
- 原设计文档保持权威，本文档作为后续 augment
