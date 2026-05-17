# UECM CLI Plan 3 — Discovery, PSO/DDC, and Plan-1 Refresh Cred

**Date:** 2026-05-17
**Status:** 设计阶段（待实施计划）
**关联**:
- 上游 spec: [2026-05-16-cli-architecture-design.md](./2026-05-16-cli-architecture-design.md)
- Plan 2 spec: [2026-05-17-cli-config-management-design.md](./2026-05-17-cli-config-management-design.md)

---

## 1. 摘要

Plan 3 把剩余的 7 个域（`project` / `ini` 集群扫描 / `health` / `gpu` / `ddc` / `pso`）连同 Plan 1 deferred 的 credential-aware `machine refresh` 一起 ship，让 `uecm-cli` 覆盖 UI 全部 backend 能力。`ddc` / `pso` 的 UE runner 长跑任务通过现有 NDJSON 事件协议流式输出（`spawned` / `log_line` / `progress` / `completed`）。

完成后 12 个 CLI 域全部可调（machine / winrm / system / cred / env / ini / share / project / health / gpu / ddc / pso），实现 user goal "全功能 CLI 完成、可直接调用使用、准确无误"。

---

## 2. 目标 / 非目标

### 目标

- **`project` 域**：list / locations / discover / create-manual / set-location / delete / delete-location（7 个 action，1:1 镜像 commands::projects）
- **`ini` 集群扫描扩展**：scan / runs / findings / get-finding / apply / skip / verify-pso-precaching（7 个新 action，在 Plan 2 的 `ini` 域之上加）
- **`health` 域**：run / runs / results（3 个 action）
- **`gpu` 域**：matrix（1 个 action）
- **`ddc` 域**：generate / verify / distribute / cancel（4 个 action，含 UE runner NDJSON 长跑流）
- **`pso` 域**：verify / collect / list / distribute（4 个 action，含 UE runner NDJSON 长跑流）
- **Plan 1 deferred**：`machine refresh --cred-alias / --user --pass / --pass-stdin` — 加 3 个 `_with_credential` 变种到 `core::winrm` + `core::discovery`，CLI args 把 cred flags 加回 `MachineAction::Refresh`

### 非目标

- SIGINT graceful cancel for batch / long-running tasks — 留 future
- Shell completion / MCP wrapper / daemon
- `share remove` 远端真销毁 — Plan-4 follow-up
- 任何 UI 改动

---

## 3. 顶层架构（仍复用 Plan 1）

- 6 个新 handler 文件 + 1 个修改：

```
src-tauri/src/cli/
├── domain_project.rs        ← 新
├── domain_health.rs         ← 新
├── domain_gpu.rs            ← 新
├── domain_ddc.rs            ← 新
├── domain_pso.rs            ← 新
├── domain_ini.rs            ← 修改：加 scan / runs / findings / apply / skip / verify-pso-precaching
├── domain_machine.rs        ← 修改：refresh 加回 cred flags + 走 _with_credential
├── args.rs                  ← 加 5 个 Domain 变种 + 5 个 ActionEnum + 扩 IniAction / MachineAction::Refresh
├── run.rs                   ← 加 5 个 dispatch arm + 扩 needs_db
└── mod.rs                   ← 加 5 个 mod 声明
```

- **core 扩展**（最小）：`core::winrm::probe_with_credential` + `core::discovery::detect_ue_versions_with_credential` + `detect_gpus_with_credential` — 每个 5-10 行薄包装，把 op_user/op_pass 传进 PS 脚本

- 其他 `core::*` / `data::*` 函数全部已存在（spec §11.3 of Plan 2 已验证）

---

## 4. 命令空间

### 4.1 `project`

| 命令 | 对应底层 |
|---|---|
| `project list` | `data::projects::list_all` |
| `project locations <project-id>` | `data::project_locations::list_for_project` |
| `project discover --machine-id <id> --roots <r1,r2>` + cred | `core::project_discovery::run_discovery` (持久化通过) |
| `project create-manual --name <n> --machine-id <id> --abs-path <p>` | `core::project_identity::manual_alias` + `data::*` insert |
| `project set-location --project-id <id> --machine-id <id> --abs-path <p>` | `core::project_identity::manual_path` |
| `project delete <id> --yes` | `data::projects::delete` (含 cascade locations) |
| `project delete-location <id> --yes` | `data::project_locations::delete` |

### 4.2 `ini` 集群扫描扩展（在 Plan 2 ini 域之上扩）

| 命令 | 对应底层 |
|---|---|
| `ini scan --machine-ids <ids>` + cred | `core::ini_scanner::scan_machine` × N，写 `scan_runs` + `ini_findings`。NDJSON 流：`started{machines:N}` → `finding × M` → `completed{scan_run_id, total, critical, warning, healthy}` |
| `ini runs [--limit N]` | `data::scan_runs::list_recent("ini", limit)` |
| `ini findings <scan-run-id> [--severity critical\|warning\|healthy\|info]` | `data::ini_findings::list_for_run` (+ filter) |
| `ini get-finding <finding-id>` | `data::ini_findings::get` |
| `ini apply <finding-id>` + cred | `core::ini_apply::apply` |
| `ini skip <finding-id>` | `data::ini_findings::mark_skipped` |
| `ini verify-pso-precaching --project-id <id>` | UI 的 `commands::ini_scanner::verify_pso_precaching` 等价；ini scanner 读 project ConsoleVariables.ini 校验 R008-R010 |

### 4.3 `health`

| 命令 | 对应底层 |
|---|---|
| `health run --machine-ids <ids>` + cred | `core::health_probes::run` × N + `core::health_check::aggregate_*`。NDJSON 流：`started{machines:N}` → `item_completed × N` → `completed{scan_run_id, summary}` |
| `health runs [--limit N]` | `data::scan_runs::list_recent("health", limit)` |
| `health results <scan-run-id>` | `data::health_check_runs::list_for_run` |

### 4.4 `gpu`

| 命令 | 对应底层 |
|---|---|
| `gpu matrix` | `core::gpu_consistency::build_matrix(db)` — 一次性输出 GpuMatrix JSON |

### 4.5 `ddc`

| 命令 | 对应底层 |
|---|---|
| `ddc generate --project-id <id> --source-machine <id>` + cred | `core::ddc_pak::preflight` → `launch_generation` 返回 RunnerHandle。NDJSON 流转发 `UeRunnerEvent`（`spawned` / `log_line` / `progress` / `completed`） |
| `ddc verify --project-id <id> --source-machine <id>` + cred | `core::ddc_pak::verify_output` |
| `ddc distribute --project-id <id> --source-machine <id> --targets <ids>` + cred | `core::pak_distribute::plan` + `run_one` × N，NDJSON 流 (`item_started` / `item_completed`) |
| `ddc cancel <job-id>` | 通过现有 `commands::ddc_pak::UeJobRegistry`-like 机制 cancel — CLI 需要轻量 in-process job registry 或者 `--no-job-registry` (single-process)，**Plan 3 简化版：不支持 cancel，因为 CLI 进程是一次性的；ddc generate ctrl-C 即结束。后续 daemon mode 再加 cancel** |

### 4.6 `pso`

| 命令 | 对应底层 |
|---|---|
| `pso verify --project-id <id>` | `core::ini_scanner` (R008-R010 CVar 检查) |
| `pso collect --project-id <id> --source-machine <id> [--resolution WxH --windowed --max-minutes N]` + cred | `core::pso_collect::launch_collection` + `enumerate_remote` + `finalize_persist`。NDJSON 流转发 `UeRunnerEvent` |
| `pso list --project-id <id>` | `data::pso_cache_files::list_for_project` |
| `pso distribute --project-id <id> --source-machine <id> --targets <ids>` + cred | `core::pso_distribute::plan` + `run_one` × N，含 GPU 一致性 preflight 拦截 |

### 4.7 `machine refresh` cred 补全（Plan 1 deferred 收口）

`MachineAction::Refresh` args 加回 `#[command(flatten)] cred: CredentialArgs`。Handler 用 `cred.resolve(db)?`，传给 `winrm::probe_with_credential` / `discovery::detect_ue_versions_with_credential` / `discovery::detect_gpus_with_credential`。

`core::winrm` 加：
```rust
pub fn probe_with_credential(host: &str, user: &str, pass: &str) -> UecmResult<ProbeResult>
```
内部走 `invoke_with_credential`（已存在），调用 `test-winrm.ps1`。

`core::discovery` 加：
```rust
pub fn detect_ue_versions_with_credential(host: &str, user: &str, pass: &str) -> UecmResult<Vec<DetectedUe>>
pub fn detect_gpus_with_credential(host: &str, user: &str, pass: &str) -> UecmResult<Vec<DetectedGpu>>
```
内部走 `invoke_json_with_credential`，与现有 host-only 变种 mirror。

---

## 5. NDJSON 事件协议（按域）

### 5.1 `ini scan`

```
{"kind":"started","task_type":"ini_scan","metadata":{"machines":N,"scan_run_id":<id>}}
{"kind":"finding","rule_id":"R001","severity":"critical","file_path":"...","section":"...","key":"..."}
... × M
{"kind":"completed","summary":{"scan_run_id":<id>,"machines":N,"findings":M,"critical":...,"warning":...,"healthy":...}}
```

### 5.2 `health run`

```
{"kind":"started","task_type":"health_run","metadata":{"machines":N,"scan_run_id":<id>}}
{"kind":"item_started","item_id":"<host>","index":i,"total":N}
{"kind":"item_completed","item_id":"<host>","index":i,"ok":true|false,"message":"..."}
... × N
{"kind":"completed","summary":{"scan_run_id":<id>,"machines":N,"ok":k,"failed":N-k}}
```

### 5.3 `ddc generate` / `pso collect`（UE runner 转发）

直接镜像 `core::ue_runner::UeRunnerEvent` enum：

```
{"kind":"started","task_type":"ddc_generate","metadata":{"project_id":..,"source_machine":...}}
{"kind":"spawned","pid":12345,"log_path":"..."}
{"kind":"log_line","text":"LogInit: Display: ...","parsed_kind":null}
{"kind":"progress","pct":0.15,"label":"compiling shaders"}
... 
{"kind":"completed","summary":{"exit_code":0,"output_path":"...","size_bytes":...}}
```

### 5.4 `ddc distribute` / `pso distribute`

NDJSON 同 `env set --hosts` 模式 — 每个 target machine 一个 `item_started` / `item_completed`，附 `bytes_copied` / `exit_code`。

---

## 6. 长跑任务取消（Plan 3 简化）

CLI 进程是一次性的：`ctrl-C` 直接终止 process，操作系统会清理 PS subprocess。Plan 3 不实现 `ddc cancel`，CLI binary 上的 `ddc cancel <job-id>` 命令 stub 返回 `not_implemented`，引导用户用 ctrl-C 或后续 daemon 模式。

实际上 spec args.rs 不暴露 `cancel` action — 删 `DdcAction::Cancel`，把对 `cancel_ue_job` 的 IPC 留给 UI。

---

## 7. needs_db 规则更新

所有 7 个新域都 `needs_db: true`（持久化 + 历史查询都要 DB）。`machine refresh` 已经是 true。

---

## 8. Redaction 契约（继承 Plan 2 §8）

- `ini apply` 写回 ini 的 `recommended_value` — 已经在 `ini_findings` 表里，handler 不需要 echo
- `ini set` / `env set` 的 redaction Plan 2 已经强制
- `health` / `gpu` / `project` / `ddc` / `pso` 都不接 secret value，无 redaction 需求

---

## 9. 验证标准

完成条件：

1. macOS 和 Windows 都能 `cargo build --release --bin uecm-cli`
2. 现有 211 lib tests + 6 integration tests 继续全绿，新增 unit / integration 测试全绿
3. `uecm-cli --help` 列 12 个顶层域
4. `ini scan` / `health run` / `pso collect` / `ddc generate` 在 lanPC 上端到端跑通（包括 NDJSON 流）
5. `machine refresh --cred-alias <a>` 在 lanPC 上跑通，证明 cred path 真的走 `_with_credential`
6. Codex adversarial review pass（spec + 关键 task 后）

---

## 10. 不做的事 (YAGNI)

- SIGINT graceful cancel
- Shell completion / MCP wrapper / daemon
- `share remove` 远端清理（Plan-4 future）
- `ddc cancel` / `pso cancel` 跨进程 — 一次性 CLI 用 ctrl-C
- 多机分布式 PSO collection
- AMD / Intel GPU E2E validation（spec 已经标 deferred）
