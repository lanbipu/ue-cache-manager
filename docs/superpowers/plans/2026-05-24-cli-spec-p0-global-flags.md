# CLI Spec P0 — 全局 Flag 补齐 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 给 `uecm-cli` 补齐 AI-Native App Interface Spec v3.0 §3.2 缺失的强制全局 flag（`--output/-o`、`--no-color`、`--no-input`、`--quiet/-q`、`--verbose/-v`、`--config`、`--input-format`），并把当前布尔 `--json` 降级为 `--output json` 的别名。

**Architecture:** 全部新 flag 加在 `cli::args::Cli`（clap derive，`global = true`）。输出格式从布尔 `json` 升级为 `OutputFormat` 枚举，`Cli::resolved_output()` 统一解析优先级。Emitter 选择逻辑从 `run.rs` 的 `if json_mode` 改为 `match fmt`。所有可测的解析逻辑抽成纯函数（`use_color` / `effective_log_level` / `resolved_output`）以便在 mac 上 `cargo test` 验证（核心命令仍 Windows-only，但这些是平台无关的 arg 逻辑）。

**Tech Stack:** Rust 2021、clap ~4.5（derive + env）、serde_yaml 0.9（已有依赖，用于 `--config`）、atty 0.2。

**Spec 基准：** `docs/CLI_DESIGN_SPEC.md` §3.2 / §3.4 / §3.5 / §9（配置文件 mode ≤ 0600）。

**已定决策（来自审计澄清）：**
- `--json` **保留**为 `--output json` 别名，不破坏现有 docs/scripts 调用。
- 全局 `--yes`/`--dry-run` **本计划不做**：已在 ~32 个子命令各自实现（§3.2"支持"已满足），改全局会撞 clap arg-id 且 global `--yes` 是 footgun。如需，另立专项。
- `AI_AGENT=1 → 默认 json`（§3.4）放在 **P2 计划**（与 env 信号、文档一起），本计划 `resolved_output()` 预留接缝。
- `--config` / `--input-format` 列在末尾（Task 6 / Task 7），优先级低于核心输出/交互 flag；`--input-format` 当前**无消费者**（无嵌套 stdin JSON 命令），按 spec 强制项加上 flag + helper，但标注 YAGNI 取舍。

**当前事实（已核对源码）：**
- `src-tauri/src/cli/args.rs:24-41` — `Cli` 仅有 `json: bool` / `db_path: Option<String>` / `log_level: String`。
- `src-tauri/src/cli/run.rs:89-127` — tracing init + emitter 选择（`if json_mode { NdjsonEmitter } else { HumanEmitter(color=atty) }`）。
- `src-tauri/src/bin/uecm-cli.rs:20-21` — 用 `argv.contains("--json")` sniff json 模式以决定 parse-error 信封格式。
- 测试入口：`cargo test -p uecm`（lib crate 名 `uecm_lib`，见 `Cargo.toml:11`）。

---

## File Structure

- **Modify** `src-tauri/src/cli/args.rs` — 加 `OutputFormat` 枚举、`Cli` 新字段、`resolved_output()` / `use_color()` / `effective_log_level()` 纯函数 + 单测。
- **Modify** `src-tauri/src/cli/run.rs` — emitter 选择改 `match fmt`；tracing filter 用 `effective_log_level`。
- **Modify** `src-tauri/src/bin/uecm-cli.rs` — json sniff 兼容 `--output json` / `-o json`。
- **Create** `src-tauri/src/cli/config_file.rs` — `--config` 文件加载 + mode 检查（Task 6）。
- **Create** `src-tauri/src/cli/stdin_input.rs` — `--input-format` stdin 读取 helper（Task 7）。
- **Modify** `src-tauri/src/cli/mod.rs` — 注册新模块。

---

## Task 1: OutputFormat 枚举 + `--output/-o` flag + `--json` 别名

**Files:**
- Modify: `src-tauri/src/cli/args.rs:16-41`（在 `BackendChoice` 之后、`Cli` 内）
- Modify: `src-tauri/src/cli/run.rs:119-127`
- Modify: `src-tauri/src/bin/uecm-cli.rs:20-21`

- [ ] **Step 1: 写失败测试 — resolved_output 优先级**

在 `src-tauri/src/cli/args.rs` 末尾的 `#[cfg(test)] mod tests`（若无则新建）加：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn cli_with(json: bool, output: Option<OutputFormat>) -> Cli {
        Cli {
            json,
            output,
            no_color: false,
            no_input: false,
            quiet: false,
            verbose: 0,
            config: None,
            input_format: None,
            db_path: None,
            log_level: "warn".into(),
            command: Domain::System { action: SystemAction::Version },
        }
    }

    #[test]
    fn output_explicit_wins_over_json() {
        let cli = cli_with(true, Some(OutputFormat::Text));
        assert_eq!(cli.resolved_output(), OutputFormat::Text);
    }

    #[test]
    fn json_alias_maps_to_json() {
        let cli = cli_with(true, None);
        assert_eq!(cli.resolved_output(), OutputFormat::Json);
    }

    #[test]
    fn default_is_text() {
        let cli = cli_with(false, None);
        assert_eq!(cli.resolved_output(), OutputFormat::Text);
    }
}
```

- [ ] **Step 2: 运行测试，确认编译失败**

Run: `cargo test -p uecm --lib cli::args::tests 2>&1 | head -30`
Expected: FAIL — `OutputFormat` / `resolved_output` / 新字段未定义，编译错误。

- [ ] **Step 3: 加 OutputFormat 枚举 + Cli 字段 + resolved_output**

在 `src-tauri/src/cli/args.rs` 的 `BackendChoice` 枚举（约 line 22）之后插入：

```rust
/// 输出格式（spec §3.5）。`text` 给人类，`json` 单次完整对象，`ndjson` 每行一对象。
/// `stream-json` 是 `ndjson` 的别名（spec §3.5）。
#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
#[clap(rename_all = "snake_case")]
pub enum OutputFormat {
    Text,
    Json,
    #[value(alias = "stream-json")]
    Ndjson,
}
```

把 `Cli` struct（line 24-41）整体替换为（新增 6 个 `global = true` 字段，`json` 保留为别名）：

```rust
#[derive(Parser, Debug)]
#[command(name = "uecm-cli", version, about = "UECM command-line interface")]
pub struct Cli {
    /// DEPRECATED 别名：等价 `--output json`。保留以兼容现有 docs/scripts。
    #[arg(long, global = true)]
    pub json: bool,

    /// Output format: text (human) / json (single object) / ndjson (one object per line).
    #[arg(long, short = 'o', global = true, value_enum)]
    pub output: Option<OutputFormat>,

    /// Disable ANSI color (also honors the NO_COLOR env var).
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Refuse any interactive prompt (recommended for AI / CI callers).
    #[arg(long, global = true)]
    pub no_input: bool,

    /// Equivalent to `--log-level error`.
    #[arg(long, short = 'q', global = true)]
    pub quiet: bool,

    /// Increase log verbosity (-v = info, -vv = debug). Overrides --log-level upward.
    #[arg(long, short = 'v', global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Load defaults from a YAML / JSON config file (mode must be <= 0600).
    #[arg(long, global = true)]
    pub config: Option<std::path::PathBuf>,

    /// Format of structured data read from stdin (json / yaml / ndjson).
    #[arg(long, global = true, value_enum)]
    pub input_format: Option<InputFormat>,

    /// Override DB path (otherwise resolved via startup module).
    #[arg(long, global = true, env = "UECM_DB_PATH")]
    pub db_path: Option<String>,

    /// Log level for tracing output to stderr.
    #[arg(long, global = true, default_value = "warn")]
    pub log_level: String,

    #[command(subcommand)]
    pub command: Domain,
}

impl Cli {
    /// 解析有效输出格式。优先级：显式 `--output` > `--json` 别名 > 默认 text。
    /// (P2 会在此处叠加 `AI_AGENT=1 → Json` 的 env 信号。)
    pub fn resolved_output(&self) -> OutputFormat {
        if let Some(fmt) = self.output {
            return fmt;
        }
        if self.json {
            return OutputFormat::Json;
        }
        OutputFormat::Text
    }
}
```

> 注：`InputFormat` 枚举在 Task 7 定义。为让本 Task 先编译，先在 `OutputFormat` 之后加占位枚举（Task 7 会补 helper，不改枚举本身）：
> ```rust
> /// stdin 结构化输入格式（spec §3.3）。helper 见 Task 7。
> #[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
> #[clap(rename_all = "snake_case")]
> pub enum InputFormat { Json, Yaml, Ndjson }
> ```

- [ ] **Step 4: 运行测试，确认通过**

Run: `cargo test -p uecm --lib cli::args::tests 2>&1 | tail -20`
Expected: PASS — 3 个 `resolved_output` 测试绿。

- [ ] **Step 5: 接 run.rs 的 emitter 选择**

`src-tauri/src/cli/run.rs`：在文件顶部 `use crate::cli::args::{Cli, Domain};` 改为 `use crate::cli::args::{Cli, Domain, OutputFormat};`。把 line 119-127（`let json_mode = cli.json;` 到 emitter 构造）替换为：

```rust
    // Emitter selection (spec §3.5). text -> human; json/ndjson -> NDJSON emitter.
    // True single-object buffering for `json` (vs streamed `ndjson`) is refined in
    // the P1 envelope plan; here both structured modes share the NDJSON emitter.
    let fmt = cli.resolved_output();
    let json_mode = !matches!(fmt, OutputFormat::Text);
    let stdout = io::stdout();
    let stderr = io::stderr();
    let emitter: Box<dyn Emitter> = match fmt {
        OutputFormat::Text => {
            let color = crate::cli::args::use_color(cli.no_color, atty::is(atty::Stream::Stdout));
            Box::new(HumanEmitter::new(stdout.lock(), stderr.lock(), color))
        }
        OutputFormat::Json | OutputFormat::Ndjson => Box::new(NdjsonEmitter::new(stdout.lock())),
    };
```

> `use_color` 在 Task 2 定义。本 Task 先用临时实现让其编译，Task 2 替换为真版 + 测试。临时实现（加在 args.rs，Task 2 会改）：
> ```rust
> pub fn use_color(no_color_flag: bool, is_tty: bool) -> bool { !no_color_flag && is_tty }
> ```

- [ ] **Step 6: bin/uecm-cli.rs json sniff 兼容 --output**

`src-tauri/src/bin/uecm-cli.rs` 第 21 行 `let json_mode = argv.iter().any(|a| a.as_os_str() == "--json");` 替换为：

```rust
    // Sniff structured-output intent from raw argv so clap parse errors are
    // formatted as a JSON envelope when the caller asked for json/ndjson.
    let json_mode = argv.iter().enumerate().any(|(i, a)| {
        let s = a.as_os_str();
        s == "--json"
            || s == "--output=json" || s == "--output=ndjson" || s == "--output=stream-json"
            || s == "-o=json" || s == "-o=ndjson" || s == "-o=stream-json"
            || ((s == "--output" || s == "-o")
                && argv.get(i + 1).map(|n| {
                    n == "json" || n == "ndjson" || n == "stream-json"
                }).unwrap_or(false))
    });
```

- [ ] **Step 7: 全量编译 + 测试**

Run: `cargo test -p uecm --lib 2>&1 | tail -15`
Expected: PASS — 全绿，无编译错误。

- [ ] **Step 8: 行为实测（mac 可跑的 system 命令）**

Run（debug build 先 `cargo build -p uecm --bin uecm-cli`）:
```bash
B=src-tauri/target/debug/uecm-cli
NO_COLOR=1 $B system version --output json 2>/dev/null | jq .
NO_COLOR=1 $B system version -o json 2>/dev/null | jq .
NO_COLOR=1 $B system version --json 2>/dev/null | jq .   # 别名仍可用
```
Expected: 三条都输出 `{"binary":"uecm-cli","version":"0.1.0"}` 且 `jq` 解析成功。

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/cli/args.rs src-tauri/src/cli/run.rs src-tauri/src/bin/uecm-cli.rs
git commit -m "feat(cli): add --output/-o flag + keep --json as alias (spec §3.5)"
```

---

## Task 2: `--no-color` + NO_COLOR env

**Files:**
- Modify: `src-tauri/src/cli/args.rs`（替换 Task 1 临时 `use_color`）

- [ ] **Step 1: 写失败测试**

在 `cli::args::tests` 加：

```rust
#[test]
fn use_color_truth_table() {
    // flag 关 + 非 TTY + 无 env -> 由 is_tty 决定
    assert!(super::use_color(false, true, false));
    assert!(!super::use_color(false, false, false));
    // --no-color 一票否决
    assert!(!super::use_color(true, true, false));
    // NO_COLOR env 一票否决
    assert!(!super::use_color(false, true, true));
}
```

- [ ] **Step 2: 运行，确认失败**

Run: `cargo test -p uecm --lib cli::args::tests::use_color_truth_table 2>&1 | head -20`
Expected: FAIL — 签名是 2 参数，调用 3 参数，编译错误。

- [ ] **Step 3: 替换 use_color 为真版**

把 Task 1 的临时 `pub fn use_color(no_color_flag: bool, is_tty: bool)` 替换为：

```rust
/// 是否启用 ANSI color。`--no-color` 或 `NO_COLOR` env 任一存在即禁用；
/// 否则跟随 stdout 是否 TTY。(spec §3.2 / §3.4)
pub fn use_color(no_color_flag: bool, is_tty: bool, no_color_env: bool) -> bool {
    !no_color_flag && !no_color_env && is_tty
}
```

- [ ] **Step 4: 更新 run.rs 调用点传入 env**

`src-tauri/src/cli/run.rs` Task 1 写的 `let color = crate::cli::args::use_color(cli.no_color, atty::is(...));` 改为：

```rust
            let color = crate::cli::args::use_color(
                cli.no_color,
                atty::is(atty::Stream::Stdout),
                std::env::var_os("NO_COLOR").is_some(),
            );
```

- [ ] **Step 5: 运行测试**

Run: `cargo test -p uecm --lib cli::args 2>&1 | tail -10`
Expected: PASS。

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/cli/args.rs src-tauri/src/cli/run.rs
git commit -m "feat(cli): --no-color flag + honor NO_COLOR env (spec §3.2)"
```

---

## Task 3: `--quiet` / `--verbose` → 有效日志级别

**Files:**
- Modify: `src-tauri/src/cli/args.rs`（加 `effective_log_level`）
- Modify: `src-tauri/src/cli/run.rs:91-96`（tracing filter）

- [ ] **Step 1: 写失败测试**

在 `cli::args::tests` 加：

```rust
#[test]
fn effective_log_level_rules() {
    // 默认透传
    assert_eq!(super::effective_log_level("warn", 0, false), "warn");
    // --quiet 压到 error，优先级最高
    assert_eq!(super::effective_log_level("debug", 2, true), "error");
    // -v -> info, -vv -> debug，覆盖基线
    assert_eq!(super::effective_log_level("warn", 1, false), "info");
    assert_eq!(super::effective_log_level("warn", 2, false), "debug");
    // -vvv 仍封顶 trace
    assert_eq!(super::effective_log_level("warn", 5, false), "trace");
}
```

- [ ] **Step 2: 运行，确认失败**

Run: `cargo test -p uecm --lib cli::args::tests::effective_log_level_rules 2>&1 | head -15`
Expected: FAIL — `effective_log_level` 未定义。

- [ ] **Step 3: 实现 effective_log_level**

加到 `src-tauri/src/cli/args.rs`：

```rust
/// 计算有效 tracing 级别。优先级：--quiet > --verbose 计数 > --log-level 基线。
/// (spec §3.2)
pub fn effective_log_level(base: &str, verbose: u8, quiet: bool) -> String {
    if quiet {
        return "error".to_string();
    }
    match verbose {
        0 => base.to_string(),
        1 => "info".to_string(),
        2 => "debug".to_string(),
        _ => "trace".to_string(),
    }
}
```

- [ ] **Step 4: 接 run.rs tracing init**

`src-tauri/src/cli/run.rs` line 91 `let filter = tracing_subscriber::EnvFilter::try_new(&cli.log_level)` 改为：

```rust
    let level = crate::cli::args::effective_log_level(&cli.log_level, cli.verbose, cli.quiet);
    let filter = tracing_subscriber::EnvFilter::try_new(&level)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"));
```
（删掉原来的 `let filter = ...try_new(&cli.log_level)...` 那两行，替换为上面三行。）

- [ ] **Step 5: 运行测试 + 编译**

Run: `cargo test -p uecm --lib cli::args 2>&1 | tail -10`
Expected: PASS。

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/cli/args.rs src-tauri/src/cli/run.rs
git commit -m "feat(cli): --quiet/--verbose map to effective log level (spec §3.2)"
```

---

## Task 4: `--no-input` flag（识别 + 默认满足）

**说明：** 当前 CLI 无任何交互式 prompt（缺 `--yes` 直接报错退出，不阻塞等待；`--pass-stdin` 是调用方显式请求的读取，非"交互 prompt"）。因此 `--no-input` 的语义"绝不交互"**已是默认行为**。本 Task 只需让 flag 被识别 + 一个测试锁定语义，无需深度接线。

**Files:**
- Modify: `src-tauri/src/cli/args.rs`（字段已在 Task 1 加好，仅补测试）

- [ ] **Step 1: 写测试 — flag 能被 clap 解析**

在 `cli::args::tests` 加：

```rust
#[test]
fn no_input_flag_parses() {
    let cli = Cli::try_parse_from(["uecm-cli", "--no-input", "system", "version"]).unwrap();
    assert!(cli.no_input);
    let cli2 = Cli::try_parse_from(["uecm-cli", "system", "version"]).unwrap();
    assert!(!cli2.no_input);
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test -p uecm --lib cli::args::tests::no_input_flag_parses 2>&1 | tail -10`
Expected: PASS（字段 Task 1 已加，无需新代码）。若 FAIL 检查 Task 1 字段是否落地。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/cli/args.rs
git commit -m "test(cli): lock --no-input flag parsing (spec §3.2; CLI has no interactive prompts)"
```

---

## Task 5: 全量回归 + 实测纯净性快照

**Files:** 无改动，验证 Task 1-4。

- [ ] **Step 1: 全量测试**

Run: `cargo test -p uecm --lib 2>&1 | tail -15`
Expected: PASS，count 不低于改前。

- [ ] **Step 2: rebuild + help 快照**

Run:
```bash
cargo build -p uecm --bin uecm-cli 2>&1 | tail -3
NO_COLOR=1 src-tauri/target/debug/uecm-cli --help 2>&1 | sed -n '/Options:/,$p'
```
Expected: Options 区出现 `--output`、`-o`、`--no-color`、`--no-input`、`-q, --quiet`、`-v, --verbose`、`--config`、`--input-format`、`--json`。

- [ ] **Step 3: stdout 纯净性回归**

Run:
```bash
B=src-tauri/target/debug/uecm-cli
NO_COLOR=1 $B system version -o json 2>/dev/null | jq . >/dev/null && echo PURE || echo DIRTY
NO_COLOR=1 $B -q system version 2>/dev/null   # text 模式，安静
```
Expected: `PURE`；`-q` 下 stderr 无 warn 噪音。

- [ ] **Step 4: Commit（若有快照文档需要更新则一并）**

```bash
git commit --allow-empty -m "test(cli): P0 global-flag regression checkpoint"
```

---

## Task 6: `--config` 文件加载 + mode ≤ 0600 检查（spec §9）

**Files:**
- Create: `src-tauri/src/cli/config_file.rs`
- Modify: `src-tauri/src/cli/mod.rs`（注册模块）
- Modify: `src-tauri/src/cli/run.rs`（加载并合并 db_path / log_level / output 默认）

- [ ] **Step 1: 写失败测试 — 解析 + mode 检查**

新建 `src-tauri/src/cli/config_file.rs`，先只放测试 + 空签名：

```rust
//! `--config <path>` loader. Supports YAML / JSON (JSON is a YAML subset, parsed
//! by serde_yaml). Enforces file mode <= 0600 on unix per spec §9.

use crate::error::{UecmError, UecmResult};
use serde::Deserialize;
use std::path::Path;

/// CLI defaults loaded from a config file. All fields optional; explicit CLI
/// flags always win over file values.
#[derive(Debug, Deserialize, Default, PartialEq)]
pub struct FileConfig {
    pub db_path: Option<String>,
    pub log_level: Option<String>,
    pub output: Option<String>,
}

pub fn load(path: &Path) -> UecmResult<FileConfig> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parses_yaml_fields() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        write!(f, "db_path: /tmp/x.db\nlog_level: debug\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(f.path(), std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        let cfg = load(f.path()).unwrap();
        assert_eq!(cfg.db_path.as_deref(), Some("/tmp/x.db"));
        assert_eq!(cfg.log_level.as_deref(), Some("debug"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_world_readable_config() {
        use std::os::unix::fs::PermissionsExt;
        let mut f = tempfile::NamedTempFile::new().unwrap();
        write!(f, "log_level: info\n").unwrap();
        std::fs::set_permissions(f.path(), std::fs::Permissions::from_mode(0o644)).unwrap();
        let err = load(f.path()).unwrap_err();
        assert!(matches!(err, UecmError::Configuration(_)));
    }
}
```

- [ ] **Step 2: 注册模块 + 运行确认失败**

`src-tauri/src/cli/mod.rs` 在 `pub mod credential_args;` 后加一行 `pub mod config_file;`。

Run: `cargo test -p uecm --lib cli::config_file 2>&1 | head -20`
Expected: FAIL — `unimplemented!()` panic。

- [ ] **Step 3: 实现 load**

把 `config_file.rs` 的 `load` 替换为：

```rust
pub fn load(path: &Path) -> UecmResult<FileConfig> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(path)
            .map_err(|e| UecmError::Configuration(format!("config {}: {}", path.display(), e)))?
            .permissions()
            .mode();
        if mode & 0o077 != 0 {
            return Err(UecmError::Configuration(format!(
                "config file {} is too permissive (mode {:o}); chmod 600 it (spec §9)",
                path.display(),
                mode & 0o777
            )));
        }
    }
    let text = std::fs::read_to_string(path)
        .map_err(|e| UecmError::Configuration(format!("read config {}: {}", path.display(), e)))?;
    serde_yaml::from_str(&text)
        .map_err(|e| UecmError::Configuration(format!("parse config {}: {}", path.display(), e)))
}
```

- [ ] **Step 4: 运行测试**

Run: `cargo test -p uecm --lib cli::config_file 2>&1 | tail -10`
Expected: PASS（unix 上两测试都跑；非 unix 只跑 `parses_yaml_fields`）。

- [ ] **Step 5: 在 run.rs 合并配置默认**

`src-tauri/src/cli/run.rs` `pub fn run(cli: Cli) -> i32 {` 之后、tracing init **之前**插入（注意：CLI 显式值优先，所以只在 CLI 未显式给时用 config 值）：

```rust
    // Load --config defaults (explicit CLI flags still win).
    let file_cfg = match &cli.config {
        Some(p) => match crate::cli::config_file::load(p) {
            Ok(c) => c,
            Err(e) => return finish_error(&e, cli.json),
        },
        None => crate::cli::config_file::FileConfig::default(),
    };
    let mut cli = cli; // shadow as mutable to apply config fallbacks
    if cli.db_path.is_none() {
        cli.db_path = file_cfg.db_path.clone();
    }
    if cli.log_level == "warn" {
        // "warn" is the clap default; treat as "not explicitly set".
        if let Some(lvl) = &file_cfg.log_level {
            cli.log_level = lvl.clone();
        }
    }
    if cli.output.is_none() && !cli.json {
        if let Some(out) = file_cfg.output.as_deref() {
            cli.output = match out {
                "text" => Some(crate::cli::args::OutputFormat::Text),
                "json" => Some(crate::cli::args::OutputFormat::Json),
                "ndjson" | "stream-json" => Some(crate::cli::args::OutputFormat::Ndjson),
                _ => None,
            };
        }
    }
```

> 注：`run(cli: Cli)` 形参改为可重绑定，故上面 `let mut cli = cli;`。后续代码原样用 `cli`。

- [ ] **Step 6: 全量测试 + 实测**

Run:
```bash
cargo test -p uecm --lib 2>&1 | tail -10
cargo build -p uecm --bin uecm-cli 2>&1 | tail -2
printf 'log_level: error\n' > /tmp/uecm.yml && chmod 600 /tmp/uecm.yml
NO_COLOR=1 src-tauri/target/debug/uecm-cli --config /tmp/uecm.yml system version -o json 2>/dev/null | jq .
chmod 644 /tmp/uecm.yml
NO_COLOR=1 src-tauri/target/debug/uecm-cli --config /tmp/uecm.yml system version 2>&1 | head -2  # 应报 too permissive
```
Expected: 第一条正常输出 JSON；第二条 stderr 报 "too permissive"。

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/cli/config_file.rs src-tauri/src/cli/mod.rs src-tauri/src/cli/run.rs
git commit -m "feat(cli): --config YAML/JSON loader + mode<=0600 check (spec §3.2/§9)"
```

---

## Task 7: `--input-format` flag + stdin helper（spec §3.3，暂无消费者）

**YAGNI 标注：** 全仓当前没有消费嵌套 stdin JSON 的命令（唯一 stdin 读取是 `--pass-stdin` 的单行密码，见 `credential_args.rs:114-121`，不走结构化解析）。本 Task 按 spec §3.2/§3.3 强制项加上 flag（Task 1 已加字段）+ 一个可复用 helper，供未来命令调用；**不**强行造消费者。

**Files:**
- Create: `src-tauri/src/cli/stdin_input.rs`
- Modify: `src-tauri/src/cli/mod.rs`

- [ ] **Step 1: 写失败测试**

新建 `src-tauri/src/cli/stdin_input.rs`：

```rust
//! Structured stdin reader for `--input-format` (spec §3.3). Currently has no
//! command consumer; provided so future nested-input commands parse stdin via
//! one shared, format-aware path instead of ad-hoc flags.

use crate::cli::args::InputFormat;
use crate::error::{UecmError, UecmResult};
use std::io::Read;

/// Parse an in-memory byte buffer per the declared input format into a JSON value.
/// (Separated from stdin I/O so it is unit-testable without a real stdin.)
pub fn parse(buf: &str, fmt: InputFormat) -> UecmResult<serde_json::Value> {
    unimplemented!()
}

/// Read all of stdin and parse it. Thin I/O wrapper over [`parse`].
pub fn read_stdin(fmt: InputFormat) -> UecmResult<serde_json::Value> {
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .map_err(|e| UecmError::InvalidInput(format!("read stdin: {}", e)))?;
    parse(&buf, fmt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_json_object() {
        let v = parse(r#"{"a":1}"#, InputFormat::Json).unwrap();
        assert_eq!(v["a"], 1);
    }

    #[test]
    fn parse_yaml_object() {
        let v = parse("a: 1\n", InputFormat::Yaml).unwrap();
        assert_eq!(v["a"], 1);
    }

    #[test]
    fn parse_ndjson_to_array() {
        let v = parse("{\"a\":1}\n{\"a\":2}\n", InputFormat::Ndjson).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 2);
    }

    #[test]
    fn parse_bad_json_is_invalid_input() {
        let err = parse("{not json", InputFormat::Json).unwrap_err();
        assert!(matches!(err, UecmError::InvalidInput(_)));
    }
}
```

- [ ] **Step 2: 注册模块 + 确认失败**

`src-tauri/src/cli/mod.rs` 加 `pub mod stdin_input;`。

Run: `cargo test -p uecm --lib cli::stdin_input 2>&1 | head -15`
Expected: FAIL — `unimplemented!()`。

- [ ] **Step 3: 实现 parse**

替换 `parse`：

```rust
pub fn parse(buf: &str, fmt: InputFormat) -> UecmResult<serde_json::Value> {
    match fmt {
        InputFormat::Json => serde_json::from_str(buf)
            .map_err(|e| UecmError::InvalidInput(format!("parse json stdin: {}", e))),
        InputFormat::Yaml => serde_yaml::from_str(buf)
            .map_err(|e| UecmError::InvalidInput(format!("parse yaml stdin: {}", e))),
        InputFormat::Ndjson => {
            let mut items = Vec::new();
            for (n, line) in buf.lines().enumerate() {
                if line.trim().is_empty() {
                    continue;
                }
                let v: serde_json::Value = serde_json::from_str(line).map_err(|e| {
                    UecmError::InvalidInput(format!("parse ndjson stdin line {}: {}", n + 1, e))
                })?;
                items.push(v);
            }
            Ok(serde_json::Value::Array(items))
        }
    }
}
```

- [ ] **Step 4: 运行测试**

Run: `cargo test -p uecm --lib cli::stdin_input 2>&1 | tail -10`
Expected: PASS — 4 测试绿。

- [ ] **Step 5: 全量测试 + Commit**

```bash
cargo test -p uecm --lib 2>&1 | tail -8
git add src-tauri/src/cli/stdin_input.rs src-tauri/src/cli/mod.rs
git commit -m "feat(cli): --input-format stdin parser helper (spec §3.3; no consumer yet)"
```

---

## Self-Review

- **Spec 覆盖（§3.2）**：`--output/-o`(T1) / `--no-color`(T2) / `--quiet/--verbose`(T3) / `--no-input`(T4) / `--config`(T6) / `--input-format`(T7) 全有任务；`--help`/`--version`/`--log-level`/`--`/`--db-path` 既有；`--yes`/`--dry-run` 明确决策为保持 per-subcommand（header 已声明）。§3.5 三档输出 T1（json/ndjson 共用 emitter，单对象 buffering 留 P1）。§3.4 AI_AGENT 留 P2（`resolved_output` 已留接缝）。§9 config mode T6。
- **占位扫描**：无 TBD/TODO；两处 `unimplemented!()` 是 TDD 的"先红"步骤，均有后续 Step 给出完整实现。
- **类型一致**：`OutputFormat`{Text,Json,Ndjson}、`InputFormat`{Json,Yaml,Ndjson}、`use_color(bool,bool,bool)`、`effective_log_level(&str,u8,bool)`、`FileConfig{db_path,log_level,output}`、`config_file::load`、`stdin_input::{parse,read_stdin}` 全程一致。`run(cli)` 形参在 T6 改为 `let mut cli = cli;` shadow。
- **顺序依赖**：T1 临时 `use_color` 被 T2 替换、临时 `InputFormat` 占位被 T7 复用；T6 依赖 T1 的 `OutputFormat`。执行需按序。
