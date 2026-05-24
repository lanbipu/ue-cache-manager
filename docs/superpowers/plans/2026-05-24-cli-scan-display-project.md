# CLI 扫描展示增强 + Project 深扫 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 uecm-cli 在 human 模式展示引擎/INI 详情、持久化并查询 DDC/PSO/Zen 配置实况，并支持按 project 深扫项目级 INI。

**Architecture:** 三个改动分三个 Phase。Phase 1 纯 CLI 展示层（给 `Emitter` 加 `emit_text` 出口 + 纯函数表格渲染）。Phase 2 扩展扫描器抓取配置快照、新增 `ini_config_snapshots` 表（FK cascade）+ `ini config` 查询命令。Phase 3 给 `ini scan` 加 `--project-id`，用 `scan_type="ini_project"` 隔离 Health 信号。每个 Phase 末尾可独立 build + test + commit。

**Tech Stack:** Rust / Tauri 2 / clap / rusqlite / SQLite。测试用内存 DB（`open_in_memory` + `schema::migrate`）+ `NdjsonEmitter` capture buffer。

**Spec:** `docs/superpowers/specs/2026-05-24-cli-scan-display-project-design.md`（Rev 2，commit 9643a47）

**Branch:** `feat/cli-scan-display-project`（已建）

---

## File Structure

| 文件 | 责任 | 改动 |
|---|---|---|
| `src-tauri/src/cli/output.rs` | Emitter trait + 两个 impl | Modify: 加 `emit_text` 方法 |
| `src-tauri/src/cli/domain_machine.rs` | machine 域 handler | Modify: refresh/detail human 表格 + 渲染纯函数 |
| `src-tauri/src/data/schema.rs` | 迁移 | Modify: MIGRATIONS 追加 `ini_config_snapshots` |
| `src-tauri/src/data/ini_config_snapshots.rs` | 新表 CRUD | Create |
| `src-tauri/src/data/mod.rs` | data 模块注册 | Modify: 注册 + re-export |
| `src-tauri/src/core/ini_config_extract.rs` | 从 ParsedFile 提取 DDC/PSO/Zen 配置 | Create |
| `src-tauri/src/core/mod.rs` | core 模块注册 | Modify: 注册模块 |
| `src-tauri/src/core/ini_scanner.rs` | 扫描器 | Modify: `ScanOutcome.config_snapshots` + scan_machine 调用提取 |
| `src-tauri/src/cli/args.rs` | clap 定义 | Modify: `IniAction::Config` 新增；`IniAction::Scan` 加 project_id/machine_id |
| `src-tauri/src/cli/domain_ini.rs` | ini 域 handler | Modify: scan_cluster 写 snapshot + project 维度 + scan_type；新增 config handler；list_runs 列两类 |
| `src-tauri/src/commands/ini_scanner.rs` | UI 版 scan | Modify: 写 snapshot |
| `src-tauri/src/data/scan_runs.rs` | scan_runs CRUD | Modify: `list_recent_types`（支持多 scan_type） |

---

## Phase 1 — 引擎信息 human 表格展示

### Task 1.1: 给 Emitter 加 `emit_text` 出口

**Files:**
- Modify: `src-tauri/src/cli/output.rs`（trait `Emitter` ~121-125；`NdjsonEmitter` impl ~183；`HumanEmitter` impl ~250）

- [ ] **Step 1: 写失败测试**（追加到 `output.rs` 的 `#[cfg(test)] mod tests`）

```rust
    #[test]
    fn human_emit_text_writes_to_stdout() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        {
            let mut emitter = HumanEmitter::new(&mut stdout, &mut stderr, false);
            emitter.emit_text("VERSION  PRIMARY").unwrap();
        }
        let s = String::from_utf8(stdout).unwrap();
        assert_eq!(s, "VERSION  PRIMARY\n");
    }

    #[test]
    fn ndjson_emit_text_is_noop() {
        let mut buf = Vec::new();
        {
            let mut emitter = NdjsonEmitter::new(&mut buf);
            emitter.emit_text("should not appear").unwrap();
        }
        assert!(buf.is_empty());
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p uecm-cli --lib cli::output::tests::human_emit_text 2>&1 | tail -20`
Expected: 编译失败 `no method named emit_text found for ... Emitter`

- [ ] **Step 3: 实现**

在 trait `Emitter`（output.rs ~121-125）加方法，带默认 no-op 实现（保持 object-safe，`&str` 参数无泛型）：

```rust
pub trait Emitter {
    fn emit_event(&mut self, event: &Event) -> io::Result<()>;
    fn emit_value(&mut self, value: &serde_json::Value) -> io::Result<()>;
    fn emit_error(&mut self, err: &UecmError);
    /// Raw human-facing text line to stdout. JSON emitters ignore it
    /// (handlers must branch on `ctx.json_mode` and only call this in
    /// human mode). Default = no-op so NDJSON stays clean.
    fn emit_text(&mut self, _text: &str) -> io::Result<()> {
        Ok(())
    }
}
```

`HumanEmitter` impl（output.rs ~250，在 `emit_error` 后追加 override）：

```rust
    fn emit_text(&mut self, text: &str) -> io::Result<()> {
        writeln!(self.stdout, "{}", text)?;
        self.stdout.flush()
    }
```

（`NdjsonEmitter` 用 trait 默认 no-op，无需改动。）

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p uecm-cli --lib cli::output::tests 2>&1 | tail -20`
Expected: PASS（含两个新测试 + 既有 output 测试全绿）

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/cli/output.rs
git commit -m "feat(cli): add Emitter::emit_text for human-mode raw output

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 1.2: 引擎/GPU 表格渲染纯函数

**Files:**
- Modify: `src-tauri/src/cli/domain_machine.rs`（文件内新增私有渲染函数 + 测试）

UE install 行类型是 `machine_ue_installs::UeInstall`（字段：`version: String`、`install_path: String`、`is_primary: bool`）。GPU 行是 `machine_gpus::GpuInfo`。

- [ ] **Step 1: 写失败测试**（追加到 `domain_machine.rs` 的 `#[cfg(test)] mod tests`）

```rust
    #[test]
    fn render_ue_installs_table_aligns_columns_and_marks_primary() {
        use crate::data::machine_ue_installs::UeInstall;
        let installs = vec![
            UeInstall { id: None, machine_id: 1, version: "5.0".into(),
                install_path: "C:\\Program Files\\Epic Games\\UE_5.0".into(), is_primary: false,
                zen_cli_intree_path: None, zen_cli_intree_version: None, zen_cli_intree_sha256: None,
                zenserver_intree_path: None, zenserver_intree_version: None, zenserver_intree_sha256: None },
            UeInstall { id: None, machine_id: 1, version: "5.4".into(),
                install_path: "C:\\Program Files\\Epic Games\\UE_5.4".into(), is_primary: true,
                zen_cli_intree_path: None, zen_cli_intree_version: None, zen_cli_intree_sha256: None,
                zenserver_intree_path: None, zenserver_intree_version: None, zenserver_intree_sha256: None },
        ];
        let out = render_ue_installs_table(&installs);
        assert!(out.contains("VERSION"));
        assert!(out.contains("PRIMARY"));
        assert!(out.contains("INSTALL PATH"));
        // primary 行带 *,非 primary 不带
        let line_54 = out.lines().find(|l| l.contains("5.4")).unwrap();
        assert!(line_54.contains('*'));
        let line_50 = out.lines().find(|l| l.contains("5.0")).unwrap();
        assert!(!line_50.contains('*'));
        assert!(line_50.contains("UE_5.0"));
    }

    #[test]
    fn render_ue_installs_table_handles_empty() {
        let out = render_ue_installs_table(&[]);
        assert!(out.contains("(no UE installs)"));
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p uecm-cli --lib cli::domain_machine::tests::render_ue_installs 2>&1 | tail -20`
Expected: 编译失败 `cannot find function render_ue_installs_table`

- [ ] **Step 3: 实现**（在 `domain_machine.rs` 顶部 use 区后、handler 区域加私有函数）

```rust
/// Render UE installs as an aligned human-mode table. Pure function — no IO.
fn render_ue_installs_table(installs: &[machine_ue_installs::UeInstall]) -> String {
    if installs.is_empty() {
        return "  (no UE installs)".to_string();
    }
    let mut out = String::from("  VERSION  PRIMARY  INSTALL PATH\n");
    for i in installs {
        let primary = if i.is_primary { "*" } else { " " };
        out.push_str(&format!(
            "  {:<7}  {:^7}  {}\n",
            i.version, primary, i.install_path
        ));
    }
    out.trim_end().to_string()
}

/// Render GPUs as an aligned human-mode table. Pure function — no IO.
fn render_gpus_table(gpus: &[machine_gpus::GpuInfo]) -> String {
    if gpus.is_empty() {
        return "  (no GPUs)".to_string();
    }
    let mut out = String::from("  GPU MODEL                      DRIVER       VRAM(MB)\n");
    for g in gpus {
        out.push_str(&format!(
            "  {:<28}  {:<11}  {}\n",
            g.gpu_model, g.driver_version, g.vram_mb
        ));
    }
    out.trim_end().to_string()
}
```

> 实现注意：核对 `machine_gpus::GpuInfo` 的真实字段名（`gpu_model` / `driver_version` / `vram_mb`）；若不同，按真实字段调整 format 参数。运行 `grep -n "pub struct GpuInfo" -A8 src-tauri/src/data/machine_gpus.rs` 确认。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p uecm-cli --lib cli::domain_machine::tests::render_ue_installs 2>&1 | tail -20`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/cli/domain_machine.rs
git commit -m "feat(cli): pure-function aligned tables for UE installs + GPUs

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 1.3: `machine detail` human 表格分流

**Files:**
- Modify: `src-tauri/src/cli/domain_machine.rs`（`fn detail` ~76-92）

- [ ] **Step 1: 写失败测试**（追加到 tests；模式参照 domain_ini 的 `make_ctx`，但 human 模式用 HumanEmitter）

```rust
    #[test]
    fn detail_human_mode_renders_tables_not_json() {
        use crate::cli::output::{Emitter, HumanEmitter};
        use crate::data::{open_in_memory, schema, machine_ue_installs::{self, UeInstall}};
        let db = open_in_memory().unwrap();
        { let mut c = db.lock().unwrap(); schema::migrate(&mut c).unwrap(); }
        let id = machines::insert(&db, &machines::Machine::new("RENDER-01", "1.2.3.4")).unwrap();
        machine_ue_installs::upsert(&db, &UeInstall { id: None, machine_id: id, version: "5.4".into(),
            install_path: "C:\\UE_5.4".into(), is_primary: true,
            zen_cli_intree_path: None, zen_cli_intree_version: None, zen_cli_intree_sha256: None,
            zenserver_intree_path: None, zenserver_intree_version: None, zenserver_intree_sha256: None }).unwrap();

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        {
            let emitter: Box<dyn Emitter> = Box::new(HumanEmitter::new(&mut stdout, &mut stderr, false));
            let mut ctx = Ctx { db: Some(db.clone()), db_path: std::path::PathBuf::from(":memory:"),
                emitter, json_mode: false };
            detail(&mut ctx, id).unwrap();
        }
        let s = String::from_utf8(stdout).unwrap();
        assert!(s.contains("VERSION"));      // 表头 — 非 JSON
        assert!(s.contains("5.4"));
        assert!(!s.contains("\"ue_installs\""));  // 不是 pretty JSON
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p uecm-cli --lib cli::domain_machine::tests::detail_human_mode 2>&1 | tail -20`
Expected: FAIL（当前 detail 总走 emit_result → human 下是 pretty JSON，断言 `!contains("\"ue_installs\"")` 失败）

- [ ] **Step 3: 实现**（改 `fn detail`，76-92）

```rust
fn detail(ctx: &mut Ctx<'_>, id: i64) -> UecmResult<()> {
    let db = ctx.require_db()?;

    let machine = machines::find_by_id(db, id)?
        .ok_or_else(|| UecmError::InvalidInput(format!("machine id={} not found", id)))?;
    let ue_installs = machine_ue_installs::list_for_machine(db, id)?;
    let gpus = machine_gpus::list_for_machine(db, id)?;

    if ctx.json_mode {
        let detail = json!({
            "machine": machine,
            "ue_installs": ue_installs,
            "gpus": gpus,
        });
        ctx.emitter.emit_result(&detail).ok();
    } else {
        let installs_tbl = render_ue_installs_table(&ue_installs);
        let gpus_tbl = render_gpus_table(&gpus);
        let text = format!(
            "Machine: {} ({})  last_seen={}\n\nUE Installs:\n{}\n\nGPUs:\n{}",
            machine.hostname, machine.ip,
            machine.last_seen.as_deref().unwrap_or("-"),
            installs_tbl, gpus_tbl,
        );
        ctx.emitter.emit_text(&text).ok();
    }
    Ok(())
}
```

> 实现注意：核对 `machines::Machine` 的 `last_seen` 字段名（`grep -n "pub struct Machine" -A12 src-tauri/src/data/machines.rs`）；若是 `last_seen_at` 等则改之。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p uecm-cli --lib cli::domain_machine::tests::detail 2>&1 | tail -20`
Expected: PASS（新测试 + 既有 detail 测试）

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/cli/domain_machine.rs
git commit -m "feat(cli): machine detail renders aligned tables in human mode

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 1.4: `machine refresh` 扫完顺带列引擎表格

**Files:**
- Modify: `src-tauri/src/cli/domain_machine.rs`（`fn refresh`，summary 构造之前 ~376）

- [ ] **Step 1: 写失败测试**

```rust
    #[test]
    fn refresh_human_mode_lists_installs_before_done() {
        // refresh 需要 RemoteExecutor; 复用既有 refresh 测试的 fake exec 模式。
        // 断言: human 模式 stdout 含 VERSION 表头 + 探测到的版本号。
        // (按既有 refresh 测试如何注入 fake exec / 预置 installs 来构造;
        //  参照本文件中现有 refresh_* 测试的 setup。)
    }
```

> 实现注意：本测试要复用 `domain_machine.rs` 现有 refresh 测试的 fake `RemoteExecutor` 注入方式（`grep -n "fn refresh\|RemoteExecutor\|FakeExec\|refresh(" src-tauri/src/cli/domain_machine.rs` 找现有 refresh 测试模板，照搬其 exec/DB 预置，仅把 emitter 换成 `HumanEmitter` 捕获 stdout，断言含 `"VERSION"` 与探测版本号）。

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p uecm-cli --lib cli::domain_machine::tests::refresh_human 2>&1 | tail -20`
Expected: FAIL（refresh 当前 human 模式不打印 installs 表）

- [ ] **Step 3: 实现**（在 `fn refresh` 构造 summary 之前、detect/upsert 完成之后插入）

```rust
    // Human-mode: list the UE installs we just detected/persisted, before
    // the `done` summary line. JSON consumers get the count in summary.
    if !ctx.json_mode {
        let db = ctx.require_db()?;
        let installs = machine_ue_installs::list_for_machine(db, id)?;
        let tbl = render_ue_installs_table(&installs);
        ctx.emitter.emit_text(&tbl).ok();
    }

    let summary = json!({
        "machine_id": id,
        "ue_versions": detected_ue.len(),
        "gpus": detected_gpus.len(),
        "latency_ms": probe.latency_ms,
        "authenticated": true,
    });
    ctx.emitter.emit_event(&Event::Completed { summary }).ok();
    Ok(())
```

> 注意借用：`ctx.require_db()?` 返回 `&Db`，与随后 `ctx.emitter` 可变借用冲突。把 installs 先收集进局部变量、结束 db 借用，再 `ctx.emitter.emit_text`（参照 scan_cluster 里"DB 操作放 scoped block"的写法）。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p uecm-cli --lib cli::domain_machine::tests 2>&1 | tail -20`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/cli/domain_machine.rs
git commit -m "feat(cli): machine refresh lists UE installs in human mode

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

**Phase 1 收尾**：`cargo build -p uecm-cli` 通过 + `cargo test -p uecm-cli --lib cli::` 全绿。

---

## Phase 2 — INI 配置快照（DDC/PSO/Zen）

### Task 2.1: 迁移 — 新增 `ini_config_snapshots` 表

**Files:**
- Modify: `src-tauri/src/data/schema.rs`（`MIGRATIONS` 数组末尾，schema.rs:7 起；追加在最后一个元组后）

- [ ] **Step 1: 写失败测试**（追加到 `schema.rs` 的 tests）

```rust
    #[test]
    fn ini_config_snapshots_table_exists_after_migrate() {
        let db = open_in_memory().unwrap();
        let mut conn = db.lock().unwrap();
        migrate(&mut conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='ini_config_snapshots'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p uecm-cli --lib data::schema::tests::ini_config_snapshots_table 2>&1 | tail -20`
Expected: FAIL（表不存在，count=0）

- [ ] **Step 3: 实现**（在 `MIGRATIONS` 数组末尾追加一个元组；`name` 用下一个序号，先 `grep -n '"0[0-9][0-9]_\|^        ("' src-tauri/src/data/schema.rs` 确认现有最大序号，本计划假设为 `022`）

```rust
    (
        "022_ini_config_snapshots",
        r#"
        CREATE TABLE IF NOT EXISTS ini_config_snapshots (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            scan_run_id  INTEGER NOT NULL,
            machine_id   INTEGER NOT NULL,
            file_path    TEXT NOT NULL,
            ue_version   TEXT,
            domain       TEXT NOT NULL,
            section      TEXT NOT NULL,
            key_name     TEXT NOT NULL,
            value        TEXT NOT NULL,
            line_number  INTEGER,
            FOREIGN KEY (scan_run_id) REFERENCES scan_runs(id) ON DELETE CASCADE,
            FOREIGN KEY (machine_id)  REFERENCES machines(id)  ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_ini_config_snapshots_run
            ON ini_config_snapshots(scan_run_id);
        "#,
    ),
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p uecm-cli --lib data::schema::tests 2>&1 | tail -20`
Expected: PASS（含既有 `migrate_records_applied_migrations`）

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/data/schema.rs
git commit -m "feat(data): migration 022 — ini_config_snapshots with FK cascade

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 2.2: data 层 — `ini_config_snapshots` CRUD

**Files:**
- Create: `src-tauri/src/data/ini_config_snapshots.rs`
- Modify: `src-tauri/src/data/mod.rs`（注册模块；`grep -n "pub mod ini_findings\|pub use" src-tauri/src/data/mod.rs` 找模式照搬）

- [ ] **Step 1: 写失败测试 + 模块骨架**（先建文件含 struct + 空 fn 签名 + 测试）

创建 `src-tauri/src/data/ini_config_snapshots.rs`：

```rust
//! CRUD for `ini_config_snapshots`. One row per captured DDC/PSO/Zen config
//! key from an INI scan. Rows are immutable; FK cascade cleans them on
//! scan_run / machine deletion.

use crate::data::Db;
use crate::error::UecmResult;
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigSnapshot {
    pub id: Option<i64>,
    pub scan_run_id: i64,
    pub machine_id: i64,
    pub file_path: String,
    pub ue_version: Option<String>,
    pub domain: String, // "ddc" | "pso" | "zen"
    pub section: String,
    pub key_name: String,
    pub value: String,
    pub line_number: Option<i64>,
}

pub fn insert(db: &Db, s: &ConfigSnapshot) -> UecmResult<i64> {
    let conn = db.lock().unwrap();
    conn.execute(
        "INSERT INTO ini_config_snapshots (
            scan_run_id, machine_id, file_path, ue_version, domain,
            section, key_name, value, line_number
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            s.scan_run_id, s.machine_id, s.file_path, s.ue_version, s.domain,
            s.section, s.key_name, s.value, s.line_number,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

fn row_to_snapshot(row: &rusqlite::Row) -> rusqlite::Result<ConfigSnapshot> {
    Ok(ConfigSnapshot {
        id: Some(row.get(0)?),
        scan_run_id: row.get(1)?,
        machine_id: row.get(2)?,
        file_path: row.get(3)?,
        ue_version: row.get(4)?,
        domain: row.get(5)?,
        section: row.get(6)?,
        key_name: row.get(7)?,
        value: row.get(8)?,
        line_number: row.get(9)?,
    })
}

pub fn list_for_run(db: &Db, scan_run_id: i64) -> UecmResult<Vec<ConfigSnapshot>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, scan_run_id, machine_id, file_path, ue_version, domain,
                section, key_name, value, line_number
         FROM ini_config_snapshots
         WHERE scan_run_id = ?
         ORDER BY machine_id, file_path, domain, section, key_name",
    )?;
    let rows = stmt.query_map(params![scan_run_id], row_to_snapshot)?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

pub fn list_for_run_domain(db: &Db, scan_run_id: i64, domain: &str) -> UecmResult<Vec<ConfigSnapshot>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, scan_run_id, machine_id, file_path, ue_version, domain,
                section, key_name, value, line_number
         FROM ini_config_snapshots
         WHERE scan_run_id = ? AND domain = ?
         ORDER BY machine_id, file_path, section, key_name",
    )?;
    let rows = stmt.query_map(params![scan_run_id, domain], row_to_snapshot)?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::machines::{insert as insert_machine, Machine};
    use crate::data::{open_in_memory, scan_runs, schema};

    fn setup() -> (Db, i64, i64) {
        let db = open_in_memory().unwrap();
        { let mut conn = db.lock().unwrap(); schema::migrate(&mut conn).unwrap(); }
        let machine_id = insert_machine(&db, &Machine::new("RENDER-01", "192.168.10.21")).unwrap();
        let scan_id = scan_runs::insert(&db, "ini", &[machine_id]).unwrap();
        (db, scan_id, machine_id)
    }

    fn sample(scan_id: i64, machine_id: i64, domain: &str, key: &str) -> ConfigSnapshot {
        ConfigSnapshot {
            id: None, scan_run_id: scan_id, machine_id,
            file_path: "C:\\Proj\\Config\\DefaultEngine.ini".into(),
            ue_version: Some("5.4".into()), domain: domain.into(),
            section: "DerivedDataBackendGraph".into(), key_name: key.into(),
            value: "(Type=FileSystem)".into(), line_number: Some(10),
        }
    }

    #[test]
    fn insert_and_list_for_run() {
        let (db, scan_id, mid) = setup();
        insert(&db, &sample(scan_id, mid, "ddc", "Root")).unwrap();
        insert(&db, &sample(scan_id, mid, "pso", "r.PSOPrecaching")).unwrap();
        let rows = list_for_run(&db, scan_id).unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn list_for_run_domain_filters() {
        let (db, scan_id, mid) = setup();
        insert(&db, &sample(scan_id, mid, "ddc", "Root")).unwrap();
        insert(&db, &sample(scan_id, mid, "pso", "r.PSOPrecaching")).unwrap();
        let ddc = list_for_run_domain(&db, scan_id, "ddc").unwrap();
        assert_eq!(ddc.len(), 1);
        assert_eq!(ddc[0].domain, "ddc");
    }

    #[test]
    fn fk_cascade_clears_snapshots_on_scan_run_delete() {
        let (db, scan_id, mid) = setup();
        insert(&db, &sample(scan_id, mid, "ddc", "Root")).unwrap();
        {
            let conn = db.lock().unwrap();
            conn.execute("DELETE FROM scan_runs WHERE id = ?", params![scan_id]).unwrap();
        }
        let rows = list_for_run(&db, scan_id).unwrap();
        assert!(rows.is_empty(), "FK cascade should have cleared snapshots");
    }
```

在 `src-tauri/src/data/mod.rs` 注册（照 `ini_findings` 那行）：

```rust
pub mod ini_config_snapshots;
```

> 注意 FK cascade 测试依赖 `PRAGMA foreign_keys=ON`。先 `grep -rn "foreign_keys" src-tauri/src/data/` 确认 `open_in_memory` / 连接初始化已开启该 pragma；若没有，FK cascade 测试会失败 —— 此时在连接初始化处加 `conn.pragma_update(None, "foreign_keys", true)`（既有 `ini_findings` 等表的 cascade 也依赖它，属共有前置，应已存在）。

- [ ] **Step 2: 跑测试确认失败/编译**

Run: `cargo test -p uecm-cli --lib data::ini_config_snapshots 2>&1 | tail -25`
Expected: 首次可能因 mod 未注册或 pragma 未开而 FAIL；逐项修到三测试全过

- [ ] **Step 3: 实现** — 代码已在 Step 1 给全；本步是修编译/pragma 问题直到通过

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p uecm-cli --lib data::ini_config_snapshots 2>&1 | tail -20`
Expected: PASS（insert_and_list_for_run / list_for_run_domain_filters / fk_cascade_clears_snapshots_on_scan_run_delete）

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/data/ini_config_snapshots.rs src-tauri/src/data/mod.rs
git commit -m "feat(data): ini_config_snapshots CRUD + FK cascade test

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 2.3: core — 从 ParsedFile 提取 DDC/PSO/Zen 配置（含双标）

**Files:**
- Create: `src-tauri/src/core/ini_config_extract.rs`
- Modify: `src-tauri/src/core/mod.rs`（`pub mod ini_config_extract;`）

数据源类型在 `ini_diagnostics`：`ParsedFile{path, category, sections}`、`ParsedSection{name, keys}`、`ParsedKey{name, value, line_number: usize}`。

- [ ] **Step 1: 写失败测试 + 实现骨架**

创建 `src-tauri/src/core/ini_config_extract.rs`：

```rust
//! Extract DDC / PSO / Zen config entries from a parsed INI file for
//! `ini_config_snapshots`. Captures the *actual* configured values
//! (not just rule findings). See spec §4.1.

use crate::core::ini_diagnostics::ParsedFile;

/// One captured config key, tagged with the concern domain. `scan_run_id` /
/// `machine_id` / `ue_version` are filled by the caller (DB context).
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigEntry {
    pub domain: &'static str, // "ddc" | "pso" | "zen"
    pub section: String,
    pub key_name: String,
    pub value: String,
    pub line_number: i64,
}

const DDC_SECTIONS: &[&str] = &[
    "DerivedDataBackendGraph",
    "/Script/UnrealEd.DerivedDataCacheSettings",
];
const INSTALLED_DDBG: &str = "InstalledDerivedDataBackendGraph";
const INSTALLED_LEGACY_DDC_KEYS: &[&str] = &["Shared", "Pak", "CompressedPak"];

fn is_pso_cvar(key: &str) -> bool {
    key.starts_with("r.PSOPrecaching")
        || key.starts_with("r.PSOPrecache.")
        || key.starts_with("r.ShaderPipelineCache.")
}

/// Returns config entries for the three concern domains. A single key may
/// yield multiple entries (dual-tag): InstalledDerivedDataBackendGraph's
/// legacy DDC keys (Shared/Pak/CompressedPak) are tagged both `ddc` and `zen`.
pub fn extract(pf: &ParsedFile) -> Vec<ConfigEntry> {
    let mut out = Vec::new();
    for sec in &pf.sections {
        let sname = sec.name.as_str();
        let is_ddc_section = DDC_SECTIONS.contains(&sname);
        let is_installed = sname == INSTALLED_DDBG;
        for k in &sec.keys {
            let mk = |domain: &'static str| ConfigEntry {
                domain,
                section: sname.to_string(),
                key_name: k.name.clone(),
                value: k.value.clone(),
                line_number: k.line_number as i64,
            };
            if is_ddc_section {
                out.push(mk("ddc"));
            }
            if is_installed {
                out.push(mk("zen"));
                if INSTALLED_LEGACY_DDC_KEYS.contains(&k.name.as_str()) {
                    out.push(mk("ddc")); // dual-tag
                }
            }
            if is_pso_cvar(&k.name) {
                out.push(mk("pso"));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ini_diagnostics::{Category, ParsedФile, ParsedKey, ParsedSection};
}
```

> ⚠️ 上面测试模块最后一行的 `ParsedФile` 是占位拼写错误的提醒——实现时写正确的 `ParsedFile` 并补全下面的测试体（见 Step 1b）。

- [ ] **Step 1b: 补全测试体**（替换上面残缺的 tests mod）

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ini_diagnostics::{Category, ParsedFile, ParsedKey, ParsedSection};

    fn key(name: &str, value: &str, line: usize) -> ParsedKey {
        ParsedKey { name: name.into(), value: value.into(), line_number: line }
    }
    fn pf(sections: Vec<ParsedSection>) -> ParsedFile {
        ParsedFile { path: "C:\\Proj\\Config\\DefaultEngine.ini".into(),
            category: Category::Project, sections }
    }

    #[test]
    fn extracts_ddc_backend_graph() {
        let f = pf(vec![ParsedSection {
            name: "DerivedDataBackendGraph".into(),
            keys: vec![key("Root", "(Type=KeyLength)", 3)],
        }]);
        let e = extract(&f);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].domain, "ddc");
        assert_eq!(e[0].key_name, "Root");
    }

    #[test]
    fn extracts_pso_cvars_any_section() {
        let f = pf(vec![ParsedSection {
            name: "SystemSettings".into(),
            keys: vec![
                key("r.ShaderPipelineCache.Enabled", "1", 5),
                key("r.PSOPrecache.Compile", "1", 6),
                key("Unrelated", "x", 7),
            ],
        }]);
        let e = extract(&f);
        assert_eq!(e.len(), 2);
        assert!(e.iter().all(|x| x.domain == "pso"));
    }

    #[test]
    fn installed_ddbg_legacy_keys_dual_tag_ddc_and_zen() {
        let f = pf(vec![ParsedSection {
            name: "InstalledDerivedDataBackendGraph".into(),
            keys: vec![
                key("Shared", "(Type=FileSystem)", 2),   // legacy DDC → dual
                key("ZenShared", "(Type=Zen)", 3),       // zen only
            ],
        }]);
        let e = extract(&f);
        // Shared: zen + ddc (2)；ZenShared: zen (1) = 3 total
        assert_eq!(e.len(), 3);
        let shared: Vec<_> = e.iter().filter(|x| x.key_name == "Shared").map(|x| x.domain).collect();
        assert!(shared.contains(&"ddc"));
        assert!(shared.contains(&"zen"));
        let zenshared: Vec<_> = e.iter().filter(|x| x.key_name == "ZenShared").map(|x| x.domain).collect();
        assert_eq!(zenshared, vec!["zen"]);
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p uecm-cli --lib core::ini_config_extract 2>&1 | tail -25`
Expected: 先因占位错字/`Category`/`ParsedFile` 字段不符而编译失败

> 实现注意：核对 `ParsedFile`/`ParsedSection`/`ParsedKey`/`Category` 是否 `pub` 且可从 `core::ini_diagnostics` 导入（`grep -n "pub struct ParsedFile\|pub struct ParsedSection\|pub struct ParsedKey\|pub enum Category" src-tauri/src/core/ini_diagnostics.rs`）。`ParsedKey.line_number` 是 `usize`。

- [ ] **Step 3: 实现** — 修占位错字 + 对齐真实字段，直到编译通过

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p uecm-cli --lib core::ini_config_extract 2>&1 | tail -20`
Expected: PASS（三个测试，覆盖 ddc / pso / 双标）

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/core/ini_config_extract.rs src-tauri/src/core/mod.rs
git commit -m "feat(core): extract DDC/PSO/Zen config entries with InstalledDDBG dual-tag

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 2.4: 扫描器产出 config 快照

**Files:**
- Modify: `src-tauri/src/core/ini_scanner.rs`（`ScanOutcome` struct ~238-243；`scan_machine` 读文件成功分支 ~253-262）

- [ ] **Step 1: 写失败测试**（追加到 `ini_scanner.rs` tests，复用其本地文件读取测试模式 `read_local_file`）

```rust
    #[test]
    fn scan_machine_collects_config_snapshots() {
        // 用本地临时 DefaultEngine.ini 作为 project_root,断言 outcome.config_snapshots 非空且含 ddc 条目。
        // (复用本文件已有的 tempfile + project_roots 测试模式; 写入
        //  "[DerivedDataBackendGraph]\nRoot=(Type=KeyLength)\n" 到临时项目 Config 目录,
        //  传 project_roots=[temp_root], 调 scan_machine, 断言:
        //  outcome.config_snapshots.iter().any(|c| c.domain == "ddc" && c.key_name == "Root"))
    }
```

> 实现注意：照搬本文件现有"写临时 ini → 构造 ScanInputs → scan_machine"的测试（`grep -n "tempfile\|TempDir\|project_roots\|scan_machine" src-tauri/src/core/ini_scanner.rs`）。

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p uecm-cli --lib core::ini_scanner::tests::scan_machine_collects_config 2>&1 | tail -20`
Expected: FAIL（`ScanOutcome` 无 `config_snapshots` 字段）

- [ ] **Step 3: 实现**

`ScanOutcome`（~238-243）加字段：

```rust
#[derive(Debug, Default)]
pub struct ScanOutcome {
    pub findings: Vec<Finding>,
    pub errors: Vec<String>,
    pub not_found: Vec<String>,
    pub read_count: usize,
    pub config_snapshots: Vec<crate::core::ini_config_extract::ConfigEntry>,
}
```

`scan_machine` 读文件成功分支（~253-262，`Ok(Some(pf)) => { ... }`）追加一行：

```rust
            Ok(Some(pf)) => {
                outcome.read_count += 1;
                outcome.config_snapshots.extend(crate::core::ini_config_extract::extract(&pf));
                outcome.findings.extend(ini_diagnostics::run_rules(&pf, &inputs.env_state));
                if let Some(ctx) = inputs.zen_ctx {
                    outcome.findings.extend(run_zen_rules_for_file(&pf, &inputs.env_state, ctx));
                }
            }
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p uecm-cli --lib core::ini_scanner 2>&1 | tail -20`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/core/ini_scanner.rs
git commit -m "feat(core): scan_machine collects config snapshots alongside findings

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 2.5: 扫描时把 config 快照写库（CLI + UI）

**Files:**
- Modify: `src-tauri/src/cli/domain_ini.rs`（`scan_cluster` 的 per-machine 块，findings insert 之后 ~677-700）
- Modify: `src-tauri/src/commands/ini_scanner.rs`（`scan_inis_summary` 对应位置）

- [ ] **Step 1: 写失败测试**（追加到 `domain_ini.rs` tests，用 `make_ctx` + 预置一台机 + 本地临时 project ini）

```rust
    #[cfg(not(windows))]
    #[test]
    fn scan_persists_config_snapshots_to_db() {
        // 预置 machine + 临时 DefaultEngine.ini([DerivedDataBackendGraph] Root=...),
        // 通过 project 维度或直接调 scan_cluster 跑扫描,
        // 断言 ini_config_snapshots::list_for_run(db, scan_run_id) 含 ddc/Root 条目。
        // (Phase 3 提供 project 维度入口; 此测试可先用 scan_cluster 直接传 project_roots,
        //  或在 Phase 3 完成后补全端到端。先写断言 DB 落库的最小用例。)
    }
```

> 实现注意：本测试依赖"能让 scan_machine 真读到一个含目标 section 的文件"。非 Windows 下 read_file 走 `read_local_file`（本地路径）。用 tempfile 造 `<root>/Config/DefaultEngine.ini`，project_roots=[root]。

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p uecm-cli --lib cli::domain_ini::tests::scan_persists_config 2>&1 | tail -20`
Expected: FAIL（snapshot 未落库）

- [ ] **Step 3: 实现** — `scan_cluster` 在收集 `outcome` 后、统计之外，把 snapshot 写库。在 per-machine scoped block 内（findings insert 循环之后）加：

```rust
            // Persist config snapshots (DDC/PSO/Zen actual values).
            let ue_hint = ue_version_hint.clone();
            for entry in &outcome.config_snapshots {
                ini_config_snapshots::insert(&db, &ini_config_snapshots::ConfigSnapshot {
                    id: None,
                    scan_run_id,
                    machine_id: mid,
                    file_path: String::new(), // 见下注
                    ue_version: ue_hint.clone(),
                    domain: entry.domain.to_string(),
                    section: entry.section.clone(),
                    key_name: entry.key_name.clone(),
                    value: entry.value.clone(),
                    line_number: Some(entry.line_number),
                })?;
            }
```

> ⚠️ `ConfigEntry` 当前不带 `file_path`（提取时按 section/key）。要让 snapshot 记录来源文件，需在 Task 2.3 的 `ConfigEntry` 加 `file_path: String`（从 `pf.path` 填）并在 `extract` 里设置 `file_path: pf.path.clone()`。**实现本 task 前先回到 `ini_config_extract::extract`：给 `ConfigEntry` 加 `pub file_path: String` 字段，`mk` 闭包里 `file_path: pf.path.clone()`，并更新 Task 2.3 的三个测试断言（增加 `file_path` 字段构造）。** 然后这里用 `entry.file_path.clone()`。

在 `src-tauri/src/cli/domain_ini.rs` 顶部 `use` 区加 `ini_config_snapshots`（核对现有 `use crate::data::{... ini_findings ...}` 行，追加 `ini_config_snapshots`）。

`src-tauri/src/commands/ini_scanner.rs` 的 `scan_inis_summary` 做同样的 snapshot 落库（该函数已 `outcome = ini_scanner::scan_machine(...)`，在其 findings 处理旁加同样循环）。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p uecm-cli --lib 2>&1 | tail -20`
Expected: PASS（含新测试；既有扫描测试不回归）

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/cli/domain_ini.rs src-tauri/src/commands/ini_scanner.rs src-tauri/src/core/ini_config_extract.rs
git commit -m "feat: persist DDC/PSO/Zen config snapshots during ini scan (CLI + UI)

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 2.6: `ini config <run_id>` 查询命令

**Files:**
- Modify: `src-tauri/src/cli/args.rs`（`IniAction` enum，`Findings`/`GetFinding` 附近 ~440-470）
- Modify: `src-tauri/src/cli/domain_ini.rs`（dispatch match ~122-128 + 新 handler `config`）

- [ ] **Step 1: 写失败测试**（args 解析 + handler emit）

args 解析测试（追加到 `args.rs` tests）：

```rust
    #[test]
    fn parses_ini_config_with_domain() {
        let cli = Cli::try_parse_from([
            "uecm-cli", "ini", "config", "37", "--domain", "ddc",
        ]).unwrap();
        match cli.domain {
            Domain::Ini { action: IniAction::Config { scan_run_id, domain } } => {
                assert_eq!(scan_run_id, 37);
                assert_eq!(domain.as_deref(), Some("ddc"));
            }
            _ => panic!("expected ini config"),
        }
    }
```

handler 测试（追加到 `domain_ini.rs` tests）：

```rust
    #[test]
    fn config_handler_emits_snapshots() {
        use crate::data::{scan_runs, machines, ini_config_snapshots as ics};
        let db = fresh_db();
        let mid = machines::insert(&db, &machines::Machine::new("R1", "1.1.1.1")).unwrap();
        let run = scan_runs::insert(&db, "ini", &[mid]).unwrap();
        ics::insert(&db, &ics::ConfigSnapshot { id: None, scan_run_id: run, machine_id: mid,
            file_path: "C:\\P\\Config\\DefaultEngine.ini".into(), ue_version: Some("5.4".into()),
            domain: "ddc".into(), section: "DerivedDataBackendGraph".into(),
            key_name: "Root".into(), value: "(Type=KeyLength)".into(), line_number: Some(3) }).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        let mut ctx = make_ctx(&mut buf, &db);
        config(&mut ctx, run, None).unwrap();
        drop(ctx);
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("DerivedDataBackendGraph"));
        assert!(s.contains("Root"));
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p uecm-cli --lib 2>&1 | grep -A3 "ini_config\|config_handler" | tail -20`
Expected: FAIL（`IniAction::Config` 与 `config` handler 不存在）

- [ ] **Step 3: 实现**

`args.rs` `IniAction` 加变体（在 `Findings`/`GetFinding` 附近）：

```rust
    /// List captured DDC/PSO/Zen config snapshots for a scan run.
    Config {
        scan_run_id: i64,
        /// Filter by concern domain (ddc / pso / zen).
        #[arg(long)]
        domain: Option<String>,
    },
```

`domain_ini.rs` dispatch（`handle` 的 match，~122-128）加：

```rust
        IniAction::Config { scan_run_id, domain } => config(ctx, scan_run_id, domain.as_deref()),
```

新 handler（放在 `list_findings` 附近）：

```rust
fn config(ctx: &mut Ctx<'_>, scan_run_id: i64, domain: Option<&str>) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let rows = match domain {
        Some(d) => ini_config_snapshots::list_for_run_domain(db, scan_run_id, d)?,
        None => ini_config_snapshots::list_for_run(db, scan_run_id)?,
    };
    if ctx.json_mode {
        ctx.emitter.emit_result(&rows).ok();
    } else {
        let mut text = String::new();
        let mut last_file = String::new();
        for r in &rows {
            if r.file_path != last_file {
                text.push_str(&format!("\n{} (UE {})\n", r.file_path,
                    r.ue_version.as_deref().unwrap_or("?")));
                last_file = r.file_path.clone();
            }
            text.push_str(&format!("  [{}] [{}] {} = {}\n",
                r.domain, r.section, r.key_name, r.value));
        }
        if rows.is_empty() { text.push_str("(no config snapshots)\n"); }
        ctx.emitter.emit_text(text.trim_end()).ok();
    }
    Ok(())
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p uecm-cli --lib 2>&1 | tail -20`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/cli/args.rs src-tauri/src/cli/domain_ini.rs
git commit -m "feat(cli): ini config <run_id> [--domain] query command

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

**Phase 2 收尾**：`cargo build -p uecm-cli` + `cargo test -p uecm-cli --lib` 全绿。

---

## Phase 3 — Project 深扫

### Task 3.1: `scan_runs` 支持多 scan_type 查询

**Files:**
- Modify: `src-tauri/src/data/scan_runs.rs`（在 `list_recent` 后加 `list_recent_types`）

- [ ] **Step 1: 写失败测试**（追加到 `scan_runs.rs` tests）

```rust
    #[test]
    fn list_recent_types_includes_both_ini_kinds() {
        let db = setup();
        let _a = insert(&db, "ini", &[1]).unwrap();
        let b = insert(&db, "ini_project", &[1]).unwrap();
        let _h = insert(&db, "health", &[1]).unwrap();
        let rows = list_recent_types(&db, &["ini", "ini_project"], 10).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, Some(b)); // 最新在前
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p uecm-cli --lib data::scan_runs::tests::list_recent_types 2>&1 | tail -20`
Expected: FAIL（`list_recent_types` 不存在）

- [ ] **Step 3: 实现**（动态构造 `IN (?,?...)` 占位符）

```rust
pub fn list_recent_types(db: &Db, scan_types: &[&str], limit: i64) -> UecmResult<Vec<ScanRun>> {
    if scan_types.is_empty() {
        return Ok(Vec::new());
    }
    let conn = db.lock().unwrap();
    let placeholders = vec!["?"; scan_types.len()].join(",");
    let sql = format!(
        "SELECT id, scan_type, started_at, finished_at, machine_ids_json, summary_json
         FROM scan_runs WHERE scan_type IN ({}) ORDER BY started_at DESC LIMIT ?",
        placeholders
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut params_vec: Vec<&dyn rusqlite::ToSql> = Vec::new();
    for t in scan_types { params_vec.push(t); }
    params_vec.push(&limit);
    let rows = stmt.query_map(params_vec.as_slice(), |row| row_to_scan_run(row))?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p uecm-cli --lib data::scan_runs 2>&1 | tail -20`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/data/scan_runs.rs
git commit -m "feat(data): scan_runs::list_recent_types for multi scan_type query

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 3.2: `ini scan` 加 `--project-id` / `--machine-id`（互斥）

**Files:**
- Modify: `src-tauri/src/cli/args.rs`（`IniAction::Scan` ~426-432）

当前：
```rust
    Scan {
        #[arg(long, value_name = "M1,M2,...", value_delimiter = ',')]
        machine_ids: Vec<i64>,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
```

- [ ] **Step 1: 写失败测试**（args 解析：project 维度 + 互斥）

```rust
    #[test]
    fn parses_ini_scan_project_id() {
        let cli = Cli::try_parse_from([
            "uecm-cli", "ini", "scan", "--project-id", "5", "--machine-id", "11",
        ]).unwrap();
        match cli.domain {
            Domain::Ini { action: IniAction::Scan { project_id, machine_id, machine_ids, .. } } => {
                assert_eq!(project_id, Some(5));
                assert_eq!(machine_id, Some(11));
                assert!(machine_ids.is_empty());
            }
            _ => panic!("expected ini scan"),
        }
    }

    #[test]
    fn ini_scan_project_id_conflicts_with_machine_ids() {
        let res = Cli::try_parse_from([
            "uecm-cli", "ini", "scan", "--project-id", "5", "--machine-ids", "1,2",
        ]);
        assert!(res.is_err(), "project-id and machine-ids must conflict");
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p uecm-cli --lib args::tests::parses_ini_scan_project 2>&1 | tail -20`
Expected: FAIL（字段不存在）

- [ ] **Step 3: 实现**

```rust
    Scan {
        #[arg(long, value_name = "M1,M2,...", value_delimiter = ',')]
        machine_ids: Vec<i64>,
        /// Project deep-scan: scan this project's INI config (via project_locations).
        #[arg(long, conflicts_with = "machine_ids")]
        project_id: Option<i64>,
        /// Narrow a multi-machine project to one machine (only with --project-id).
        #[arg(long, requires = "project_id")]
        machine_id: Option<i64>,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p uecm-cli --lib args::tests 2>&1 | tail -20`
Expected: PASS（两新测试 + 既有 args 测试）

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/cli/args.rs
git commit -m "feat(cli): ini scan --project-id / --machine-id (mutually exclusive)

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 3.3: `scan_cluster` 接收 project_roots + scan_type

**Files:**
- Modify: `src-tauri/src/cli/domain_ini.rs`（`fn scan_cluster` ~567-755；调用点 dispatch ~122）

当前 `scan_cluster(ctx, machine_ids, cred)`，内部 `scan_runs::insert(&db, "ini", machine_ids)` 与 `project_roots: &[]` 硬编码。

- [ ] **Step 1: 写失败测试**（断言 project 维度时 scan_type 是 ini_project + project_roots 生效）

```rust
    #[cfg(not(windows))]
    #[test]
    fn scan_cluster_project_mode_uses_ini_project_type_and_roots() {
        use crate::data::{scan_runs, machines};
        let db = fresh_db();
        let mid = machines::insert(&db, &machines::Machine::new("R1", "127.0.0.1")).unwrap();
        // tempdir 造 <root>/Config/DefaultEngine.ini 含 [DerivedDataBackendGraph]
        let tmp = tempfile::tempdir().unwrap();
        let cfg_dir = tmp.path().join("Config");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        std::fs::write(cfg_dir.join("DefaultEngine.ini"),
            "[DerivedDataBackendGraph]\nRoot=(Type=KeyLength)\n").unwrap();
        let root = tmp.path().to_string_lossy().to_string();

        let mut buf: Vec<u8> = Vec::new();
        let mut ctx = make_ctx(&mut buf, &db);
        let mut roots = std::collections::HashMap::new();
        roots.insert(mid, vec![root]);
        let cred = CredentialArgs { cred_alias: None, user: None, pass: None, pass_stdin: false };
        scan_cluster(&mut ctx, &[mid], roots, "ini_project", &cred).unwrap();
        drop(ctx);

        // 最新 ini_project run 应有 config snapshot
        let runs = scan_runs::list_recent(&db, "ini_project", 1).unwrap();
        assert_eq!(runs.len(), 1);
        let snaps = ini_config_snapshots::list_for_run(&db, runs[0].id.unwrap()).unwrap();
        assert!(snaps.iter().any(|s| s.domain == "ddc" && s.key_name == "Root"));
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p uecm-cli --lib cli::domain_ini::tests::scan_cluster_project 2>&1 | tail -20`
Expected: FAIL（`scan_cluster` 签名不接受 roots/scan_type）

- [ ] **Step 3: 实现** — 改 `scan_cluster` 签名与内部：

```rust
fn scan_cluster(
    ctx: &mut Ctx<'_>,
    machine_ids: &[i64],
    project_paths_per_machine: std::collections::HashMap<i64, Vec<String>>,
    scan_type: &str,
    cred: &CredentialArgs,
) -> UecmResult<()> {
```

内部改动：
- `scan_runs::insert(&db, scan_type, machine_ids)?`（替换硬编码 `"ini"`）。
- 构造 `ScanInputs` 时 `project_roots`：
  ```rust
  let project_roots: Vec<String> =
      project_paths_per_machine.get(&mid).cloned().unwrap_or_default();
  // ...
  project_roots: &project_roots,
  ```
  （替换硬编码 `project_roots: &[]`）
- Task 2.5 的 snapshot 落库循环保持不变（在 outcome 处理处）。

dispatch 调用点（machine 维度，~122）改为：
```rust
        IniAction::Scan { machine_ids, project_id, machine_id, cred } =>
            scan_dispatch(ctx, machine_ids, project_id, machine_id, &cred),
```
（`scan_dispatch` 在 Task 3.4 实现；本 task 先让 machine-only 路径调用 `scan_cluster(ctx, &machine_ids, HashMap::new(), "ini", &cred)` 以保持现有行为——可在 Task 3.4 统一。为本 task 编译通过，临时在 dispatch 写 machine-only 分支。）

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p uecm-cli --lib cli::domain_ini 2>&1 | tail -20`
Expected: PASS（含既有 machine-scoped scan 测试，行为不变、scan_type 仍 "ini"）

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/cli/domain_ini.rs
git commit -m "refactor(cli): scan_cluster takes project_roots + scan_type

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 3.4: `scan_dispatch` — 解析 project_id → project_roots

**Files:**
- Modify: `src-tauri/src/cli/domain_ini.rs`（新 `fn scan_dispatch`）

- [ ] **Step 1: 写失败测试**

```rust
    #[cfg(not(windows))]
    #[test]
    fn scan_dispatch_project_resolves_locations() {
        use crate::data::{machines, projects::{self, Project}, project_locations::{self, ProjectLocation, DiscoveryStatus}};
        let db = fresh_db();
        let mid = machines::insert(&db, &machines::Machine::new("R1", "127.0.0.1")).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("Config")).unwrap();
        std::fs::write(tmp.path().join("Config").join("DefaultEngine.ini"),
            "[DerivedDataBackendGraph]\nRoot=(Type=KeyLength)\n").unwrap();
        let pid = projects::upsert(&db, &Project { id: None, uproject_name: "Demo.uproject".into(),
            uproject_stem_lower: "demo".into(), uproject_guid: None, display_name: None,
            first_seen_at: None, last_seen_at: None, ue_version_major: None, ue_version_minor: None,
            engine_association_raw: None, engine_association_kind: None }).unwrap();
        project_locations::upsert(&db, &ProjectLocation { id: None, project_id: pid, machine_id: mid,
            abs_path: tmp.path().to_string_lossy().to_string(),
            uproject_path: format!("{}\\Demo.uproject", tmp.path().to_string_lossy()),
            discovery_status: DiscoveryStatus::Auto, discovered_at: None }).unwrap();

        let mut buf: Vec<u8> = Vec::new();
        let mut ctx = make_ctx(&mut buf, &db);
        let cred = CredentialArgs { cred_alias: None, user: None, pass: None, pass_stdin: false };
        scan_dispatch(&mut ctx, vec![], Some(pid), None, &cred).unwrap();
        drop(ctx);
        let runs = crate::data::scan_runs::list_recent(&db, "ini_project", 1).unwrap();
        assert_eq!(runs.len(), 1);
    }

    #[test]
    fn scan_dispatch_project_without_location_errors() {
        let db = fresh_db();
        let pid = crate::data::projects::upsert(&db, &crate::data::projects::Project { id: None,
            uproject_name: "X.uproject".into(), uproject_stem_lower: "x".into(), uproject_guid: None,
            display_name: None, first_seen_at: None, last_seen_at: None, ue_version_major: None,
            ue_version_minor: None, engine_association_raw: None, engine_association_kind: None }).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        let mut ctx = make_ctx(&mut buf, &db);
        let cred = CredentialArgs { cred_alias: None, user: None, pass: None, pass_stdin: false };
        let err = scan_dispatch(&mut ctx, vec![], Some(pid), None, &cred).unwrap_err();
        assert!(matches!(err, UecmError::InvalidInput(_)));
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p uecm-cli --lib cli::domain_ini::tests::scan_dispatch 2>&1 | tail -20`
Expected: FAIL（`scan_dispatch` 不存在）

- [ ] **Step 3: 实现**

```rust
fn scan_dispatch(
    ctx: &mut Ctx<'_>,
    machine_ids: Vec<i64>,
    project_id: Option<i64>,
    machine_id: Option<i64>,
    cred: &CredentialArgs,
) -> UecmResult<()> {
    use std::collections::HashMap;
    match project_id {
        None => {
            // 机器维度: 现有行为, scan_type="ini", 无 project_roots
            scan_cluster(ctx, &machine_ids, HashMap::new(), "ini", cred)
        }
        Some(pid) => {
            let db = ctx.require_db()?;
            let mut locs = project_locations::list_by_project(db, pid)?;
            if let Some(only) = machine_id {
                locs.retain(|l| l.machine_id == only);
            }
            if locs.is_empty() {
                return Err(UecmError::InvalidInput(format!(
                    "project {} has no locations (run `project discover` first)",
                    pid
                )));
            }
            let mut roots: HashMap<i64, Vec<String>> = HashMap::new();
            let mut mids: Vec<i64> = Vec::new();
            for l in &locs {
                roots.entry(l.machine_id).or_default().push(l.abs_path.clone());
                if !mids.contains(&l.machine_id) { mids.push(l.machine_id); }
            }
            scan_cluster(ctx, &mids, roots, "ini_project", cred)
        }
    }
}
```

在 `domain_ini.rs` 顶部 `use` 区确认 `project_locations` 已导入（`use crate::data::{... project_locations ...}`；没有则加）。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p uecm-cli --lib cli::domain_ini 2>&1 | tail -20`
Expected: PASS（project 解析 + 无 location 报错）

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/cli/domain_ini.rs
git commit -m "feat(cli): scan_dispatch resolves --project-id via project_locations

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 3.5: `ini runs` 同时列 ini + ini_project

**Files:**
- Modify: `src-tauri/src/cli/domain_ini.rs`（`fn list_runs` ~758-762）

- [ ] **Step 1: 写失败测试**

```rust
    #[test]
    fn list_runs_includes_project_scans() {
        use crate::data::scan_runs;
        let db = fresh_db();
        scan_runs::insert(&db, "ini", &[1]).unwrap();
        scan_runs::insert(&db, "ini_project", &[1]).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        let mut ctx = make_ctx(&mut buf, &db);
        list_runs(&mut ctx, 10).unwrap();
        drop(ctx);
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("ini_project"));
        assert!(s.contains("\"ini\"") || s.contains("ini"));
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p uecm-cli --lib cli::domain_ini::tests::list_runs_includes_project 2>&1 | tail -20`
Expected: FAIL（list_runs 只查 "ini"，不含 ini_project）

- [ ] **Step 3: 实现**

```rust
fn list_runs(ctx: &mut Ctx<'_>, limit: i64) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let runs = scan_runs::list_recent_types(db, &["ini", "ini_project"], limit)?;
    ctx.emitter.emit_result(&runs).ok();
    Ok(())
}
```

（json 模式输出含 `scan_type` 字段即标注 scope；ScanRun 序列化已含 `scan_type`。）

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p uecm-cli --lib cli::domain_ini 2>&1 | tail -20`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/cli/domain_ini.rs
git commit -m "feat(cli): ini runs lists both ini and ini_project scans

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 3.6: Codex #1 回归 — project 深扫不污染 Health

**Files:**
- Test: `src-tauri/src/commands/health_check.rs`（追加测试到其 tests mod，复用 `health_check.rs` 现有 setup）

- [ ] **Step 1: 写测试**

```rust
    #[test]
    fn health_ini_signal_ignores_project_scan() {
        // 1. 建一台机 m1。
        // 2. 插一个 machine-scoped "ini" run R_ini + 给 m1 插 1 条 critical finding。
        // 3. 再插一个更晚的 "ini_project" run R_proj(不含 m1 的 finding)。
        // 4. 调 Health 取 INI 信号的函数(对应 health_check.rs:318 的 list_recent("ini",1) 路径)。
        // 5. 断言:Health 用的是 R_ini(不是更晚的 R_proj),m1 的 critical 计数=1 而非 0。
        // (按 health_check.rs 现有测试如何驱动该判定函数来写;关键断言:
        //  scan_runs::list_recent(db,"ini",1) 返回 R_ini,不被 R_proj 顶替。)
    }
```

> 实现注意：本测试不需要改 Health 生产代码（Task 3.4 已用 `ini_project` 隔离）。它是**回归护栏**，证明隔离生效。照搬 `health_check.rs` 现有测试的 DB setup。若发现 Health 实际取信号路径与 `list_recent("ini",1)` 不同，按真实路径断言。

- [ ] **Step 2: 跑测试确认通过**（隔离已由 Task 3.4 实现，这里应直接 PASS）

Run: `cargo test -p uecm-cli --lib commands::health_check::tests::health_ini_signal_ignores_project 2>&1 | tail -20`
Expected: PASS

- [ ] **Step 3-5: Commit**（无生产改动，仅测试）

```bash
git add src-tauri/src/commands/health_check.rs
git commit -m "test: project scan (ini_project) does not poison Health INI signal

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Phase 3 收尾 / 全量验证

- [ ] `cargo build -p uecm-cli` 通过
- [ ] `cargo test -p uecm-cli` 全绿（含原 968 + 新增）
- [ ] build 复制到 lanPC（参照 CLAUDE.md 的 `pnpm tauri build --no-bundle` + tar `COPYFILE_DISABLE=1` 流程），真机验证：
  - `uecm-cli machine refresh 11` → human 模式列出 6 个引擎版本+路径
  - `uecm-cli machine detail 11` → 对齐表格
  - `uecm-cli project discover --machine-id 11 --roots <razer UE 项目根>` → 发现项目
  - `uecm-cli project list` → 拿 project id
  - `uecm-cli ini scan --project-id <id>` → scan_type=ini_project，落 config 快照
  - `uecm-cli ini config <run_id> --domain ddc` → 展示 DDC 实况
  - `uecm-cli ini runs` → 同时看到 ini 与 ini_project

---

## Self-Review（plan 作者已执行）

**Spec coverage**：改动1=Phase1（Task1.1-1.4）；改动2=Phase2（迁移/data/提取/扫描/落库/查询，含 InstalledDDBG 双标 Task2.3 + FK cascade Task2.1/2.2）；改动3=Phase3（多type查询/args/scan_cluster重构/dispatch/list_runs，含 Codex#1 隔离 Task3.4 + 回归 Task3.6）。spec §4.1 双标、§4.2 FK、§5.3 scope 隔离均有对应 task。

**已知实现期待办（plan 内已显式标注，非 placeholder）**：
- Task 2.3 的 `ConfigEntry` 需加 `file_path` 字段（Task 2.5 Step 3 已说明何时加 + 如何回填测试）。
- 多处 "实现注意" 要求执行者用 `grep` 核对真实字段名（`GpuInfo`/`Machine.last_seen`/`ParsedKey`/最大迁移序号）——这是 against-source 校验，非占位。
- Task 1.4 / 2.5 / 3.6 的测试体标注了"复用现有测试模板"——执行者需照搬同文件既有测试的 fake exec / tempfile / DB setup（因这些 setup 细节随文件演进，强行抄死代码反而易错）。

**Type consistency**：`ConfigSnapshot`（data 层，9 字段 + id）与 `ConfigEntry`（core 层，提取用，加 file_path 后 5 字段）职责分离，caller 转换；`scan_cluster` 新签名 `(ctx, &[i64], HashMap<i64,Vec<String>>, &str, &CredentialArgs)` 在 Task3.3 定义、Task3.4 调用一致；`list_recent_types(&[&str])` 在 Task3.1 定义、Task3.5 调用一致。
