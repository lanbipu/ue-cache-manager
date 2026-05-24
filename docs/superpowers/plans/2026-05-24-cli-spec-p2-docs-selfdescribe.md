# CLI Spec P2 — 文档产物 + 自描述 + Core 清理 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 补齐 spec §12 Step4 必备文档产物（`docs/exit-codes.md` / `docs/schema-versions.md` / `CHANGELOG.md`）、加 `uecm-cli system completion <shell>`（§10.1）、识别 `AI_AGENT=1` env 信号（§3.4）、把 Core 层直接 `eprintln!` 改为 `tracing`（§1.3）；并在 exit-codes 文档里**显式声明** usage error 用 64 而非 spec §5 的 2 是**已接受偏离**（用户决策：保留 64 改文档）。

**Architecture:** 文档产物从权威源生成或对齐（`system exit-codes` / `system schema` / `manifest` 命令输出），避免手抄漂移。`completion` 走 `clap_complete`（新依赖）。`AI_AGENT` 信号叠加进 `Cli::resolved_output()`（P0 已留接缝），抽纯函数测试。Core `eprintln!` 替换为 `tracing::warn!`，行为不变。

**Tech Stack:** Rust 2021、clap ~4.5、clap_complete 4.5（**新增依赖**）、tracing 0.1（已有）。

**Spec 基准：** `docs/CLI_DESIGN_SPEC.md` §1.3 / §3.4 / §5 / §10.1 / §12 Step4。

**前置依赖：** 建议在 **P1** 之后（`docs/contract-manifest.json` 已由 P1 Task 10 生成；schema-versions.md 引用 `schema_version`/`contract_version` 均来自 P1）。若 P1 未做，schema-versions.md 中"输出 envelope schema_version"一节标注为 planned。

**已定决策：** exit 64 **保留不改代码**，仅在 `docs/exit-codes.md` 声明为相对 spec §5 的已接受偏离（理由：区分 argv-shape vs handler invalid_input）。

**当前事实（已核对源码）：**
- `src-tauri/src/cli/domain_system.rs:99-119` — `exit_code_table()` / `error_code_table()` 已含 64 及其解释；`schema()`/`exit_codes()` handler。
- `src-tauri/src/cli/args.rs` — `SystemAction`（Version/DbPath/PsDir/MigrateDb/Echo/Schema/ExitCodes，line 138-156）；`Cli::resolved_output()`（P0 加）。
- `src-tauri/src/core/ini_scanner.rs:270` — `eprintln!("[ini_scanner] {}", msg)`（production 路径，非 test）。
- `src-tauri/Cargo.toml:42` — `clap = { version = "~4.5", features=["derive","env"] }`；无 `clap_complete`。

---

## File Structure

- **Create** `docs/exit-codes.md` — 退出码表 + 64 偏离声明。
- **Create** `docs/schema-versions.md` — 输出契约/manifest 版本说明。
- **Create** `CHANGELOG.md` — 初版 + contract_version 1.0 条目。
- **Modify** `src-tauri/Cargo.toml` — 加 `clap_complete`。
- **Modify** `src-tauri/src/cli/args.rs` — `SystemAction::Completion`；`resolved_output` 叠加 AI_AGENT。
- **Modify** `src-tauri/src/cli/domain_system.rs` — completion handler。
- **Modify** `src-tauri/src/cli/run.rs` — needs_db 加 Completion 分支。
- **Modify** `src-tauri/src/core/ini_scanner.rs:270` — eprintln→tracing。

---

## Task 1: `AI_AGENT=1` env 信号 → 默认 json（spec §3.4）

**Files:**
- Modify: `src-tauri/src/cli/args.rs`（`resolved_output` 抽纯函数 + 叠 env）

- [ ] **Step 1: 写失败测试**

在 `cli::args::tests` 加：

```rust
#[test]
fn ai_agent_env_defaults_to_json() {
    use super::OutputFormat;
    // 无显式 output、无 --json，但 AI_AGENT=1 -> Json
    assert_eq!(super::resolve_output(None, false, true), OutputFormat::Json);
    // 显式 --output text 压过 AI_AGENT
    assert_eq!(super::resolve_output(Some(OutputFormat::Text), false, true), OutputFormat::Text);
    // 无任何信号 -> Text
    assert_eq!(super::resolve_output(None, false, false), OutputFormat::Text);
    // --json 别名仍 -> Json
    assert_eq!(super::resolve_output(None, true, false), OutputFormat::Json);
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p uecm --lib cli::args::tests::ai_agent_env_defaults_to_json 2>&1 | head -15`
Expected: FAIL — `resolve_output` 自由函数未定义。

- [ ] **Step 3: 抽纯函数 + 在 resolved_output 读 env**

`src-tauri/src/cli/args.rs` 把 P0 加的 `Cli::resolved_output` 改为委托纯函数：

```rust
impl Cli {
    /// 解析有效输出格式。优先级：显式 --output > --json 别名 > AI_AGENT=1 env > 默认 text。
    pub fn resolved_output(&self) -> OutputFormat {
        let ai_agent = std::env::var("AI_AGENT").map(|v| v == "1").unwrap_or(false);
        resolve_output(self.output, self.json, ai_agent)
    }
}

/// 纯函数核心（可单测，不读 env）。spec §3.4：AI_AGENT=1 是 AI 调用的显式信号。
pub fn resolve_output(output: Option<OutputFormat>, json: bool, ai_agent: bool) -> OutputFormat {
    if let Some(fmt) = output {
        return fmt;
    }
    if json {
        return OutputFormat::Json;
    }
    if ai_agent {
        return OutputFormat::Json;
    }
    OutputFormat::Text
}
```

- [ ] **Step 4: 运行测试 + 实测**

Run:
```bash
cargo test -p uecm --lib cli::args 2>&1 | tail -10
cargo build -p uecm --bin uecm-cli 2>&1 | tail -2
AI_AGENT=1 NO_COLOR=1 src-tauri/target/debug/uecm-cli system version 2>/dev/null | jq .
```
Expected: 测试 PASS；最后一行即便没带 `--output json` 也输出纯 JSON（AI_AGENT 触发）。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/cli/args.rs
git commit -m "feat(cli): AI_AGENT=1 env signal defaults output to json (spec §3.4)"
```

---

## Task 2: `uecm-cli system completion <shell>`（spec §10.1）

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/cli/args.rs`（`SystemAction::Completion`）
- Modify: `src-tauri/src/cli/domain_system.rs`（handler）
- Modify: `src-tauri/src/cli/run.rs`（needs_db）

- [ ] **Step 1: 加依赖**

`src-tauri/Cargo.toml` 在 `clap = ...` 行（line 42）下加：

```toml
clap_complete = "4.5"
```

Run: `cargo build -p uecm 2>&1 | tail -3`
Expected: 拉取 clap_complete 成功，编译通过。

- [ ] **Step 2: 写失败测试**

在 `cli::args::tests` 加：

```rust
#[test]
fn completion_command_parses_shell() {
    let cli = Cli::try_parse_from(["uecm-cli","system","completion","bash"]).unwrap();
    match cli.command {
        Domain::System { action: SystemAction::Completion { shell } } => {
            assert_eq!(shell, clap_complete::Shell::Bash);
        }
        _ => panic!("expected system completion bash"),
    }
}
```

- [ ] **Step 3: 运行确认失败**

Run: `cargo test -p uecm --lib cli::args::tests::completion_command_parses_shell 2>&1 | head -15`
Expected: FAIL — `SystemAction::Completion` 不存在。

- [ ] **Step 4: 加 SystemAction::Completion + 同步更新 manifest（Codex #1，必须同一步落地）**

**为什么同一步**：P1 的 `manifest::operation_id_for` 对 `SystemAction` 是**穷尽 match（无 `_` 兜底）**，又有 `operations()` 表 + `output_schema_for` 与命令树叶子的一致性哨兵（P1 Task 7）。一旦只加 enum 变体不补 manifest，crate 直接非穷尽编译失败 / 哨兵测试红。所以下面三处必须和 enum 变体一起改。

4a. `src-tauri/src/cli/args.rs` 在 `SystemAction`（line 138-156）末尾 `ExitCodes` 之后加：

```rust
    /// Generate a shell completion script (bash / zsh / fish / powershell / elvish).
    Completion {
        /// Target shell.
        shell: clap_complete::Shell,
    },
```

4b.（**仅当 P1 已合入**）`src-tauri/src/cli/manifest.rs` 的 `operation_id_for` 在 `SystemAction` 那个 match 里 `ExitCodes => "system.exit_codes",` 之后加一臂：

```rust
            SystemAction::Completion { .. } => "system.completion",
```

4c.（仅当 P1 已合入）`operations()` 的 `OPS` 表加一行：

```rust
        Operation { operation_id: "system.completion", summary: "Generate a shell completion script", cli_command: "uecm-cli system completion", side_effects: SideEffects{writes:false,external_calls:false,idempotent:true}, exit_codes: &[0,2] },
```

4d.（仅当 P1 已合入）`output_schema_for` 加一臂——completion 输出是裸 shell 脚本（非结构化），用动态 object schema 占位：

```rust
        "system.completion" => dynamic_object_schema(),
```

> 若 P2 先于 P1 执行（不推荐），跳过 4b/4c/4d；待 P1 落地时再补这三处。

4e. 跑 manifest 测试，确认穷尽 + 唯一 + 一致性哨兵仍绿：

```bash
cargo test -p uecm --lib cli::manifest 2>&1 | tail -10
```
Expected: PASS。`operation_count_covers_command_leaves` 因新增 1 个叶子 + 1 个 op 而平移，仍成立；若该断言被锁死为常量需同步 +1。

- [ ] **Step 5: 加 handler**

`src-tauri/src/cli/domain_system.rs`：
1. `handle` 的 match（line 23-31）加一臂：`SystemAction::Completion { shell } => completion(ctx, shell),`。
2. 文件加 handler 函数：

```rust
fn completion(ctx: &mut Ctx<'_>, shell: clap_complete::Shell) -> UecmResult<()> {
    use clap::CommandFactory;
    let mut cmd = crate::cli::args::Cli::command();
    let bin = cmd.get_name().to_string();
    // 补全脚本是 shell 源码，不是结构化数据——直接写裸 stdout，不走 envelope。
    let mut out: Vec<u8> = Vec::new();
    clap_complete::generate(shell, &mut cmd, bin, &mut out);
    use std::io::Write;
    let _ = std::io::stdout().write_all(&out);
    let _ = ctx; // completion 不用 emitter（裸 shell 脚本输出）
    Ok(())
}
```

- [ ] **Step 6: needs_db 分支**

`src-tauri/src/cli/run.rs` 的 `needs_db`：`Domain::System { action }` 那臂（line 51）当前是 `matches!(action, SystemAction::MigrateDb)`，保持不变即可（Completion 不匹配 MigrateDb，自然返回 false）。无需改动——确认即可。

- [ ] **Step 7: 运行测试 + 实测（含 stdout 纯净性）**

Run:
```bash
cargo test -p uecm --lib 2>&1 | tail -10
cargo build -p uecm --bin uecm-cli 2>&1 | tail -2
B=src-tauri/target/debug/uecm-cli
$B system completion bash 2>/dev/null | head -5
$B system completion zsh 2>/dev/null | head -3
echo "--- completion 在 --output json 下也只吐裸脚本，不掺 envelope（靠 P1 JsonEmitter.finish 空时 no-op）---"
NO_COLOR=1 $B system completion bash --output json 2>/dev/null | grep -c '"schema_version"' || true
```
Expected: 测试全 PASS；bash/zsh 补全脚本前几行输出（含 `_uecm-cli` 之类）；最后一行 grep 计数为 `0`（裸脚本里**没有** envelope，证明 stdout 未被污染）。

- [ ] **Step 8: 重新生成 manifest 快照（completion 已进 manifest）**

Run（仅当 P1 已合入、`docs/contract-manifest.json` 存在时）:
```bash
NO_COLOR=1 src-tauri/target/debug/uecm-cli manifest --output json 2>/dev/null \
  | jq '.data' > docs/contract-manifest.json
jq '.operations[] | select(.operation_id=="system.completion") | .operation_id' docs/contract-manifest.json
```
Expected: 打印 `"system.completion"`，确认它进了快照。

- [ ] **Step 9: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/cli/args.rs src-tauri/src/cli/domain_system.rs src-tauri/src/cli/manifest.rs docs/contract-manifest.json
git commit -m "feat(cli): system completion <shell> + manifest entry (spec §10.1; Codex #1)"
```

---

## Task 3: Core 层 `eprintln!` → `tracing`（spec §1.3）

**Files:**
- Modify: `src-tauri/src/core/ini_scanner.rs:270`

- [ ] **Step 1: 核对上下文**

Run: `sed -n '265,275p' src-tauri/src/core/ini_scanner.rs`（或 Read）
确认 line 270 是 `eprintln!("[ini_scanner] {}", msg);` 形态及其变量名 `msg`。

- [ ] **Step 2: 替换**

把该 `eprintln!("[ini_scanner] {}", msg);` 替换为：

```rust
            tracing::warn!(target: "ini_scanner", "{}", msg);
```
（保留原缩进。`tracing` 已是依赖，core 其它处也用。）

- [ ] **Step 3: 编译 + 确认无残留 production eprintln**

Run:
```bash
cargo build -p uecm 2>&1 | tail -3
grep -rn "eprintln!" src-tauri/src/core src-tauri/src/data | grep -viE "test|UECM_IT_HOST|skip:" || echo "no production eprintln in core/data"
```
Expected: 编译通过；grep 仅剩 test/集成测试守卫内的 eprintln（discovery.rs / ssh.rs 那些），production 路径 0 条。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/core/ini_scanner.rs
git commit -m "refactor(core): ini_scanner eprintln -> tracing::warn (spec §1.3)"
```

---

## Task 4: `docs/exit-codes.md`（含 64 偏离声明）

**Files:**
- Create: `docs/exit-codes.md`

- [ ] **Step 1: 抓权威数据**

Run: `cargo build -p uecm --bin uecm-cli 2>&1 | tail -2 && NO_COLOR=1 src-tauri/target/debug/uecm-cli system exit-codes 2>/dev/null | jq .`
记下 `exit_codes` / `error_codes` 两张表的真实内容（写文档以此为准）。

- [ ] **Step 2: 写文档**

新建 `docs/exit-codes.md`：

```markdown
# uecm-cli Exit Codes

权威来源：`uecm-cli system exit-codes --output json`（运行时生成；本文件是其人类可读快照）。
映射实现见 `src-tauri/src/cli/output.rs::exit_code_for` 与 `src-tauri/src/bin/uecm-cli.rs`。

## 进程退出码

| Code | Name | 含义 |
|---|---|---|
| 0 | ok | 成功 |
| 1 | operation_failed | 运行期业务逻辑失败（未分类） |
| 2 | invalid_input | 用户运行期数据非法（未知 id、坏 CIDR 等） |
| 3 | environment_error | 配置 / 数据库 / IO 问题，需用户修复环境（含 SSH 连接、超时、脚本暂存） |
| 4 | powershell_failed | 远端 PowerShell / 节点脚本调用失败 |
| 64 | usage_error | argv 形态错误：缺必填 flag、未知子命令、互斥冲突（sysexits.h EX_USAGE） |

## 错误 envelope 的 `error.code`（`--output json` 下）

| error.code | exit | 来源 |
|---|---|---|
| invalid_input | 2 | handler 校验 |
| operation_failed | 1 | handler 运行期失败 |
| environment_error | 3 | 配置 / 数据库 / IO |
| powershell_failed | 4 | 远端 PowerShell |
| usage_error | 64 | clap argv 解析 |

## 与 AI-Native Spec v3.0 §5 的已接受偏离

Spec §5 把"CLI usage / argument syntax error"归为 exit **2**。本项目**有意**把 **argv 解析层**的用法错误用 **64**（sysexits.h `EX_USAGE`），而把 **handler 层**的运行期数据非法（如未知 machine id）保留为 **2**（`invalid_input`）。

**理由：** 让自动化能区分"命令行拼写/形态写错"（64，改命令行即可）与"命令行合法但运行期数据无效"（2，需改数据/环境）。两类对调用方的修复动作不同。spec §5 把二者都收敛到 2，本项目认为牺牲了这一可区分性。

**实现位置：** `src-tauri/src/bin/uecm-cli.rs`（clap parse 失败 → exit 64）。本偏离不计划改回；如需对齐 spec，把该处改为 exit 2 并用 `error.code` 承载区分（见 P0 计划讨论）。

其余退出码（spec §5 的 5/6/7/8/9）当前未细分使用：超时归入 3、外部依赖失败归入 4，未单独分配 5（not found）/6（conflict）/9（partial）。后续如需细化在 `exit_code_for` 扩展。
```

- [ ] **Step 3: Commit**

```bash
git add docs/exit-codes.md
git commit -m "docs: exit-codes.md with documented 64-vs-2 spec deviation (spec §5/§12)"
```

---

## Task 5: `docs/schema-versions.md`

**Files:**
- Create: `docs/schema-versions.md`

- [ ] **Step 1: 写文档**

新建 `docs/schema-versions.md`：

```markdown
# uecm-cli Schema / Contract Versions

## 输出 envelope schema_version

- 当前：`1.0`（常量 `src-tauri/src/cli/envelope.rs::SCHEMA_VERSION`）。
- 语义化 `MAJOR.MINOR`：breaking 改动（删字段 / 改类型 / 改含义）→ MAJOR+1；新增可选字段 → MINOR+1。
- 出现在每个 `--output json` 成功 / 错误 envelope 顶层，以及每条 ndjson 事件（spec §4.4）。

## Contract Manifest contract_version

- 当前：`1.0`（`src-tauri/src/cli/manifest.rs::manifest_json`）。
- 与 envelope `schema_version` 对齐演进（spec §4.4）。
- 权威快照：`docs/contract-manifest.json`（由 `uecm-cli manifest --output json` 生成）。

## CLI self-description spec_version

- `uecm-cli system schema` 输出的 `spec_version: 1`，标识命令树 dump 格式版本（与上面两者独立，仅描述 schema 自描述结构）。

## 内部 DB schema 版本

- SQLite 迁移版本与上述输出契约**无关**，由 `src-tauri/src/data/schema.rs` 的 migration 序列管理，不对外承诺稳定性。

## 变更流程

改动输出字段时：
1. 评估是否 breaking；按规则 bump `SCHEMA_VERSION` 与 `contract_version`。
2. 重新生成 `docs/contract-manifest.json`。
3. 在 `CHANGELOG.md` 记录 `contract_version` 变化。
```

- [ ] **Step 2: Commit**

```bash
git add docs/schema-versions.md
git commit -m "docs: schema-versions.md (spec §4.4/§12)"
```

---

## Task 6: `CHANGELOG.md`

**Files:**
- Create: `CHANGELOG.md`

- [ ] **Step 1: 写初版**

新建仓库根 `CHANGELOG.md`：

```markdown
# Changelog

本项目变更记录。标注 `contract_version` 变化（见 `docs/schema-versions.md`）。
格式参考 Keep a Changelog；版本对齐 `src-tauri/Cargo.toml` 的 `version`。

## [Unreleased]

### Added
- CLI 全局 flag：`--output/-o`（text/json/ndjson）、`--no-color`、`--no-input`、`--quiet/-q`、`--verbose/-v`、`--config`、`--input-format`（spec §3.2）。
- `AI_AGENT=1` env 信号默认输出 json（spec §3.4）。
- Contract Manifest：`uecm-cli manifest` 命令 + canonical `operation_id` 体系（spec §2），快照 `docs/contract-manifest.json`。
- 统一输出 envelope：所有 `--output json/ndjson` 输出套 `schema_version`/`status`/`operation_id`/`data`/`meta`（spec §4）。**contract_version: 1.0**。
- `uecm-cli system completion <shell>`（spec §10.1）。
- 文档：`docs/exit-codes.md`、`docs/schema-versions.md`。

### Changed
- `--json` 降级为 `--output json` 的别名（向后兼容保留）。
- ndjson 事件字段 `kind` 改名为 `type`，并新增 `sequence`/`timestamp`/`request_id`/`final`（spec §4.3）。**breaking**：消费 `kind` 字段的旧脚本需改读 `type`。
- Core `ini_scanner` 的 `eprintln!` 改为 `tracing::warn!`（spec §1.3）。

### Removed
- `--pass` argv flag：密码不再经命令行传递，仅 `--pass-stdin` / `--cred-alias`（spec §9）。

### Notes
- usage error 退出码保留 `64`（sysexits EX_USAGE），相对 spec §5（=2）为已接受偏离，理由见 `docs/exit-codes.md`。
```

> 注：上述条目覆盖 P0 / P1 / P2 三个计划的产出。如分阶段合入，按实际落地拆条目即可。

- [ ] **Step 2: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs: add CHANGELOG.md with contract_version 1.0 (spec §12)"
```

---

## Task 7: §12 Step4 产物齐全性校验

**Files:** 无改动，验证。

- [ ] **Step 1: 列出 §12 Step4 五项产物**

Run:
```bash
ls -la README.md CHANGELOG.md docs/contract-manifest.json docs/exit-codes.md docs/schema-versions.md
```
Expected: 五个文件全部存在（`docs/contract-manifest.json` 由 P1 Task 10 产出；若 P1 未做，此项缺失需先补 P1）。

- [ ] **Step 2: 自描述命令齐全性**

Run:
```bash
B=src-tauri/target/debug/uecm-cli
NO_COLOR=1 $B --help >/dev/null 2>&1 && echo "help OK"
NO_COLOR=1 $B manifest --output json 2>/dev/null | jq '.data.contract_version // .contract_version' 
NO_COLOR=1 $B system schema --output json >/dev/null 2>&1 && echo "schema OK"
$B system completion bash >/dev/null 2>&1 && echo "completion OK"
NO_COLOR=1 $B system version --output json >/dev/null 2>&1 && echo "version OK"
```
Expected: §10.1 五项（help / manifest / schema / completion / version）全 OK，manifest 打印 `"1.0"`。

- [ ] **Step 3: Commit（空提交作里程碑）**

```bash
git commit --allow-empty -m "chore: CLI spec P2 self-description + docs artifacts complete (spec §10.1/§12)"
```

---

## Self-Review

- **Codex #1 已并入**：T2 Step4 把"加 `SystemAction::Completion` 变体"与"同步更新 `manifest.rs`（operation_id_for 臂 + OPS 行 + output_schema_for 臂）+ 重生成 contract-manifest.json"绑成同一步，避免 P1 穷尽 match 编译失败 / 一致性哨兵变红；并验证 completion 在 `--output json` 下不污染 stdout（依赖 P1 JsonEmitter.finish 空时 no-op）。
- **Spec 覆盖**：§1.3 Core eprintln（T3）；§3.4 AI_AGENT（T1）；§5 exit-codes 文档 + 64 偏离声明（T4，落实用户"保留 64 改文档"决策）；§10.1 completion（T2）+ manifest/schema/version 齐全性（T7）；§12 Step4 五产物 exit-codes.md（T4）/ schema-versions.md（T5）/ CHANGELOG.md（T6）/ contract-manifest.json（P1 T10）/ README（既有）。
- **占位扫描**：无 TBD/TODO。文档内容均为完整正文，数据从 `system exit-codes` / `manifest` 等权威命令对齐。
- **类型一致**：`resolve_output(Option<OutputFormat>, bool, bool) -> OutputFormat` 与 P0 的 `OutputFormat`{Text,Json,Ndjson} 一致；`SystemAction::Completion{shell: clap_complete::Shell}` 与 handler 签名一致。
- **依赖顺序**：T1（AI_AGENT，依赖 P0 的 `OutputFormat`/`resolved_output` 接缝）→ T2（completion，独立）→ T3（core，独立）→ T4/T5/T6（文档，T5/T6 引用 P1 的 schema_version/contract_version）→ T7（校验，依赖 P1 的 contract-manifest.json 已生成）。
- **风险点**：T7 Step1 依赖 P1 已产出 `docs/contract-manifest.json`；若 P2 先于 P1 执行，该项会缺，需先补 P1 Task 10 或暂跳该断言。
