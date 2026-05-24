# CLI Spec P1 — Contract Manifest + 输出 Envelope 全量 + 移除 --pass Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 给 `uecm-cli` 加**完整** Contract Manifest（canonical `operation_id` + 每个操作带 `input_schema`/`output_schema`/`error_schema` + `uecm-cli manifest` 命令），让**所有域**的 JSON 输出套上 spec §4 的统一 envelope（成功/错误/ndjson 事件），`--output json` 与 `--output ndjson` 行为分离（json=单一对象，ndjson=流式），并移除经 argv 传密的 `--pass`。

**Architecture:** 三条主线——
1. **Envelope（不手改 219 个域 emit 点）**：operation_id / request_id / 起始时刻在 `run.rs` dispatch 处算好注入 emitter；`NdjsonEmitter`（流式）与新增 `JsonEmitter`（缓冲，单对象）都 envelope-aware。`emit_value`→SuccessEnvelope、`emit_error`→ErrorEnvelope、`emit_event`→事件信封。emitter trait 加 `finish()`，`JsonEmitter` 在 finish 时吐**恰好一个**最终 envelope（修 Codex #2）。
2. **完整 Contract Manifest（Codex #3 决策：补全）**：引入 `schemars`，给所有 operation 的输出类型派生 `JsonSchema`。manifest 运行时构建：`error_schema` 全局共享（`ErrorBody`）、`input_schema` 从 clap 命令树派生、`output_schema` 按 operation_id 取该操作的结果类型 schema（流式操作用共享 `Event` schema，ad-hoc json 输出用显式 object schema）。一致性测试在**任一 operation 缺三套 schema 之一时 fail**。
3. **usage error 信封统一（Codex #4）**：`bin/uecm-cli.rs` 的 clap parse 错误也输出 ErrorEnvelope 形状（exit 仍 64，仅改 JSON 结构），argv sniff 兼容 `--output`/`AI_AGENT`。

**Tech Stack:** Rust 2021、serde/serde_json、chrono 0.4、rand 0.8（均已依赖）、**schemars 0.8（新增依赖）**。

**Spec 基准：** `docs/CLI_DESIGN_SPEC.md` §2 / §3.5 / §4 / §9 / §10.1。

**前置依赖：** 建议先合 **P0**（`--output` 已就位）。

**当前事实（已核对源码）：**
- `src-tauri/src/cli/output.rs:10-77` — `Event` enum `#[serde(tag="kind", rename_all="snake_case")]`；`NdjsonEmitter`（`stream_started`/`stream_terminated`）；`HumanEmitter`；`EmitSerialize::emit_result<T>`。
- `src-tauri/src/cli/output.rs:84-113` — `error_code(&UecmError)->&str`、`exit_code_for(&UecmError)->i32`。
- `src-tauri/src/cli/run.rs:13-36` — `Ctx{db,db_path,emitter,json_mode}`；`run.rs:131-150` dispatch；`run.rs:161-170` `finish_error`。
- `src-tauri/src/cli/domain_system.rs:11-20` — `VersionInfo{binary,version}` / `PathInfo{path}`（**私有**，需改 pub）。`migrate_db`/`echo`/`schema`/`exit_codes` 输出是 ad-hoc `serde_json::Value`（无命名类型）。
- 已有 operation 标签可复用：`ddc_pak.generate/distribute`、`zen.*`。
- `UecmError` 9 变体（见 `error.rs`）。
- emit 点分布：zen 56 / ini 32 / machine 31 / health 16 / pso 14 / ddc 13 / project 10 / system 8 / env 8 / share 7 / winrm 5 / secret 5 / local_cache 5 / cred 4 / deploy 2 / ssh 1 / log 1 / gpu 1。
- `--pass`：`credential_args.rs:21-28`；`inline()`（72-87）内部复用 `pass` 字段。
- `clap_complete` 未引入（P2 引入）；`schemars` 未引入（本计划 Task 1 引入）。

---

## File Structure

- **Modify** `src-tauri/Cargo.toml` — 加 `schemars = "0.8"`。
- **Create** `src-tauri/src/cli/envelope.rs` — `Meta`/`SuccessEnvelope`/`ErrorBody`(JsonSchema)/`ErrorEnvelope`/`retryable_for`/`gen_request_id`/`now_iso8601`。
- **Create** `src-tauri/src/cli/manifest.rs` — `Operation`/`operations()`/`operation_id_for`/`error_schema`/`event_schema`/`output_schema_for`/`input_schema_for`/`manifest_json`。
- **Modify** `src-tauri/src/cli/output.rs` — `Event` tag `kind`→`type` + `#[derive(JsonSchema)]`；emitter trait 加 `finish`；`NdjsonEmitter` envelope-aware + `sequence`；新增 `JsonEmitter`。
- **Modify** `src-tauri/src/cli/run.rs` — `Ctx` 加 op_id/request_id；按格式选 emitter；dispatch 后 `finish`；`finish_error` 走 ErrorEnvelope。
- **Modify** `src-tauri/src/bin/uecm-cli.rs` — parse 错误走 ErrorEnvelope + AI_AGENT sniff。
- **Modify** `src-tauri/src/cli/args.rs` — `Domain::Manifest`；`credential_args.rs` 移除 `--pass`。
- **Modify** `src-tauri/src/cli/domain_system.rs` — `VersionInfo`/`PathInfo` 改 pub + 派生 JsonSchema。
- **Create** `docs/contract-manifest.json` — 快照（Task 10）。
- **Modify** `src-tauri/src/cli/mod.rs` — 注册新模块。

---

## Task 1: schemars 依赖 + Envelope 类型（含 JsonSchema）

**Files:**
- Modify: `src-tauri/Cargo.toml:42`（clap 行下）
- Create: `src-tauri/src/cli/envelope.rs`
- Modify: `src-tauri/src/cli/mod.rs`

- [ ] **Step 1: 加 schemars 依赖**

`src-tauri/Cargo.toml` 在 `clap = { version = "~4.5", ... }`（line 42）下加：

```toml
schemars = "0.8"
```

Run: `cargo build -p uecm 2>&1 | tail -3`
Expected: 拉取 schemars 成功，编译通过。

- [ ] **Step 2: 写 envelope 模块 + 测试**

新建 `src-tauri/src/cli/envelope.rs`：

```rust
//! Shared output envelope (spec §4). One-shot results -> SuccessEnvelope; failures
//! -> ErrorEnvelope; ndjson events get per-event metadata (see output.rs).
//! `ErrorBody` derives JsonSchema so it can serve as the manifest error_schema.

use crate::error::UecmError;
use schemars::JsonSchema;
use serde::Serialize;

pub const SCHEMA_VERSION: &str = "1.0";

#[derive(Debug, Serialize)]
pub struct Meta {
    pub request_id: String,
    pub duration_ms: u128,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct SuccessEnvelope<'a> {
    pub schema_version: &'static str,
    pub status: &'static str, // "ok"
    pub operation_id: &'a str,
    pub data: serde_json::Value,
    pub meta: Meta,
}

/// 错误信封的 `error` 体。**派生 JsonSchema** 作为 manifest 的共享 error_schema。
#[derive(Debug, Serialize, JsonSchema)]
pub struct ErrorBody {
    pub code: String,
    pub exit_code: i32,
    pub message: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    #[schemars(skip)]
    pub details: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct ErrorEnvelope<'a> {
    pub schema_version: &'static str,
    pub status: &'static str, // "error"
    pub operation_id: &'a str,
    pub error: ErrorBody,
    pub meta: Meta,
}

/// 瞬态故障可重试；参数/配置类不可重试（spec §4.2 retryable）。
pub fn retryable_for(err: &UecmError) -> bool {
    matches!(
        err,
        UecmError::Timeout(_)
            | UecmError::SshConnect(_)
            | UecmError::PowerShell(_)
            | UecmError::NodeScript { .. }
    )
}

/// uuid-v4 形态随机 request id（用已有 rand，避免引 uuid crate）。
pub fn gen_request_id() -> String {
    use rand::RngCore;
    let mut b = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0],b[1],b[2],b[3],b[4],b[5],b[6],b[7],b[8],b[9],b[10],b[11],b[12],b[13],b[14],b[15]
    )
}

/// 当前 UTC ISO-8601 时间戳（spec §4.5）。
pub fn now_iso8601() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_id_is_uuid_v4_shaped() {
        let id = gen_request_id();
        assert_eq!(id.len(), 36);
        assert_eq!(id.as_bytes()[14], b'4');
        assert_eq!(id.matches('-').count(), 4);
    }

    #[test]
    fn retryable_classification() {
        assert!(retryable_for(&UecmError::Timeout("t".into())));
        assert!(!retryable_for(&UecmError::InvalidInput("x".into())));
    }

    #[test]
    fn timestamp_is_utc_z() {
        assert!(now_iso8601().ends_with('Z'));
    }

    #[test]
    fn error_body_has_schema() {
        let s = serde_json::to_value(schemars::schema_for!(ErrorBody)).unwrap();
        assert!(s["properties"]["code"].is_object());
        assert!(s["properties"]["exit_code"].is_object());
        assert!(s["properties"]["retryable"].is_object());
    }
}
```

- [ ] **Step 3: 注册 + 运行测试**

`src-tauri/src/cli/mod.rs` 加 `pub mod envelope;`。

Run: `cargo test -p uecm --lib cli::envelope 2>&1 | tail -12`
Expected: PASS — 4 测试绿。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/cli/envelope.rs src-tauri/src/cli/mod.rs
git commit -m "feat(cli): schemars dep + envelope types with JsonSchema error body (spec §4)"
```

---

## Task 2: Contract Manifest 模块（运行时构建 + 三套 schema）

**Files:**
- Create: `src-tauri/src/cli/manifest.rs`
- Modify: `src-tauri/src/cli/mod.rs`
- Modify: `src-tauri/src/cli/domain_system.rs:11-20`（`VersionInfo`/`PathInfo` 改 pub + JsonSchema）

**operation_id 约定：`<domain>.<action_snake_case>`。** 本 Task 落地 infra + `system`/`machine` 全量 action（含输出 schema），其余域 Task 6 补。

- [ ] **Step 1: 让 system 输出类型可被 schema**

`src-tauri/src/cli/domain_system.rs` 把 `VersionInfo`/`PathInfo`（line 11-20）改为 pub + 派生 JsonSchema：

```rust
#[derive(Serialize, schemars::JsonSchema)]
pub struct VersionInfo {
    pub binary: &'static str,
    pub version: &'static str,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PathInfo {
    pub path: String,
}
```
（同步把 `version()`/`db_path()`/`ps_dir()` 里构造时的字段访问改为 pub 字段——它们本就同模块，无需改调用，仅 struct 头加 `pub`。）

- [ ] **Step 2: 写 manifest 模块骨架 + 失败测试**

新建 `src-tauri/src/cli/manifest.rs`：

```rust
//! Contract Manifest (spec §2). Canonical operation_id registry; every operation
//! carries input_schema (from clap), output_schema (per-result type), and a shared
//! error_schema. Built at runtime so output_schema can call schema_for!.

use crate::cli::args::Domain;
use schemars::schema_for;

#[derive(Debug, Clone, Copy)]
pub struct SideEffects {
    pub writes: bool,
    pub external_calls: bool,
    pub idempotent: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Operation {
    pub operation_id: &'static str,
    pub summary: &'static str,
    pub cli_command: &'static str,
    pub side_effects: SideEffects,
    pub exit_codes: &'static [i32],
}

/// 静态操作表（不含 schema；schema 在 manifest_json 运行时拼装）。Task 6 补其余域。
pub fn operations() -> &'static [Operation] {
    const OPS: &[Operation] = &[
        Operation { operation_id: "system.version",    summary: "Print binary + library version",         cli_command: "uecm-cli system version",    side_effects: SideEffects{writes:false,external_calls:false,idempotent:true}, exit_codes: &[0] },
        Operation { operation_id: "system.db_path",    summary: "Print resolved SQLite DB path",           cli_command: "uecm-cli system db-path",    side_effects: SideEffects{writes:false,external_calls:false,idempotent:true}, exit_codes: &[0,3] },
        Operation { operation_id: "system.ps_dir",     summary: "Print resolved ps-scripts dir",           cli_command: "uecm-cli system ps-dir",     side_effects: SideEffects{writes:false,external_calls:false,idempotent:true}, exit_codes: &[0] },
        Operation { operation_id: "system.migrate_db", summary: "Force-run schema migrations",             cli_command: "uecm-cli system migrate-db", side_effects: SideEffects{writes:true, external_calls:false,idempotent:true}, exit_codes: &[0,3] },
        Operation { operation_id: "system.echo",       summary: "Round-trip a message via PowerShell",     cli_command: "uecm-cli system echo",       side_effects: SideEffects{writes:false,external_calls:true, idempotent:true}, exit_codes: &[0,4] },
        Operation { operation_id: "system.schema",     summary: "Dump clap command tree as JSON",          cli_command: "uecm-cli system schema",     side_effects: SideEffects{writes:false,external_calls:false,idempotent:true}, exit_codes: &[0] },
        Operation { operation_id: "system.exit_codes", summary: "Print documented exit-code table",        cli_command: "uecm-cli system exit-codes", side_effects: SideEffects{writes:false,external_calls:false,idempotent:true}, exit_codes: &[0] },
        Operation { operation_id: "machine.list",      summary: "List all known machines",                 cli_command: "uecm-cli machine list",      side_effects: SideEffects{writes:false,external_calls:false,idempotent:true}, exit_codes: &[0,3] },
        Operation { operation_id: "machine.scan",      summary: "Probe a CIDR for live hosts",             cli_command: "uecm-cli machine scan",      side_effects: SideEffects{writes:false,external_calls:true, idempotent:true}, exit_codes: &[0,2] },
        Operation { operation_id: "machine.add",       summary: "Add a machine to inventory",              cli_command: "uecm-cli machine add",       side_effects: SideEffects{writes:true, external_calls:false,idempotent:false}, exit_codes: &[0,2,3] },
        Operation { operation_id: "machine.refresh",   summary: "Refresh a machine (probe + detect)",      cli_command: "uecm-cli machine refresh",   side_effects: SideEffects{writes:true, external_calls:true, idempotent:true}, exit_codes: &[0,2,3,4] },
        Operation { operation_id: "machine.detail",    summary: "Show machine detail",                     cli_command: "uecm-cli machine detail",    side_effects: SideEffects{writes:false,external_calls:false,idempotent:true}, exit_codes: &[0,2,5] },
        Operation { operation_id: "machine.delete",    summary: "Delete machine(s)",                       cli_command: "uecm-cli machine delete",    side_effects: SideEffects{writes:true, external_calls:false,idempotent:true}, exit_codes: &[0,2] },
        Operation { operation_id: "machine.rename",    summary: "Rename a machine",                        cli_command: "uecm-cli machine rename",    side_effects: SideEffects{writes:true, external_calls:false,idempotent:true}, exit_codes: &[0,2] },
        Operation { operation_id: "machine.deep_scan", summary: "Refresh + INI scan + health per machine", cli_command: "uecm-cli machine deep-scan", side_effects: SideEffects{writes:true, external_calls:true, idempotent:true}, exit_codes: &[0,2,3,4] },
        Operation { operation_id: "machine.authorize", summary: "Authorize machines for remote mgmt",      cli_command: "uecm-cli machine authorize", side_effects: SideEffects{writes:true, external_calls:true, idempotent:true}, exit_codes: &[0,2,4] },
    ];
    OPS
}

pub fn operation_id_for(cmd: &Domain) -> &'static str {
    use crate::cli::args::{MachineAction, SystemAction};
    match cmd {
        Domain::System { action } => match action {
            SystemAction::Version => "system.version",
            SystemAction::DbPath => "system.db_path",
            SystemAction::PsDir => "system.ps_dir",
            SystemAction::MigrateDb => "system.migrate_db",
            SystemAction::Echo { .. } => "system.echo",
            SystemAction::Schema => "system.schema",
            SystemAction::ExitCodes => "system.exit_codes",
        },
        Domain::Machine { action } => match action {
            MachineAction::List => "machine.list",
            MachineAction::Scan { .. } => "machine.scan",
            MachineAction::Add { .. } => "machine.add",
            MachineAction::Refresh { .. } => "machine.refresh",
            MachineAction::Detail { .. } => "machine.detail",
            MachineAction::Delete { .. } => "machine.delete",
            MachineAction::Rename { .. } => "machine.rename",
            MachineAction::DeepScan { .. } => "machine.deep_scan",
            MachineAction::Authorize { .. } => "machine.authorize",
            // 不加 `_ =>`：保持穷尽，新增变体编译器强制来补。执行方先
            // `grep -n "pub enum MachineAction" -A60 src-tauri/src/cli/args.rs` 读全。
        },
        // Task 6 在此追加其余 16 域。临时兜底（§2.3 / schema 完整性测试会抓到）：
        Domain::Winrm { .. } => "winrm.unmapped",
        Domain::Ssh { .. } => "ssh.unmapped",
        Domain::Cred { .. } => "cred.unmapped",
        Domain::Secret { .. } => "secret.unmapped",
        Domain::Env { .. } => "env.unmapped",
        Domain::Ini { .. } => "ini.unmapped",
        Domain::Share { .. } => "share.unmapped",
        Domain::Project { .. } => "project.unmapped",
        Domain::Health { .. } => "health.unmapped",
        Domain::Gpu { .. } => "gpu.unmapped",
        Domain::Ddc { .. } => "ddc.unmapped",
        Domain::Pso { .. } => "pso.unmapped",
        Domain::Log { .. } => "log.unmapped",
        Domain::LocalCache { .. } => "localcache.unmapped",
        Domain::Deploy { .. } => "deploy.unmapped",
        Domain::Zen { .. } => "zen.unmapped",
    }
}

/// 共享错误 schema（所有 operation 的 error_schema 都是这个）。
pub fn error_schema() -> serde_json::Value {
    serde_json::to_value(schema_for!(crate::cli::envelope::ErrorBody)).unwrap()
}

/// 流式操作（emit_event 序列）的共享输出 schema。
pub fn event_schema() -> serde_json::Value {
    serde_json::to_value(schema_for!(crate::cli::output::Event)).unwrap()
}

/// ad-hoc `serde_json::Value` 输出（无命名类型）用的宽 object schema。
fn dynamic_object_schema() -> serde_json::Value {
    serde_json::json!({ "type": "object", "additionalProperties": true })
}

/// 每个操作的输出（`data`）schema。typed 结果用 schema_for!；流式用 event_schema；
/// ad-hoc json 用 dynamic_object_schema。Task 6 为其余域补 match 臂。
pub fn output_schema_for(operation_id: &str) -> serde_json::Value {
    match operation_id {
        "system.version" => serde_json::to_value(schema_for!(crate::cli::domain_system::VersionInfo)).unwrap(),
        "system.db_path" | "system.ps_dir" => serde_json::to_value(schema_for!(crate::cli::domain_system::PathInfo)).unwrap(),
        "system.migrate_db" | "system.echo" | "system.schema" | "system.exit_codes" => dynamic_object_schema(),
        "machine.scan" | "machine.deep_scan" | "machine.refresh" | "machine.authorize" => event_schema(),
        // machine.list/add/detail/delete/rename 等返回 ad-hoc json（Task 6 可换成命名类型）：
        s if s.starts_with("machine.") => dynamic_object_schema(),
        // Task 6 之前其余域走兜底（schema 完整性测试会盯着 unmapped 不放）：
        _ => dynamic_object_schema(),
    }
}

/// 从 clap 命令树为某操作派生 input_schema（参数 -> JSON Schema properties）。
pub fn input_schema_for(cli_command: &str) -> serde_json::Value {
    use clap::CommandFactory;
    let parts: Vec<&str> = cli_command.split_whitespace().skip(1).collect(); // drop "uecm-cli"
    let root = crate::cli::args::Cli::command();
    let mut current: &clap::Command = &root;
    for p in &parts {
        match current.find_subcommand(p) {
            Some(sub) => current = sub,
            None => return dynamic_object_schema(),
        }
    }
    let mut props = serde_json::Map::new();
    let mut required = Vec::new();
    for arg in current.get_arguments() {
        let id = arg.get_id().as_str();
        if id == "help" || id == "version" {
            continue;
        }
        let ty = if arg.get_action().takes_values() { "string" } else { "boolean" };
        props.insert(id.to_string(), serde_json::json!({ "type": ty }));
        if arg.is_required_set() {
            required.push(serde_json::json!(id));
        }
    }
    serde_json::json!({
        "type": "object",
        "properties": props,
        "required": required,
        "additionalProperties": false
    })
}

/// 渲染 spec §2.1 完整 manifest 文档。
pub fn manifest_json() -> serde_json::Value {
    let err = error_schema();
    let ops: Vec<serde_json::Value> = operations()
        .iter()
        .map(|op| {
            serde_json::json!({
                "operation_id": op.operation_id,
                "summary": op.summary,
                "input_schema": input_schema_for(op.cli_command),
                "output_schema": output_schema_for(op.operation_id),
                "error_schema": err,
                "side_effects": {
                    "writes": op.side_effects.writes,
                    "external_calls": op.side_effects.external_calls,
                    "idempotent": op.side_effects.idempotent,
                },
                "exit_codes": op.exit_codes,
                "cli": { "command": op.cli_command }
            })
        })
        .collect();
    serde_json::json!({ "contract_version": "1.0", "operations": ops })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_ids_are_unique() {
        let mut ids: Vec<&str> = operations().iter().map(|o| o.operation_id).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, ids.len(), "duplicate operation_id");
    }

    #[test]
    fn manifest_has_three_schemas_per_op() {
        let m = manifest_json();
        for op in m["operations"].as_array().unwrap() {
            let id = op["operation_id"].as_str().unwrap();
            assert!(op["input_schema"].is_object(), "{id} missing input_schema");
            assert!(op["output_schema"].is_object(), "{id} missing output_schema");
            assert!(op["error_schema"].is_object(), "{id} missing error_schema");
        }
    }
}
```

- [ ] **Step 3: 注册 + 运行测试**

`src-tauri/src/cli/mod.rs` 加 `pub mod manifest;`。

Run: `cargo test -p uecm --lib cli::manifest 2>&1 | tail -14`
Expected: PASS。若 `MachineAction` 有未覆盖变体，按注释补 + OPS 加行。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/cli/manifest.rs src-tauri/src/cli/mod.rs src-tauri/src/cli/domain_system.rs
git commit -m "feat(cli): Contract Manifest with input/output/error schemas (spec §2.1)"
```

---

## Task 3: `uecm-cli manifest` 命令

**Files:**
- Modify: `src-tauri/src/cli/args.rs`（`Domain` 加 `Manifest`）
- Modify: `src-tauri/src/cli/run.rs`（dispatch + needs_db）

- [ ] **Step 1: 写失败测试**

在 `manifest.rs` tests 加：

```rust
#[test]
fn manifest_command_parses() {
    use crate::cli::args::{Cli, Domain};
    use clap::Parser;
    let cli = Cli::try_parse_from(["uecm-cli", "manifest"]).unwrap();
    assert!(matches!(cli.command, Domain::Manifest));
}
```

- [ ] **Step 2: 确认失败**

Run: `cargo test -p uecm --lib cli::manifest::tests::manifest_command_parses 2>&1 | head -15`
Expected: FAIL — `Domain::Manifest` 不存在。

- [ ] **Step 3: 加 Domain::Manifest**

`src-tauri/src/cli/args.rs` 在 `Domain` enum 末尾（`Zen{...}` 之后）加：

```rust
    /// Print the Contract Manifest (canonical operation registry + schemas; spec §2 / §10.1).
    Manifest,
```

- [ ] **Step 4: dispatch + needs_db**

`src-tauri/src/cli/run.rs`：
1. `needs_db` match 加：`Domain::Manifest => false,`。
2. dispatch match 加（`Domain::Zen` 之后）：

```rust
        Domain::Manifest => {
            ctx.emitter.emit_value(&crate::cli::manifest::manifest_json()).ok();
            Ok(())
        }
```

- [ ] **Step 5: 测试 + Commit**

```bash
cargo test -p uecm --lib cli::manifest 2>&1 | tail -8
git add src-tauri/src/cli/args.rs src-tauri/src/cli/run.rs
git commit -m "feat(cli): add 'uecm-cli manifest' command (spec §10.1)"
```

---

## Task 4: Ctx 注入 operation_id / request_id / 起始时刻

**Files:**
- Modify: `src-tauri/src/cli/run.rs:13-36`、构造处
- Modify: 所有测试里构造 `Ctx{}` 的地方

- [ ] **Step 1: Ctx 加字段**

`run.rs` `pub struct Ctx<'a>` 改为：

```rust
pub struct Ctx<'a> {
    pub db: Option<Db>,
    pub db_path: PathBuf,
    pub emitter: Box<dyn Emitter + 'a>,
    pub json_mode: bool,
    pub operation_id: &'static str,
    pub request_id: String,
}
```

- [ ] **Step 2: 构造时计算**

`run.rs` 在 emitter 构造之前加：

```rust
    let operation_id = crate::cli::manifest::operation_id_for(&cli.command);
    let request_id = crate::cli::envelope::gen_request_id();
    let started = std::time::Instant::now();
```

`let mut ctx = Ctx { ... }` 补 `operation_id, request_id: request_id.clone()`。`started` 留给 Task 5 传 emitter。

- [ ] **Step 3: 补所有测试构造点**

Run: `grep -rn "Ctx {" src-tauri/src/cli`
对每处（如 `domain_system.rs` 的 `echo_returns_powershell_error_on_non_windows`）补 `operation_id: "system.echo", request_id: "test-req".into(),`。

- [ ] **Step 4: 测试 + Commit**

```bash
cargo test -p uecm --lib cli 2>&1 | tail -10
git add src-tauri/src/cli/run.rs src-tauri/src/cli/domain_system.rs
git commit -m "feat(cli): thread operation_id + request_id through Ctx (spec §2.2/§4.1)"
```

---

## Task 5: Envelope-aware emitter + JsonEmitter（修 Codex #2：json 单对象 vs ndjson 流式）

**这是 P1 核心。** trait 加 `finish()`；`Event` tag `kind`→`type` + JsonSchema；`NdjsonEmitter` 流式包信封；新增 `JsonEmitter` 缓冲为**单一**最终 envelope。

**Files:**
- Modify: `src-tauri/src/cli/output.rs`
- Modify: `src-tauri/src/cli/run.rs`

- [ ] **Step 1: 写失败测试**

在 `output.rs` tests 加：

```rust
    fn env_ctx() -> EnvelopeCtx {
        EnvelopeCtx { operation_id: "system.version".into(), request_id: "rq".into(), started: std::time::Instant::now() }
    }

    #[test]
    fn ndjson_stream_events_carry_type_sequence_and_final() {
        let mut buf = Vec::new();
        {
            let mut e = NdjsonEmitter::new(&mut buf).with_envelope(env_ctx());
            e.emit_event(&Event::HostProbe{ ip:"1.1.1.1".into(), winrm_open:true, smb_open:false, rpc_open:false }).unwrap();
            e.emit_event(&Event::Completed{ summary: serde_json::json!({"n":1}) }).unwrap();
            e.finish().unwrap();
        }
        let s = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = s.trim_end().split('\n').collect();
        let l0: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(l0["type"], "host_probe");
        assert_eq!(l0["sequence"], 0);
        assert_eq!(l0["request_id"], "rq");
        let l1: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(l1["type"], "completed");
        assert_eq!(l1["final"], true);
    }

    #[test]
    fn json_emitter_one_shot_is_single_success_envelope() {
        let mut buf = Vec::new();
        {
            let mut e = JsonEmitter::new(&mut buf, env_ctx());
            e.emit_value(&serde_json::json!({"version":"0.1.0"})).unwrap();
            e.finish().unwrap();
        }
        // 必须恰好一个 JSON 对象（整 buf 可被 jq/from_slice 一次解析）
        let v: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(v["status"], "ok");
        assert_eq!(v["operation_id"], "system.version");
        assert_eq!(v["data"]["version"], "0.1.0");
    }

    #[test]
    fn json_emitter_stream_collapses_to_single_object() {
        let mut buf = Vec::new();
        {
            let mut e = JsonEmitter::new(&mut buf, env_ctx());
            e.emit_event(&Event::HostProbe{ ip:"1.1.1.1".into(), winrm_open:true, smb_open:true, rpc_open:true }).unwrap();
            e.emit_event(&Event::Completed{ summary: serde_json::json!({"n":1}) }).unwrap();
            e.finish().unwrap();
        }
        // 仍是恰好一个对象；流事件收进 data.events
        let v: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(v["status"], "ok");
        assert_eq!(v["data"]["events"].as_array().unwrap().len(), 2);
    }
```

> 同时把既有 `ndjson_emits_one_line_per_event` 里 `parsed["kind"]` 改为 `parsed["type"]`。

- [ ] **Step 2: 确认失败**

Run: `cargo test -p uecm --lib cli::output 2>&1 | head -20`
Expected: FAIL — `EnvelopeCtx`/`with_envelope`/`JsonEmitter`/`finish` 未定义。

- [ ] **Step 3: 改 output.rs**

3a. `Event` 改 tag + 派生 JsonSchema。line 10-11：

```rust
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
```

3b. trait `Emitter` 加 `finish`（默认实现 no-op，便于 Human/legacy）：

```rust
pub trait Emitter {
    fn emit_event(&mut self, event: &Event) -> io::Result<()>;
    fn emit_value(&mut self, value: &serde_json::Value) -> io::Result<()>;
    fn emit_error(&mut self, err: &UecmError);
    /// 终结输出。JsonEmitter 在此吐出恰好一个 envelope；其它实现 no-op。
    fn finish(&mut self) -> io::Result<()> { Ok(()) }
}
```

3c. 加 `EnvelopeCtx` + 终止判断（在 trait 之前）：

```rust
use crate::cli::envelope::{ErrorBody, ErrorEnvelope, Meta, SuccessEnvelope, SCHEMA_VERSION};

pub struct EnvelopeCtx {
    pub operation_id: String,
    pub request_id: String,
    pub started: std::time::Instant,
}
impl EnvelopeCtx {
    fn meta(&self) -> Meta {
        Meta { request_id: self.request_id.clone(), duration_ms: self.started.elapsed().as_millis(), timestamp: crate::cli::envelope::now_iso8601() }
    }
    fn success(&self, data: serde_json::Value) -> serde_json::Value {
        serde_json::to_value(SuccessEnvelope { schema_version: SCHEMA_VERSION, status: "ok", operation_id: &self.operation_id, data, meta: self.meta() }).unwrap_or(serde_json::Value::Null)
    }
    fn error(&self, err: &UecmError) -> serde_json::Value {
        let body = ErrorBody { code: error_code(err).into(), exit_code: exit_code_for(err), message: err.to_string(), retryable: crate::cli::envelope::retryable_for(err), details: serde_json::Value::Null };
        serde_json::to_value(ErrorEnvelope { schema_version: SCHEMA_VERSION, status: "error", operation_id: &self.operation_id, error: body, meta: self.meta() }).unwrap_or(serde_json::Value::Null)
    }
}
fn is_terminal(event: &Event) -> bool {
    matches!(event, Event::Completed{..} | Event::Cancelled{..} | Event::Error{..})
}
```

3d. `NdjsonEmitter`：加字段 `envelope: Option<EnvelopeCtx>` + `sequence: u64`（两个构造器初始化 `envelope: None, sequence: 0`），加 builder：

```rust
impl<W: Write, E: Write> NdjsonEmitter<W, E> {
    pub fn with_envelope(mut self, ctx: EnvelopeCtx) -> Self { self.envelope = Some(ctx); self }
}
```

`emit_value`（流式：一次 SuccessEnvelope）：

```rust
    fn emit_value(&mut self, value: &serde_json::Value) -> io::Result<()> {
        let payload = match &self.envelope { Some(c) => c.success(value.clone()), None => value.clone() };
        serde_json::to_writer(&mut self.writer, &payload)?;
        self.writer.write_all(b"\n")?; self.writer.flush()
    }
```

`emit_event`（加元数据）：

```rust
    fn emit_event(&mut self, event: &Event) -> io::Result<()> {
        let mut obj = serde_json::to_value(event)?;
        if let (Some(c), Some(map)) = (&self.envelope, obj.as_object_mut()) {
            map.insert("sequence".into(), serde_json::json!(self.sequence));
            map.insert("timestamp".into(), serde_json::json!(crate::cli::envelope::now_iso8601()));
            map.insert("request_id".into(), serde_json::json!(c.request_id));
            map.insert("schema_version".into(), serde_json::json!(SCHEMA_VERSION));
            if is_terminal(event) { map.insert("final".into(), serde_json::json!(true)); }
        }
        self.sequence += 1;
        serde_json::to_writer(&mut self.writer, &obj)?;
        self.writer.write_all(b"\n")?; self.writer.flush()?;
        self.stream_started = true;
        if is_terminal(event) { self.stream_terminated = true; }
        Ok(())
    }
```

`emit_error`（保留 stdout terminator 段不动；stderr 段换成 ErrorEnvelope）：把原 `let envelope = Event::Error{...}; ... error_writer ...`（line 226-234）替换为：

```rust
        let env = match &self.envelope {
            Some(c) => c.error(err),
            None => serde_json::json!({ "type":"error", "code": error_code(err), "message": err.to_string() }),
        };
        if serde_json::to_writer(&mut self.error_writer, &env).is_ok() {
            let _ = self.error_writer.write_all(b"\n"); let _ = self.error_writer.flush();
        }
```

3e. 新增 `JsonEmitter`（缓冲 → 单一 envelope）。加到 `HumanEmitter` 之后：

```rust
/// 单对象 JSON 输出（spec §3.5 的 `json`）。缓冲所有 emit，在 finish 吐出恰好一个
/// envelope；流事件收进 data.events，确保 `--output json` 永远是一个可被 jq 解析的对象。
pub struct JsonEmitter<W: Write, E: Write = io::Stderr> {
    writer: W,
    error_writer: E,
    envelope: EnvelopeCtx,
    data: Option<serde_json::Value>,
    events: Vec<serde_json::Value>,
    errored: bool,
    finished: bool,
}

impl<W: Write> JsonEmitter<W, io::Stderr> {
    pub fn new(writer: W, envelope: EnvelopeCtx) -> Self {
        Self { writer, error_writer: io::stderr(), envelope, data: None, events: Vec::new(), errored: false, finished: false }
    }
}

impl<W: Write, E: Write> Emitter for JsonEmitter<W, E> {
    fn emit_event(&mut self, event: &Event) -> io::Result<()> {
        self.events.push(serde_json::to_value(event)?);
        Ok(())
    }
    fn emit_value(&mut self, value: &serde_json::Value) -> io::Result<()> {
        self.data = Some(value.clone());
        Ok(())
    }
    fn emit_error(&mut self, err: &UecmError) {
        self.errored = true;
        let env = self.envelope.error(err);
        if serde_json::to_writer(&mut self.error_writer, &env).is_ok() {
            let _ = self.error_writer.write_all(b"\n"); let _ = self.error_writer.flush();
        }
    }
    fn finish(&mut self) -> io::Result<()> {
        if self.finished { return Ok(()); }
        self.finished = true;
        if self.errored { return Ok(()); } // 错误已发 stderr，stdout 不发成功体
        let data = match self.data.take() {
            Some(v) => v,
            // 什么都没 emit（如 `system completion` 直写裸 shell 脚本到 stdout，
            // 绕过 emitter）-> finish no-op，避免在裸输出后再吐一个空 envelope 污染 stdout。
            None if self.events.is_empty() => return Ok(()),
            None => serde_json::json!({ "events": std::mem::take(&mut self.events) }),
        };
        let payload = self.envelope.success(data);
        serde_json::to_writer(&mut self.writer, &payload)?;
        self.writer.write_all(b"\n")?; self.writer.flush()
    }
}
```

> 注：`error_code` / `exit_code_for` 已在本文件，`EnvelopeCtx::error/success` 调它们。`JsonEmitter` 的 `error_writer` 复用 stderr。

- [ ] **Step 4: run.rs 按格式选 emitter + dispatch 后 finish**

`run.rs` emitter `match fmt`（P0 版）改为：

```rust
    let emitter: Box<dyn Emitter> = match fmt {
        OutputFormat::Text => {
            let color = crate::cli::args::use_color(cli.no_color, atty::is(atty::Stream::Stdout), std::env::var_os("NO_COLOR").is_some());
            Box::new(HumanEmitter::new(stdout.lock(), stderr.lock(), color))
        }
        OutputFormat::Ndjson => {
            let env = crate::cli::output::EnvelopeCtx { operation_id: operation_id.to_string(), request_id: request_id.clone(), started };
            Box::new(NdjsonEmitter::new(stdout.lock()).with_envelope(env))
        }
        OutputFormat::Json => {
            let env = crate::cli::output::EnvelopeCtx { operation_id: operation_id.to_string(), request_id: request_id.clone(), started };
            Box::new(crate::cli::output::JsonEmitter::new(stdout.lock(), env))
        }
    };
```

dispatch（`match result { Ok(())=>0, ... }`，line 152-158）改为在返回前 finish：

```rust
    let code = match result {
        Ok(()) => { let _ = ctx.emitter.finish(); 0 }
        Err(e) => {
            ctx.emitter.emit_error(&e);
            let _ = ctx.emitter.finish();
            exit_code_for(&e)
        }
    };
    code
```

- [ ] **Step 5: 测试**

Run: `cargo test -p uecm --lib cli::output 2>&1 | tail -16`
Expected: PASS — 新增 3 测试 + 改名旧测试全绿。

- [ ] **Step 6: 全量回归 + 实测 json vs ndjson**

Run:
```bash
cargo test -p uecm --lib 2>&1 | tail -12
cargo build -p uecm --bin uecm-cli 2>&1 | tail -2
B=src-tauri/target/debug/uecm-cli
echo "--- json: 单对象 ---"; NO_COLOR=1 $B system version --output json 2>/dev/null | jq '{status,operation_id,data}'
echo "--- ndjson 流式命令仍逐行（mac 可跑 scan，连不通也会有 started/completed）---"
NO_COLOR=1 $B machine scan 127.0.0.1/32 --output ndjson 2>/dev/null | jq -c '{type,sequence}' | head
echo "--- 同一流式命令 json 模式：必须单对象 ---"
NO_COLOR=1 $B machine scan 127.0.0.1/32 --output json 2>/dev/null | jq '{status, n: (.data.events|length)}'
```
Expected: json 模式两条都是**单个**可解析对象；ndjson 模式逐行带 `type`/`sequence`。

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/cli/output.rs src-tauri/src/cli/run.rs
git commit -m "feat(cli): envelope emitters; JsonEmitter single-object vs NdjsonEmitter stream (spec §3.5/§4)"
```

---

## Task 6: 逐域补 operation_id 映射 + output_schema（machine 之外 16 域）

**机械重复任务。** 每域 sub-task，pattern 相同：

1. 读该域 action 枚举：`grep -n "pub enum <X>Action" -A40 src-tauri/src/cli/args.rs`。
2. `operation_id_for` 把该域临时兜底臂替换为对 action 的穷尽 `match`，映射 `"<domain>.<action_snake_case>"`（已有标签如 `ddc.generate`/`zen.enable` 沿用既有字符串，与 DB operations 表一致）。
3. `operations()` 的 `OPS` 表为每个 action 加一行。
4. `output_schema_for` 为该域加 match 臂：
   - 输出是命名 Serialize struct → 给该 struct（在其定义文件）加 `#[derive(schemars::JsonSchema)]` 并设 `pub`，再 `schema_for!(path::To::Type)`；
   - 输出是 emit_event 流 → `event_schema()`；
   - 输出是 ad-hoc `serde_json::Value` → `dynamic_object_schema()`（并在 commit message 标注该 op 输出为动态）。
5. 跑 `cargo test -p uecm --lib cli::manifest`（唯一性 + 三套 schema 完整性）。

**域顺序（emit 量小→大）：** gpu → log → ssh → deploy → cred → local_cache → secret → winrm → share → env → project → ddc → pso → health → ini → zen。

- [ ] **Step 1: 完整样例 — env 域**

`operation_id_for` 把 `Domain::Env { .. } => "env.unmapped",` 替换为（按 `EnvAction` 实际变体）：

```rust
        Domain::Env { action } => match action {
            crate::cli::args::EnvAction::Get { .. } => "env.get",
            crate::cli::args::EnvAction::Set { .. } => "env.set",
            crate::cli::args::EnvAction::List { .. } => "env.list",
        },
```

`operations()` OPS 加：

```rust
        Operation { operation_id: "env.get",  summary: "Read a remote env var",  cli_command: "uecm-cli env get",  side_effects: SideEffects{writes:false,external_calls:true,idempotent:true},  exit_codes: &[0,2,4] },
        Operation { operation_id: "env.set",  summary: "Write a remote env var",  cli_command: "uecm-cli env set",  side_effects: SideEffects{writes:true, external_calls:true,idempotent:true},  exit_codes: &[0,2,4] },
        Operation { operation_id: "env.list", summary: "List remote env vars",    cli_command: "uecm-cli env list", side_effects: SideEffects{writes:false,external_calls:true,idempotent:true},  exit_codes: &[0,2,4] },
```

`output_schema_for` 加（假设 env 输出为 ad-hoc json；若有命名类型按上面 4.1 处理）：

```rust
        s if s.starts_with("env.") => dynamic_object_schema(),
```

- [ ] **Step 2: 运行该域测试**

Run: `cargo test -p uecm --lib cli::manifest 2>&1 | tail -8`
Expected: PASS（穷尽 + 唯一 + 三套 schema 齐）。

- [ ] **Step 3: 重复其余 15 域**

对每域重复 Step 1-2。每 3-4 域 commit：

```bash
git add src-tauri/src/cli/manifest.rs src-tauri/src/cli/domain_*.rs
git commit -m "feat(cli): operation_id + output_schema for <domains> (spec §2)"
```

- [ ] **Step 4: 清零 unmapped**

Run: `grep -n "unmapped" src-tauri/src/cli/manifest.rs || echo "no unmapped (good)"`
Expected: 无输出。`output_schema_for` 末尾 `_ => dynamic_object_schema()` 兜底可保留（防御），但不应再有 `*.unmapped` 字符串。

- [ ] **Step 5: 全量 + Commit**

```bash
cargo test -p uecm --lib 2>&1 | tail -8
git add -A src-tauri/src/cli
git commit -m "feat(cli): complete operation_id + output_schema mapping for all domains (spec §2)"
```

---

## Task 7: 一致性 + schema 完整性哨兵（spec §2.3，修 Codex #3）

**Files:**
- Modify: `src-tauri/src/cli/manifest.rs`（tests）

- [ ] **Step 1: 写哨兵测试**

在 `manifest.rs` tests 加：

```rust
#[test]
fn every_operation_id_well_formed_and_known_domain() {
    let known = ["system","machine","winrm","ssh","cred","secret","env","ini","share","project","health","gpu","ddc","pso","log","localcache","deploy","zen"];
    for op in operations() {
        let domain = op.operation_id.split('.').next().unwrap();
        assert!(known.contains(&domain), "unknown domain in {}", op.operation_id);
        assert!(op.operation_id.contains('.'), "id must be <domain>.<action>: {}", op.operation_id);
    }
}

#[test]
fn no_operation_has_empty_or_unmapped_schema() {
    let m = manifest_json();
    for op in m["operations"].as_array().unwrap() {
        let id = op["operation_id"].as_str().unwrap();
        assert!(!id.contains("unmapped"), "{id} still unmapped");
        for key in ["input_schema","output_schema","error_schema"] {
            let s = &op[key];
            assert!(s.is_object(), "{id}.{key} missing");
            // schema 必须至少有 type 或 $ref 或 properties，避免空对象冒充
            assert!(s.get("type").is_some() || s.get("$ref").is_some() || s.get("properties").is_some(),
                "{id}.{key} is an empty/invalid schema");
        }
    }
}

#[test]
fn operation_count_covers_command_leaves() {
    use clap::CommandFactory;
    let cmd = crate::cli::args::Cli::command();
    let mut leaves = 0usize;
    for sub in cmd.get_subcommands() {
        if sub.get_name() == "help" { continue; }
        let inner = sub.get_subcommands().filter(|s| s.get_name() != "help").count();
        leaves += if inner == 0 { 1 } else { inner };
    }
    // manifest 是元命令，不在 operations() 表里 -> 允许差 1。跑一次看真实 leaves 再锁断言。
    assert!(operations().len() + 1 >= leaves && operations().len() <= leaves,
        "manifest ops ({}) drift from CLI leaves ({})", operations().len(), leaves);
}
```

- [ ] **Step 2: 运行 + 锁阈值**

Run: `cargo test -p uecm --lib cli::manifest 2>&1 | tail -14`
Expected: 先看 `operation_count_covers_command_leaves` 实际 leaves，对齐断言后全 PASS。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/cli/manifest.rs
git commit -m "test(cli): manifest schema-completeness + CLI-tree consistency guards (spec §2.3)"
```

---

## Task 8: usage parse 错误走 ErrorEnvelope（修 Codex #4）

**Files:**
- Modify: `src-tauri/src/bin/uecm-cli.rs`

**目标：** clap parse 失败（未知 flag / 缺子命令）在结构化模式下输出 **ErrorEnvelope 形状**（与 handler 错误一致），exit 仍 64（保留 sysexits 偏离，见 P2 文档）；sniff 同时识别 `AI_AGENT=1`。

- [ ] **Step 1: 改 bin 的错误分支**

`src-tauri/src/bin/uecm-cli.rs`：
1. json sniff（P0 改后）追加 AI_AGENT：在 `let json_mode = argv.iter()...` 之后加：

```rust
    let json_mode = json_mode || std::env::var("AI_AGENT").map(|v| v == "1").unwrap_or(false);
```

2. parse 错误的 `if json_mode { ... }` 分支（P0 是扁平 `{"kind":"error",...}`）替换为 ErrorEnvelope 形状：

```rust
            if json_mode {
                let payload = serde_json::json!({
                    "schema_version": "1.0",
                    "status": "error",
                    "operation_id": "",
                    "error": {
                        "code": "usage_error",
                        "exit_code": 64,
                        "message": e.to_string(),
                        "retryable": false,
                        "clap_kind": format!("{:?}", e.kind()),
                    },
                    "meta": {
                        "request_id": "",
                        "duration_ms": 0,
                        "timestamp": uecm_lib::cli::envelope::now_iso8601(),
                    }
                });
                let mut stderr = io::stderr().lock();
                let _ = serde_json::to_writer(&mut stderr, &payload);
                let _ = stderr.write_all(b"\n");
                std::process::exit(64);
            } else {
```

（保留 `else` 分支的 `writeln!(stderr, "{}", e); exit(64)` 不变。）

- [ ] **Step 2: 写测试（结构化 usage 错误）**

在 `bin/uecm-cli.rs` 没有 lib test 入口；改为在 `src-tauri/tests/` 加一个集成测试（若已有 tests 目录沿用）。新建/追加 `src-tauri/tests/usage_envelope.rs`：

```rust
use std::process::Command;

#[test]
fn invalid_flag_json_is_error_envelope_exit_64() {
    let exe = env!("CARGO_BIN_EXE_uecm-cli");
    let out = Command::new(exe)
        .args(["--no-such-flag", "--output", "json"])
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(64));
    let err: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(err["status"], "error");
    assert_eq!(err["error"]["code"], "usage_error");
    assert_eq!(err["error"]["exit_code"], 64);
}
```

> 需要 `serde_json` 在 dev-deps（`Cargo.toml` 已有 `serde_json` 作普通依赖，集成测试可用）。`CARGO_BIN_EXE_uecm-cli` 由 cargo 在测试时注入。

- [ ] **Step 3: 测试**

Run: `cargo test -p uecm --test usage_envelope 2>&1 | tail -12`
Expected: PASS — exit 64 + ErrorEnvelope 形状。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/bin/uecm-cli.rs src-tauri/tests/usage_envelope.rs
git commit -m "fix(cli): usage parse errors emit ErrorEnvelope shape + AI_AGENT sniff (spec §4.2; Codex #4)"
```

---

## Task 9: 移除 `--pass` argv flag（spec §9）

**Files:**
- Modify: `src-tauri/src/cli/credential_args.rs:21-37`

- [ ] **Step 1: 写失败测试**

在 `credential_args.rs` tests 加：

```rust
#[test]
fn pass_flag_is_not_accepted_on_cli() {
    use crate::cli::args::Cli;
    use clap::Parser;
    let r = Cli::try_parse_from(["uecm-cli","machine","refresh","1","--user","u","--pass","p"]);
    assert!(r.is_err(), "--pass should no longer be a CLI flag");
    let ok = Cli::try_parse_from(["uecm-cli","machine","refresh","1","--user","u","--pass-stdin"]);
    assert!(ok.is_ok());
}
```

- [ ] **Step 2: 确认失败**

Run: `cargo test -p uecm --lib cli::credential_args::tests::pass_flag_is_not_accepted_on_cli 2>&1 | head -15`
Expected: FAIL — `--pass` 仍被接受。

- [ ] **Step 3: 改字段为 skip**

`credential_args.rs` 把 `pass` 字段（line 21-28）替换为：

```rust
    /// Internal-only password carrier (set by `inline()`); NOT a CLI flag.
    /// Passwords arrive via --pass-stdin or --cred-alias (spec §9: no argv secrets).
    #[arg(skip)]
    pub pass: Option<String>,
```

把 `pass_stdin`（line 31-36）的 `conflicts_with_all = ["pass", "cred_alias"]` 改为 `conflicts_with_all = ["cred_alias"]`。`user` 的 `requires = "secret"` 不变（secret 组现仅含 pass_stdin）。`resolve()`/`preflight()`/`inline()` 逻辑不改（仍读 `self.pass` 字段）。

- [ ] **Step 4: 测试 + 实测**

```bash
cargo test -p uecm --lib cli::credential_args 2>&1 | tail -12
cargo build -p uecm --bin uecm-cli 2>&1 | tail -2
NO_COLOR=1 src-tauri/target/debug/uecm-cli machine refresh --help 2>&1 | grep -i "pass" || echo "no --pass (good)"
```
Expected: 既有 `resolve_*`/`inline_*` 测试 + 新测试全绿；help 只见 `--pass-stdin`。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/cli/credential_args.rs
git commit -m "fix(cli): drop --pass argv flag; passwords via stdin/alias only (spec §9)"
```

---

## Task 10: `docs/contract-manifest.json` 快照（§12 Step4）

**Files:**
- Create: `docs/contract-manifest.json`

- [ ] **Step 1: 生成**

Run:
```bash
cargo build -p uecm --bin uecm-cli 2>&1 | tail -2
NO_COLOR=1 src-tauri/target/debug/uecm-cli manifest --output json 2>/dev/null \
  | jq '.data' > docs/contract-manifest.json
jq '.contract_version, (.operations|length), (.operations[0]|keys)' docs/contract-manifest.json
```
Expected: 写出文件；打印 `"1.0"`、operation 总数、单个 op 的 key 列表含 `input_schema`/`output_schema`/`error_schema`。

> 注：`manifest` 走 envelope，故 `--output json` 把 manifest 包进 `.data`；`jq '.data'` 取裸 manifest 作快照。

- [ ] **Step 2: Commit**

```bash
git add docs/contract-manifest.json
git commit -m "docs: contract-manifest.json snapshot with full schemas (spec §12 Step4)"
```

---

## Self-Review

- **Spec 覆盖**：§2.1 完整 manifest（T2 三套 schema：input 从 clap / output 按类型或 event / error 共享 ErrorBody；T6 全域；T7 完整性哨兵 fail-on-missing；T10 快照）；§2.2 operation_id canonical（T2/T6）；§2.3 一致性（T7）；§3.5 json 单对象 vs ndjson 流式（T5 JsonEmitter / NdjsonEmitter，修 Codex #2）；§4.1/4.2/4.3 信封（T1/T5）；§4.4 schema_version；§4.5 ISO8601；§9 禁 argv 密码（T9）；§10.1 manifest 命令（T3）。usage 错误信封统一（T8，修 Codex #4）。
- **Codex 四发现处置**：#1 在 P2 计划处理（completion 同步更新 manifest）；#2 T5 JsonEmitter；#3 T2/T6/T7 三套 schema + fail-on-missing 测试（用户决策：补全完整 Contract Manifest）；#4 T8。
- **占位扫描**：无 TBD/TODO。T6 机械重复任务给了完整 env 样例 + pattern + 域顺序 + schema 三来源规则；穷尽性由编译器、唯一性/完整性由 T7 强制。
- **类型一致**：`EnvelopeCtx{operation_id:String,request_id:String,started:Instant}` + `success/error/meta` 方法、`Operation{operation_id,summary,cli_command,side_effects,exit_codes}`、`operations()->&'static[Operation]`、`error_schema/event_schema/output_schema_for/input_schema_for/manifest_json`、`Emitter::finish`、`JsonEmitter::new(W,EnvelopeCtx)`、`NdjsonEmitter::with_envelope` 全程一致。Event tag `kind→type` 同步改既有测试断言。
- **依赖顺序**：T1→T2→T3→T4→T5→T6→T7→T8（独立）→T9（独立）→T10（依赖 T3+T6）。T4 提醒补所有 `Ctx{}` 测试构造点。
- **风险点 / 假设**：
  - schemars 0.8 对 internally-tagged enum（`Event` `tag="type"`）的 schema 生成——若 derive 报错，T5 Step3 处给 `Event` 加 `#[schemars(...)]` 或退化为 `dynamic_object_schema()` 作 event_schema（执行时验证）。
  - `ErrorEnvelope`/`SuccessEnvelope` 带生命周期，**不**对它们 `schema_for!`；error_schema 用无生命周期的 `ErrorBody`（已验证可派生）。
  - `input_schema_for` 会把 clap global flags（--output 等）也列进每个 op 的 properties——属正确（它们是合法输入），如需精简可在 T2 过滤 `global` arg（非必须）。
  - 大量域输出是 ad-hoc `serde_json::Value` → 用 `dynamic_object_schema()`（`{"type":"object"}`），这是"存在且合法但宽松"的 schema；T7 只校验存在+有效，不强制精确。若要更严，T6 把这些 ad-hoc 输出重构为命名类型（范围更大，按需）。
