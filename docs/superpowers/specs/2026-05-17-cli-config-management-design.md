# UECM CLI Plan 2 — Config Management Design

**日期**：2026-05-17
**状态**：设计阶段（待实施计划）
**作者**：lanbipu + Claude (brainstorming)
**关联文档**：
- 上游 spec：[2026-05-16-cli-architecture-design.md](./2026-05-16-cli-architecture-design.md)
- 上游 plan：[2026-05-16-cli-plan-1-foundation-discovery.md](../plans/2026-05-16-cli-plan-1-foundation-discovery.md)
- 上游 changelog：[2026-05-16-cli-plan-1.md](../changelog/2026-05-16-cli-plan-1.md)

---

## 1. 摘要

给 `uecm-cli` 加 4 个新能力域（**`cred` / `env` / `ini` / `share`**），让 AI 客户端和命令行用户能远程修改 UE 集群的 DDC 相关配置：保存凭据、读写系统环境变量、修改 INI 单键、建 SMB 共享。`env set` / `ini set` / `ini remove` 支持 `--hosts a,b,c` 批量一次扫一组机器。

对应 4 大底层能力清单的 **#2「配置 DDC 路径」**。

---

## 2. 问题背景

Plan 1 完成后 CLI 能「看见集群」（`machine scan` / `refresh`），但只能**读**。配置 DDC 路径所需的操作（写远程环境变量、写远程 INI、建共享、管凭据）都还锁在 UI IPC 后面。

实际配置 DDC 路径的端到端用例：

```
add machine → save cred → env set UE-SharedDataCachePath → 
(可选) ini set DefaultEngine.ini → (可选) share create + inject SYSTEM
```

Plan 2 把这条链路在 CLI 上打通，AI / 脚本能完整跑这个流程，不依赖 WebView。

---

## 3. 目标 / 非目标

### 目标

- **`cred` 域** — list / save / delete DPAPI-encrypted 凭据。`save` 支持 `--pass-stdin` 避免密码进 shell history
- **`env` 域** — get / set system 级环境变量。`set` 支持 `--host` 单机 + `--hosts` 批量
- **`ini` 域** — read / set / remove 远程 INI 单键。`set` / `remove` 支持 `--hosts` 批量
- **`share` 域** — list / create (Mode A/B) / delete SMB share；inject SYSTEM 凭据
- **批量 `--hosts a,b,c`** — `env set` / `ini set` / `ini remove` 走 `core::batch::run_batch`（并发上限 8），NDJSON 事件流（Started / ItemStarted×N / ItemCompleted×N / Completed）
- **凭据契约统一** — 所有需要远程凭据的 subcommand 接受三选一：`--cred-alias <a>`（DPAPI）/ `--user --pass`（inline）/ `--user --pass-stdin`（stdin）。共享 `CredentialArgs` struct 通过 `#[command(flatten)]` 嵌入
- **不重写 Plan 1 基础设施** — `Ctx`、`Emitter`、`needs_db`、startup module、命名约定全部原样复用

### 非目标

- **`ini` 集群扫描**（`scan` / `findings` / `apply` / `skip` / `runs`）— 推到 Plan 3「集群 DDC 配置发现」（能力 ③）
- **`machine refresh` 加 cred flag** — Plan 1 deferred 项继续 defer 到 Plan 3。理由：refresh 是能力 ① 的尾巴，跟 Plan 2 主线无关
- **`ddc` / `pso` / `project` / `health` / `gpu` 域** — Plan 3 / Plan 4 范畴
- **SIGINT graceful cancel for batch** — 留 future，Plan 1 一致策略
- **Shell completion / MCP / daemon** — Plan 4+

---

## 4. 顶层架构

复用 Plan 1。新增点：

```
src-tauri/src/cli/
├── args.rs                  ← 加 4 个 ActionEnum + 扩 Domain enum
├── credential_args.rs       ← 新：CredentialArgs + resolve()
├── host_args.rs             ← 新：HostArgs (--host | --hosts 互斥)
├── domain_cred.rs           ← 新
├── domain_env.rs            ← 新
├── domain_ini.rs            ← 新
├── domain_share.rs          ← 新
├── mod.rs                   ← 加 4 个 domain + 2 个 helper 模块声明
└── run.rs                   ← 加 4 个 dispatch 分支 + needs_db 规则
```

底层 `core::*` 全部已有，**Plan 2 不扩 core**。具体已验证存在的：

| 域 | 调用的 core / data 函数 |
|---|---|
| cred | `core::credentials::store / delete / list_uecm_aliases / resolve_password`；`data::credentials::insert / delete / list_all` |
| env | `core::env_vars::set_with_credential / get_with_credential / set / get` |
| ini | `core::ini_editor::read_section_with_credential / set_key_with_credential / remove_key_with_credential` + non-cred 版 |
| share | `core::shares::create_mode_a / create_mode_b / generate_svc_password`；`core::psexec::inject_system_credential`；`data::share_configs::insert / list_all / delete` |
| batch | `core::batch::run_batch`（已有） |

---

## 5. 命令空间

### 5.1 `cred`

| 命令 | 对应底层 |
|---|---|
| `cred list` | `data::credentials::list_all(db)` → 返回 `Vec<CredentialRecord>` |
| `cred save --alias <a> --user <u> [--pass <p> \| --pass-stdin]` | `core::credentials::store + store_password`；`data::credentials::insert` |
| `cred delete <alias>` | `core::credentials::delete + delete_password`；`data::credentials::delete` |

**`--pass-stdin`** 从 stdin 读一行（trim `\r\n`），作为密码。避免密码进 shell history 或 process argv。

### 5.2 `env`

| 命令 | 对应底层 |
|---|---|
| `env get --host <h> --name <n>` + `CredentialArgs` | `core::env_vars::get_with_credential` 或 `get` |
| `env set --host <h>\|--hosts h1,h2 --name <n> --value <v>` + `CredentialArgs` | 单机 → `set_with_credential`；批量 → `core::batch::run_batch` 包 `set_with_credential` |

### 5.3 `ini`

| 命令 | 对应底层 |
|---|---|
| `ini read --host <h> --file <f> --section <s>` + cred | `read_section_with_credential` |
| `ini set --host <h>\|--hosts <hs> --file <f> --section <s> --key <k> --value <v>` + cred | 单机 / 批量同 env |
| `ini remove --host <h>\|--hosts <hs> --file <f> --section <s> --key <k>` + cred | 单机 / 批量同 env |

### 5.4 `share`

| 命令 | 对应底层 |
|---|---|
| `share list` | `data::share_configs::list_all` |
| `share create --mode a\|b --host <h> --share <n> --local-path <p>` + cred | Mode A: `create_mode_a`；Mode B: `generate_svc_password` + `create_mode_b` + `core::credentials::store_password` 把 svc cred 也存 DPAPI |
| `share delete <id> --yes` | `data::share_configs::delete`。**注意**：当前 UI 的 `commands::shares::delete_share` 也只删 SQLite row，远端 SMB share 和 cmdkey alias 由 Plan-4 follow-up 用 `ps-scripts/remove-share.ps1` 处理；Plan 2 保持相同语义 |
| `share inject-system-cred --client-host <h> --target-host <h> [--svc-user <u>]` + cred | `core::psexec::inject_system_credential` |

---

## 6. 凭据传递契约（共享 args struct）

### 6.1 定义

```rust
// src-tauri/src/cli/credential_args.rs
use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct CredentialArgs {
    /// Resolve credentials from a saved DPAPI alias.
    #[arg(long, value_name = "ALIAS", group = "cred")]
    pub cred_alias: Option<String>,

    /// Inline username; use with --pass or --pass-stdin.
    #[arg(long, value_name = "USER", group = "cred", requires = "secret")]
    pub user: Option<String>,

    /// Inline password. Leaks into shell history — prefer --pass-stdin
    /// or --cred-alias.
    #[arg(long, value_name = "PASS", group = "secret", conflicts_with = "pass_stdin")]
    pub pass: Option<String>,

    /// Read password from stdin (one line, \r\n trimmed).
    #[arg(long, group = "secret", conflicts_with = "pass")]
    pub pass_stdin: bool,
}

impl CredentialArgs {
    /// Returns `(user, pass)` if any credential was provided; `None` means
    /// inherit caller's Kerberos/NTLM context.
    pub fn resolve(&self) -> UecmResult<Option<(String, String)>>;
}
```

`resolve()` 的优先级：

1. `--cred-alias` 设了 → 从 DPAPI 解出 password；从 `data::credentials` 查回 username；返回 `Some((user, pw))`
2. `--user --pass` 设了 → 直接返回 `Some((u, p))`
3. `--user --pass-stdin` 设了 → 从 stdin 读一行，trim，返回 `Some((u, line))`
4. 都没设 → 返回 `None`，caller 选择 host-only API 或当作 InvalidInput（看 subcommand）

### 6.2 嵌入用法

```rust
#[derive(Subcommand, Debug)]
pub enum EnvAction {
    Get {
        #[arg(long)]
        host: String,
        #[arg(long)]
        name: String,
        #[command(flatten)]
        cred: CredentialArgs,
    },
    Set {
        #[command(flatten)]
        target: HostArgs,
        #[arg(long)]
        name: String,
        #[arg(long)]
        value: String,
        #[command(flatten)]
        cred: CredentialArgs,
    },
}
```

---

## 7. `--host` / `--hosts` 互斥契约（共享 args struct）

```rust
// src-tauri/src/cli/host_args.rs
use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct HostArgs {
    /// Single host. Mutually exclusive with --hosts.
    #[arg(long, group = "target", value_name = "HOST")]
    pub host: Option<String>,

    /// Comma-separated host list. Mutually exclusive with --host.
    #[arg(long, group = "target", value_name = "H1,H2,...", value_delimiter = ',')]
    pub hosts: Option<Vec<String>>,
}

impl HostArgs {
    pub fn require_one(&self) -> UecmResult<HostTarget>;
}

pub enum HostTarget {
    Single(String),
    Batch(Vec<String>),
}
```

clap 用 `group = "target"` 让 `--host` 和 `--hosts` 互斥。`require_one()` 返回 enum，handler 用 match 分支。如果两个都没给（虽然 group 应当拦截），返回 `InvalidInput`。

---

## 8. 批量 `--hosts` NDJSON 协议

`env set --hosts a,b,c --name X --value Y --json` 输出：

```
{"kind":"started","task_type":"env_set","metadata":{"hosts":3,"name":"X","value":"Y"}}
{"kind":"item_started","item_id":"a","index":0,"total":3}
{"kind":"item_completed","item_id":"a","index":0,"ok":true}
{"kind":"item_started","item_id":"b","index":1,"total":3}
{"kind":"item_completed","item_id":"b","index":1,"ok":false,"message":"winrm probe failed: ..."}
{"kind":"item_started","item_id":"c","index":2,"total":3}
{"kind":"item_completed","item_id":"c","index":2,"ok":true}
{"kind":"completed","summary":{"hosts":3,"ok":2,"failed":1}}
```

实现：

- 用 `tokio::runtime::Builder::new_current_thread().build()` 起 runtime（Plan 1 `machine scan` 同样姿势）
- 调 `core::batch::run_batch(hosts, 8, |host| async move { ... })`，并发上限 8
- 把 `run_batch` 的 mpsc 进度流出端口映射到 NDJSON `ItemStarted` / `ItemCompleted` 事件
- 终态 emit `Completed { summary: { hosts, ok, failed } }`

**退出码**：
- 全部 item ok → 0
- 至少一个 fail → 1（OperationFailed semantic）

不引入新退出码。

---

## 9. 错误码与 args 校验

继承 Plan 1：

| code | 含义 | 触发场景 |
|---|---|---|
| 0 | success | 所有 item ok |
| 1 | OperationFailed | 业务失败 / batch 有一项 fail |
| 2 | InvalidInput | clap 拦截 / `--cred-alias` + `--user` 同时设 / `--host` + `--hosts` 同时设 |
| 3 | environment_error | DB / IO / Configuration |
| 4 | powershell_failed | 远端 WinRM / sidecar 失败 |

新的 cred-related InvalidInput case：

- `--cred-alias x --user y` 同时设 → clap `group = "cred"` 拦截，exit 2
- `--user x --pass y --pass-stdin` 同时设 → clap `conflicts_with` 拦截，exit 2
- `cred save` 但既没 `--pass` 也没 `--pass-stdin` → clap `requires = "secret"` 拦截，exit 2

---

## 10. `needs_db` 规则更新

```rust
fn needs_db(cmd: &Domain) -> bool {
    use crate::cli::args::*;
    match cmd {
        Domain::Machine { action } => !matches!(action, MachineAction::Scan { .. }),
        Domain::System { action } => matches!(action, SystemAction::MigrateDb),
        Domain::Winrm { .. } => false,
        
        // Plan 2 additions:
        Domain::Cred { .. } => true,        // list/save/delete 都要 DB
        Domain::Env { .. } => true,         // 需要 cred alias 解析 + 可能写 operations log
        Domain::Ini { .. } => true,         // 同上
        Domain::Share { .. } => true,       // share_configs 表读写
    }
}
```

所有 4 个新域都要 DB（cred alias 解析、share table、operations log）。

---

## 11. 实现拓扑

### 11.1 新文件

```
src-tauri/src/cli/credential_args.rs
src-tauri/src/cli/host_args.rs
src-tauri/src/cli/domain_cred.rs
src-tauri/src/cli/domain_env.rs
src-tauri/src/cli/domain_ini.rs
src-tauri/src/cli/domain_share.rs
```

### 11.2 修改文件

```
src-tauri/src/cli/mod.rs       ← 加 6 个 mod 声明
src-tauri/src/cli/args.rs      ← 加 4 个 ActionEnum + 扩 Domain
src-tauri/src/cli/run.rs       ← 加 4 个 dispatch + needs_db 4 个分支
src-tauri/tests/cli_smoke.rs   ← 加 3-4 个跨平台 smoke
```

### 11.3 完全不动

- `src-tauri/src/core/*` — 已全部具备所需函数
- `src-tauri/src/data/*` — 已全部具备所需函数
- `src-tauri/src/commands/*` — Tauri 专属，不动
- `src-tauri/src/bin/uecm-cli.rs` — entry 不变
- `src-tauri/src/startup.rs` — 不变
- `src/` 前端 — 不动

---

## 12. 测试策略

### 12.1 Unit tests（每个 domain 文件至少）

- args 解析：mutex group 拒绝非法组合（cred_alias + user 共存）
- handler 返回 InvalidInput / OperationFailed 路径
- 在 non-Windows 上，远端调用返回 `UecmError::PowerShell`

### 12.2 跨模块 env-var lock

继续用 Plan 1 的 `crate::ENV_TEST_LOCK`。新域如果有 env-mutating test，照例加 lock。

### 12.3 NDJSON 流式测试

- `domain_env::tests::set_hosts_emits_item_lifecycle`：用 in-memory DB，对一组 unreachable host 调 `set` batch，断言至少有 started + N item_started + N item_completed + completed，所有 item_completed 的 ok 字段都是 false。

### 12.4 集成测试（`tests/cli_smoke.rs`）

新增 3 条：
- `cred_list_on_fresh_db_returns_empty_array`
- `env_set_without_target_returns_invalid_input`（不给 `--host` 也不给 `--hosts`）
- `cred_save_with_conflicting_secret_flags_rejects`（`--pass` + `--pass-stdin`）

### 12.5 lanPC 端到端（Plan 2 验收）

按"配置 DDC 路径"完整剧本跑：

1. `cred save --alias winrm-admin --user lanPC\Administrator --pass-stdin`
2. `env set --host lanPC --name UECM_PLAN2_TEST --value ok --cred-alias winrm-admin`
3. `env get --host lanPC --name UECM_PLAN2_TEST --cred-alias winrm-admin` → 拿回 `ok`
4. `ini read --host lanPC --file <some .ini> --section <s> --cred-alias winrm-admin` → 真读
5. `share create --mode a --host lanPC --share UECM_PLAN2_TEST --local-path D:\Temp\plan2-test --cred-alias winrm-admin` → 真建
6. UI 启动，machine detail / shares 都看得到 CLI 写入

---

## 13. 验证标准

Plan 2 验收通过的条件：

1. `cargo build --release --bin uecm-cli` 在 macOS 和 Windows 都过
2. Tauri main binary build 不受影响
3. 现有 Plan 1 测试（191 + 3）继续全绿
4. 新增 unit + integration 测试全绿
5. lanPC 上"配置 DDC 路径"端到端剧本 6 步全过
6. UI 启动后能看到 CLI 写入的 cred alias / share row（DB 共享契约）
7. 所有需要凭据的 subcommand 都接受 `--cred-alias` / `--user --pass` / `--user --pass-stdin` 三通道
8. 批量 `--hosts` 至少在 `env set` / `ini set` / `ini remove` 上工作
9. `env set --hosts a,b,c --json` 输出严格符合 §8 NDJSON 协议
10. 退出码符合 §9 表格

---

## 14. 不做的事（YAGNI）

- INI 集群扫描（findings / apply / skip / runs / get-finding）
- credential-aware `machine refresh`（Plan 1 deferred 项继续 deferred）
- DDC pak / PSO collect / project discovery / health check / GPU matrix
- batch `--hosts` 的 SIGINT graceful cancel
- batch operation 中间失败后 rollback
- `cred save` 交互式 prompt
- shell completion

---

## 15. 后续工作

- **Plan 3**：能力 ③「集群 DDC 配置发现」— ini scan + findings / apply 工作流 + health + gpu + project（需要 project 因为 ini scan 要项目根目录）
- **Plan 4**：能力 ④「PSO + DDC」— ddc / pso 域 + UE runner CLI 出口（最复杂，要 NDJSON 流处理 10 分钟级长任务）
- **Plan 5+**：MCP wrapper / shell completion / daemon

---

## 附录 A：典型用法 — AI 客户端配置一组 render node

```bash
# 1. AI 先保存 DPAPI 凭据
echo "$ADMIN_PASS" | uecm-cli cred save \
  --alias winrm-admin --user 'lanPC\Administrator' --pass-stdin

# 2. 看现有机器
HOSTS=$(uecm-cli machine list --json | \
  jq -r '.[] | select(.status=="online") | .hostname' | paste -sd, -)

# 3. 批量设环境变量
uecm-cli env set --hosts "$HOSTS" \
  --name UE-SharedDataCachePath \
  --value '\\nas\ddc-share' \
  --cred-alias winrm-admin --json | \
  tee env-set.ndjson

# 4. 校验
for h in $(echo "$HOSTS" | tr , ' '); do
  uecm-cli env get --host "$h" --name UE-SharedDataCachePath \
    --cred-alias winrm-admin --json
done

# 5. 在 NAS 上建共享（如果还没建）
uecm-cli share create --mode b --host nas --share ddc \
  --local-path 'D:\DDC' --cred-alias nas-admin --json

# 6. 给客户端注 SYSTEM 凭据（让 RenderStream service 能挂共享）
for h in $(echo "$HOSTS" | tr , ' '); do
  uecm-cli share inject-system-cred \
    --client-host "$h" --target-host nas \
    --cred-alias winrm-admin --json
done
```
