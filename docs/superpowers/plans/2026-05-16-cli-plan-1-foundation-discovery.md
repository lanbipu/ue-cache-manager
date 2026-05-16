# UECM CLI — Plan 1: Foundation + LAN Discovery

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up the `uecm-cli` binary, shared startup module, CLI framework (args + NDJSON / human output), `system` domain, and `machine` + `winrm` domains — enough to run `uecm-cli machine scan <cidr> --json` end-to-end on lanPC and validate the LAN-discovery capability.

**Architecture:** Second binary `uecm-cli` next to existing `uecm` (Tauri). Both share `uecm_lib` and a new `startup` module that resolves DB path + ps-scripts dir. CLI handlers call `core::*` / `data::*` directly, bypassing the `commands/*.rs` Tauri layer. Long-running operations stream NDJSON events to stdout (one JSON object per line); single-shot reads emit one JSON. Default mode is human-readable; `--json` flips to structured.

**Tech Stack:** Rust 1.75+, clap 4.5 (derive), tokio, rusqlite (bundled, WAL), serde_json, directories 5, atty 0.2. Tests use `#[cfg(test)] mod tests` per file; integration tests via `std::process::Command` in `tests/cli/`.

**Relates to:** [2026-05-16-cli-architecture-design.md](../specs/2026-05-16-cli-architecture-design.md)

---

## File Structure

### New files

```
src-tauri/src/startup.rs                     ← shared init (DB / ps-scripts paths)
src-tauri/src/bin/uecm-cli.rs                ← CLI entry point
src-tauri/src/cli/mod.rs                     ← cli module root
src-tauri/src/cli/args.rs                    ← clap derive structs
src-tauri/src/cli/output.rs                  ← Event enum + Emitter trait + impls
src-tauri/src/cli/run.rs                     ← top-level dispatch (args → handler)
src-tauri/src/cli/domain_system.rs           ← system subcommand handlers
src-tauri/src/cli/domain_machine.rs          ← machine subcommand handlers
src-tauri/src/cli/domain_winrm.rs            ← winrm subcommand handlers
src-tauri/tests/cli_smoke.rs                 ← end-to-end smoke test
```

### Modified files

```
src-tauri/Cargo.toml                         ← add [[bin]], clap, directories, atty
src-tauri/src/lib.rs                         ← use startup module; expose cli module
src-tauri/src/core/powershell.rs             ← script_path uses startup::resolve_ps_script_dir
```

### Untouched

`src/` (frontend), `src-tauri/src/main.rs`, `src-tauri/src/commands/*.rs`, all other `core::*` business logic.

---

## Phase 0 — Infrastructure

### Task 0.1: Add `uecm-cli` binary skeleton + dependencies

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/bin/uecm-cli.rs`

- [ ] **Step 1: Edit `src-tauri/Cargo.toml`** — append `[[bin]]` entries and new dependencies

Insert after the existing `[lib]` block (line 9-11) — declare both bins explicitly. Append the deps under `[dependencies]`:

```toml
[[bin]]
name = "uecm"
path = "src/main.rs"

[[bin]]
name = "uecm-cli"
path = "src/bin/uecm-cli.rs"
```

Add to `[dependencies]`:

```toml
clap = { version = "4.5", features = ["derive", "env"] }
directories = "5.0"
atty = "0.2"
```

- [ ] **Step 2: Create `src-tauri/src/bin/uecm-cli.rs`** — minimal hello binary to confirm build chain works

```rust
fn main() {
    println!("uecm-cli skeleton; not implemented yet");
    std::process::exit(0);
}
```

- [ ] **Step 3: Verify both binaries build**

Run: `cd src-tauri && cargo build --bin uecm && cargo build --bin uecm-cli`
Expected: both succeed; `uecm-cli` produces a runnable binary at `target/debug/uecm-cli`.

- [ ] **Step 4: Verify skeleton runs**

Run: `cd src-tauri && cargo run --bin uecm-cli`
Expected: prints `uecm-cli skeleton; not implemented yet` and exits 0.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/bin/uecm-cli.rs src-tauri/Cargo.lock
git commit -m "feat(cli): add uecm-cli binary skeleton and dependencies"
```

---

### Task 0.2: Shared `startup` module — DB path + DB open + ps-scripts dir

**Files:**
- Create: `src-tauri/src/startup.rs`
- Modify: `src-tauri/src/lib.rs` (declare module only; usage in Task 0.4)

- [ ] **Step 1: Create `src-tauri/src/startup.rs`** with full body

```rust
//! Shared startup paths and bootstrapping for both binaries (`uecm`, `uecm-cli`).
//!
//! Replaces the Tauri-only `app.path().app_data_dir()` and `app.path().resolve()`
//! lookups so the CLI can initialize without a Tauri Builder context.

use crate::data::{self, Db};
use crate::error::{UecmError, UecmResult};
use directories::ProjectDirs;
use std::env;
use std::path::{Path, PathBuf};

/// Resolves the SQLite DB path. Same location for UI and CLI so both share state.
/// Override with `UECM_DB_PATH` env var (used in tests and ad-hoc debug sessions).
pub fn resolve_db_path() -> UecmResult<PathBuf> {
    if let Ok(override_path) = env::var("UECM_DB_PATH") {
        return Ok(PathBuf::from(override_path));
    }
    let dirs = ProjectDirs::from("com", "uecm", "app").ok_or_else(|| {
        UecmError::Configuration("failed to resolve application data directory".into())
    })?;
    let data_dir = dirs.data_dir();
    std::fs::create_dir_all(data_dir).map_err(|e| {
        UecmError::Configuration(format!("create data dir {}: {}", data_dir.display(), e))
    })?;
    Ok(data_dir.join("uecm.sqlite"))
}

/// Opens the DB, sets WAL mode, runs idempotent migrations. Both binaries call this.
pub fn open_and_migrate_db(path: &Path) -> UecmResult<Db> {
    let db = data::open(path)?;
    {
        let mut conn = db.lock().unwrap();
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|e| UecmError::Configuration(format!("set WAL mode: {}", e)))?;
        data::schema::migrate(&mut conn)?;
    }
    Ok(db)
}

/// Resolves the directory containing PowerShell sidecar scripts.
/// Priority:
///   1. `UECM_PS_DIR` env var
///   2. `<exe-dir>/ps-scripts` (release binaries ship scripts alongside)
///   3. `<repo-root>/ps-scripts` via CARGO_MANIFEST_DIR (dev builds)
pub fn resolve_ps_script_dir() -> PathBuf {
    if let Ok(override_path) = env::var("UECM_PS_DIR") {
        return PathBuf::from(override_path);
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidate = parent.join("ps-scripts");
            if candidate.is_dir() {
                return candidate;
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("ps-scripts")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_db_path_uses_env_override() {
        let custom = "/tmp/test-override-uecm.sqlite";
        env::set_var("UECM_DB_PATH", custom);
        let path = resolve_db_path().unwrap();
        assert_eq!(path, PathBuf::from(custom));
        env::remove_var("UECM_DB_PATH");
    }

    #[test]
    fn resolve_db_path_falls_back_to_project_dirs() {
        env::remove_var("UECM_DB_PATH");
        let path = resolve_db_path().unwrap();
        assert!(path.ends_with("uecm.sqlite"));
        assert!(path.parent().unwrap().is_dir());
    }

    #[test]
    fn resolve_ps_script_dir_uses_env_override() {
        env::set_var("UECM_PS_DIR", "/tmp/test-ps");
        let path = resolve_ps_script_dir();
        assert_eq!(path, PathBuf::from("/tmp/test-ps"));
        env::remove_var("UECM_PS_DIR");
    }

    #[test]
    fn resolve_ps_script_dir_finds_repo_scripts_in_dev() {
        env::remove_var("UECM_PS_DIR");
        let path = resolve_ps_script_dir();
        assert!(
            path.ends_with("ps-scripts"),
            "expected path ending in ps-scripts, got {}",
            path.display()
        );
    }

    #[test]
    fn open_and_migrate_db_creates_and_migrates() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.sqlite");
        let db = open_and_migrate_db(&path).unwrap();
        let conn = db.lock().unwrap();
        let mode: String = conn
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
        assert_eq!(mode.to_lowercase(), "wal");
        let count: i64 = conn
            .query_row("SELECT count(*) FROM machines", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
}
```

- [ ] **Step 2: Declare the module in `src-tauri/src/lib.rs`**

Edit `src-tauri/src/lib.rs` — add `pub mod startup;` after the existing `pub mod` declarations (around line 4). Do not touch `run()` yet.

```rust
pub mod commands;
pub mod core;
pub mod data;
pub mod error;
pub mod startup;
```

- [ ] **Step 3: Check `UecmError::Configuration` exists**

Open `src-tauri/src/error.rs`. If `Configuration(String)` variant is missing, add it:

```rust
#[error("configuration error: {0}")]
Configuration(String),
```

Place it next to existing variants. If already present, skip this step.

- [ ] **Step 4: Run tests**

Run: `cd src-tauri && cargo test --lib startup::`
Expected: 5 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/startup.rs src-tauri/src/lib.rs src-tauri/src/error.rs
git commit -m "feat(startup): shared DB path + WAL migrate + ps-scripts dir resolver"
```

---

### Task 0.3: `core::powershell::script_path` uses `startup::resolve_ps_script_dir`

**Files:**
- Modify: `src-tauri/src/core/powershell.rs`

- [ ] **Step 1: Read existing `script_path` implementation**

Open `src-tauri/src/core/powershell.rs`. Locate the `pub fn script_path(name: &str) -> PathBuf` function (around line 30-50, the implementation that currently tries Tauri resource resolution).

- [ ] **Step 2: Add a fallback test that asserts env override**

Append to the existing `#[cfg(test)] mod tests` block (create one if absent):

```rust
#[test]
fn script_path_respects_env_override() {
    std::env::set_var("UECM_PS_DIR", "/tmp/test-ps-override");
    let p = super::script_path("foo.ps1");
    assert_eq!(p, std::path::PathBuf::from("/tmp/test-ps-override/foo.ps1"));
    std::env::remove_var("UECM_PS_DIR");
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cd src-tauri && cargo test --lib powershell::tests::script_path_respects_env_override`
Expected: FAIL (env override not honored yet).

- [ ] **Step 4: Rewrite `script_path` to call `startup::resolve_ps_script_dir`**

Replace the body of `pub fn script_path(name: &str) -> PathBuf` with:

```rust
pub fn script_path(name: &str) -> PathBuf {
    crate::startup::resolve_ps_script_dir().join(name)
}
```

Keep `vendor_path` and other functions as-is; if `vendor_path` follows the same pattern (Tauri resource lookup), apply the same simplification — point it at `<exe-dir>/vendor` or `<repo-root>/vendor`. Add a sibling helper if needed:

```rust
pub fn vendor_path(name: &str) -> PathBuf {
    if let Ok(over) = std::env::var("UECM_VENDOR_DIR") {
        return PathBuf::from(over).join(name);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidate = parent.join("vendor").join(name);
            if candidate.exists() {
                return candidate;
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("vendor")
        .join(name)
}
```

Remove any `tauri::AppHandle` parameter if present in the signature (caller chain in `commands/*.rs` should already not pass one — verify and adjust).

- [ ] **Step 5: Run all powershell unit tests**

Run: `cd src-tauri && cargo test --lib powershell::`
Expected: all pass including the new override test.

- [ ] **Step 6: Run full library test suite to make sure no regression**

Run: `cd src-tauri && cargo test --lib`
Expected: pre-existing tests still green.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/core/powershell.rs
git commit -m "refactor(powershell): script_path + vendor_path resolve via startup module"
```

---

### Task 0.4: `lib.rs::run` uses `startup::open_and_migrate_db`

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Replace the DB setup block inside `run()`**

In `src-tauri/src/lib.rs`, find the `tauri::Builder::default().setup(|app| { ... })` block. Replace the path-resolution + `data::open` + `migrate` lines with a single call:

Before (current):
```rust
.setup(|app| {
    let db_path: PathBuf = app
        .path()
        .app_data_dir()
        .expect("failed to resolve app_data_dir")
        .join("uecm.sqlite");

    std::fs::create_dir_all(db_path.parent().unwrap())?;
    let db = data::open(&db_path)?;
    {
        let mut conn = db.lock().unwrap();
        data::schema::migrate(&mut conn)?;
    }
    app.manage(db);
    // ...
})
```

After:
```rust
.setup(|app| {
    let db_path = crate::startup::resolve_db_path()
        .expect("failed to resolve DB path");
    let db = crate::startup::open_and_migrate_db(&db_path)
        .expect("failed to open / migrate DB");
    app.manage(db);
    app.manage(commands::ddc_pak::UeJobRegistry::default());
    tracing::info!("UECM started, database at {}", db_path.display());
    Ok(())
})
```

Remove the unused `PathBuf` import + `tauri::Manager` import only if they become orphaned (`Manager` is likely still needed for `app.manage` — keep it).

- [ ] **Step 2: Build the Tauri binary**

Run: `cd src-tauri && cargo build --bin uecm`
Expected: builds clean.

- [ ] **Step 3: Run full library tests**

Run: `cd src-tauri && cargo test --lib`
Expected: green.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "refactor(tauri): lib.rs::run delegates DB setup to startup module"
```

---

## Phase 1 — CLI Framework

### Task 1.1: `cli::args` — clap derive structures

**Files:**
- Create: `src-tauri/src/cli/mod.rs`
- Create: `src-tauri/src/cli/args.rs`
- Modify: `src-tauri/src/lib.rs` (declare module)

- [ ] **Step 1: Declare `cli` module in `lib.rs`**

Add to `src-tauri/src/lib.rs`:

```rust
pub mod cli;
```

- [ ] **Step 2: Create `src-tauri/src/cli/mod.rs`**

```rust
//! CLI implementation for `uecm-cli`. Bypasses Tauri runtime; calls core/data directly.

pub mod args;
pub mod output;
pub mod run;
pub mod domain_system;
pub mod domain_machine;
pub mod domain_winrm;
```

- [ ] **Step 3: Create `src-tauri/src/cli/args.rs`** with the top-level + every Plan 1 subcommand

```rust
//! clap-derive structures for all `uecm-cli` subcommands.

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "uecm-cli", version, about = "UECM command-line interface")]
pub struct Cli {
    /// Emit machine-readable JSON / NDJSON instead of human-friendly output.
    #[arg(long, global = true)]
    pub json: bool,

    /// Override DB path (otherwise resolved via startup module).
    #[arg(long, global = true, env = "UECM_DB_PATH")]
    pub db_path: Option<String>,

    /// Log level for tracing output to stderr.
    #[arg(long, global = true, default_value = "warn")]
    pub log_level: String,

    #[command(subcommand)]
    pub command: Domain,
}

#[derive(Subcommand, Debug)]
pub enum Domain {
    /// Diagnostic / self-test commands.
    System {
        #[command(subcommand)]
        action: SystemAction,
    },
    /// Machine inventory + discovery.
    Machine {
        #[command(subcommand)]
        action: MachineAction,
    },
    /// WinRM probe + onboarding.
    Winrm {
        #[command(subcommand)]
        action: WinrmAction,
    },
}

// ---------- system ----------
#[derive(Subcommand, Debug)]
pub enum SystemAction {
    /// Print binary + library version.
    Version,
    /// Print resolved SQLite DB path.
    DbPath,
    /// Print resolved ps-scripts directory.
    PsDir,
    /// Force-run schema migrations on the DB.
    MigrateDb,
    /// Round-trip a message through the PowerShell bridge.
    Echo { message: String },
}

// ---------- machine ----------
#[derive(Subcommand, Debug)]
pub enum MachineAction {
    /// List all known machines.
    List,
    /// Probe a CIDR for live hosts (ports 5985 / 445).
    Scan {
        /// CIDR (e.g. 192.168.10.0/24).
        cidr: String,
        /// Per-port TCP connect timeout (ms).
        #[arg(long, default_value_t = 1000)]
        timeout_ms: u64,
    },
    /// Add a machine to the inventory by IP / hostname.
    Add {
        #[arg(long)]
        ip: String,
        #[arg(long)]
        hostname: Option<String>,
    },
    /// Refresh a machine: WinRM probe + detect UE installs + GPUs.
    Refresh {
        /// Machine row id.
        id: i64,
        #[arg(long, group = "cred")]
        cred_alias: Option<String>,
        #[arg(long, group = "cred")]
        user: Option<String>,
        #[arg(long, requires = "user")]
        pass: Option<String>,
    },
    /// Show machine detail (UE installs, GPUs, last-seen).
    Detail { id: i64 },
    /// Delete a machine row.
    Delete {
        id: i64,
        #[arg(long)]
        yes: bool,
    },
    /// Rename a machine.
    Rename { id: i64, hostname: String },
}

// ---------- winrm ----------
#[derive(Subcommand, Debug)]
pub enum WinrmAction {
    /// Probe a single host's WinRM endpoint.
    Probe { host: String },
    /// Print the manual WinRM enable script (no-arg PS1 body).
    BootstrapScript {
        /// Write to this file instead of stdout.
        #[arg(long)]
        output: Option<String>,
    },
    /// Remote bootstrap WinRM via PsExec.
    Bootstrap {
        host: String,
        #[arg(long)]
        user: String,
        #[arg(long)]
        pass: String,
        #[arg(long)]
        enable_local_admin: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parses_machine_scan() {
        let cli = Cli::try_parse_from(["uecm-cli", "machine", "scan", "192.168.10.0/24"]).unwrap();
        match cli.command {
            Domain::Machine { action: MachineAction::Scan { cidr, timeout_ms } } => {
                assert_eq!(cidr, "192.168.10.0/24");
                assert_eq!(timeout_ms, 1000);
            }
            _ => panic!("wrong variant"),
        }
        assert!(!cli.json);
    }

    #[test]
    fn parses_global_json_flag_before_subcommand() {
        let cli = Cli::try_parse_from(["uecm-cli", "--json", "system", "version"]).unwrap();
        assert!(cli.json);
    }

    #[test]
    fn parses_machine_refresh_with_cred_alias() {
        let cli = Cli::try_parse_from([
            "uecm-cli", "machine", "refresh", "3", "--cred-alias", "winrm-admin",
        ])
        .unwrap();
        match cli.command {
            Domain::Machine { action: MachineAction::Refresh { id, cred_alias, user, pass } } => {
                assert_eq!(id, 3);
                assert_eq!(cred_alias.as_deref(), Some("winrm-admin"));
                assert!(user.is_none());
                assert!(pass.is_none());
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn rejects_pass_without_user() {
        let res = Cli::try_parse_from(["uecm-cli", "machine", "refresh", "3", "--pass", "p"]);
        assert!(res.is_err());
    }

    #[test]
    fn rejects_user_with_cred_alias() {
        let res = Cli::try_parse_from([
            "uecm-cli", "machine", "refresh", "3",
            "--cred-alias", "a", "--user", "u",
        ]);
        assert!(res.is_err());
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cd src-tauri && cargo test --lib cli::args::`
Expected: 5 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/cli/mod.rs src-tauri/src/cli/args.rs src-tauri/src/lib.rs
git commit -m "feat(cli): args module with clap derive for plan-1 subcommands"
```

---

### Task 1.2: `cli::output` — Event enum + Emitter trait + `NdjsonEmitter`

**Files:**
- Create: `src-tauri/src/cli/output.rs`

- [ ] **Step 1: Create `src-tauri/src/cli/output.rs`** with full body

```rust
//! Event types + emitter abstraction. NDJSON for `--json` mode; human-readable otherwise.
//!
//! Event taxonomy matches §8.2 of the design spec.

use crate::error::UecmError;
use serde::Serialize;
use std::io::{self, Write};

/// All events emitted to stdout. Long-running tasks emit one event per stream item.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    Started {
        task_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        task_id: Option<String>,
        #[serde(skip_serializing_if = "serde_json::Value::is_null")]
        metadata: serde_json::Value,
    },
    HostProbe {
        ip: String,
        winrm_open: bool,
        smb_open: bool,
    },
    Spawned {
        pid: i64,
        log_path: String,
    },
    LogLine {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        parsed_kind: Option<String>,
    },
    Progress {
        #[serde(skip_serializing_if = "Option::is_none")]
        pct: Option<f32>,
        label: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        current: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        total: Option<i64>,
    },
    ItemStarted {
        item_id: String,
        index: i64,
        total: i64,
    },
    ItemCompleted {
        item_id: String,
        index: i64,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    Finding {
        rule_id: String,
        severity: String,
        file_path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        section: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        key: Option<String>,
    },
    Cancelled {
        reason: String,
    },
    Error {
        code: String,
        message: String,
        #[serde(skip_serializing_if = "serde_json::Value::is_null")]
        details: serde_json::Value,
    },
    Completed {
        summary: serde_json::Value,
    },
}

/// Map `UecmError` to a stable string code for the `error` event.
pub fn error_code(err: &UecmError) -> &'static str {
    match err {
        UecmError::InvalidInput(_) => "invalid_input",
        UecmError::OperationFailed(_) => "operation_failed",
        UecmError::PowerShell(_) => "powershell_failed",
        UecmError::Configuration(_) => "configuration_error",
        UecmError::NotFound(_) => "not_found",
        UecmError::Sqlite(_) => "sqlite_error",
        UecmError::Io(_) => "io_error",
        _ => "internal_error",
    }
}

/// Process exit code mapping (§6.3 of spec).
pub fn exit_code_for(err: &UecmError) -> i32 {
    match err {
        UecmError::InvalidInput(_) => 2,
        UecmError::Configuration(_) => 3,
        UecmError::PowerShell(_) => 4,
        UecmError::NotFound(_) => 1,
        _ => 1,
    }
}

pub trait Emitter {
    fn emit_event(&mut self, event: &Event) -> io::Result<()>;
    fn emit_result<T: Serialize>(&mut self, value: &T) -> io::Result<()>;
    fn emit_error(&mut self, err: &UecmError);
}

pub struct NdjsonEmitter<W: Write> {
    pub writer: W,
}

impl<W: Write> NdjsonEmitter<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
}

impl<W: Write> Emitter for NdjsonEmitter<W> {
    fn emit_event(&mut self, event: &Event) -> io::Result<()> {
        serde_json::to_writer(&mut self.writer, event)?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()
    }

    fn emit_result<T: Serialize>(&mut self, value: &T) -> io::Result<()> {
        serde_json::to_writer(&mut self.writer, value)?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()
    }

    fn emit_error(&mut self, err: &UecmError) {
        let ev = Event::Error {
            code: error_code(err).into(),
            message: err.to_string(),
            details: serde_json::Value::Null,
        };
        let _ = self.emit_event(&ev);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ndjson_emits_one_line_per_event() {
        let mut buf = Vec::new();
        {
            let mut emitter = NdjsonEmitter::new(&mut buf);
            emitter
                .emit_event(&Event::HostProbe {
                    ip: "192.168.10.20".into(),
                    winrm_open: true,
                    smb_open: true,
                })
                .unwrap();
            emitter
                .emit_event(&Event::Completed {
                    summary: serde_json::json!({"hosts": 1}),
                })
                .unwrap();
        }
        let s = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = s.trim_end().split('\n').collect();
        assert_eq!(lines.len(), 2);
        let parsed: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(parsed["kind"], "host_probe");
        assert_eq!(parsed["ip"], "192.168.10.20");
        assert_eq!(parsed["winrm_open"], true);
    }

    #[test]
    fn ndjson_omits_none_fields() {
        let mut buf = Vec::new();
        {
            let mut emitter = NdjsonEmitter::new(&mut buf);
            emitter
                .emit_event(&Event::LogLine {
                    text: "hello".into(),
                    parsed_kind: None,
                })
                .unwrap();
        }
        let s = String::from_utf8(buf).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(s.trim_end()).unwrap();
        assert!(parsed.get("parsed_kind").is_none());
    }

    #[test]
    fn error_event_uses_stable_code() {
        let err = UecmError::InvalidInput("bad".into());
        assert_eq!(error_code(&err), "invalid_input");
        assert_eq!(exit_code_for(&err), 2);
    }
}
```

- [ ] **Step 2: Confirm `UecmError` variants used in `error_code` exist**

Open `src-tauri/src/error.rs`. If `NotFound(String)`, `Sqlite(...)`, or `Io(...)` variants do not exist, simplify the `error_code` match to use only the variants currently in the enum. Compile to verify.

- [ ] **Step 3: Run tests**

Run: `cd src-tauri && cargo test --lib cli::output::`
Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/cli/output.rs
git commit -m "feat(cli): NDJSON event emitter + error code mapping"
```

---

### Task 1.3: `cli::output` — `HumanEmitter`

**Files:**
- Modify: `src-tauri/src/cli/output.rs`

- [ ] **Step 1: Append `HumanEmitter` to `output.rs`**

After the `NdjsonEmitter` block, add:

```rust
pub struct HumanEmitter<W: Write, E: Write> {
    pub stdout: W,
    pub stderr: E,
    pub use_color: bool,
}

impl<W: Write, E: Write> HumanEmitter<W, E> {
    pub fn new(stdout: W, stderr: E, use_color: bool) -> Self {
        Self { stdout, stderr, use_color }
    }
}

impl<W: Write, E: Write> Emitter for HumanEmitter<W, E> {
    fn emit_event(&mut self, event: &Event) -> io::Result<()> {
        match event {
            Event::Started { task_type, .. } => {
                writeln!(self.stderr, "→ starting {}", task_type)?;
            }
            Event::HostProbe { ip, winrm_open, smb_open } => {
                let badges = format!(
                    "winrm={} smb={}",
                    if *winrm_open { "✓" } else { "✗" },
                    if *smb_open { "✓" } else { "✗" }
                );
                writeln!(self.stdout, "  {}  {}", ip, badges)?;
            }
            Event::Spawned { pid, log_path } => {
                writeln!(self.stderr, "→ spawned pid={} log={}", pid, log_path)?;
            }
            Event::LogLine { text, .. } => {
                writeln!(self.stderr, "  | {}", text)?;
            }
            Event::Progress { pct, label, current, total, .. } => {
                let suffix = match (current, total) {
                    (Some(c), Some(t)) => format!(" ({}/{})", c, t),
                    _ => String::new(),
                };
                match pct {
                    Some(p) => writeln!(self.stderr, "→ [{:>5.1}%] {}{}", p * 100.0, label, suffix)?,
                    None => writeln!(self.stderr, "→ {}{}", label, suffix)?,
                }
            }
            Event::ItemStarted { item_id, index, total } => {
                writeln!(self.stderr, "→ [{}/{}] {}", index + 1, total, item_id)?;
            }
            Event::ItemCompleted { item_id, ok, message, .. } => {
                let mark = if *ok { "✓" } else { "✗" };
                let suffix = message.as_deref().unwrap_or("");
                writeln!(self.stderr, "  {} {} {}", mark, item_id, suffix)?;
            }
            Event::Finding { rule_id, severity, file_path, section, key } => {
                writeln!(
                    self.stdout,
                    "  [{}] {} {} :: {} {}",
                    severity, rule_id, file_path,
                    section.as_deref().unwrap_or("-"),
                    key.as_deref().unwrap_or("-"),
                )?;
            }
            Event::Cancelled { reason } => {
                writeln!(self.stderr, "✗ cancelled: {}", reason)?;
            }
            Event::Error { code, message, .. } => {
                writeln!(self.stderr, "✗ error ({}): {}", code, message)?;
            }
            Event::Completed { summary } => {
                writeln!(self.stderr, "✓ done {}", summary)?;
            }
        }
        Ok(())
    }

    fn emit_result<T: Serialize>(&mut self, value: &T) -> io::Result<()> {
        // Default human rendering of arbitrary value: pretty JSON to stdout.
        // Individual handlers can take over with custom table rendering.
        let s = serde_json::to_string_pretty(value).unwrap_or_else(|_| "<unserializable>".into());
        writeln!(self.stdout, "{}", s)
    }

    fn emit_error(&mut self, err: &UecmError) {
        let _ = writeln!(self.stderr, "✗ error: {}", err);
    }
}
```

- [ ] **Step 2: Add a human emitter unit test**

Append to the existing `#[cfg(test)] mod tests` block in `output.rs`:

```rust
#[test]
fn human_emits_host_probe_with_badges() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    {
        let mut emitter = HumanEmitter::new(&mut stdout, &mut stderr, false);
        emitter
            .emit_event(&Event::HostProbe {
                ip: "192.168.10.20".into(),
                winrm_open: true,
                smb_open: false,
            })
            .unwrap();
    }
    let s = String::from_utf8(stdout).unwrap();
    assert!(s.contains("192.168.10.20"));
    assert!(s.contains("winrm=✓"));
    assert!(s.contains("smb=✗"));
}
```

- [ ] **Step 3: Run tests**

Run: `cd src-tauri && cargo test --lib cli::output::`
Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/cli/output.rs
git commit -m "feat(cli): human-readable emitter"
```

---

### Task 1.4: `cli::run` + `bin/uecm-cli.rs` main entry — wire args → dispatch

**Files:**
- Create: `src-tauri/src/cli/run.rs`
- Modify: `src-tauri/src/bin/uecm-cli.rs`

- [ ] **Step 1: Create `src-tauri/src/cli/run.rs`**

```rust
//! Top-level dispatch. Bin entry parses args, builds emitter, opens DB, hands off to domain.

use crate::cli::args::{Cli, Domain};
use crate::cli::output::{Emitter, HumanEmitter, NdjsonEmitter, exit_code_for};
use crate::cli::{domain_machine, domain_system, domain_winrm};
use crate::data::Db;
use crate::error::{UecmError, UecmResult};
use crate::startup;
use std::io::{self, Write};

pub struct Ctx<'a> {
    pub db: &'a Db,
    pub emitter: Box<dyn Emitter + 'a>,
    pub json_mode: bool,
}

pub fn run(cli: Cli) -> i32 {
    // tracing init
    let filter = tracing_subscriber::EnvFilter::try_new(&cli.log_level)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(io::stderr)
        .try_init();

    // DB
    let db_path = match cli.db_path.clone() {
        Some(p) => std::path::PathBuf::from(p),
        None => match startup::resolve_db_path() {
            Ok(p) => p,
            Err(e) => return finish_error(&e, cli.json),
        },
    };
    let db = match startup::open_and_migrate_db(&db_path) {
        Ok(db) => db,
        Err(e) => return finish_error(&e, cli.json),
    };

    // Emitter
    let json_mode = cli.json;
    let stdout = io::stdout();
    let stderr = io::stderr();
    let emitter: Box<dyn Emitter> = if json_mode {
        Box::new(NdjsonEmitter::new(stdout.lock()))
    } else {
        let color = atty::is(atty::Stream::Stdout);
        Box::new(HumanEmitter::new(stdout.lock(), stderr.lock(), color))
    };

    let mut ctx = Ctx { db: &db, emitter, json_mode };

    let result = match cli.command {
        Domain::System { action } => domain_system::handle(&mut ctx, action),
        Domain::Machine { action } => domain_machine::handle(&mut ctx, action),
        Domain::Winrm { action } => domain_winrm::handle(&mut ctx, action),
    };

    match result {
        Ok(()) => 0,
        Err(e) => {
            ctx.emitter.emit_error(&e);
            exit_code_for(&e)
        }
    }
}

fn finish_error(err: &UecmError, json: bool) -> i32 {
    if json {
        let mut e = NdjsonEmitter::new(io::stdout().lock());
        e.emit_error(err);
    } else {
        let _ = writeln!(io::stderr(), "✗ {}", err);
    }
    exit_code_for(err)
}
```

- [ ] **Step 2: Create stub handler files (so `run.rs` compiles)**

Create `src-tauri/src/cli/domain_system.rs`:

```rust
use crate::cli::args::SystemAction;
use crate::cli::run::Ctx;
use crate::error::{UecmError, UecmResult};

pub fn handle(_ctx: &mut Ctx<'_>, _action: SystemAction) -> UecmResult<()> {
    Err(UecmError::OperationFailed("system: not yet implemented".into()))
}
```

Create `src-tauri/src/cli/domain_machine.rs` and `src-tauri/src/cli/domain_winrm.rs` with the same shape (substitute the `system` literal). These stubs will be filled in Phase 2 / 3.

- [ ] **Step 3: Replace `src-tauri/src/bin/uecm-cli.rs` content**

```rust
//! `uecm-cli` entry point. Parses args, hands off to `uecm_lib::cli::run`.

use clap::Parser;
use uecm_lib::cli::args::Cli;
use uecm_lib::cli::run;

fn main() {
    let cli = Cli::parse();
    let code = run::run(cli);
    std::process::exit(code);
}
```

- [ ] **Step 4: Build both binaries**

Run: `cd src-tauri && cargo build --bin uecm && cargo build --bin uecm-cli`
Expected: both succeed.

- [ ] **Step 5: Verify CLI binary runs and shows help**

Run: `cd src-tauri && cargo run --bin uecm-cli -- --help`
Expected: clap renders top-level help with `system`, `machine`, `winrm` subcommands.

- [ ] **Step 6: Verify subcommand stub error**

Run: `cd src-tauri && cargo run --bin uecm-cli -- system version`
Expected: prints `✗ error: operation failed: system: not yet implemented` and exits non-zero.

- [ ] **Step 7: Verify JSON stub error**

Run: `cd src-tauri && cargo run --bin uecm-cli -- --json system version`
Expected: stdout has one NDJSON line with `"kind":"error"`, exit non-zero.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/cli/ src-tauri/src/bin/uecm-cli.rs
git commit -m "feat(cli): top-level dispatch + emitter wiring + stub handlers"
```

---

## Phase 2 — `system` Domain

### Task 2.1: `system version` / `db-path` / `ps-dir`

**Files:**
- Modify: `src-tauri/src/cli/domain_system.rs`

- [ ] **Step 1: Write test (`system version` happy path)**

Replace stub `src-tauri/src/cli/domain_system.rs` body with the dispatch shell + first impl. Append `#[cfg(test)] mod tests` at the bottom.

```rust
//! `uecm-cli system <action>` handlers.

use crate::cli::args::SystemAction;
use crate::cli::output::Event;
use crate::cli::run::Ctx;
use crate::error::UecmResult;
use crate::startup;
use serde::Serialize;

#[derive(Serialize)]
struct VersionInfo {
    binary: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
struct PathInfo {
    path: String,
}

pub fn handle(ctx: &mut Ctx<'_>, action: SystemAction) -> UecmResult<()> {
    match action {
        SystemAction::Version => version(ctx),
        SystemAction::DbPath => db_path(ctx),
        SystemAction::PsDir => ps_dir(ctx),
        SystemAction::MigrateDb => migrate_db(ctx),
        SystemAction::Echo { message } => echo(ctx, &message),
    }
}

fn version(ctx: &mut Ctx<'_>) -> UecmResult<()> {
    let info = VersionInfo { binary: "uecm-cli", version: env!("CARGO_PKG_VERSION") };
    ctx.emitter.emit_result(&info).ok();
    Ok(())
}

fn db_path(ctx: &mut Ctx<'_>) -> UecmResult<()> {
    let path = startup::resolve_db_path()?;
    let info = PathInfo { path: path.to_string_lossy().into() };
    ctx.emitter.emit_result(&info).ok();
    Ok(())
}

fn ps_dir(ctx: &mut Ctx<'_>) -> UecmResult<()> {
    let path = startup::resolve_ps_script_dir();
    let info = PathInfo { path: path.to_string_lossy().into() };
    ctx.emitter.emit_result(&info).ok();
    Ok(())
}

fn migrate_db(ctx: &mut Ctx<'_>) -> UecmResult<()> {
    // open_and_migrate_db at startup already ran; running again is a no-op.
    let path = startup::resolve_db_path()?;
    let _ = startup::open_and_migrate_db(&path)?;
    let summary = serde_json::json!({ "migrated": true, "path": path.to_string_lossy() });
    ctx.emitter.emit_event(&Event::Completed { summary }).ok();
    Ok(())
}

fn echo(_ctx: &mut Ctx<'_>, _message: &str) -> UecmResult<()> {
    // Implemented in Task 2.3.
    Err(crate::error::UecmError::OperationFailed("echo: pending Task 2.3".into()))
}
```

- [ ] **Step 2: Run the build to catch compile errors**

Run: `cd src-tauri && cargo build --bin uecm-cli`
Expected: builds clean.

- [ ] **Step 3: End-to-end smoke — version**

Run: `cd src-tauri && cargo run --bin uecm-cli -- --json system version`
Expected: stdout has one JSON line with `"binary":"uecm-cli"` and a `version` field matching `Cargo.toml`.

- [ ] **Step 4: End-to-end smoke — db-path**

Run: `cd src-tauri && cargo run --bin uecm-cli -- --json system db-path`
Expected: stdout has `{"path":"<some absolute path>/uecm.sqlite"}`. The path should exist (the file may not, but its parent directory must).

- [ ] **Step 5: End-to-end smoke — ps-dir**

Run: `cd src-tauri && cargo run --bin uecm-cli -- --json system ps-dir`
Expected: stdout has `{"path":"…/ps-scripts"}`.

- [ ] **Step 6: Verify human mode**

Run: `cd src-tauri && cargo run --bin uecm-cli -- system db-path`
Expected: pretty-printed JSON object on stdout (human emitter falls back to pretty JSON for `emit_result`).

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/cli/domain_system.rs
git commit -m "feat(cli): system version / db-path / ps-dir subcommands"
```

---

### Task 2.2: `system migrate-db` (verify against fresh DB)

**Files:** (no new — covered in Task 2.1 already; this task is verification only)

- [ ] **Step 1: Verify against a clean override DB**

```bash
cd src-tauri
TMP_DB="$(mktemp -t uecm-test.XXXXXX.sqlite)"
UECM_DB_PATH="$TMP_DB" cargo run --bin uecm-cli -- --json system migrate-db
```
Expected: stdout has `{"kind":"completed","summary":{"migrated":true,"path":"…"}}`.

- [ ] **Step 2: Verify DB has expected tables**

```bash
sqlite3 "$TMP_DB" '.tables' 2>/dev/null || echo "(sqlite3 not installed — skip)"
```
Expected (if sqlite3 available): includes `machines`, `machine_ue_installs`, `machine_gpus`, `credentials`, `scan_runs`, `ini_findings`, etc.

Skip cleanup; the temp DB is fine.

- [ ] **Step 3: Commit (none — no code change; this is verification only)**

---

### Task 2.3: `system echo` (PowerShell bridge round-trip)

**Files:**
- Modify: `src-tauri/src/cli/domain_system.rs`

- [ ] **Step 1: Add a test asserting echo round-trips on Windows, errors gracefully on non-Windows**

Append to `domain_system.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::output::{Emitter, NdjsonEmitter};
    use crate::data::open_in_memory;

    fn make_ctx<'a>(buf: &'a mut Vec<u8>, db: &'a crate::data::Db) -> Ctx<'a> {
        let emitter: Box<dyn Emitter> = Box::new(NdjsonEmitter::new(buf));
        Ctx { db, emitter, json_mode: true }
    }

    #[test]
    fn version_emits_binary_name_and_version() {
        let db = open_in_memory().unwrap();
        let mut buf = Vec::new();
        let mut ctx = make_ctx(&mut buf, &db);
        version(&mut ctx).unwrap();
        let s = String::from_utf8(buf).unwrap();
        let v: serde_json::Value = serde_json::from_str(s.trim_end()).unwrap();
        assert_eq!(v["binary"], "uecm-cli");
        assert!(v["version"].is_string());
    }

    #[cfg(not(windows))]
    #[test]
    fn echo_returns_powershell_error_on_non_windows() {
        let db = open_in_memory().unwrap();
        let mut buf = Vec::new();
        let mut ctx = make_ctx(&mut buf, &db);
        let result = echo(&mut ctx, "hello");
        assert!(matches!(result, Err(crate::error::UecmError::PowerShell(_))));
    }
}
```

(Adjust `make_ctx` lifetime if borrow-checker complains by separating mutable borrows.)

- [ ] **Step 2: Replace stubbed `echo` body**

Replace the `fn echo(...)` placeholder in `domain_system.rs` with:

```rust
fn echo(ctx: &mut Ctx<'_>, message: &str) -> UecmResult<()> {
    #[derive(serde::Deserialize, serde::Serialize)]
    struct EchoOut { ok: bool, message: String }
    let result: EchoOut = crate::core::powershell::run_json(
        &crate::core::powershell::script_path("test-echo.ps1"),
        &["-Message", message],
    )?;
    ctx.emitter.emit_result(&result).ok();
    Ok(())
}
```

- [ ] **Step 3: Run unit tests (host-side)**

Run: `cd src-tauri && cargo test --lib cli::domain_system::`
Expected: passes on macOS (echo errors with `PowerShell` variant as asserted); on Windows the `version_emits_*` test still passes (the non-windows test is skipped).

- [ ] **Step 4: End-to-end smoke (Windows-only — defer until lanPC tests run)**

On lanPC (or any Windows host with `pwsh.exe`):
```
uecm-cli --json system echo "hello cli"
```
Expected: stdout has `{"ok":true,"message":"hello cli"}` (assuming `test-echo.ps1` follows the existing pattern of echoing its `-Message`).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/cli/domain_system.rs
git commit -m "feat(cli): system echo via PowerShell bridge"
```

---

## Phase 3 — `machine` + `winrm` Domains

### Task 3.1: `winrm probe` + `machine list`

**Files:**
- Modify: `src-tauri/src/cli/domain_winrm.rs`
- Modify: `src-tauri/src/cli/domain_machine.rs`

- [ ] **Step 1: Implement `domain_winrm.rs`**

```rust
//! `uecm-cli winrm <action>` handlers.

use crate::cli::args::WinrmAction;
use crate::cli::run::Ctx;
use crate::core::{bootstrap, winrm};
use crate::error::UecmResult;
use serde::Serialize;

#[derive(Serialize)]
struct ProbeOut {
    host: String,
    ok: bool,
    message: String,
    latency_ms: i64,
}

pub fn handle(ctx: &mut Ctx<'_>, action: WinrmAction) -> UecmResult<()> {
    match action {
        WinrmAction::Probe { host } => probe(ctx, &host),
        WinrmAction::BootstrapScript { output } => bootstrap_script(ctx, output),
        WinrmAction::Bootstrap { host, user, pass, enable_local_admin } => {
            bootstrap_remote(ctx, &host, &user, &pass, enable_local_admin)
        }
    }
}

fn probe(ctx: &mut Ctx<'_>, host: &str) -> UecmResult<()> {
    let result = winrm::probe(host)?;
    let out = ProbeOut {
        host: host.into(),
        ok: result.ok,
        message: result.message,
        latency_ms: result.latency_ms,
    };
    ctx.emitter.emit_result(&out).ok();
    Ok(())
}

fn bootstrap_script(ctx: &mut Ctx<'_>, output_path: Option<String>) -> UecmResult<()> {
    let body = bootstrap::manual_winrm_script();
    match output_path {
        Some(p) => {
            std::fs::write(&p, &body).map_err(|e| {
                crate::error::UecmError::Configuration(format!("write {}: {}", p, e))
            })?;
            let summary = serde_json::json!({ "written_to": p, "bytes": body.len() });
            ctx.emitter
                .emit_event(&crate::cli::output::Event::Completed { summary })
                .ok();
        }
        None => {
            // Print raw script to stdout (no JSON wrapping — caller redirects to .ps1)
            print!("{}", body);
        }
    }
    Ok(())
}

fn bootstrap_remote(
    ctx: &mut Ctx<'_>,
    host: &str,
    user: &str,
    pass: &str,
    enable_local_admin: bool,
) -> UecmResult<()> {
    let result = bootstrap::enable_winrm_with_psexec(host, user, pass, enable_local_admin)?;
    ctx.emitter.emit_result(&result).ok();
    Ok(())
}
```

- [ ] **Step 2: Implement first slice of `domain_machine.rs` — list + stub others**

```rust
//! `uecm-cli machine <action>` handlers.

use crate::cli::args::MachineAction;
use crate::cli::run::Ctx;
use crate::data::machines;
use crate::error::{UecmError, UecmResult};

pub fn handle(ctx: &mut Ctx<'_>, action: MachineAction) -> UecmResult<()> {
    match action {
        MachineAction::List => list(ctx),
        MachineAction::Scan { .. } => Err(UecmError::OperationFailed("scan: pending Task 3.3".into())),
        MachineAction::Add { .. } => Err(UecmError::OperationFailed("add: pending Task 3.2".into())),
        MachineAction::Refresh { .. } => {
            Err(UecmError::OperationFailed("refresh: pending Task 3.4".into()))
        }
        MachineAction::Detail { .. } => {
            Err(UecmError::OperationFailed("detail: pending Task 3.2".into()))
        }
        MachineAction::Delete { .. } => {
            Err(UecmError::OperationFailed("delete: pending Task 3.2".into()))
        }
        MachineAction::Rename { .. } => {
            Err(UecmError::OperationFailed("rename: pending Task 3.2".into()))
        }
    }
}

fn list(ctx: &mut Ctx<'_>) -> UecmResult<()> {
    let rows = machines::list_all(ctx.db)?;
    ctx.emitter.emit_result(&rows).ok();
    Ok(())
}
```

If `machines::list_all` does not exist, locate the equivalent function name in `src-tauri/src/data/machines.rs` (likely `list` or `all`) and adjust. Do not add a new data function; use what's there.

- [ ] **Step 3: Build and smoke-test `machine list` against an empty DB**

```bash
cd src-tauri
TMP_DB="$(mktemp -t uecm-test.XXXXXX.sqlite)"
UECM_DB_PATH="$TMP_DB" cargo run --bin uecm-cli -- --json machine list
```
Expected: stdout has `[]` (empty array, fresh DB).

- [ ] **Step 4: Smoke-test `winrm probe` on a definitely-unreachable host (non-Windows)**

```bash
cd src-tauri && cargo run --bin uecm-cli -- --json winrm probe 192.0.2.1
```
Expected on macOS: stderr emits error event with `code: "powershell_failed"` (WinRM is Windows-only). Exit code is 4. This is the documented behavior.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/cli/domain_machine.rs src-tauri/src/cli/domain_winrm.rs
git commit -m "feat(cli): winrm probe + machine list + winrm bootstrap-script"
```

---

### Task 3.2: `machine add` / `delete` / `rename` / `detail`

**Files:**
- Modify: `src-tauri/src/cli/domain_machine.rs`

- [ ] **Step 1: Replace the stubbed `Add`, `Delete`, `Rename`, `Detail` arms**

Locate the `handle` match in `domain_machine.rs`. Replace those four arms with real handlers, and add the helper fns at the bottom:

```rust
pub fn handle(ctx: &mut Ctx<'_>, action: MachineAction) -> UecmResult<()> {
    match action {
        MachineAction::List => list(ctx),
        MachineAction::Scan { .. } => Err(UecmError::OperationFailed("scan: pending Task 3.3".into())),
        MachineAction::Add { ip, hostname } => add(ctx, &ip, hostname.as_deref()),
        MachineAction::Refresh { .. } => {
            Err(UecmError::OperationFailed("refresh: pending Task 3.4".into()))
        }
        MachineAction::Detail { id } => detail(ctx, id),
        MachineAction::Delete { id, yes } => delete(ctx, id, yes),
        MachineAction::Rename { id, hostname } => rename(ctx, id, &hostname),
    }
}

fn add(ctx: &mut Ctx<'_>, ip: &str, hostname: Option<&str>) -> UecmResult<()> {
    let host = hostname.unwrap_or(ip);
    let machine = crate::data::Machine::new(host, ip);
    let id = machines::insert(ctx.db, &machine)?;
    let summary = serde_json::json!({ "id": id, "ip": ip, "hostname": host });
    ctx.emitter
        .emit_event(&crate::cli::output::Event::Completed { summary })
        .ok();
    Ok(())
}

fn detail(ctx: &mut Ctx<'_>, id: i64) -> UecmResult<()> {
    let machine = machines::find_by_id(ctx.db, id)?
        .ok_or_else(|| UecmError::NotFound(format!("machine id={}", id)))?;
    let ue_installs = crate::data::machine_ue_installs::list_for_machine(ctx.db, id)?;
    let gpus = crate::data::machine_gpus::list_for_machine(ctx.db, id)?;
    let payload = serde_json::json!({
        "machine": machine,
        "ue_installs": ue_installs,
        "gpus": gpus,
    });
    ctx.emitter.emit_result(&payload).ok();
    Ok(())
}

fn delete(ctx: &mut Ctx<'_>, id: i64, yes: bool) -> UecmResult<()> {
    if !yes {
        return Err(UecmError::InvalidInput(
            "delete is destructive; pass --yes to confirm".into(),
        ));
    }
    machines::delete(ctx.db, id)?;
    let summary = serde_json::json!({ "id": id, "deleted": true });
    ctx.emitter
        .emit_event(&crate::cli::output::Event::Completed { summary })
        .ok();
    Ok(())
}

fn rename(ctx: &mut Ctx<'_>, id: i64, hostname: &str) -> UecmResult<()> {
    machines::rename(ctx.db, id, hostname)?;
    let summary = serde_json::json!({ "id": id, "hostname": hostname });
    ctx.emitter
        .emit_event(&crate::cli::output::Event::Completed { summary })
        .ok();
    Ok(())
}
```

If `machines::find_by_id`, `machines::delete`, `machines::rename`, `machine_ue_installs::list_for_machine`, or `machine_gpus::list_for_machine` have different names, grep `src-tauri/src/data/` and adjust to existing names. Do not add new data fns.

- [ ] **Step 2: Write a round-trip unit test (add → list → detail → rename → delete)**

Create `src-tauri/src/cli/tests_machine.rs` — but to keep tests inside the file, append at the bottom of `domain_machine.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::output::{Emitter, NdjsonEmitter};
    use crate::data::open_in_memory;

    fn run_round_trip() -> String {
        let db = open_in_memory().unwrap();
        {
            let mut conn = db.lock().unwrap();
            crate::data::schema::migrate(&mut conn).unwrap();
        }
        let mut buf: Vec<u8> = Vec::new();
        let emitter: Box<dyn Emitter> = Box::new(NdjsonEmitter::new(&mut buf));
        let mut ctx = Ctx { db: &db, emitter, json_mode: true };

        add(&mut ctx, "10.0.0.5", Some("render-01")).unwrap();
        list(&mut ctx).unwrap();
        // can't directly access inserted id without reading buf — fetch via list_all
        drop(ctx);
        let rows = crate::data::machines::list_all(&db).unwrap();
        let id = rows[0].id.unwrap();
        let mut buf2: Vec<u8> = Vec::new();
        let emitter2: Box<dyn Emitter> = Box::new(NdjsonEmitter::new(&mut buf2));
        let mut ctx = Ctx { db: &db, emitter: emitter2, json_mode: true };
        detail(&mut ctx, id).unwrap();
        rename(&mut ctx, id, "render-renamed").unwrap();
        delete(&mut ctx, id, true).unwrap();
        String::from_utf8(buf2).unwrap()
    }

    #[test]
    fn machine_round_trip_via_handlers() {
        let out = run_round_trip();
        assert!(out.contains("\"hostname\":\"render-01\""));
        assert!(out.contains("render-renamed"));
        assert!(out.contains("\"deleted\":true"));
    }

    #[test]
    fn delete_without_yes_flag_returns_invalid_input() {
        let db = open_in_memory().unwrap();
        {
            let mut conn = db.lock().unwrap();
            crate::data::schema::migrate(&mut conn).unwrap();
        }
        let mut buf: Vec<u8> = Vec::new();
        let emitter: Box<dyn Emitter> = Box::new(NdjsonEmitter::new(&mut buf));
        let mut ctx = Ctx { db: &db, emitter, json_mode: true };
        let result = delete(&mut ctx, 999, false);
        assert!(matches!(result, Err(UecmError::InvalidInput(_))));
    }
}
```

- [ ] **Step 3: Adjust function names if real data API differs**

Run `cd src-tauri && cargo build --bin uecm-cli`. If errors mention missing fns (e.g., `Machine::new`, `list_all`), open `src-tauri/src/data/machines.rs` and align names. Common existing names: `insert`, `list_all` or `list`, `find_by_id` or `get`, `delete`, `update_hostname` or `rename`. Use whatever exists.

- [ ] **Step 4: Run tests**

Run: `cd src-tauri && cargo test --lib cli::domain_machine::`
Expected: 2 tests pass.

- [ ] **Step 5: E2E smoke (in-memory env override)**

```bash
cd src-tauri
TMP_DB="$(mktemp -t uecm-test.XXXXXX.sqlite)"
UECM_DB_PATH="$TMP_DB" cargo run --bin uecm-cli -- --json machine add --ip 10.0.0.7 --hostname render-test
UECM_DB_PATH="$TMP_DB" cargo run --bin uecm-cli -- --json machine list
```
Expected: first command emits `{"kind":"completed","summary":{"id":1,...}}`; second emits `[{"id":1,"hostname":"render-test",...}]`.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/cli/domain_machine.rs
git commit -m "feat(cli): machine add / detail / delete / rename"
```

---

### Task 3.3: `machine scan` — NDJSON streaming

**Files:**
- Modify: `src-tauri/src/cli/domain_machine.rs`

- [ ] **Step 1: Replace the stubbed `Scan` arm + add handler**

In `domain_machine.rs`, update the `handle` match to dispatch `Scan` to a new `scan` fn. Add the function:

```rust
fn scan(ctx: &mut Ctx<'_>, cidr: &str, timeout_ms: u64) -> UecmResult<()> {
    use crate::cli::output::Event;
    ctx.emitter
        .emit_event(&Event::Started {
            task_type: "machine_scan".into(),
            task_id: None,
            metadata: serde_json::json!({ "cidr": cidr, "timeout_ms": timeout_ms }),
        })
        .ok();

    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| UecmError::Configuration(format!("tokio runtime: {}", e)))?;
    let hosts = runtime.block_on(crate::core::network::scan_cidr(cidr, timeout_ms))?;
    let total = hosts.len() as i64;
    for h in &hosts {
        ctx.emitter
            .emit_event(&Event::HostProbe {
                ip: h.ip.clone(),
                winrm_open: h.winrm_open,
                smb_open: h.smb_open,
            })
            .ok();
    }
    ctx.emitter
        .emit_event(&Event::Completed {
            summary: serde_json::json!({ "hosts": total }),
        })
        .ok();
    Ok(())
}
```

Update the match arm:
```rust
MachineAction::Scan { cidr, timeout_ms } => scan(ctx, &cidr, timeout_ms),
```

- [ ] **Step 2: Add a unit test using a tiny CIDR in TEST-NET-3**

Append to the `mod tests` block in `domain_machine.rs`:

```rust
#[test]
fn scan_emits_started_and_completed_events_for_unreachable_cidr() {
    let db = open_in_memory().unwrap();
    {
        let mut conn = db.lock().unwrap();
        crate::data::schema::migrate(&mut conn).unwrap();
    }
    let mut buf: Vec<u8> = Vec::new();
    let emitter: Box<dyn Emitter> = Box::new(NdjsonEmitter::new(&mut buf));
    let mut ctx = Ctx { db: &db, emitter, json_mode: true };
    // TEST-NET-3 /30 = 2 usable hosts; per-port timeout 200ms → completes in well under 2s.
    scan(&mut ctx, "203.0.113.0/30", 200).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert!(s.contains("\"kind\":\"started\""));
    assert!(s.contains("\"kind\":\"completed\""));
}
```

- [ ] **Step 3: Run tests**

Run: `cd src-tauri && cargo test --lib cli::domain_machine::scan_emits`
Expected: passes (CIDR is unreachable; we don't assert host_probe count because some networks intercept TEST-NET-3).

- [ ] **Step 4: E2E smoke — small CIDR on loopback**

```bash
cd src-tauri && cargo run --bin uecm-cli -- --json machine scan 127.0.0.1/32 --timeout-ms 200
```
Expected: stdout has a `started` line, possibly zero or one `host_probe`, then `completed`. (`/32` expands to 0 usable hosts in the strict sense; this exercises the empty path.)

Then try with the local subnet (on macOS this will likely find the gateway):

```bash
cargo run --bin uecm-cli -- --json machine scan 192.168.10.0/29 --timeout-ms 300
```
Expected: NDJSON stream with one `host_probe` per responding host, ending with `completed`.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/cli/domain_machine.rs
git commit -m "feat(cli): machine scan with NDJSON streaming"
```

---

### Task 3.4: `machine refresh` — WinRM + UE + GPU detection

**Files:**
- Modify: `src-tauri/src/cli/domain_machine.rs`

- [ ] **Step 1: Add credential resolution helper**

At the bottom of `domain_machine.rs` (above `mod tests`), add:

```rust
fn resolve_credential(
    cred_alias: Option<&str>,
    user: Option<&str>,
    pass: Option<&str>,
) -> UecmResult<Option<(String, String)>> {
    if let Some(alias) = cred_alias {
        let pw = crate::core::credentials::resolve_password(alias)?;
        let display_user = crate::data::credentials::find_username_by_alias(/* db lookup */)
            .unwrap_or_else(|| alias.to_string());
        return Ok(Some((display_user, pw)));
    }
    if let (Some(u), Some(p)) = (user, pass) {
        return Ok(Some((u.to_string(), p.to_string())));
    }
    Ok(None)
}
```

If `data::credentials::find_username_by_alias` does not exist, replace that call with a placeholder of `alias.to_string()` (the resolved username is informational here). Adjust based on actual data API.

- [ ] **Step 2: Replace the `Refresh` arm + add handler**

```rust
MachineAction::Refresh { id, cred_alias, user, pass } => {
    refresh(ctx, id, cred_alias.as_deref(), user.as_deref(), pass.as_deref())
}
```

Add the function:

```rust
fn refresh(
    ctx: &mut Ctx<'_>,
    id: i64,
    cred_alias: Option<&str>,
    user: Option<&str>,
    pass: Option<&str>,
) -> UecmResult<()> {
    use crate::cli::output::Event;
    let machine = machines::find_by_id(ctx.db, id)?
        .ok_or_else(|| UecmError::NotFound(format!("machine id={}", id)))?;
    let host = &machine.hostname;

    ctx.emitter
        .emit_event(&Event::Started {
            task_type: "machine_refresh".into(),
            task_id: Some(format!("machine:{}", id)),
            metadata: serde_json::json!({ "host": host }),
        })
        .ok();

    let _cred = resolve_credential(cred_alias, user, pass)?;
    // Note: detect_ue_versions / detect_gpus currently do not take credentials.
    // If they grow `_with_credential` variants, use them here.

    ctx.emitter
        .emit_event(&Event::Progress {
            pct: None,
            label: "winrm probe".into(),
            current: None,
            total: None,
        })
        .ok();
    let probe = crate::core::winrm::probe(host)?;
    if !probe.ok {
        return Err(UecmError::OperationFailed(format!(
            "winrm probe failed: {}",
            probe.message
        )));
    }

    ctx.emitter
        .emit_event(&Event::Progress {
            pct: None,
            label: "detect ue installs".into(),
            current: None,
            total: None,
        })
        .ok();
    let ue_versions = crate::core::discovery::detect_ue_versions(host)?;

    ctx.emitter
        .emit_event(&Event::Progress {
            pct: None,
            label: "detect gpus".into(),
            current: None,
            total: None,
        })
        .ok();
    let gpus = crate::core::discovery::detect_gpus(host)?;

    // Persist
    crate::data::machine_ue_installs::replace_all(ctx.db, id, &ue_versions)?;
    crate::data::machine_gpus::replace_all(ctx.db, id, &gpus)?;
    machines::touch_last_seen(ctx.db, id)?;

    let summary = serde_json::json!({
        "machine_id": id,
        "ue_versions": ue_versions.len(),
        "gpus": gpus.len(),
        "latency_ms": probe.latency_ms,
    });
    ctx.emitter.emit_event(&Event::Completed { summary }).ok();
    Ok(())
}
```

Function names like `replace_all` / `touch_last_seen` may not exist verbatim. Open `src-tauri/src/data/machine_ue_installs.rs`, `machine_gpus.rs`, and `machines.rs` and use the actual names. The pattern is: refresh wipes any existing rows for `machine_id` then inserts the freshly detected ones. If only `insert` + `delete_for_machine` exist, do those two calls. Do not add new fns.

- [ ] **Step 3: Build, then E2E on macOS**

Run: `cd src-tauri && cargo build --bin uecm-cli`
Expected: clean build.

Add a machine row, then refresh against a non-Windows context (will return `PowerShell` error — that's the expected dev-machine behavior):

```bash
cd src-tauri
TMP_DB="$(mktemp -t uecm-test.XXXXXX.sqlite)"
UECM_DB_PATH="$TMP_DB" cargo run --bin uecm-cli -- --json machine add --ip 192.0.2.1 --hostname does-not-exist
UECM_DB_PATH="$TMP_DB" cargo run --bin uecm-cli -- --json machine refresh 1
```
Expected: `started` event, then error event with `code: "powershell_failed"`, exit code 4. This proves the dispatch + event emission + error mapping.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/cli/domain_machine.rs
git commit -m "feat(cli): machine refresh with NDJSON event stream"
```

---

### Task 3.5: `winrm bootstrap` (verification only — already implemented in Task 3.1)

**Files:** (none — code shipped in Task 3.1)

- [ ] **Step 1: Verify `winrm bootstrap-script` produces a non-empty script**

```bash
cd src-tauri && cargo run --bin uecm-cli -- winrm bootstrap-script | wc -l
```
Expected: > 0 lines.

- [ ] **Step 2: Verify `--output` writes to disk**

```bash
cd src-tauri && cargo run --bin uecm-cli -- --json winrm bootstrap-script --output /tmp/uecm-bootstrap.ps1
cat /tmp/uecm-bootstrap.ps1 | head -5
```
Expected: JSON `completed` event on stdout; `/tmp/uecm-bootstrap.ps1` exists and starts with the same content as `ps-scripts/enable-winrm.ps1`.

- [ ] **Step 3: Verify `winrm bootstrap` returns `PowerShell` error on non-Windows**

```bash
cd src-tauri && cargo run --bin uecm-cli -- --json winrm bootstrap 192.0.2.1 --user a --pass b
```
Expected: error event with `code: "powershell_failed"` (matches existing core::bootstrap behavior on dev macOS).

- [ ] **Step 4: No commit (verification only)**

---

### Task 3.6: End-to-end smoke test (integration test file)

**Files:**
- Create: `src-tauri/tests/cli_smoke.rs`

- [ ] **Step 1: Create integration test**

```rust
//! End-to-end smoke tests for `uecm-cli`. Spawns the compiled binary.
//! Cross-platform — no PowerShell required for the assertions here.

use std::process::Command;

fn bin() -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("target");
    p.push(if cfg!(debug_assertions) { "debug" } else { "release" });
    p.push(if cfg!(windows) { "uecm-cli.exe" } else { "uecm-cli" });
    p
}

#[test]
fn version_subcommand_works() {
    let out = Command::new(bin())
        .args(["--json", "system", "version"])
        .output()
        .expect("spawn uecm-cli");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(stdout.trim_end()).unwrap();
    assert_eq!(v["binary"], "uecm-cli");
}

#[test]
fn machine_list_on_fresh_db_returns_empty_array() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_string_lossy().to_string();
    let out = Command::new(bin())
        .env("UECM_DB_PATH", &path)
        .args(["--json", "machine", "list"])
        .output()
        .expect("spawn");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(stdout.trim_end()).unwrap();
    assert_eq!(v, serde_json::Value::Array(vec![]));
}

#[test]
fn invalid_cidr_returns_invalid_input_exit_code() {
    let out = Command::new(bin())
        .args(["--json", "machine", "scan", "not-a-cidr"])
        .output()
        .expect("spawn");
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(2), "expected exit code 2 (invalid_input)");
}
```

- [ ] **Step 2: Build the binary before running tests (cargo test does not always build bins)**

Run: `cd src-tauri && cargo build --bin uecm-cli`

- [ ] **Step 3: Run integration tests**

Run: `cd src-tauri && cargo test --test cli_smoke`
Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/tests/cli_smoke.rs
git commit -m "test(cli): end-to-end smoke (version / list / invalid cidr)"
```

---

## Phase 4 — Final Verification (lanPC)

### Task 4.1: Build release CLI and deploy to lanPC

**Files:** none

- [ ] **Step 1: Build release on macOS (host can build for macOS only; lanPC builds Windows itself)**

Run: `cd src-tauri && cargo build --release --bin uecm-cli`
Expected: succeeds; produces `src-tauri/target/release/uecm-cli`.

- [ ] **Step 2: On lanPC, sync source and build Windows release**

`cargo build --bin uecm-cli` is **safe and distinct** from `pnpm tauri build`. It only compiles the `uecm-cli` binary target declared in `Cargo.toml`; it does not invoke `tauri::Builder` setup, does not require the frontend dev server (localhost:5173), and does not load `tauri.conf.json` capabilities. The user-memory warning against bare `cargo build --release` applies only to the `uecm` (Tauri) binary, not to `uecm-cli`.

Reuse the existing build directory on lanPC (`E:\uecm-plan4-test\`) — its cargo cache will accelerate the build.

```bash
# From mac (worktree path):
cd /Users/bip.lan/AIWorkspace/vp/ue-cache-manager/.claude/worktrees/feat+cli-architecture
COPYFILE_DISABLE=1 tar czf /tmp/uecm-cli-src.tgz \
  --exclude='node_modules' --exclude='target' --exclude='dist' \
  --exclude='src-tauri/target' --exclude='.git' \
  src-tauri/ ps-scripts/ vendor/
scp /tmp/uecm-cli-src.tgz lanpc@192.168.10.20:E:/uecm-plan4-test/
```

On lanPC (via ssh + powershell):
```powershell
cd E:\uecm-plan4-test
# Overlay new src on top of existing build dir (keeps cargo cache hot)
tar xzf uecm-cli-src.tgz
cd src-tauri
cargo build --release --bin uecm-cli
```

Expected: `E:\uecm-plan4-test\src-tauri\target\release\uecm-cli.exe` exists. Build should be fast (incremental) since the lib was already compiled here for plan-4.

- [ ] **Step 3: Place binary next to `ps-scripts` for path resolution**

```powershell
copy E:\uecm-cli-build\src-tauri\target\release\uecm-cli.exe C:\Tools\UECM\uecm-cli.exe
# uecm-cli will find ps-scripts at C:\Tools\UECM\ps-scripts\ (deployed alongside the UI)
```

- [ ] **Step 4: Smoke-test on lanPC**

```powershell
C:\Tools\UECM\uecm-cli.exe system version
C:\Tools\UECM\uecm-cli.exe system db-path
C:\Tools\UECM\uecm-cli.exe system ps-dir
C:\Tools\UECM\uecm-cli.exe --json system echo "hello from lanpc"
```

Expected: all four succeed. `system db-path` points at `%APPDATA%\com.uecm.app\uecm.sqlite` (the same DB the UI uses).

- [ ] **Step 5: No commit (deploy artifact, not source)**

---

### Task 4.2: LAN-discovery end-to-end verification

**Files:** none

- [ ] **Step 1: Scan the lanPC subnet**

```powershell
C:\Tools\UECM\uecm-cli.exe --json machine scan 192.168.10.0/24 --timeout-ms 1000 > C:\Temp\uecm-scan.ndjson
type C:\Temp\uecm-scan.ndjson
```

Expected: NDJSON stream:
- One `{"kind":"started",...}` event
- One `{"kind":"host_probe","ip":"…","winrm_open":…,"smb_open":…}` per responding host (should include `192.168.10.20` itself with `winrm_open=true`)
- One `{"kind":"completed","summary":{"hosts":N}}` event

- [ ] **Step 2: Add lanPC itself + refresh**

```powershell
C:\Tools\UECM\uecm-cli.exe --json machine add --ip 192.168.10.20 --hostname lanPC
# Note the returned id, then:
C:\Tools\UECM\uecm-cli.exe --json machine refresh 1
```

Expected: NDJSON stream with `started` → `progress` (winrm probe) → `progress` (detect ue installs) → `progress` (detect gpus) → `completed`. The `completed` summary shows non-zero `ue_versions` (UE 5.x installed on lanPC) and `gpus` count.

- [ ] **Step 3: Verify persistence**

```powershell
C:\Tools\UECM\uecm-cli.exe --json machine detail 1
```

Expected: JSON object with `machine`, `ue_installs` (≥ 1 entry), `gpus` (≥ 1 entry showing RTX GPU + correct VRAM in the 10240 range).

- [ ] **Step 4: Verify DB sharing with UI**

Launch the UECM UI (`C:\Tools\UECM\uecm.exe`). The machines view should show `lanPC` with the same UE installs / GPU info written by the CLI in step 2.

- [ ] **Step 5: Verify the four-spec verification criteria from §12**

Walk through the validation criteria from the spec §12 and tick:
- ✓ macOS + Windows build clean
- ✓ Tauri UI build unaffected
- ✓ `machine scan` NDJSON correct
- ✓ `machine refresh` returns UE + GPU info with correct VRAM
- ✓ UI sees CLI-written rows

PSO / DDC / config-management criteria from §12 are out of Plan 1 scope — they're covered by Plan 2 / 3.

- [ ] **Step 6: Final commit (release notes)**

Create `docs/superpowers/changelog/2026-05-16-cli-plan-1.md`:

```markdown
# UECM CLI Plan 1 — Foundation + LAN Discovery

Date: 2026-05-16

## Shipped

- `uecm-cli` binary alongside the Tauri UI binary; shared `uecm_lib` + SQLite (WAL mode)
- `startup` module — DB path / DB open / ps-scripts dir all resolvable without Tauri runtime
- `cli::args` (clap derive), `cli::output` (NDJSON + human emitter), `cli::run` dispatch
- `system` domain: version / db-path / ps-dir / migrate-db / echo
- `machine` domain: list / add / detail / delete / rename / scan / refresh
- `winrm` domain: probe / bootstrap-script / bootstrap

## Validated

- macOS dev build + Windows release build both green
- LAN scan + machine refresh on lanPC end-to-end works; UI sees CLI-written rows

## Out of scope (deferred to Plan 2 / 3)

- `cred`, `env`, `ini`, `share`, `project`, `ddc`, `pso`, `health`, `gpu` domains
- shell completion, daemon mode, MCP wrapper
```

Commit:
```bash
git add docs/superpowers/changelog/2026-05-16-cli-plan-1.md
git commit -m "docs: CLI Plan 1 changelog"
```

---

## Summary of Tasks

| Phase | Task | Description |
|---|---|---|
| 0 | 0.1 | binary skeleton + Cargo deps |
| 0 | 0.2 | `startup` module (DB path + WAL migrate + ps-scripts dir) |
| 0 | 0.3 | `core::powershell` uses `startup::resolve_ps_script_dir` |
| 0 | 0.4 | `lib.rs::run` uses `startup::open_and_migrate_db` |
| 1 | 1.1 | `cli::args` clap derive |
| 1 | 1.2 | `cli::output` NDJSON emitter + Event enum |
| 1 | 1.3 | `cli::output` human emitter |
| 1 | 1.4 | `cli::run` dispatch + `bin/uecm-cli.rs` |
| 2 | 2.1 | `system version` / `db-path` / `ps-dir` |
| 2 | 2.2 | `system migrate-db` (verify) |
| 2 | 2.3 | `system echo` |
| 3 | 3.1 | `winrm probe` + `machine list` + `winrm bootstrap-script` |
| 3 | 3.2 | `machine add` / `detail` / `delete` / `rename` |
| 3 | 3.3 | `machine scan` (NDJSON stream) |
| 3 | 3.4 | `machine refresh` (WinRM + UE + GPU) |
| 3 | 3.5 | `winrm bootstrap` (verify) |
| 3 | 3.6 | end-to-end smoke integration test |
| 4 | 4.1 | release build + deploy to lanPC |
| 4 | 4.2 | LAN-discovery end-to-end on lanPC |

**Out of scope for this plan** (covered by future plans):

- `cred` / `env` / `ini` / `share` / `project` / `ddc` / `pso` / `health` / `gpu` domains
- Long-running PSO / DDC workflows
- Batch `--hosts` flag handling (depends on `core::batch`)
- SIGINT / Ctrl-C graceful cancel (spec §8.4) — Plan 1 emits no long-running UE work yet; `machine scan` is sub-second
- Exit code 10 (post-error NDJSON) and 130 (SIGINT) — placeholder in `exit_code_for`; full enforcement when long-running domains land

## Known assumptions

Several handlers (Task 3.2 / 3.4) call into `data::machines`, `data::machine_ue_installs`, `data::machine_gpus`, `data::credentials` with function names like `find_by_id` / `list_all` / `replace_all` / `touch_last_seen` / `find_username_by_alias`. Plan author did not full-read every data module; **implementing agent must grep the actual files first** and substitute the real function names. The behavior contract (insert / list / find / delete) is stable; only the names may need adjustment. Do not add new data functions — use what exists.
