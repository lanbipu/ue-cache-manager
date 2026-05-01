# UECM Plan 2 — Discovery & Single-Machine Configuration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

## Execution Mode (READ FIRST — overrides default skill behavior)

**Mode: AUTO-CONTINUOUS.** Run all 20 tasks back-to-back without pausing for human approval between them. Default subagent-driven-development behavior is "stop after each task for user review" — for this plan, **do not stop between tasks**. Mark each task complete in TodoWrite and immediately dispatch the next implementer.

**Stop and ask the user ONLY in these cases:**

1. **Plan vs reality conflict** that requires re-design (not a small drift — a structural mismatch where continuing would produce wrong work).
2. **Destructive operation requiring authorization**: `rm -rf` outside the workspace, `git push --force`, dropping/altering machine env vars or registry on lanPC, deleting credentials the user might still need, modifying SSH config / firewall.
3. **Critical-severity code review finding with no obvious fix** — the reviewer flags Critical AND the implementer has multiple equally-valid remediation paths.
4. **lanPC unreachable or WinRM not enabled** when a Windows E2E verification step requires it.
5. **A new dependency decision** not covered by the plan (e.g. plan says use crate X, but X is yanked or incompatible — picking a substitute is a structural choice).

**Do NOT stop for:**
- Spec/quality review finding Important or Minor issues. Implementer fixes them in a follow-up `fix:` commit and proceeds to the next task.
- Tests passing on macOS but Windows-gated tests skipped — that's expected; run them via lanPC at the end.
- DONE_WITH_CONCERNS where the concern is a noted observation (record it, move on).
- README / docs cleanup. Do them inline.

**At the very end, produce a single summary report containing:**
- Full commit list (sha + subject) for the plan
- Frontend test count + backend test count (pass/fail)
- Every task that ended `DONE_WITH_CONCERNS` and the concern verbatim
- Production build outcome (artifact paths + size + duration)
- Any deferred items not closed (e.g. lanPC E2E steps that need user attention)

---

**Goal:** Build the discovery + single-machine configuration layer of UECM: scan a LAN CIDR for reachable machines, probe each one's WinRM/SMB ports, detect installed UE versions and GPU/driver info via PowerShell sidecar over WinRM, store everything in SQLite, manage Windows Credential Manager entries, and provide UI to set DDC environment variables and edit a project's `DefaultEngine.ini` keys on a single remote machine.

**Architecture:** All cross-machine operations route through PowerShell sidecar scripts invoked locally (`powershell.exe -File ...`). PowerShell scripts call `Invoke-Command -ComputerName <host>` to run code on the target machine. Credentials are stored via `cmdkey` and consumed transparently by Windows when WinRM connects to the matching target. Rust `core/` modules wrap the sidecar invocations and parse JSON outputs into typed structs. Network scanning is pure-Rust async TCP probing using `tokio` + `ipnet`. Frontend gains a Machines detail panel, a discovery wizard modal, and three single-machine config modals (credential / env var / INI edit).

**Tech Stack:** Builds on Plan 1 stack. New Rust deps: `ipnet = "2.9"` (CIDR parsing). New PowerShell scripts under `ps-scripts/`. No new frontend deps.

**Out of scope for this plan (deferred to later plans):**
- SMB share creation, Mode A/B wizards (Plan 3)
- SYSTEM-level credential injection via `psexec -s cmdkey` (Plan 3)
- Cluster/batch configuration pushes (Plan 3)
- INI conflict detection / auto-fix wizard (Plan 4)
- Health check matrix (Plan 4)
- DDC Pak operations (Plan 5)
- PSO Cache operations (Plan 6)
- Rust-native WinRM SOAP client (deferred indefinitely; PowerShell sidecar covers Plan 2-5)
- ICMP ping / mDNS discovery (TCP port probe is sufficient for v1)
- Visual design polish (Plan 6)

**Deliverable at end:**
1. User can click "Scan Network", enter `192.168.10.0/24`, see all reachable machines with WinRM/SMB port status.
2. User can save WinRM credentials per machine (alias stored in SQLite, password in Windows Credential Manager via `cmdkey`).
3. User can refresh a machine to populate UE installs + GPU info.
4. Machines view shows a detail panel with hostname/IP/role/OS/GPU/driver/UE versions.
5. User can set a DDC env var (`UE-SharedDataCachePath`) on a remote machine; change is verified by re-reading.
6. User can read a `DefaultEngine.ini` section on a remote machine, edit a single key, apply with auto-backup (`.uecm-bak-<timestamp>`).
7. Production build still succeeds. All tests green.

---

## Prerequisites (engineer must have installed before starting)

Same as Plan 1, plus everything in this section.

### Designated Windows Test Machine

End-to-end verification of WinRM-dependent features uses **lanPC** on the home LAN.

| Field | Value |
|---|---|
| Hostname | `lanPC` |
| IP | `192.168.10.20` |
| SSH access (from this dev machine) | `ssh lanpc@192.168.10.20` |
| SSH auth | Public-key via 1Password SSH Agent (no password). Microsoft account user, no password auth available. |
| Local Windows username (for WinRM) | `lanpc` (Microsoft account; the SSH user matches the Windows user) |
| OS | Windows 11 |

**How a subagent uses lanPC for verification (when a task says "manual Windows verification"):**

1. **Run a local PowerShell sidecar against lanPC over SSH (preferred for CI-style scripted verification):**

   ```bash
   # Run a sidecar script that targets lanPC. Pipe its stdout back as JSON.
   ssh lanpc@192.168.10.20 "powershell -NoProfile -ExecutionPolicy Bypass -Command -" < ps-scripts/<script>.ps1
   ```

2. **Or open an SSH session and execute commands interactively:**

   ```bash
   ssh lanpc@192.168.10.20
   # in the resulting Windows shell:
   powershell -NoProfile -ExecutionPolicy Bypass -File C:\path\to\script.ps1 -ArgumentList ...
   ```

3. **For UECM-internal WinRM testing**, lanPC must be both the WinRM CLIENT and the WinRM TARGET in this Plan. Concretely: the UECM app running on this dev machine cannot directly invoke WinRM (PowerShell sidecar is Windows-only — Rust returns `UecmError::PowerShell("WinRM is Windows-only")` on macOS). For end-to-end verification:
   - **Option A (preferred):** Build the UECM `.exe` (`pnpm tauri build`), copy it to lanPC, run it there. lanPC's UECM then targets `127.0.0.1` (itself) for WinRM probes. Quick and isolating.
   - **Option B:** Build the `.exe` and run it on a *different* Windows machine that targets lanPC. Only viable if a second Windows host is available.
   - **Option C (for individual `.ps1` script smoke tests, no UECM involved):** Just `ssh lanpc@... < ps-scripts/<script>.ps1` to confirm the script's local behavior.

   Default to Option A for Plan 2 verification unless a task explicitly says otherwise.

### One-time WinRM enablement on lanPC

Before Plan 2 Task 6 can be verified end-to-end, the engineer must run this once on lanPC (via SSH, or sit at the console). Idempotent — re-running is safe.

```powershell
# Run as admin on lanPC:
Enable-PSRemoting -Force
Set-Item WSMan:\localhost\Client\TrustedHosts -Value '*' -Force
winrm quickconfig -force
```

To verify it stuck:

```powershell
Test-WSMan -ComputerName 127.0.0.1   # should return Product/Vendor metadata, no error
```

### Cross-platform testing rules (recap)

- All PowerShell-dependent Rust unit tests are gated `#[cfg(windows)]`. They are skipped on macOS during dev.
- Non-Windows-gated tests (network scanner, INI/path helpers, frontend mocks) run normally on macOS.
- A subagent that finishes a backend task on macOS should report all tests passing for the cross-platform subset, and explicitly note "Windows-gated tests not run on this host" — they will be exercised by Option A above before the plan is marked complete.

### Subagent etiquette for Windows verification steps

Each task that has a manual-verification step calls out **whether subagent should attempt it via lanPC SSH** or **stop with `DONE_WITH_CONCERNS` for the user to verify in person**. The default is: subagent attempts SSH-based scripted verification (Option C above) where possible; falls back to "user must verify with built `.exe`" for full UI flows. Do not skip the verification step silently.

---

## File Structure

```
UECacheManager/
├── ps-scripts/
│   ├── test-echo.ps1                          # existing (Plan 1)
│   ├── test-winrm.ps1                         # NEW
│   ├── invoke-remote.ps1                      # NEW
│   ├── query-ue-versions.ps1                  # NEW
│   ├── query-gpu-driver.ps1                   # NEW
│   ├── cred-set.ps1                           # NEW
│   ├── cred-delete.ps1                        # NEW
│   ├── cred-list.ps1                          # NEW
│   ├── setx-machine.ps1                       # NEW
│   ├── getx-machine.ps1                       # NEW
│   ├── read-ini-section.ps1                   # NEW
│   └── write-ini-key.ps1                      # NEW
│
├── src-tauri/
│   ├── Cargo.toml                             # MODIFY (add ipnet)
│   └── src/
│       ├── lib.rs                             # MODIFY (register new commands)
│       │
│       ├── commands/
│       │   ├── credentials.rs                 # NEW
│       │   ├── discovery.rs                   # NEW
│       │   ├── env_vars.rs                    # NEW
│       │   ├── ini_editor.rs                  # NEW
│       │   ├── machines.rs                    # MODIFY (add refresh, get_detail)
│       │   ├── mod.rs                         # MODIFY (add new modules)
│       │   └── system.rs                      # existing (Plan 1)
│       │
│       ├── core/
│       │   ├── credentials.rs                 # NEW
│       │   ├── discovery.rs                   # NEW (UE + GPU detect)
│       │   ├── env_vars.rs                    # NEW
│       │   ├── ini_editor.rs                  # NEW
│       │   ├── mod.rs                         # MODIFY (add new modules)
│       │   ├── network.rs                     # NEW (TCP CIDR probe)
│       │   ├── powershell.rs                  # existing (Plan 1)
│       │   └── winrm.rs                       # NEW
│       │
│       └── data/
│           ├── credentials.rs                 # NEW (CRUD for credentials table)
│           ├── machine_gpus.rs                # NEW (CRUD for machine_gpus table)
│           ├── machine_ue_installs.rs         # NEW (CRUD for machine_ue_installs table)
│           ├── mod.rs                         # MODIFY (re-export new modules)
│           └── schema.rs                      # MODIFY (add migrations 003-005)
│
├── src/
│   ├── services/
│   │   └── tauri.ts                           # MODIFY (add new types + functions)
│   │
│   ├── stores/
│   │   ├── credentials.ts                     # NEW
│   │   ├── discovery.ts                       # NEW
│   │   └── machines.ts                        # MODIFY (add refresh + selected detail)
│   │
│   ├── components/
│   │   ├── modals/
│   │   │   ├── BaseModal.vue                  # NEW (reusable modal shell)
│   │   │   ├── CredentialDialog.vue           # NEW
│   │   │   ├── DiscoveryWizard.vue            # NEW
│   │   │   ├── EnvVarConfigModal.vue          # NEW
│   │   │   └── IniEditModal.vue               # NEW
│   │   └── machines/
│   │       └── MachineDetail.vue              # NEW
│   │
│   ├── views/
│   │   └── Machines.vue                       # MODIFY (split layout + detail panel)
│   │
│   └── __tests__/
│       ├── BaseModal.spec.ts                  # NEW
│       ├── CredentialDialog.spec.ts           # NEW
│       ├── DiscoveryWizard.spec.ts            # NEW
│       ├── EnvVarConfigModal.spec.ts          # NEW
│       ├── IniEditModal.spec.ts               # NEW
│       ├── MachineDetail.spec.ts              # NEW
│       ├── credentials-store.spec.ts          # NEW
│       ├── discovery-store.spec.ts            # NEW
│       └── machines-store.spec.ts             # MODIFY (add new behavior tests)
```

**Reasoning for file split:**
- One PowerShell script per atomic operation. Easy to test in isolation, easy to log, easy for ops to inspect.
- `core/` = business logic (no Tauri types). Each file = one logical capability.
- `commands/` = thin Tauri command handlers, one file per feature group, no business logic.
- `data/` = SQLite CRUD, one file per table.
- Frontend: one modal per user flow, all under `components/modals/`. Each modal stays under ~150 lines so it's reviewable in one screen.

---

## Approach Notes

**WinRM via PowerShell sidecar (not native Rust SOAP):** All remote operations are wrapped as `Invoke-Command -ComputerName <host>`. The Rust side just shells out to PowerShell, parses JSON. Keeps Plan 2 risk low. Performance is acceptable for LAN-scale (4-30 machines). Plan 6 may revisit if needed.

**Credential storage:** Windows Credential Manager via `cmdkey`. SQLite stores only the alias + display username. We never read passwords back into our process — we let Windows transparently use stored credentials when WinRM connects. This means our Rust code never holds plaintext passwords in memory after the user submits them. Slight UX downside: we can't show "stored vs not stored" beyond what `cmdkey /list` reports.

**INI editing strategy:** All INI manipulation runs on the remote machine via PowerShell. Backup file naming: `<original>.uecm-bak-<UnixTimestamp>`. PowerShell text manipulation is sufficient for v1 (no `rust-ini` needed). v2 may revisit if INI conflicts get complex.

**Network discovery strategy:** TCP `connect()` to ports 5985 (WinRM HTTP) and 445 (SMB) with a 1-second timeout per host, all hosts in parallel via `tokio::spawn`. No ICMP, no mDNS — these add complexity without proportional value for the LAN target.

**Cross-platform testing gates:** PowerShell-dependent tests are `#[cfg(windows)]`. Other tests work on macOS/Linux. CI must run on both Windows (for Windows-gated tests) and a Unix host (for the cross-platform majority).

---

## Task 1: Add `ipnet` dep + 3 new SQLite migrations

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/data/schema.rs`

- [ ] **Step 1: Add ipnet to Cargo.toml**

In `[dependencies]` section, add:

```toml
ipnet = "2.9"
```

After saving, run `cd src-tauri && cargo check` to fetch the dep. Expect success.

- [ ] **Step 2: Add migrations 003, 004, 005 to schema.rs**

Edit `src-tauri/src/data/schema.rs`. Append three new entries to the `MIGRATIONS` const array (preserving the existing `001_machines_table`):

```rust
const MIGRATIONS: &[(&str, &str)] = &[
    (
        "001_machines_table",
        r#"
        CREATE TABLE IF NOT EXISTS machines (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            hostname TEXT NOT NULL,
            ip TEXT NOT NULL UNIQUE,
            role TEXT NOT NULL DEFAULT 'unknown',
            status TEXT NOT NULL DEFAULT 'unknown',
            last_seen_at TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_machines_status ON machines(status);
        "#,
    ),
    (
        "003_machine_ue_installs",
        r#"
        CREATE TABLE IF NOT EXISTS machine_ue_installs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            machine_id INTEGER NOT NULL,
            version TEXT NOT NULL,
            install_path TEXT NOT NULL,
            is_primary INTEGER NOT NULL DEFAULT 0,
            detected_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(machine_id, version),
            FOREIGN KEY (machine_id) REFERENCES machines(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_machine_ue_installs_machine ON machine_ue_installs(machine_id);
        "#,
    ),
    (
        "004_machine_gpus",
        r#"
        CREATE TABLE IF NOT EXISTS machine_gpus (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            machine_id INTEGER NOT NULL,
            gpu_model TEXT NOT NULL,
            driver_version TEXT NOT NULL,
            vendor TEXT NOT NULL DEFAULT 'unknown',
            vram_mb INTEGER,
            detected_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (machine_id) REFERENCES machines(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_machine_gpus_machine ON machine_gpus(machine_id);
        "#,
    ),
    (
        "005_credentials",
        r#"
        CREATE TABLE IF NOT EXISTS credentials (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            alias TEXT NOT NULL UNIQUE,
            kind TEXT NOT NULL,
            username TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_credentials_alias ON credentials(alias);
        "#,
    ),
];
```

- [ ] **Step 3: Add a test verifying the new tables exist after migration**

In `src-tauri/src/data/schema.rs` `mod tests` block, append:

```rust
#[test]
fn migrate_creates_machine_ue_installs_table() {
    let db = open_in_memory().unwrap();
    let mut conn = db.lock().unwrap();
    migrate(&mut conn).unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='machine_ue_installs'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn migrate_creates_machine_gpus_table() {
    let db = open_in_memory().unwrap();
    let mut conn = db.lock().unwrap();
    migrate(&mut conn).unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='machine_gpus'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn migrate_creates_credentials_table() {
    let db = open_in_memory().unwrap();
    let mut conn = db.lock().unwrap();
    migrate(&mut conn).unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='credentials'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}
```

- [ ] **Step 4: Run tests**

```bash
export PATH="/Users/bip.lan/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
cd src-tauri && cargo test --lib data::schema && cd ..
```

Expected: 6 tests pass (3 from Plan 1 + 3 new).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/data/schema.rs
git commit -m "feat(rust): add ipnet dep + migrations for ue_installs, gpus, credentials"
```

---

## Task 2: Data CRUD for `machine_ue_installs`

**Files:**
- Create: `src-tauri/src/data/machine_ue_installs.rs`
- Modify: `src-tauri/src/data/mod.rs`

- [ ] **Step 1: Write the test file with the struct + tests**

Create `src-tauri/src/data/machine_ue_installs.rs`:

```rust
//! CRUD for the `machine_ue_installs` table.

use crate::data::Db;
use crate::error::UecmResult;
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UeInstall {
    pub id: Option<i64>,
    pub machine_id: i64,
    pub version: String,        // e.g. "5.4", "5.5"
    pub install_path: String,   // e.g. "C:\\Program Files\\Epic Games\\UE_5.4"
    pub is_primary: bool,
}

pub fn upsert(db: &Db, install: &UeInstall) -> UecmResult<i64> {
    let conn = db.lock().unwrap();
    conn.execute(
        "INSERT INTO machine_ue_installs (machine_id, version, install_path, is_primary)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(machine_id, version) DO UPDATE SET
            install_path = excluded.install_path,
            is_primary = excluded.is_primary,
            detected_at = CURRENT_TIMESTAMP",
        params![
            install.machine_id,
            install.version,
            install.install_path,
            install.is_primary as i32,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list_for_machine(db: &Db, machine_id: i64) -> UecmResult<Vec<UeInstall>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, machine_id, version, install_path, is_primary
         FROM machine_ue_installs WHERE machine_id = ? ORDER BY version DESC",
    )?;
    let rows = stmt.query_map(params![machine_id], |row| {
        Ok(UeInstall {
            id: Some(row.get(0)?),
            machine_id: row.get(1)?,
            version: row.get(2)?,
            install_path: row.get(3)?,
            is_primary: row.get::<_, i32>(4)? != 0,
        })
    })?;
    let mut result = Vec::new();
    for r in rows {
        result.push(r?);
    }
    Ok(result)
}

pub fn delete_for_machine(db: &Db, machine_id: i64) -> UecmResult<()> {
    let conn = db.lock().unwrap();
    conn.execute(
        "DELETE FROM machine_ue_installs WHERE machine_id = ?",
        params![machine_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{machines, open_in_memory, schema, Machine};

    fn setup() -> (Db, i64) {
        let db = open_in_memory().unwrap();
        {
            let mut conn = db.lock().unwrap();
            schema::migrate(&mut conn).unwrap();
        }
        let machine_id = machines::insert(
            &db,
            &Machine::new("RENDER-01", "192.168.10.21"),
        )
        .unwrap();
        (db, machine_id)
    }

    #[test]
    fn upsert_inserts_when_new() {
        let (db, machine_id) = setup();
        let install = UeInstall {
            id: None,
            machine_id,
            version: "5.4".to_string(),
            install_path: "C:\\UE_5.4".to_string(),
            is_primary: true,
        };
        let id = upsert(&db, &install).unwrap();
        assert!(id > 0);
    }

    #[test]
    fn upsert_updates_when_machine_version_exists() {
        let (db, machine_id) = setup();
        let install = UeInstall {
            id: None,
            machine_id,
            version: "5.4".to_string(),
            install_path: "C:\\OldPath".to_string(),
            is_primary: false,
        };
        upsert(&db, &install).unwrap();

        let updated = UeInstall {
            install_path: "C:\\NewPath".to_string(),
            is_primary: true,
            ..install
        };
        upsert(&db, &updated).unwrap();

        let installs = list_for_machine(&db, machine_id).unwrap();
        assert_eq!(installs.len(), 1);
        assert_eq!(installs[0].install_path, "C:\\NewPath");
        assert!(installs[0].is_primary);
    }

    #[test]
    fn list_for_machine_returns_all_versions_desc() {
        let (db, machine_id) = setup();
        upsert(&db, &UeInstall { id: None, machine_id, version: "5.3".into(), install_path: "C:\\A".into(), is_primary: false }).unwrap();
        upsert(&db, &UeInstall { id: None, machine_id, version: "5.5".into(), install_path: "C:\\C".into(), is_primary: false }).unwrap();
        upsert(&db, &UeInstall { id: None, machine_id, version: "5.4".into(), install_path: "C:\\B".into(), is_primary: true }).unwrap();

        let installs = list_for_machine(&db, machine_id).unwrap();
        assert_eq!(installs.len(), 3);
        assert_eq!(installs[0].version, "5.5");
        assert_eq!(installs[1].version, "5.4");
        assert_eq!(installs[2].version, "5.3");
    }

    #[test]
    fn delete_for_machine_removes_all_installs() {
        let (db, machine_id) = setup();
        upsert(&db, &UeInstall { id: None, machine_id, version: "5.4".into(), install_path: "C:\\A".into(), is_primary: true }).unwrap();
        delete_for_machine(&db, machine_id).unwrap();
        let installs = list_for_machine(&db, machine_id).unwrap();
        assert!(installs.is_empty());
    }
}
```

- [ ] **Step 2: Update `src-tauri/src/data/mod.rs`**

```rust
pub mod connection;
pub mod machine_ue_installs;
pub mod machines;
pub mod schema;

pub use connection::{open, open_in_memory, Db};
pub use machine_ue_installs::UeInstall;
pub use machines::Machine;
```

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test --lib data::machine_ue_installs && cd ..
```

Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/data
git commit -m "feat(rust): add UeInstall struct with upsert/list/delete CRUD"
```

---

## Task 3: Data CRUD for `machine_gpus`

**Files:**
- Create: `src-tauri/src/data/machine_gpus.rs`
- Modify: `src-tauri/src/data/mod.rs`

- [ ] **Step 1: Write the file with struct + tests**

Create `src-tauri/src/data/machine_gpus.rs`:

```rust
//! CRUD for the `machine_gpus` table.

use crate::data::Db;
use crate::error::UecmResult;
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GpuInfo {
    pub id: Option<i64>,
    pub machine_id: i64,
    pub gpu_model: String,
    pub driver_version: String,
    pub vendor: String,        // "nvidia" | "amd" | "intel" | "unknown"
    pub vram_mb: Option<i64>,
}

pub fn insert(db: &Db, gpu: &GpuInfo) -> UecmResult<i64> {
    let conn = db.lock().unwrap();
    conn.execute(
        "INSERT INTO machine_gpus (machine_id, gpu_model, driver_version, vendor, vram_mb)
         VALUES (?, ?, ?, ?, ?)",
        params![
            gpu.machine_id,
            gpu.gpu_model,
            gpu.driver_version,
            gpu.vendor,
            gpu.vram_mb,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list_for_machine(db: &Db, machine_id: i64) -> UecmResult<Vec<GpuInfo>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, machine_id, gpu_model, driver_version, vendor, vram_mb
         FROM machine_gpus WHERE machine_id = ? ORDER BY id",
    )?;
    let rows = stmt.query_map(params![machine_id], |row| {
        Ok(GpuInfo {
            id: Some(row.get(0)?),
            machine_id: row.get(1)?,
            gpu_model: row.get(2)?,
            driver_version: row.get(3)?,
            vendor: row.get(4)?,
            vram_mb: row.get(5)?,
        })
    })?;
    let mut result = Vec::new();
    for r in rows {
        result.push(r?);
    }
    Ok(result)
}

pub fn replace_for_machine(db: &Db, machine_id: i64, gpus: &[GpuInfo]) -> UecmResult<()> {
    let mut conn = db.lock().unwrap();
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM machine_gpus WHERE machine_id = ?", params![machine_id])?;
    for gpu in gpus {
        tx.execute(
            "INSERT INTO machine_gpus (machine_id, gpu_model, driver_version, vendor, vram_mb)
             VALUES (?, ?, ?, ?, ?)",
            params![
                machine_id,
                gpu.gpu_model,
                gpu.driver_version,
                gpu.vendor,
                gpu.vram_mb,
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{machines, open_in_memory, schema, Machine};

    fn setup() -> (Db, i64) {
        let db = open_in_memory().unwrap();
        {
            let mut conn = db.lock().unwrap();
            schema::migrate(&mut conn).unwrap();
        }
        let machine_id = machines::insert(
            &db,
            &Machine::new("RENDER-01", "192.168.10.21"),
        )
        .unwrap();
        (db, machine_id)
    }

    fn sample_gpu(machine_id: i64, model: &str) -> GpuInfo {
        GpuInfo {
            id: None,
            machine_id,
            gpu_model: model.to_string(),
            driver_version: "551.86".to_string(),
            vendor: "nvidia".to_string(),
            vram_mb: Some(24576),
        }
    }

    #[test]
    fn insert_returns_new_id() {
        let (db, machine_id) = setup();
        let id = insert(&db, &sample_gpu(machine_id, "RTX 4090")).unwrap();
        assert!(id > 0);
    }

    #[test]
    fn list_for_machine_returns_inserted_gpus() {
        let (db, machine_id) = setup();
        insert(&db, &sample_gpu(machine_id, "RTX 4090")).unwrap();
        insert(&db, &sample_gpu(machine_id, "RTX 4080")).unwrap();
        let gpus = list_for_machine(&db, machine_id).unwrap();
        assert_eq!(gpus.len(), 2);
    }

    #[test]
    fn replace_for_machine_atomically_swaps_gpu_set() {
        let (db, machine_id) = setup();
        insert(&db, &sample_gpu(machine_id, "RTX 4090")).unwrap();
        insert(&db, &sample_gpu(machine_id, "RTX 4080")).unwrap();

        replace_for_machine(&db, machine_id, &[sample_gpu(machine_id, "RTX 5090")]).unwrap();

        let gpus = list_for_machine(&db, machine_id).unwrap();
        assert_eq!(gpus.len(), 1);
        assert_eq!(gpus[0].gpu_model, "RTX 5090");
    }
}
```

- [ ] **Step 2: Update `src-tauri/src/data/mod.rs`**

```rust
pub mod connection;
pub mod machine_gpus;
pub mod machine_ue_installs;
pub mod machines;
pub mod schema;

pub use connection::{open, open_in_memory, Db};
pub use machine_gpus::GpuInfo;
pub use machine_ue_installs::UeInstall;
pub use machines::Machine;
```

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test --lib data::machine_gpus && cd ..
```

Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/data
git commit -m "feat(rust): add GpuInfo struct with insert/list/replace_for_machine CRUD"
```

---

## Task 4: Data CRUD for `credentials`

**Files:**
- Create: `src-tauri/src/data/credentials.rs`
- Modify: `src-tauri/src/data/mod.rs`

- [ ] **Step 1: Write the file with struct + tests**

Create `src-tauri/src/data/credentials.rs`:

```rust
//! CRUD for the `credentials` table. Stores ONLY alias + display username;
//! the actual password lives in Windows Credential Manager (set via cmdkey).

use crate::data::Db;
use crate::error::UecmResult;
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CredentialRecord {
    pub id: Option<i64>,
    pub alias: String,         // e.g. "UECM:winrm:RENDER-01"
    pub kind: String,          // "winrm" | "share"
    pub username: String,      // for display only
}

pub fn insert(db: &Db, cred: &CredentialRecord) -> UecmResult<i64> {
    let conn = db.lock().unwrap();
    conn.execute(
        "INSERT INTO credentials (alias, kind, username) VALUES (?, ?, ?)",
        params![cred.alias, cred.kind, cred.username],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list_all(db: &Db) -> UecmResult<Vec<CredentialRecord>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, alias, kind, username FROM credentials ORDER BY alias",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(CredentialRecord {
            id: Some(row.get(0)?),
            alias: row.get(1)?,
            kind: row.get(2)?,
            username: row.get(3)?,
        })
    })?;
    let mut result = Vec::new();
    for r in rows {
        result.push(r?);
    }
    Ok(result)
}

pub fn find_by_alias(db: &Db, alias: &str) -> UecmResult<Option<CredentialRecord>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, alias, kind, username FROM credentials WHERE alias = ?",
    )?;
    let mut rows = stmt.query(params![alias])?;
    if let Some(row) = rows.next()? {
        Ok(Some(CredentialRecord {
            id: Some(row.get(0)?),
            alias: row.get(1)?,
            kind: row.get(2)?,
            username: row.get(3)?,
        }))
    } else {
        Ok(None)
    }
}

pub fn delete_by_alias(db: &Db, alias: &str) -> UecmResult<()> {
    let conn = db.lock().unwrap();
    conn.execute("DELETE FROM credentials WHERE alias = ?", params![alias])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{open_in_memory, schema};

    fn setup() -> Db {
        let db = open_in_memory().unwrap();
        {
            let mut conn = db.lock().unwrap();
            schema::migrate(&mut conn).unwrap();
        }
        db
    }

    fn sample(alias: &str, user: &str) -> CredentialRecord {
        CredentialRecord {
            id: None,
            alias: alias.to_string(),
            kind: "winrm".to_string(),
            username: user.to_string(),
        }
    }

    #[test]
    fn insert_returns_new_id() {
        let db = setup();
        let id = insert(&db, &sample("UECM:winrm:HOST-A", "admin")).unwrap();
        assert!(id > 0);
    }

    #[test]
    fn list_all_returns_inserted_in_alpha_order() {
        let db = setup();
        insert(&db, &sample("UECM:winrm:B-HOST", "admin")).unwrap();
        insert(&db, &sample("UECM:winrm:A-HOST", "admin")).unwrap();
        let creds = list_all(&db).unwrap();
        assert_eq!(creds.len(), 2);
        assert_eq!(creds[0].alias, "UECM:winrm:A-HOST");
    }

    #[test]
    fn find_by_alias_returns_matching_record() {
        let db = setup();
        insert(&db, &sample("UECM:winrm:HOST-A", "admin")).unwrap();
        let found = find_by_alias(&db, "UECM:winrm:HOST-A").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().username, "admin");
    }

    #[test]
    fn find_by_alias_returns_none_when_missing() {
        let db = setup();
        let found = find_by_alias(&db, "UECM:winrm:UNKNOWN").unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn delete_by_alias_removes_record() {
        let db = setup();
        insert(&db, &sample("UECM:winrm:HOST-A", "admin")).unwrap();
        delete_by_alias(&db, "UECM:winrm:HOST-A").unwrap();
        assert!(find_by_alias(&db, "UECM:winrm:HOST-A").unwrap().is_none());
    }

    #[test]
    fn duplicate_alias_returns_database_error() {
        let db = setup();
        insert(&db, &sample("UECM:winrm:HOST-A", "admin")).unwrap();
        let result = insert(&db, &sample("UECM:winrm:HOST-A", "other"));
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: Update `src-tauri/src/data/mod.rs`**

```rust
pub mod connection;
pub mod credentials;
pub mod machine_gpus;
pub mod machine_ue_installs;
pub mod machines;
pub mod schema;

pub use connection::{open, open_in_memory, Db};
pub use credentials::CredentialRecord;
pub use machine_gpus::GpuInfo;
pub use machine_ue_installs::UeInstall;
pub use machines::Machine;
```

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test --lib data::credentials && cd ..
```

Expected: 6 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/data
git commit -m "feat(rust): add CredentialRecord struct with insert/list/find/delete CRUD"
```

---

## Task 5: PowerShell scripts — WinRM probe + invoke wrappers

**Files:**
- Create: `ps-scripts/test-winrm.ps1`
- Create: `ps-scripts/invoke-remote.ps1`

- [ ] **Step 1: Write `ps-scripts/test-winrm.ps1`**

```powershell
# Tests WinRM connectivity to a remote host.
# Returns JSON: { ok: bool, message: string, latency_ms: int }
# Usage: powershell.exe -NoProfile -ExecutionPolicy Bypass -File test-winrm.ps1 -Host RENDER-01

param(
    [Parameter(Mandatory=$true)]
    [string]$HostName
)

$ErrorActionPreference = 'Stop'
$started = Get-Date

try {
    $null = Test-WSMan -ComputerName $HostName -ErrorAction Stop
    $elapsed = [int]((Get-Date) - $started).TotalMilliseconds
    @{
        ok = $true
        message = "WinRM reachable"
        latency_ms = $elapsed
    } | ConvertTo-Json -Compress
}
catch {
    $elapsed = [int]((Get-Date) - $started).TotalMilliseconds
    @{
        ok = $false
        message = $_.Exception.Message
        latency_ms = $elapsed
    } | ConvertTo-Json -Compress
}
```

- [ ] **Step 2: Write `ps-scripts/invoke-remote.ps1`**

```powershell
# Runs an arbitrary PowerShell scriptblock on a remote host via WinRM.
# Stdin: the scriptblock body (UTF-8, no BOM).
# Returns: stdout from the remote scriptblock (the script is responsible for
# producing its own JSON if structured output is needed). On failure, prints
# JSON { error: string } to stderr and exits non-zero.
# Usage:
#   echo '<script body>' | powershell.exe -NoProfile -ExecutionPolicy Bypass -File invoke-remote.ps1 -Host RENDER-01

param(
    [Parameter(Mandatory=$true)]
    [string]$HostName
)

$ErrorActionPreference = 'Stop'

# Read entire stdin as the scriptblock body
$scriptText = [Console]::In.ReadToEnd()
if ([string]::IsNullOrWhiteSpace($scriptText)) {
    [Console]::Error.WriteLine((@{ error = "empty script body on stdin" } | ConvertTo-Json -Compress))
    exit 2
}

$scriptBlock = [scriptblock]::Create($scriptText)

try {
    Invoke-Command -ComputerName $HostName -ScriptBlock $scriptBlock -ErrorAction Stop
    exit 0
}
catch {
    [Console]::Error.WriteLine((@{ error = $_.Exception.Message } | ConvertTo-Json -Compress))
    exit 1
}
```

- [ ] **Step 3: (Optional, Windows only) Manual verification**

Skip if no Windows host available. On Windows with a reachable target:

```powershell
.\ps-scripts\test-winrm.ps1 -HostName <reachable-host>
# expect: {"ok":true,"message":"WinRM reachable","latency_ms":123}
```

- [ ] **Step 4: Commit**

```bash
git add ps-scripts/test-winrm.ps1 ps-scripts/invoke-remote.ps1
git commit -m "feat(ps): add WinRM probe and remote-invoke sidecar scripts"
```

---

## Task 6: Rust `core/winrm.rs` wrapper

**Files:**
- Create: `src-tauri/src/core/winrm.rs`
- Modify: `src-tauri/src/core/mod.rs`

- [ ] **Step 1: Write `src-tauri/src/core/winrm.rs`**

```rust
//! Thin Rust wrapper over the WinRM PowerShell sidecar scripts.
//! All operations are Windows-only at runtime; non-Windows returns
//! `UecmError::PowerShell("WinRM is Windows-only")` so the codebase still builds + tests on dev machines.

use crate::core::powershell;
use crate::error::{UecmError, UecmResult};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct ProbeResult {
    pub ok: bool,
    pub message: String,
    pub latency_ms: i64,
}

/// Probe a single host's WinRM availability.
pub fn probe(host: &str) -> UecmResult<ProbeResult> {
    let script = script_path("test-winrm.ps1");
    powershell::run_json::<ProbeResult>(&script, &["-HostName", host])
}

/// Invoke a PowerShell scriptblock on a remote host. Returns combined stdout.
/// The script body is passed via stdin (no escaping required).
#[cfg(windows)]
pub fn invoke(host: &str, script_body: &str) -> UecmResult<String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let wrapper = script_path("invoke-remote.ps1");

    let mut child = Command::new("powershell.exe")
        .arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(&wrapper)
        .arg("-HostName")
        .arg(host)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| UecmError::PowerShell(format!("failed to spawn powershell.exe: {}", e)))?;

    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| UecmError::PowerShell("failed to open stdin".to_string()))?;
        stdin
            .write_all(script_body.as_bytes())
            .map_err(|e| UecmError::PowerShell(format!("failed to write stdin: {}", e)))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| UecmError::PowerShell(format!("wait failed: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(UecmError::PowerShell(format!(
            "remote invoke failed (exit {}): {}",
            output.status.code().unwrap_or(-1),
            stderr
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(not(windows))]
pub fn invoke(host: &str, script_body: &str) -> UecmResult<String> {
    let _ = (host, script_body);
    Err(UecmError::PowerShell(
        "WinRM is Windows-only".to_string(),
    ))
}

/// Invoke a remote script and parse stdout as JSON of type T.
pub fn invoke_json<T: serde::de::DeserializeOwned>(host: &str, script_body: &str) -> UecmResult<T> {
    let raw = invoke(host, script_body)?;
    serde_json::from_str(&raw).map_err(|e| {
        UecmError::PowerShell(format!("failed to parse remote JSON: {} (raw: {})", e, raw))
    })
}

fn script_path(name: &str) -> PathBuf {
    // Same convention as `commands/system.rs::resolve_script_path` — relative to src-tauri/.
    Path::new("..").join("ps-scripts").join(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn invoke_returns_error_on_non_windows() {
        let result = invoke("RENDER-01", "Get-Date");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), UecmError::PowerShell(_)));
    }

    #[cfg(not(windows))]
    #[test]
    fn probe_returns_error_on_non_windows() {
        // probe goes through powershell::run_script which also fails on non-Windows
        let result = probe("RENDER-01");
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: Update `src-tauri/src/core/mod.rs`**

```rust
pub mod powershell;
pub mod winrm;
```

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test --lib core::winrm && cd ..
```

Expected on macOS: 2 tests pass. On Windows: 0 tests run (both gated `cfg(not(windows))`).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/core
git commit -m "feat(rust): add WinRM wrapper (probe + invoke + invoke_json)"
```

---

## Task 7: PowerShell scripts — UE versions + GPU detection

**Files:**
- Create: `ps-scripts/query-ue-versions.ps1`
- Create: `ps-scripts/query-gpu-driver.ps1`

These two scripts run on the **remote** machine (passed to `invoke-remote.ps1` as scriptblock bodies via stdin in later tasks). They must produce compact JSON only — no other output.

- [ ] **Step 1: Write `ps-scripts/query-ue-versions.ps1`**

```powershell
# Reads installed Unreal Engine versions from registry.
# Designed to run via `invoke-remote.ps1` (passed as scriptblock body via stdin),
# but also runnable standalone for local testing.
# Output: JSON array of { version, install_path }, e.g.
#   [{"version":"5.4","install_path":"C:\\Program Files\\Epic Games\\UE_5.4"}]

$ErrorActionPreference = 'SilentlyContinue'

$results = @()

$keys = @(
    'HKLM:\SOFTWARE\EpicGames\Unreal Engine',
    'HKLM:\SOFTWARE\WOW6432Node\EpicGames\Unreal Engine'
)

foreach ($keyPath in $keys) {
    if (Test-Path $keyPath) {
        Get-ChildItem $keyPath | ForEach-Object {
            $version = $_.PSChildName
            $installedDir = (Get-ItemProperty $_.PSPath -Name 'InstalledDirectory' -ErrorAction SilentlyContinue).InstalledDirectory
            if ($installedDir) {
                $results += [PSCustomObject]@{
                    version = $version
                    install_path = $installedDir
                }
            }
        }
    }
}

# Deduplicate by (version, install_path)
$results = $results | Sort-Object version, install_path -Unique

# Always emit valid JSON, even for empty
ConvertTo-Json -InputObject @($results) -Compress
```

- [ ] **Step 2: Write `ps-scripts/query-gpu-driver.ps1`**

```powershell
# Queries GPU model + driver version via WMI.
# Output: JSON array of { gpu_model, driver_version, vendor, vram_mb }
# Designed to run via `invoke-remote.ps1`.

$ErrorActionPreference = 'SilentlyContinue'

$controllers = Get-CimInstance -ClassName Win32_VideoController

$results = @()
foreach ($c in $controllers) {
    $name = $c.Name
    $vendor = 'unknown'
    if ($name -match 'NVIDIA')   { $vendor = 'nvidia' }
    elseif ($name -match 'AMD' -or $name -match 'Radeon') { $vendor = 'amd' }
    elseif ($name -match 'Intel') { $vendor = 'intel' }

    $vramBytes = [int64]$c.AdapterRAM
    # AdapterRAM is unsigned 32-bit and may report negative for >4GB; recompute via DXGI fallback if available.
    # For now, accept the WMI value; null if we can't compute.
    $vramMb = $null
    if ($vramBytes -gt 0) {
        $vramMb = [int64]([math]::Round($vramBytes / 1MB))
    }

    $results += [PSCustomObject]@{
        gpu_model = $name
        driver_version = $c.DriverVersion
        vendor = $vendor
        vram_mb = $vramMb
    }
}

ConvertTo-Json -InputObject @($results) -Compress
```

- [ ] **Step 3: (Optional, Windows only) Local sanity check**

```powershell
.\ps-scripts\query-ue-versions.ps1
# expect: [] or [{"version":"5.4","install_path":"..."}]
.\ps-scripts\query-gpu-driver.ps1
# expect: [{"gpu_model":"NVIDIA RTX 4090","driver_version":"...","vendor":"nvidia","vram_mb":24576}]
```

- [ ] **Step 4: Commit**

```bash
git add ps-scripts/query-ue-versions.ps1 ps-scripts/query-gpu-driver.ps1
git commit -m "feat(ps): add UE version + GPU driver discovery scripts"
```

---

## Task 8: Rust `core/network.rs` — async TCP CIDR probe

**Files:**
- Create: `src-tauri/src/core/network.rs`
- Modify: `src-tauri/src/core/mod.rs`

- [ ] **Step 1: Write `src-tauri/src/core/network.rs`**

```rust
//! Async TCP port probe for LAN discovery. Cross-platform — works on macOS
//! during development. No raw sockets, no ICMP, no privileges required.

use crate::error::{UecmError, UecmResult};
use ipnet::Ipv4Net;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;

/// Port probe result for a single host.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbedHost {
    pub ip: String,
    pub winrm_open: bool,   // port 5985
    pub smb_open: bool,     // port 445
}

/// Default per-port connect timeout. Public so callers (and tests) can tune.
pub const DEFAULT_TIMEOUT_MS: u64 = 1000;

/// Maximum hosts to scan in one call. Guard against accidentally scanning a /16.
pub const MAX_HOSTS: usize = 1024;

const PORT_WINRM: u16 = 5985;
const PORT_SMB: u16 = 445;

pub async fn scan_cidr(cidr: &str, timeout_ms: u64) -> UecmResult<Vec<ProbedHost>> {
    let net = Ipv4Net::from_str(cidr)
        .map_err(|e| UecmError::InvalidInput(format!("invalid CIDR '{}': {}", cidr, e)))?;

    let hosts: Vec<IpAddr> = net.hosts().map(IpAddr::V4).collect();

    if hosts.len() > MAX_HOSTS {
        return Err(UecmError::InvalidInput(format!(
            "CIDR expands to {} hosts (max {})",
            hosts.len(),
            MAX_HOSTS
        )));
    }

    let mut handles = Vec::with_capacity(hosts.len());
    for ip in hosts {
        let h = tokio::spawn(async move { probe_host(ip, timeout_ms).await });
        handles.push(h);
    }

    let mut results = Vec::with_capacity(handles.len());
    for h in handles {
        if let Ok(probed) = h.await {
            results.push(probed);
        }
    }

    // Only return hosts where at least one port responded.
    Ok(results
        .into_iter()
        .filter(|r| r.winrm_open || r.smb_open)
        .collect())
}

async fn probe_host(ip: IpAddr, timeout_ms: u64) -> ProbedHost {
    let winrm = probe_port(ip, PORT_WINRM, timeout_ms).await;
    let smb = probe_port(ip, PORT_SMB, timeout_ms).await;
    ProbedHost {
        ip: ip.to_string(),
        winrm_open: winrm,
        smb_open: smb,
    }
}

async fn probe_port(ip: IpAddr, port: u16, timeout_ms: u64) -> bool {
    let addr = SocketAddr::new(ip, port);
    matches!(
        timeout(Duration::from_millis(timeout_ms), TcpStream::connect(addr)).await,
        Ok(Ok(_))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn invalid_cidr_returns_error() {
        let result = scan_cidr("not-a-cidr", 100).await;
        assert!(matches!(result, Err(UecmError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn cidr_too_large_returns_error() {
        // /16 = 65534 hosts, well over MAX_HOSTS
        let result = scan_cidr("10.0.0.0/16", 100).await;
        assert!(matches!(result, Err(UecmError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn small_cidr_completes_within_reasonable_time() {
        // /30 = 2 usable hosts, in TEST-NET-3 (RFC 5737) — guaranteed unreachable
        let started = std::time::Instant::now();
        let result = scan_cidr("203.0.113.0/30", 200).await.unwrap();
        let elapsed = started.elapsed();
        // All probes parallel; 4-port-probe x 2 hosts at 200ms each should
        // complete in well under 1s if concurrency works.
        assert!(elapsed.as_millis() < 1500, "scan took {}ms", elapsed.as_millis());
        // No host should respond from TEST-NET-3
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn loopback_smb_probe_does_not_panic() {
        // /32 expands to 0 hosts, so use /31 with two addresses
        // Just confirm probe_port returns bool without panicking
        let _ = probe_port(IpAddr::from_str("127.0.0.1").unwrap(), 1, 50).await;
    }
}
```

- [ ] **Step 2: Update `src-tauri/src/core/mod.rs`**

```rust
pub mod network;
pub mod powershell;
pub mod winrm;
```

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test --lib core::network && cd ..
```

Expected: 4 tests pass. The "small_cidr" test takes ~400ms due to two unreachable hosts × 200ms timeout × 2 ports run in parallel.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/core src-tauri/Cargo.lock
git commit -m "feat(rust): add async TCP CIDR scanner (probes ports 5985 + 445)"
```

---

## Task 9: Rust `core/discovery.rs` — UE + GPU detection over WinRM

**Files:**
- Create: `src-tauri/src/core/discovery.rs`
- Modify: `src-tauri/src/core/mod.rs`

- [ ] **Step 1: Write `src-tauri/src/core/discovery.rs`**

```rust
//! Discovery probes that run on a remote host via WinRM:
//! - UE installed versions (registry read)
//! - GPU model + driver version (WMI Win32_VideoController)

use crate::core::winrm;
use crate::error::UecmResult;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct DetectedUe {
    pub version: String,
    pub install_path: String,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct DetectedGpu {
    pub gpu_model: String,
    pub driver_version: String,
    pub vendor: String,
    pub vram_mb: Option<i64>,
}

pub fn detect_ue_versions(host: &str) -> UecmResult<Vec<DetectedUe>> {
    let body = read_script("query-ue-versions.ps1")?;
    let result: Vec<DetectedUe> = winrm::invoke_json(host, &body)?;
    Ok(result)
}

pub fn detect_gpus(host: &str) -> UecmResult<Vec<DetectedGpu>> {
    let body = read_script("query-gpu-driver.ps1")?;
    let result: Vec<DetectedGpu> = winrm::invoke_json(host, &body)?;
    Ok(result)
}

fn read_script(name: &str) -> UecmResult<String> {
    let path: PathBuf = Path::new("..").join("ps-scripts").join(name);
    Ok(fs::read_to_string(&path)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::UecmError;

    #[test]
    fn read_script_returns_powershell_text() {
        let body = read_script("query-ue-versions.ps1").unwrap();
        assert!(body.contains("HKLM:\\SOFTWARE\\EpicGames"));
    }

    #[cfg(not(windows))]
    #[test]
    fn detect_ue_versions_returns_powershell_error_on_non_windows() {
        let result = detect_ue_versions("RENDER-01");
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }

    #[cfg(not(windows))]
    #[test]
    fn detect_gpus_returns_powershell_error_on_non_windows() {
        let result = detect_gpus("RENDER-01");
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }
}
```

- [ ] **Step 2: Update `src-tauri/src/core/mod.rs`**

```rust
pub mod discovery;
pub mod network;
pub mod powershell;
pub mod winrm;
```

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test --lib core::discovery && cd ..
```

Expected on macOS: 3 tests pass. The `read_script_returns_powershell_text` test confirms the PS scripts from Task 7 are reachable from the test cwd (which is the package root, `src-tauri/`).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/core
git commit -m "feat(rust): add discovery module for UE + GPU detection over WinRM"
```

---

## Task 10: Tauri commands for discovery + Machine refresh

**Files:**
- Create: `src-tauri/src/commands/discovery.rs`
- Modify: `src-tauri/src/commands/machines.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write `src-tauri/src/commands/discovery.rs`**

```rust
//! Tauri commands for network scan + per-machine refresh.

use crate::core::{discovery, network, winrm};
use crate::data::{
    machine_gpus, machine_ue_installs, machines as data_machines, Db, GpuInfo, Machine, UeInstall,
};
use crate::error::{UecmError, UecmResult};
use serde::Serialize;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct ScanResult {
    pub probed: Vec<network::ProbedHost>,
}

#[tauri::command]
pub async fn scan_network(cidr: String) -> UecmResult<ScanResult> {
    let probed = network::scan_cidr(&cidr, network::DEFAULT_TIMEOUT_MS).await?;
    Ok(ScanResult { probed })
}

/// Adds a discovered IP as a Machine row (or no-op if already present).
/// hostname defaults to the IP — caller can rename later.
#[tauri::command]
pub fn add_discovered_machine(
    db: State<'_, Db>,
    ip: String,
    hostname: Option<String>,
) -> UecmResult<i64> {
    let display_name = hostname.unwrap_or_else(|| ip.clone());
    // If a machine with this IP already exists, return its id; else insert.
    let existing = data_machines::list_all(&db)?
        .into_iter()
        .find(|m| m.ip == ip);
    if let Some(m) = existing {
        return m
            .id
            .ok_or_else(|| UecmError::OperationFailed("machine missing id".to_string()));
    }
    let machine = Machine::new(&display_name, &ip);
    data_machines::insert(&db, &machine)
}

#[derive(Debug, Serialize)]
pub struct RefreshResult {
    pub machine_id: i64,
    pub winrm_ok: bool,
    pub ue_installs: Vec<UeInstall>,
    pub gpus: Vec<GpuInfo>,
    pub error: Option<String>,
}

/// Probes WinRM connectivity to a known machine, then re-queries UE + GPU
/// info if reachable, persisting results into the data layer.
#[tauri::command]
pub fn refresh_machine(db: State<'_, Db>, machine_id: i64) -> UecmResult<RefreshResult> {
    let machine = data_machines::list_all(&db)?
        .into_iter()
        .find(|m| m.id == Some(machine_id))
        .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", machine_id)))?;

    let probe = match winrm::probe(&machine.ip) {
        Ok(p) if p.ok => Some(p),
        Ok(_) => None,
        Err(e) => {
            return Ok(RefreshResult {
                machine_id,
                winrm_ok: false,
                ue_installs: vec![],
                gpus: vec![],
                error: Some(format!("probe failed: {}", e)),
            });
        }
    };

    if probe.is_none() {
        return Ok(RefreshResult {
            machine_id,
            winrm_ok: false,
            ue_installs: vec![],
            gpus: vec![],
            error: Some("WinRM unreachable".to_string()),
        });
    }

    let detected_ue = match discovery::detect_ue_versions(&machine.ip) {
        Ok(v) => v,
        Err(e) => {
            return Ok(RefreshResult {
                machine_id,
                winrm_ok: true,
                ue_installs: vec![],
                gpus: vec![],
                error: Some(format!("UE detection failed: {}", e)),
            });
        }
    };

    let detected_gpus = match discovery::detect_gpus(&machine.ip) {
        Ok(v) => v,
        Err(e) => {
            return Ok(RefreshResult {
                machine_id,
                winrm_ok: true,
                ue_installs: vec![],
                gpus: vec![],
                error: Some(format!("GPU detection failed: {}", e)),
            });
        }
    };

    // Persist UE installs (upsert per version)
    for d in &detected_ue {
        machine_ue_installs::upsert(
            &db,
            &UeInstall {
                id: None,
                machine_id,
                version: d.version.clone(),
                install_path: d.install_path.clone(),
                is_primary: false,
            },
        )?;
    }

    // Replace GPU set wholesale (GPUs change as a unit on hardware swap)
    let gpu_records: Vec<GpuInfo> = detected_gpus
        .iter()
        .map(|g| GpuInfo {
            id: None,
            machine_id,
            gpu_model: g.gpu_model.clone(),
            driver_version: g.driver_version.clone(),
            vendor: g.vendor.clone(),
            vram_mb: g.vram_mb,
        })
        .collect();
    machine_gpus::replace_for_machine(&db, machine_id, &gpu_records)?;

    Ok(RefreshResult {
        machine_id,
        winrm_ok: true,
        ue_installs: machine_ue_installs::list_for_machine(&db, machine_id)?,
        gpus: machine_gpus::list_for_machine(&db, machine_id)?,
        error: None,
    })
}
```

- [ ] **Step 2: Add a `get_machine_detail` command to `src-tauri/src/commands/machines.rs`**

Replace the file with:

```rust
//! Tauri command handlers for machine CRUD + detail lookup.

use crate::data::{
    machine_gpus, machine_ue_installs, machines as data_machines, Db, GpuInfo, Machine, UeInstall,
};
use crate::error::{UecmError, UecmResult};
use serde::Serialize;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct MachineDetail {
    pub machine: Machine,
    pub ue_installs: Vec<UeInstall>,
    pub gpus: Vec<GpuInfo>,
}

#[tauri::command]
pub fn list_machines(db: State<'_, Db>) -> UecmResult<Vec<Machine>> {
    data_machines::list_all(&db)
}

#[tauri::command]
pub fn add_machine(
    db: State<'_, Db>,
    hostname: String,
    ip: String,
) -> UecmResult<i64> {
    let machine = Machine::new(&hostname, &ip);
    data_machines::insert(&db, &machine)
}

#[tauri::command]
pub fn delete_machine(db: State<'_, Db>, id: i64) -> UecmResult<()> {
    data_machines::delete(&db, id)
}

#[tauri::command]
pub fn get_machine_detail(db: State<'_, Db>, id: i64) -> UecmResult<MachineDetail> {
    let machine = data_machines::list_all(&db)?
        .into_iter()
        .find(|m| m.id == Some(id))
        .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", id)))?;
    let ue_installs = machine_ue_installs::list_for_machine(&db, id)?;
    let gpus = machine_gpus::list_for_machine(&db, id)?;
    Ok(MachineDetail {
        machine,
        ue_installs,
        gpus,
    })
}
```

- [ ] **Step 3: Update `src-tauri/src/commands/mod.rs`**

```rust
pub mod discovery;
pub mod machines;
pub mod system;
```

- [ ] **Step 4: Register new commands in `src-tauri/src/lib.rs`**

In the `invoke_handler!` block, add the new commands so it reads:

```rust
.invoke_handler(tauri::generate_handler![
    commands::machines::list_machines,
    commands::machines::add_machine,
    commands::machines::delete_machine,
    commands::machines::get_machine_detail,
    commands::discovery::scan_network,
    commands::discovery::add_discovered_machine,
    commands::discovery::refresh_machine,
    commands::system::test_powershell_bridge,
])
```

- [ ] **Step 5: Build to verify**

```bash
cd src-tauri && cargo build && cd ..
```

Expected: succeeds.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands src-tauri/src/lib.rs
git commit -m "feat(rust): add Tauri commands for scan_network, refresh_machine, get_machine_detail"
```

---

## Task 11: Credential management — PowerShell + Rust + Tauri commands

**Files:**
- Create: `ps-scripts/cred-set.ps1`
- Create: `ps-scripts/cred-delete.ps1`
- Create: `ps-scripts/cred-list.ps1`
- Create: `src-tauri/src/core/credentials.rs`
- Create: `src-tauri/src/commands/credentials.rs`
- Modify: `src-tauri/src/core/mod.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write `ps-scripts/cred-set.ps1`**

```powershell
# Stores a generic credential in Windows Credential Manager via cmdkey.
# Parameters: -Alias <string> -Username <string> -Password <string>
# Output: JSON { ok: bool, message: string }

param(
    [Parameter(Mandatory=$true)] [string]$Alias,
    [Parameter(Mandatory=$true)] [string]$Username,
    [Parameter(Mandatory=$true)] [string]$Password
)

$ErrorActionPreference = 'Stop'

try {
    # Use Start-Process so the password isn't visible in `Get-History` / Process Explorer
    # cmdkey expects /pass:<value> with no quoting; we pass via argument list to avoid shell quoting.
    $p = Start-Process -FilePath 'cmdkey.exe' `
        -ArgumentList @("/generic:$Alias", "/user:$Username", "/pass:$Password") `
        -NoNewWindow -Wait -PassThru -RedirectStandardOutput 'NUL'
    if ($p.ExitCode -ne 0) {
        @{ ok = $false; message = "cmdkey exited $($p.ExitCode)" } | ConvertTo-Json -Compress
        exit 1
    }
    @{ ok = $true; message = "credential stored" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
```

- [ ] **Step 2: Write `ps-scripts/cred-delete.ps1`**

```powershell
# Deletes a stored credential by alias.
# Parameters: -Alias <string>
# Output: JSON { ok: bool, message: string }

param(
    [Parameter(Mandatory=$true)] [string]$Alias
)

$ErrorActionPreference = 'Stop'

try {
    $p = Start-Process -FilePath 'cmdkey.exe' `
        -ArgumentList @("/delete:$Alias") `
        -NoNewWindow -Wait -PassThru -RedirectStandardOutput 'NUL'
    if ($p.ExitCode -ne 0) {
        @{ ok = $false; message = "cmdkey exited $($p.ExitCode)" } | ConvertTo-Json -Compress
        exit 1
    }
    @{ ok = $true; message = "credential deleted" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
```

- [ ] **Step 3: Write `ps-scripts/cred-list.ps1`**

```powershell
# Lists all credentials matching prefix UECM:* (we never list system creds).
# Output: JSON array of { alias, target_type } — note: passwords are NOT exposed.

$ErrorActionPreference = 'SilentlyContinue'

$raw = & cmdkey.exe /list:UECM:*
# cmdkey output is human-readable, not JSON. Parse line-by-line for "Target: <alias>".
$results = @()
foreach ($line in $raw) {
    if ($line -match '^\s*Target:\s*(.+)$') {
        $alias = $Matches[1].Trim()
        $results += [PSCustomObject]@{ alias = $alias }
    }
}
ConvertTo-Json -InputObject @($results) -Compress
```

- [ ] **Step 4: Write `src-tauri/src/core/credentials.rs`**

```rust
//! Wraps the cred-{set,delete,list}.ps1 sidecar scripts. Stores the alias +
//! display username in SQLite (via `data::credentials`); the password lives
//! in Windows Credential Manager and is never read back into our process.

use crate::core::powershell;
use crate::error::{UecmError, UecmResult};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct CmdKeyResult {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct CmdKeyAlias {
    pub alias: String,
}

pub fn store(alias: &str, username: &str, password: &str) -> UecmResult<()> {
    let result: CmdKeyResult = powershell::run_json(
        &script_path("cred-set.ps1"),
        &[
            "-Alias", alias,
            "-Username", username,
            "-Password", password,
        ],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "cred-set failed: {}",
            result.message
        )));
    }
    Ok(())
}

pub fn delete(alias: &str) -> UecmResult<()> {
    let result: CmdKeyResult = powershell::run_json(
        &script_path("cred-delete.ps1"),
        &["-Alias", alias],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "cred-delete failed: {}",
            result.message
        )));
    }
    Ok(())
}

pub fn list_uecm_aliases() -> UecmResult<Vec<String>> {
    let result: Vec<CmdKeyAlias> = powershell::run_json(
        &script_path("cred-list.ps1"),
        &[],
    )?;
    Ok(result.into_iter().map(|c| c.alias).collect())
}

fn script_path(name: &str) -> PathBuf {
    Path::new("..").join("ps-scripts").join(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn store_returns_powershell_error_on_non_windows() {
        let result = store("UECM:winrm:HOST", "admin", "p@ss");
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }

    #[cfg(not(windows))]
    #[test]
    fn delete_returns_powershell_error_on_non_windows() {
        let result = delete("UECM:winrm:HOST");
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }

    #[cfg(not(windows))]
    #[test]
    fn list_returns_powershell_error_on_non_windows() {
        let result = list_uecm_aliases();
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }
}
```

- [ ] **Step 5: Write `src-tauri/src/commands/credentials.rs`**

```rust
//! Tauri commands for credential management. Combines the SQLite alias
//! record with the cmdkey side effect.

use crate::core::credentials as core_creds;
use crate::data::{credentials as data_creds, CredentialRecord, Db};
use crate::error::UecmResult;
use tauri::State;

#[tauri::command]
pub fn list_credentials(db: State<'_, Db>) -> UecmResult<Vec<CredentialRecord>> {
    data_creds::list_all(&db)
}

#[tauri::command]
pub fn save_credential(
    db: State<'_, Db>,
    alias: String,
    kind: String,
    username: String,
    password: String,
) -> UecmResult<i64> {
    // Write to Credential Manager first; if that fails, don't pollute SQLite.
    core_creds::store(&alias, &username, &password)?;
    let record = CredentialRecord {
        id: None,
        alias: alias.clone(),
        kind,
        username,
    };
    // If alias already exists, delete + re-insert for an effective upsert.
    if data_creds::find_by_alias(&db, &alias)?.is_some() {
        data_creds::delete_by_alias(&db, &alias)?;
    }
    data_creds::insert(&db, &record)
}

#[tauri::command]
pub fn delete_credential(db: State<'_, Db>, alias: String) -> UecmResult<()> {
    // Try Credential Manager first; if delete fails, still clean SQLite so the
    // UI doesn't display a phantom alias.
    let cm_result = core_creds::delete(&alias);
    data_creds::delete_by_alias(&db, &alias)?;
    cm_result
}
```

- [ ] **Step 6: Update mod.rs files + register commands**

`src-tauri/src/core/mod.rs`:

```rust
pub mod credentials;
pub mod discovery;
pub mod network;
pub mod powershell;
pub mod winrm;
```

`src-tauri/src/commands/mod.rs`:

```rust
pub mod credentials;
pub mod discovery;
pub mod machines;
pub mod system;
```

`src-tauri/src/lib.rs` `invoke_handler!`: add the three credential commands so the block reads:

```rust
.invoke_handler(tauri::generate_handler![
    commands::machines::list_machines,
    commands::machines::add_machine,
    commands::machines::delete_machine,
    commands::machines::get_machine_detail,
    commands::discovery::scan_network,
    commands::discovery::add_discovered_machine,
    commands::discovery::refresh_machine,
    commands::credentials::list_credentials,
    commands::credentials::save_credential,
    commands::credentials::delete_credential,
    commands::system::test_powershell_bridge,
])
```

- [ ] **Step 7: Build + run tests**

```bash
cd src-tauri && cargo build && cargo test --lib && cd ..
```

Expected: build succeeds. All Plan 1 + new tests pass (Plan 2 added: 3 schema + 4 ue_installs + 3 gpus + 6 credentials + 4 network + 3 discovery + 3 cred-core = 26 new ⇒ 14 + 26 = 40 backend tests total).

- [ ] **Step 8: Commit**

```bash
git add ps-scripts src-tauri/src
git commit -m "feat: add credential management (cmdkey via PowerShell + SQLite alias index)"
```

---

## Task 12: Single-machine env var configuration

**Files:**
- Create: `ps-scripts/setx-machine.ps1`
- Create: `ps-scripts/getx-machine.ps1`
- Create: `src-tauri/src/core/env_vars.rs`
- Create: `src-tauri/src/commands/env_vars.rs`
- Modify: `src-tauri/src/core/mod.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write `ps-scripts/setx-machine.ps1`**

```powershell
# Sets a system-level environment variable on a remote host via WinRM.
# Parameters: -HostName <string> -Name <string> -Value <string>
# Output: JSON { ok: bool, message: string }
# Uses [System.Environment]::SetEnvironmentVariable with "Machine" target.
# Requires the WinRM session user to be admin on the remote host.

param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$Name,
    [Parameter(Mandatory=$true)] [string]$Value
)

$ErrorActionPreference = 'Stop'

try {
    $script = {
        param($Name, $Value)
        [System.Environment]::SetEnvironmentVariable($Name, $Value, 'Machine')
        # Verify by reading back
        $readback = [System.Environment]::GetEnvironmentVariable($Name, 'Machine')
        if ($readback -ne $Value) {
            throw "verify failed: read '$readback', expected '$Value'"
        }
        return $true
    }
    Invoke-Command -ComputerName $HostName -ScriptBlock $script -ArgumentList $Name, $Value -ErrorAction Stop | Out-Null
    @{ ok = $true; message = "set $Name on $HostName" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
```

- [ ] **Step 2: Write `ps-scripts/getx-machine.ps1`**

```powershell
# Reads a system-level environment variable on a remote host via WinRM.
# Parameters: -HostName <string> -Name <string>
# Output: JSON { ok: bool, value: string|null, message: string }

param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$Name
)

$ErrorActionPreference = 'Stop'

try {
    $value = Invoke-Command -ComputerName $HostName -ScriptBlock {
        param($Name)
        [System.Environment]::GetEnvironmentVariable($Name, 'Machine')
    } -ArgumentList $Name -ErrorAction Stop
    @{ ok = $true; value = $value; message = "" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; value = $null; message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
```

- [ ] **Step 3: Write `src-tauri/src/core/env_vars.rs`**

```rust
//! Single-machine environment variable read/write via PowerShell sidecar.

use crate::core::powershell;
use crate::error::{UecmError, UecmResult};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct SetResult {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct GetResult {
    pub ok: bool,
    pub value: Option<String>,
    pub message: String,
}

pub fn set(host: &str, name: &str, value: &str) -> UecmResult<()> {
    let result: SetResult = powershell::run_json(
        &script_path("setx-machine.ps1"),
        &[
            "-HostName", host,
            "-Name", name,
            "-Value", value,
        ],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "set env var failed: {}",
            result.message
        )));
    }
    Ok(())
}

pub fn get(host: &str, name: &str) -> UecmResult<Option<String>> {
    let result: GetResult = powershell::run_json(
        &script_path("getx-machine.ps1"),
        &[
            "-HostName", host,
            "-Name", name,
        ],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "get env var failed: {}",
            result.message
        )));
    }
    Ok(result.value)
}

fn script_path(name: &str) -> PathBuf {
    Path::new("..").join("ps-scripts").join(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn set_returns_powershell_error_on_non_windows() {
        let result = set("RENDER-01", "UE-SharedDataCachePath", "\\\\HOST\\DDC");
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }

    #[cfg(not(windows))]
    #[test]
    fn get_returns_powershell_error_on_non_windows() {
        let result = get("RENDER-01", "UE-SharedDataCachePath");
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }
}
```

- [ ] **Step 4: Write `src-tauri/src/commands/env_vars.rs`**

```rust
//! Tauri commands for reading/writing remote env vars on a single machine.

use crate::core::env_vars;
use crate::data::{machines as data_machines, Db};
use crate::error::{UecmError, UecmResult};
use tauri::State;

fn ip_for(db: &Db, machine_id: i64) -> UecmResult<String> {
    let machine = data_machines::list_all(db)?
        .into_iter()
        .find(|m| m.id == Some(machine_id))
        .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", machine_id)))?;
    Ok(machine.ip)
}

#[tauri::command]
pub fn set_machine_env_var(
    db: State<'_, Db>,
    machine_id: i64,
    name: String,
    value: String,
) -> UecmResult<()> {
    let host = ip_for(&db, machine_id)?;
    env_vars::set(&host, &name, &value)
}

#[tauri::command]
pub fn get_machine_env_var(
    db: State<'_, Db>,
    machine_id: i64,
    name: String,
) -> UecmResult<Option<String>> {
    let host = ip_for(&db, machine_id)?;
    env_vars::get(&host, &name)
}
```

- [ ] **Step 5: Update mod.rs + register commands**

`src-tauri/src/core/mod.rs`:

```rust
pub mod credentials;
pub mod discovery;
pub mod env_vars;
pub mod network;
pub mod powershell;
pub mod winrm;
```

`src-tauri/src/commands/mod.rs`:

```rust
pub mod credentials;
pub mod discovery;
pub mod env_vars;
pub mod machines;
pub mod system;
```

`src-tauri/src/lib.rs` `invoke_handler!` block — add the two env_vars commands:

```rust
.invoke_handler(tauri::generate_handler![
    commands::machines::list_machines,
    commands::machines::add_machine,
    commands::machines::delete_machine,
    commands::machines::get_machine_detail,
    commands::discovery::scan_network,
    commands::discovery::add_discovered_machine,
    commands::discovery::refresh_machine,
    commands::credentials::list_credentials,
    commands::credentials::save_credential,
    commands::credentials::delete_credential,
    commands::env_vars::set_machine_env_var,
    commands::env_vars::get_machine_env_var,
    commands::system::test_powershell_bridge,
])
```

- [ ] **Step 6: Build + tests**

```bash
cd src-tauri && cargo build && cargo test --lib && cd ..
```

Expected: build OK, 42 backend tests pass (40 from Task 11 + 2 new env_vars).

- [ ] **Step 7: Commit**

```bash
git add ps-scripts src-tauri/src
git commit -m "feat: add single-machine env var get/set via remote setx"
```

---

## Task 13: Single-machine INI editing

**Files:**
- Create: `ps-scripts/read-ini-section.ps1`
- Create: `ps-scripts/write-ini-key.ps1`
- Create: `src-tauri/src/core/ini_editor.rs`
- Create: `src-tauri/src/commands/ini_editor.rs`
- Modify: `src-tauri/src/core/mod.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write `ps-scripts/read-ini-section.ps1`**

```powershell
# Reads a single [section] from an INI file on a remote host.
# Parameters: -HostName <string> -FilePath <string> -Section <string>
# Output: JSON { ok: bool, keys: [{ name, value }], message: string }

param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$FilePath,
    [Parameter(Mandatory=$true)] [string]$Section
)

$ErrorActionPreference = 'Stop'

try {
    $keys = Invoke-Command -ComputerName $HostName -ScriptBlock {
        param($FilePath, $Section)
        if (-not (Test-Path $FilePath)) {
            throw "file not found: $FilePath"
        }
        $lines = Get-Content -Path $FilePath -Encoding UTF8
        $inSection = $false
        $sectionPattern = "[$Section]"
        $result = @()
        foreach ($line in $lines) {
            $trim = $line.Trim()
            if ($trim -eq $sectionPattern) { $inSection = $true; continue }
            if ($inSection -and $trim.StartsWith('[') -and $trim.EndsWith(']')) { break }
            if ($inSection -and $trim -and -not $trim.StartsWith(';') -and -not $trim.StartsWith('#')) {
                $eq = $trim.IndexOf('=')
                if ($eq -gt 0) {
                    $name = $trim.Substring(0, $eq).Trim()
                    $value = $trim.Substring($eq + 1).Trim()
                    $result += [PSCustomObject]@{ name = $name; value = $value }
                }
            }
        }
        return ,$result
    } -ArgumentList $FilePath, $Section -ErrorAction Stop

    @{ ok = $true; keys = @($keys); message = "" } | ConvertTo-Json -Compress -Depth 4
}
catch {
    @{ ok = $false; keys = @(); message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
```

- [ ] **Step 2: Write `ps-scripts/write-ini-key.ps1`**

```powershell
# Sets a single key in an INI section on a remote host with auto-backup.
# Parameters: -HostName <string> -FilePath <string> -Section <string>
#             -Name <string> -Value <string>
# Output: JSON { ok: bool, backup_path: string, message: string }

param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$FilePath,
    [Parameter(Mandatory=$true)] [string]$Section,
    [Parameter(Mandatory=$true)] [string]$Name,
    [Parameter(Mandatory=$true)] [string]$Value
)

$ErrorActionPreference = 'Stop'

try {
    $remoteResult = Invoke-Command -ComputerName $HostName -ScriptBlock {
        param($FilePath, $Section, $Name, $Value)

        if (-not (Test-Path $FilePath)) {
            throw "file not found: $FilePath"
        }

        # Backup
        $ts = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
        $backup = "$FilePath.uecm-bak-$ts"
        Copy-Item -Path $FilePath -Destination $backup -Force

        $lines = Get-Content -Path $FilePath -Encoding UTF8
        $sectionPattern = "[$Section]"
        $inSection = $false
        $found = $false
        $newLines = New-Object System.Collections.Generic.List[string]
        $sectionIndex = -1
        $i = 0

        foreach ($line in $lines) {
            $trim = $line.Trim()
            if ($trim -eq $sectionPattern) {
                $inSection = $true
                $sectionIndex = $i
                $newLines.Add($line)
            }
            elseif ($inSection -and $trim.StartsWith('[') -and $trim.EndsWith(']')) {
                if (-not $found) {
                    # Insert key at the end of this section
                    $newLines.Add("$Name=$Value")
                    $found = $true
                }
                $inSection = $false
                $newLines.Add($line)
            }
            elseif ($inSection -and ($trim -match "^\s*$([regex]::Escape($Name))\s*=")) {
                $newLines.Add("$Name=$Value")
                $found = $true
            }
            else {
                $newLines.Add($line)
            }
            $i++
        }

        if ($inSection -and -not $found) {
            $newLines.Add("$Name=$Value")
            $found = $true
        }

        if (-not $found -and $sectionIndex -lt 0) {
            # Section did not exist; append section + key.
            $newLines.Add("")
            $newLines.Add("[$Section]")
            $newLines.Add("$Name=$Value")
        }

        Set-Content -Path $FilePath -Value $newLines -Encoding UTF8
        return $backup
    } -ArgumentList $FilePath, $Section, $Name, $Value -ErrorAction Stop

    @{ ok = $true; backup_path = $remoteResult; message = "" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; backup_path = ""; message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
```

- [ ] **Step 3: Write `src-tauri/src/core/ini_editor.rs`**

```rust
//! Single-machine INI section read + key write via PowerShell sidecar.

use crate::core::powershell;
use crate::error::{UecmError, UecmResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IniKey {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct ReadResult {
    pub ok: bool,
    pub keys: Vec<IniKey>,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct WriteResult {
    pub ok: bool,
    pub backup_path: String,
    pub message: String,
}

pub fn read_section(host: &str, file_path: &str, section: &str) -> UecmResult<Vec<IniKey>> {
    let result: ReadResult = powershell::run_json(
        &script_path("read-ini-section.ps1"),
        &[
            "-HostName", host,
            "-FilePath", file_path,
            "-Section", section,
        ],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "read INI failed: {}",
            result.message
        )));
    }
    Ok(result.keys)
}

pub fn set_key(
    host: &str,
    file_path: &str,
    section: &str,
    name: &str,
    value: &str,
) -> UecmResult<String> {
    let result: WriteResult = powershell::run_json(
        &script_path("write-ini-key.ps1"),
        &[
            "-HostName", host,
            "-FilePath", file_path,
            "-Section", section,
            "-Name", name,
            "-Value", value,
        ],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "write INI failed: {}",
            result.message
        )));
    }
    Ok(result.backup_path)
}

fn script_path(name: &str) -> PathBuf {
    Path::new("..").join("ps-scripts").join(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn read_section_returns_powershell_error_on_non_windows() {
        let result = read_section("RENDER-01", "C:\\proj\\Config\\DefaultEngine.ini", "Core.System");
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }

    #[cfg(not(windows))]
    #[test]
    fn set_key_returns_powershell_error_on_non_windows() {
        let result = set_key(
            "RENDER-01",
            "C:\\proj\\Config\\DefaultEngine.ini",
            "Core.System",
            "Paths",
            "../Content",
        );
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }
}
```

- [ ] **Step 4: Write `src-tauri/src/commands/ini_editor.rs`**

```rust
//! Tauri commands for reading + writing INI keys on a single remote machine.

use crate::core::ini_editor;
use crate::data::{machines as data_machines, Db};
use crate::error::{UecmError, UecmResult};
use serde::Serialize;
use tauri::State;

fn ip_for(db: &Db, machine_id: i64) -> UecmResult<String> {
    let m = data_machines::list_all(db)?
        .into_iter()
        .find(|m| m.id == Some(machine_id))
        .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", machine_id)))?;
    Ok(m.ip)
}

#[derive(Debug, Serialize)]
pub struct WriteIniResponse {
    pub backup_path: String,
}

#[tauri::command]
pub fn read_ini_section(
    db: State<'_, Db>,
    machine_id: i64,
    file_path: String,
    section: String,
) -> UecmResult<Vec<ini_editor::IniKey>> {
    let host = ip_for(&db, machine_id)?;
    ini_editor::read_section(&host, &file_path, &section)
}

#[tauri::command]
pub fn set_ini_key(
    db: State<'_, Db>,
    machine_id: i64,
    file_path: String,
    section: String,
    name: String,
    value: String,
) -> UecmResult<WriteIniResponse> {
    let host = ip_for(&db, machine_id)?;
    let backup_path = ini_editor::set_key(&host, &file_path, &section, &name, &value)?;
    Ok(WriteIniResponse { backup_path })
}
```

- [ ] **Step 5: Update mod.rs + register commands**

`src-tauri/src/core/mod.rs`:

```rust
pub mod credentials;
pub mod discovery;
pub mod env_vars;
pub mod ini_editor;
pub mod network;
pub mod powershell;
pub mod winrm;
```

`src-tauri/src/commands/mod.rs`:

```rust
pub mod credentials;
pub mod discovery;
pub mod env_vars;
pub mod ini_editor;
pub mod machines;
pub mod system;
```

`src-tauri/src/lib.rs` `invoke_handler!` — add the two ini_editor commands:

```rust
.invoke_handler(tauri::generate_handler![
    commands::machines::list_machines,
    commands::machines::add_machine,
    commands::machines::delete_machine,
    commands::machines::get_machine_detail,
    commands::discovery::scan_network,
    commands::discovery::add_discovered_machine,
    commands::discovery::refresh_machine,
    commands::credentials::list_credentials,
    commands::credentials::save_credential,
    commands::credentials::delete_credential,
    commands::env_vars::set_machine_env_var,
    commands::env_vars::get_machine_env_var,
    commands::ini_editor::read_ini_section,
    commands::ini_editor::set_ini_key,
    commands::system::test_powershell_bridge,
])
```

- [ ] **Step 6: Build + tests**

```bash
cd src-tauri && cargo build && cargo test --lib && cd ..
```

Expected: 44 backend tests pass.

- [ ] **Step 7: Commit**

```bash
git add ps-scripts src-tauri/src
git commit -m "feat: add single-machine INI section read + key set with auto-backup"
```

---

## Task 14: Frontend service wrapper + Pinia stores extension

**Files:**
- Modify: `src/services/tauri.ts`
- Modify: `src/stores/machines.ts`
- Create: `src/stores/discovery.ts`
- Create: `src/stores/credentials.ts`
- Create: `src/__tests__/discovery-store.spec.ts`
- Create: `src/__tests__/credentials-store.spec.ts`
- Modify: `src/__tests__/machines-store.spec.ts`

- [ ] **Step 1: Extend `src/services/tauri.ts`**

Replace the file with:

```typescript
import { invoke } from "@tauri-apps/api/core";

export interface Machine {
  id: number | null;
  hostname: string;
  ip: string;
  role: string;
  status: string;
  last_seen_at: string | null;
}

export interface UeInstall {
  id: number | null;
  machine_id: number;
  version: string;
  install_path: string;
  is_primary: boolean;
}

export interface GpuInfo {
  id: number | null;
  machine_id: number;
  gpu_model: string;
  driver_version: string;
  vendor: string;
  vram_mb: number | null;
}

export interface MachineDetail {
  machine: Machine;
  ue_installs: UeInstall[];
  gpus: GpuInfo[];
}

export interface ProbedHost {
  ip: string;
  winrm_open: boolean;
  smb_open: boolean;
}

export interface ScanResult {
  probed: ProbedHost[];
}

export interface RefreshResult {
  machine_id: number;
  winrm_ok: boolean;
  ue_installs: UeInstall[];
  gpus: GpuInfo[];
  error: string | null;
}

export interface CredentialRecord {
  id: number | null;
  alias: string;
  kind: string;       // "winrm" | "share"
  username: string;
}

export interface IniKey {
  name: string;
  value: string;
}

export interface WriteIniResponse {
  backup_path: string;
}

export interface EchoResult {
  received: string;
  timestamp: string;
  machine: string;
}

export interface UecmError {
  code: string;
  message: string;
}

export const tauriApi = {
  // Machines
  async listMachines(): Promise<Machine[]> {
    return invoke<Machine[]>("list_machines");
  },
  async addMachine(hostname: string, ip: string): Promise<number> {
    return invoke<number>("add_machine", { hostname, ip });
  },
  async deleteMachine(id: number): Promise<void> {
    return invoke<void>("delete_machine", { id });
  },
  async getMachineDetail(id: number): Promise<MachineDetail> {
    return invoke<MachineDetail>("get_machine_detail", { id });
  },

  // Discovery
  async scanNetwork(cidr: string): Promise<ScanResult> {
    return invoke<ScanResult>("scan_network", { cidr });
  },
  async addDiscoveredMachine(ip: string, hostname: string | null): Promise<number> {
    return invoke<number>("add_discovered_machine", { ip, hostname });
  },
  async refreshMachine(machineId: number): Promise<RefreshResult> {
    return invoke<RefreshResult>("refresh_machine", { machineId });
  },

  // Credentials
  async listCredentials(): Promise<CredentialRecord[]> {
    return invoke<CredentialRecord[]>("list_credentials");
  },
  async saveCredential(
    alias: string,
    kind: string,
    username: string,
    password: string,
  ): Promise<number> {
    return invoke<number>("save_credential", { alias, kind, username, password });
  },
  async deleteCredential(alias: string): Promise<void> {
    return invoke<void>("delete_credential", { alias });
  },

  // Env vars
  async setMachineEnvVar(machineId: number, name: string, value: string): Promise<void> {
    return invoke<void>("set_machine_env_var", { machineId, name, value });
  },
  async getMachineEnvVar(machineId: number, name: string): Promise<string | null> {
    return invoke<string | null>("get_machine_env_var", { machineId, name });
  },

  // INI editor
  async readIniSection(
    machineId: number,
    filePath: string,
    section: string,
  ): Promise<IniKey[]> {
    return invoke<IniKey[]>("read_ini_section", { machineId, filePath, section });
  },
  async setIniKey(
    machineId: number,
    filePath: string,
    section: string,
    name: string,
    value: string,
  ): Promise<WriteIniResponse> {
    return invoke<WriteIniResponse>("set_ini_key", {
      machineId,
      filePath,
      section,
      name,
      value,
    });
  },

  // System (Plan 1)
  async testPowerShellBridge(message: string): Promise<EchoResult> {
    return invoke<EchoResult>("test_powershell_bridge", { message });
  },
};
```

- [ ] **Step 2: Extend `src/stores/machines.ts`**

Replace with:

```typescript
import { defineStore } from "pinia";
import { ref } from "vue";
import {
  tauriApi,
  type Machine,
  type MachineDetail,
  type RefreshResult,
  type UecmError,
} from "@/services/tauri";

export const useMachinesStore = defineStore("machines", () => {
  const machines = ref<Machine[]>([]);
  const isLoading = ref(false);
  const error = ref<UecmError | null>(null);

  const selectedDetail = ref<MachineDetail | null>(null);
  const isDetailLoading = ref(false);

  const lastRefresh = ref<RefreshResult | null>(null);
  const isRefreshing = ref(false);

  async function loadMachines() {
    isLoading.value = true;
    error.value = null;
    try {
      machines.value = await tauriApi.listMachines();
    } catch (e) {
      error.value = e as UecmError;
    } finally {
      isLoading.value = false;
    }
  }

  async function addMachine(hostname: string, ip: string) {
    error.value = null;
    try {
      await tauriApi.addMachine(hostname, ip);
      await loadMachines();
    } catch (e) {
      error.value = e as UecmError;
    }
  }

  async function deleteMachine(id: number) {
    error.value = null;
    try {
      await tauriApi.deleteMachine(id);
      if (selectedDetail.value?.machine.id === id) {
        selectedDetail.value = null;
      }
      await loadMachines();
    } catch (e) {
      error.value = e as UecmError;
    }
  }

  async function selectMachine(id: number) {
    isDetailLoading.value = true;
    error.value = null;
    try {
      selectedDetail.value = await tauriApi.getMachineDetail(id);
    } catch (e) {
      error.value = e as UecmError;
      selectedDetail.value = null;
    } finally {
      isDetailLoading.value = false;
    }
  }

  function clearSelection() {
    selectedDetail.value = null;
  }

  async function refreshSelected() {
    if (!selectedDetail.value?.machine.id) return;
    const id = selectedDetail.value.machine.id;
    isRefreshing.value = true;
    error.value = null;
    try {
      lastRefresh.value = await tauriApi.refreshMachine(id);
      // Re-read detail to pick up updated UE/GPU rows
      await selectMachine(id);
    } catch (e) {
      error.value = e as UecmError;
    } finally {
      isRefreshing.value = false;
    }
  }

  return {
    machines,
    isLoading,
    error,
    selectedDetail,
    isDetailLoading,
    lastRefresh,
    isRefreshing,
    loadMachines,
    addMachine,
    deleteMachine,
    selectMachine,
    clearSelection,
    refreshSelected,
  };
});
```

- [ ] **Step 3: Create `src/stores/discovery.ts`**

```typescript
import { defineStore } from "pinia";
import { ref } from "vue";
import { tauriApi, type ProbedHost, type UecmError } from "@/services/tauri";

export const useDiscoveryStore = defineStore("discovery", () => {
  const cidr = ref("192.168.10.0/24");
  const probed = ref<ProbedHost[]>([]);
  const isScanning = ref(false);
  const error = ref<UecmError | null>(null);

  async function scan(input?: string) {
    if (input) cidr.value = input;
    isScanning.value = true;
    error.value = null;
    probed.value = [];
    try {
      const result = await tauriApi.scanNetwork(cidr.value);
      probed.value = result.probed;
    } catch (e) {
      error.value = e as UecmError;
    } finally {
      isScanning.value = false;
    }
  }

  async function addToInventory(ip: string, hostname: string | null) {
    error.value = null;
    try {
      return await tauriApi.addDiscoveredMachine(ip, hostname);
    } catch (e) {
      error.value = e as UecmError;
      throw e;
    }
  }

  return {
    cidr,
    probed,
    isScanning,
    error,
    scan,
    addToInventory,
  };
});
```

- [ ] **Step 4: Create `src/stores/credentials.ts`**

```typescript
import { defineStore } from "pinia";
import { ref } from "vue";
import { tauriApi, type CredentialRecord, type UecmError } from "@/services/tauri";

export const useCredentialsStore = defineStore("credentials", () => {
  const credentials = ref<CredentialRecord[]>([]);
  const isLoading = ref(false);
  const error = ref<UecmError | null>(null);

  async function load() {
    isLoading.value = true;
    error.value = null;
    try {
      credentials.value = await tauriApi.listCredentials();
    } catch (e) {
      error.value = e as UecmError;
    } finally {
      isLoading.value = false;
    }
  }

  async function save(alias: string, kind: string, username: string, password: string) {
    error.value = null;
    try {
      await tauriApi.saveCredential(alias, kind, username, password);
      await load();
    } catch (e) {
      error.value = e as UecmError;
      throw e;
    }
  }

  async function remove(alias: string) {
    error.value = null;
    try {
      await tauriApi.deleteCredential(alias);
      await load();
    } catch (e) {
      error.value = e as UecmError;
      throw e;
    }
  }

  return {
    credentials,
    isLoading,
    error,
    load,
    save,
    remove,
  };
});
```

- [ ] **Step 5: Create `src/__tests__/discovery-store.spec.ts`**

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    scanNetwork: vi.fn(),
    addDiscoveredMachine: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));

import { useDiscoveryStore } from "@/stores/discovery";

describe("discovery store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mockApi.scanNetwork.mockReset();
    mockApi.addDiscoveredMachine.mockReset();
  });

  it("starts with default CIDR and empty probed list", () => {
    const store = useDiscoveryStore();
    expect(store.cidr).toBe("192.168.10.0/24");
    expect(store.probed).toEqual([]);
    expect(store.isScanning).toBe(false);
  });

  it("scan populates probed list", async () => {
    mockApi.scanNetwork.mockResolvedValue({
      probed: [
        { ip: "192.168.10.21", winrm_open: true, smb_open: true },
      ],
    });
    const store = useDiscoveryStore();
    await store.scan();
    expect(store.probed).toHaveLength(1);
    expect(store.probed[0].ip).toBe("192.168.10.21");
  });

  it("scan toggles isScanning", async () => {
    mockApi.scanNetwork.mockImplementation(
      () => new Promise((r) => setTimeout(() => r({ probed: [] }), 10)),
    );
    const store = useDiscoveryStore();
    const p = store.scan();
    expect(store.isScanning).toBe(true);
    await p;
    expect(store.isScanning).toBe(false);
  });

  it("scan with explicit input updates CIDR", async () => {
    mockApi.scanNetwork.mockResolvedValue({ probed: [] });
    const store = useDiscoveryStore();
    await store.scan("10.0.0.0/24");
    expect(store.cidr).toBe("10.0.0.0/24");
  });

  it("addToInventory delegates to api", async () => {
    mockApi.addDiscoveredMachine.mockResolvedValue(7);
    const store = useDiscoveryStore();
    const id = await store.addToInventory("192.168.10.99", null);
    expect(mockApi.addDiscoveredMachine).toHaveBeenCalledWith("192.168.10.99", null);
    expect(id).toBe(7);
  });

  it("captures errors during scan", async () => {
    mockApi.scanNetwork.mockRejectedValue({ code: "INVALID_INPUT", message: "bad cidr" });
    const store = useDiscoveryStore();
    await store.scan();
    expect(store.error).toEqual({ code: "INVALID_INPUT", message: "bad cidr" });
  });
});
```

- [ ] **Step 6: Create `src/__tests__/credentials-store.spec.ts`**

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    listCredentials: vi.fn(),
    saveCredential: vi.fn(),
    deleteCredential: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));

import { useCredentialsStore } from "@/stores/credentials";

describe("credentials store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mockApi.listCredentials.mockReset();
    mockApi.saveCredential.mockReset();
    mockApi.deleteCredential.mockReset();
  });

  it("starts empty", () => {
    const store = useCredentialsStore();
    expect(store.credentials).toEqual([]);
  });

  it("load populates list", async () => {
    mockApi.listCredentials.mockResolvedValue([
      { id: 1, alias: "UECM:winrm:H1", kind: "winrm", username: "admin" },
    ]);
    const store = useCredentialsStore();
    await store.load();
    expect(store.credentials).toHaveLength(1);
  });

  it("save calls api then reloads", async () => {
    mockApi.saveCredential.mockResolvedValue(1);
    mockApi.listCredentials.mockResolvedValue([
      { id: 1, alias: "UECM:winrm:H1", kind: "winrm", username: "admin" },
    ]);
    const store = useCredentialsStore();
    await store.save("UECM:winrm:H1", "winrm", "admin", "p");
    expect(mockApi.saveCredential).toHaveBeenCalledWith("UECM:winrm:H1", "winrm", "admin", "p");
    expect(store.credentials).toHaveLength(1);
  });

  it("remove calls api then reloads", async () => {
    mockApi.deleteCredential.mockResolvedValue(undefined);
    mockApi.listCredentials.mockResolvedValue([]);
    const store = useCredentialsStore();
    await store.remove("UECM:winrm:H1");
    expect(mockApi.deleteCredential).toHaveBeenCalledWith("UECM:winrm:H1");
    expect(store.credentials).toEqual([]);
  });
});
```

- [ ] **Step 7: Update `src/__tests__/machines-store.spec.ts` — append two new tests**

Add the following to the existing `mockApi` object inside `vi.hoisted`:

```typescript
getMachineDetail: vi.fn(),
refreshMachine: vi.fn(),
```

The full updated `vi.hoisted` block becomes:

```typescript
const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    listMachines: vi.fn(),
    addMachine: vi.fn(),
    deleteMachine: vi.fn(),
    getMachineDetail: vi.fn(),
    refreshMachine: vi.fn(),
  },
}));
```

In `beforeEach`, also reset the new mocks:

```typescript
mockApi.getMachineDetail.mockReset();
mockApi.refreshMachine.mockReset();
```

Then append two `it()` blocks at the end of the existing `describe("machines store", ...)`:

```typescript
it("selectMachine populates selectedDetail", async () => {
  mockApi.getMachineDetail.mockResolvedValue({
    machine: { id: 5, hostname: "X", ip: "1.1.1.1", role: "render", status: "online", last_seen_at: null },
    ue_installs: [],
    gpus: [],
  });
  const store = useMachinesStore();
  await store.selectMachine(5);
  expect(store.selectedDetail?.machine.hostname).toBe("X");
});

it("refreshSelected re-reads detail after refresh", async () => {
  mockApi.getMachineDetail.mockResolvedValueOnce({
    machine: { id: 5, hostname: "X", ip: "1.1.1.1", role: "render", status: "online", last_seen_at: null },
    ue_installs: [],
    gpus: [],
  });
  mockApi.refreshMachine.mockResolvedValue({
    machine_id: 5,
    winrm_ok: true,
    ue_installs: [],
    gpus: [],
    error: null,
  });
  mockApi.getMachineDetail.mockResolvedValueOnce({
    machine: { id: 5, hostname: "X", ip: "1.1.1.1", role: "render", status: "online", last_seen_at: null },
    ue_installs: [{ id: 1, machine_id: 5, version: "5.4", install_path: "C:\\UE_5.4", is_primary: false }],
    gpus: [],
  });
  const store = useMachinesStore();
  await store.selectMachine(5);
  await store.refreshSelected();
  expect(mockApi.refreshMachine).toHaveBeenCalledWith(5);
  expect(store.selectedDetail?.ue_installs).toHaveLength(1);
});
```

- [ ] **Step 8: Run tests**

```bash
pnpm test
```

Expected: 20 (existing) + 6 (discovery store) + 4 (credentials store) + 2 (new machines store tests) = 32 passing.

- [ ] **Step 9: Commit**

```bash
git add src/services/tauri.ts src/stores src/__tests__
git commit -m "feat(frontend): extend tauri service + add discovery and credentials stores"
```

---

## Task 15: BaseModal component (reusable shell)

**Files:**
- Create: `src/components/modals/BaseModal.vue`
- Create: `src/__tests__/BaseModal.spec.ts`

A small reusable modal so the four feature modals (Discovery, Credential, EnvVar, IniEdit) share consistent behavior (escape-to-close, backdrop click, focus trap is out of scope for v1).

- [ ] **Step 1: Write `src/__tests__/BaseModal.spec.ts`**

```typescript
import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import BaseModal from "@/components/modals/BaseModal.vue";

describe("BaseModal", () => {
  it("renders title slot content", () => {
    const wrapper = mount(BaseModal, {
      props: { open: true, title: "Test Title" },
    });
    expect(wrapper.text()).toContain("Test Title");
  });

  it("renders default slot content", () => {
    const wrapper = mount(BaseModal, {
      props: { open: true, title: "T" },
      slots: { default: "<p>body content</p>" },
    });
    expect(wrapper.html()).toContain("body content");
  });

  it("does not render anything when open=false", () => {
    const wrapper = mount(BaseModal, {
      props: { open: false, title: "T" },
    });
    expect(wrapper.find("[data-modal]").exists()).toBe(false);
  });

  it("emits close on backdrop click", async () => {
    const wrapper = mount(BaseModal, {
      props: { open: true, title: "T" },
    });
    await wrapper.find("[data-modal-backdrop]").trigger("click");
    expect(wrapper.emitted("close")).toBeTruthy();
  });

  it("emits close on close-button click", async () => {
    const wrapper = mount(BaseModal, {
      props: { open: true, title: "T" },
    });
    await wrapper.find("[data-modal-close]").trigger("click");
    expect(wrapper.emitted("close")).toBeTruthy();
  });

  it("does NOT emit close when clicking inside the panel", async () => {
    const wrapper = mount(BaseModal, {
      props: { open: true, title: "T" },
      slots: { default: '<div data-test-inner>inner</div>' },
    });
    await wrapper.find("[data-test-inner]").trigger("click");
    expect(wrapper.emitted("close")).toBeFalsy();
  });
});
```

- [ ] **Step 2: Run test to confirm it fails**

```bash
pnpm test
```

Expected: FAIL — `Cannot find module '@/components/modals/BaseModal.vue'`.

- [ ] **Step 3: Write `src/components/modals/BaseModal.vue`**

```vue
<script setup lang="ts">
defineProps<{
  open: boolean;
  title: string;
}>();

const emit = defineEmits<{
  (e: "close"): void;
}>();

function onBackdrop() {
  emit("close");
}

function onCloseClick() {
  emit("close");
}

function stopBubble(e: Event) {
  e.stopPropagation();
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open"
      data-modal
      class="fixed inset-0 z-50 flex items-center justify-center"
    >
      <div
        data-modal-backdrop
        class="absolute inset-0 bg-black/40"
        @click="onBackdrop"
      ></div>
      <div
        class="relative bg-white rounded shadow-lg w-[480px] max-w-full"
        @click="stopBubble"
      >
        <header class="flex items-center justify-between border-b px-4 py-3">
          <h2 class="font-medium">{{ title }}</h2>
          <button
            data-modal-close
            class="text-gray-500 hover:text-gray-900 text-lg leading-none"
            @click="onCloseClick"
          >
            ×
          </button>
        </header>
        <div class="p-4">
          <slot></slot>
        </div>
        <footer v-if="$slots.footer" class="flex justify-end gap-2 border-t px-4 py-3">
          <slot name="footer"></slot>
        </footer>
      </div>
    </div>
  </Teleport>
</template>
```

- [ ] **Step 4: Run tests**

```bash
pnpm test
```

Expected: PASS — 6 new tests.

- [ ] **Step 5: Commit**

```bash
git add src/components/modals/BaseModal.vue src/__tests__/BaseModal.spec.ts
git commit -m "feat(frontend): add reusable BaseModal component with backdrop close"
```

---

## Task 16: MachineDetail component + Machines view split layout

**Files:**
- Create: `src/components/machines/MachineDetail.vue`
- Create: `src/__tests__/MachineDetail.spec.ts`
- Modify: `src/views/Machines.vue`
- Modify: `src/__tests__/Machines-view.spec.ts`

- [ ] **Step 1: Write `src/__tests__/MachineDetail.spec.ts`**

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    getMachineDetail: vi.fn(),
    refreshMachine: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));

import MachineDetail from "@/components/machines/MachineDetail.vue";
import { useMachinesStore } from "@/stores/machines";

describe("MachineDetail", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mockApi.getMachineDetail.mockReset();
    mockApi.refreshMachine.mockReset();
  });

  it("shows empty state when no detail selected", () => {
    const wrapper = mount(MachineDetail);
    expect(wrapper.text()).toContain("Select a machine");
  });

  it("renders hostname/ip from selectedDetail", async () => {
    mockApi.getMachineDetail.mockResolvedValue({
      machine: {
        id: 1,
        hostname: "RENDER-01",
        ip: "192.168.10.21",
        role: "render",
        status: "online",
        last_seen_at: null,
      },
      ue_installs: [],
      gpus: [],
    });
    const store = useMachinesStore();
    await store.selectMachine(1);
    const wrapper = mount(MachineDetail);
    await flushPromises();
    expect(wrapper.text()).toContain("RENDER-01");
    expect(wrapper.text()).toContain("192.168.10.21");
  });

  it("renders UE installs and GPUs when present", async () => {
    mockApi.getMachineDetail.mockResolvedValue({
      machine: {
        id: 1,
        hostname: "RENDER-01",
        ip: "192.168.10.21",
        role: "render",
        status: "online",
        last_seen_at: null,
      },
      ue_installs: [
        { id: 1, machine_id: 1, version: "5.4", install_path: "C:\\UE_5.4", is_primary: true },
      ],
      gpus: [
        { id: 1, machine_id: 1, gpu_model: "RTX 4090", driver_version: "551.86", vendor: "nvidia", vram_mb: 24576 },
      ],
    });
    const store = useMachinesStore();
    await store.selectMachine(1);
    const wrapper = mount(MachineDetail);
    await flushPromises();
    expect(wrapper.text()).toContain("5.4");
    expect(wrapper.text()).toContain("C:\\UE_5.4");
    expect(wrapper.text()).toContain("RTX 4090");
    expect(wrapper.text()).toContain("551.86");
  });

  it("clicking refresh button calls store.refreshSelected", async () => {
    mockApi.getMachineDetail.mockResolvedValue({
      machine: {
        id: 1,
        hostname: "RENDER-01",
        ip: "192.168.10.21",
        role: "render",
        status: "online",
        last_seen_at: null,
      },
      ue_installs: [],
      gpus: [],
    });
    mockApi.refreshMachine.mockResolvedValue({
      machine_id: 1,
      winrm_ok: true,
      ue_installs: [],
      gpus: [],
      error: null,
    });
    const store = useMachinesStore();
    await store.selectMachine(1);
    const wrapper = mount(MachineDetail);
    await flushPromises();
    await wrapper.find("[data-refresh-btn]").trigger("click");
    await flushPromises();
    expect(mockApi.refreshMachine).toHaveBeenCalledWith(1);
  });
});
```

- [ ] **Step 2: Write `src/components/machines/MachineDetail.vue`**

```vue
<script setup lang="ts">
import { useMachinesStore } from "@/stores/machines";

const store = useMachinesStore();

const emit = defineEmits<{
  (e: "openEnvVarModal"): void;
  (e: "openIniEditModal"): void;
  (e: "openCredentialModal"): void;
}>();
</script>

<template>
  <div class="h-full flex flex-col">
    <div v-if="!store.selectedDetail" class="p-6 text-sm text-gray-500">
      Select a machine from the list to view details.
    </div>

    <div v-else class="p-6 overflow-auto">
      <header class="flex items-start justify-between mb-4">
        <div>
          <h2 class="text-xl font-semibold">{{ store.selectedDetail.machine.hostname }}</h2>
          <p class="text-sm text-gray-500">{{ store.selectedDetail.machine.ip }}</p>
        </div>
        <div class="flex gap-2">
          <button
            data-refresh-btn
            :disabled="store.isRefreshing"
            class="px-3 py-1 text-sm border rounded hover:bg-gray-100 disabled:opacity-50"
            @click="store.refreshSelected()"
          >
            {{ store.isRefreshing ? "Refreshing..." : "Refresh" }}
          </button>
          <button
            class="px-3 py-1 text-sm border rounded hover:bg-gray-100"
            @click="emit('openCredentialModal')"
          >
            Credentials
          </button>
          <button
            class="px-3 py-1 text-sm border rounded hover:bg-gray-100"
            @click="emit('openEnvVarModal')"
          >
            Env vars
          </button>
          <button
            class="px-3 py-1 text-sm border rounded hover:bg-gray-100"
            @click="emit('openIniEditModal')"
          >
            Edit INI
          </button>
        </div>
      </header>

      <section class="mt-4">
        <h3 class="font-medium mb-2">Basics</h3>
        <table class="text-sm w-full">
          <tbody>
            <tr><td class="py-1 text-gray-500 w-32">Role</td><td>{{ store.selectedDetail.machine.role }}</td></tr>
            <tr><td class="py-1 text-gray-500">Status</td><td>{{ store.selectedDetail.machine.status }}</td></tr>
            <tr><td class="py-1 text-gray-500">Last seen</td><td>{{ store.selectedDetail.machine.last_seen_at ?? "—" }}</td></tr>
          </tbody>
        </table>
      </section>

      <section class="mt-6">
        <h3 class="font-medium mb-2">UE installs</h3>
        <p v-if="store.selectedDetail.ue_installs.length === 0" class="text-sm text-gray-500">
          No UE installs detected. Click Refresh to scan.
        </p>
        <table v-else class="text-sm w-full border">
          <thead class="bg-gray-50">
            <tr>
              <th class="text-left px-3 py-1">Version</th>
              <th class="text-left px-3 py-1">Install path</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="install in store.selectedDetail.ue_installs"
              :key="install.id ?? install.version"
              class="border-t"
            >
              <td class="px-3 py-1">{{ install.version }}</td>
              <td class="px-3 py-1 font-mono text-xs">{{ install.install_path }}</td>
            </tr>
          </tbody>
        </table>
      </section>

      <section class="mt-6">
        <h3 class="font-medium mb-2">GPUs</h3>
        <p v-if="store.selectedDetail.gpus.length === 0" class="text-sm text-gray-500">
          No GPU info. Click Refresh to scan.
        </p>
        <table v-else class="text-sm w-full border">
          <thead class="bg-gray-50">
            <tr>
              <th class="text-left px-3 py-1">Model</th>
              <th class="text-left px-3 py-1">Driver</th>
              <th class="text-left px-3 py-1">VRAM</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="gpu in store.selectedDetail.gpus"
              :key="gpu.id ?? gpu.gpu_model"
              class="border-t"
            >
              <td class="px-3 py-1">{{ gpu.gpu_model }}</td>
              <td class="px-3 py-1">{{ gpu.driver_version }}</td>
              <td class="px-3 py-1">{{ gpu.vram_mb ? gpu.vram_mb + " MB" : "—" }}</td>
            </tr>
          </tbody>
        </table>
      </section>

      <section v-if="store.lastRefresh && !store.lastRefresh.winrm_ok" class="mt-4 text-sm text-red-600">
        Refresh failed: {{ store.lastRefresh.error ?? "WinRM unreachable" }}
      </section>
    </div>
  </div>
</template>
```

- [ ] **Step 3: Modify `src/views/Machines.vue` — split layout (list left, detail right)**

```vue
<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useMachinesStore } from "@/stores/machines";
import MachineDetail from "@/components/machines/MachineDetail.vue";
import DiscoveryWizard from "@/components/modals/DiscoveryWizard.vue";

const store = useMachinesStore();
const showDiscovery = ref(false);

onMounted(() => {
  store.loadMachines();
});

async function onSelect(id: number | null) {
  if (id === null) return;
  await store.selectMachine(id);
}

async function onDelete(id: number | null) {
  if (id === null) return;
  await store.deleteMachine(id);
}
</script>

<template>
  <div class="h-full flex">
    <aside class="w-80 border-r overflow-auto p-4">
      <header class="flex items-center justify-between mb-3">
        <h1 class="text-lg font-semibold">Machines</h1>
        <button
          data-discover-btn
          class="px-3 py-1 text-sm bg-gray-200 rounded hover:bg-gray-300"
          @click="showDiscovery = true"
        >
          Scan
        </button>
      </header>

      <p v-if="store.isLoading" class="text-sm text-gray-500">Loading...</p>
      <p v-else-if="store.machines.length === 0" class="text-sm text-gray-500">
        No machines yet. Click Scan to discover.
      </p>
      <ul v-else class="space-y-1">
        <li
          v-for="m in store.machines"
          :key="m.id ?? m.ip"
          data-machine-row
          class="px-2 py-2 rounded cursor-pointer hover:bg-gray-100 flex items-center justify-between"
          :class="store.selectedDetail?.machine.id === m.id ? 'bg-gray-200 font-medium' : ''"
          @click="onSelect(m.id)"
        >
          <span class="truncate">
            {{ m.hostname }}<br />
            <span class="text-xs text-gray-500 font-normal">{{ m.ip }}</span>
          </span>
          <button
            class="text-xs text-red-600 hover:underline ml-2"
            @click.stop="onDelete(m.id)"
          >
            ×
          </button>
        </li>
      </ul>

      <p v-if="store.error" class="mt-3 text-xs text-red-600">
        {{ store.error.message }}
      </p>
    </aside>

    <main class="flex-1 overflow-auto">
      <MachineDetail />
    </main>

    <DiscoveryWizard
      :open="showDiscovery"
      @close="showDiscovery = false"
    />
  </div>
</template>
```

(NOTE: `DiscoveryWizard.vue` is created in Task 17. The Machines view will fail to type-check temporarily until then — that's expected. Tests run after Task 17 will pass.)

- [ ] **Step 4: Update `src/__tests__/Machines-view.spec.ts`**

The previous Machines view tests added a row + form. The new view replaces the form with a "Scan" button + split layout. Replace the file:

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    listMachines: vi.fn(),
    addMachine: vi.fn(),
    deleteMachine: vi.fn(),
    getMachineDetail: vi.fn(),
    refreshMachine: vi.fn(),
    scanNetwork: vi.fn(),
    addDiscoveredMachine: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));

import Machines from "@/views/Machines.vue";

describe("Machines view", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((m: any) => m.mockReset());
  });

  it("shows empty state when no machines", async () => {
    mockApi.listMachines.mockResolvedValue([]);
    const wrapper = mount(Machines);
    await flushPromises();
    expect(wrapper.text()).toContain("No machines");
  });

  it("renders a row for each machine", async () => {
    mockApi.listMachines.mockResolvedValue([
      { id: 1, hostname: "RENDER-01", ip: "192.168.10.21", role: "render", status: "online", last_seen_at: null },
      { id: 2, hostname: "HOST-NAS", ip: "192.168.10.2", role: "host", status: "online", last_seen_at: null },
    ]);
    const wrapper = mount(Machines);
    await flushPromises();
    const rows = wrapper.findAll("[data-machine-row]");
    expect(rows).toHaveLength(2);
    expect(wrapper.text()).toContain("RENDER-01");
  });

  it("clicking Scan button reveals discovery wizard", async () => {
    mockApi.listMachines.mockResolvedValue([]);
    const wrapper = mount(Machines);
    await flushPromises();
    await wrapper.find("[data-discover-btn]").trigger("click");
    // DiscoveryWizard renders via Teleport; check `document.body` for the modal
    expect(document.body.innerHTML).toContain("data-modal");
  });

  it("clicking a row calls selectMachine", async () => {
    mockApi.listMachines.mockResolvedValue([
      { id: 5, hostname: "RENDER-05", ip: "192.168.10.25", role: "render", status: "online", last_seen_at: null },
    ]);
    mockApi.getMachineDetail.mockResolvedValue({
      machine: { id: 5, hostname: "RENDER-05", ip: "192.168.10.25", role: "render", status: "online", last_seen_at: null },
      ue_installs: [],
      gpus: [],
    });
    const wrapper = mount(Machines);
    await flushPromises();
    await wrapper.find("[data-machine-row]").trigger("click");
    await flushPromises();
    expect(mockApi.getMachineDetail).toHaveBeenCalledWith({ id: 5 });
  });
});
```

- [ ] **Step 5: Defer running tests until Task 17 ships DiscoveryWizard**

The Machines view template references `DiscoveryWizard`. Tests will fail until Task 17 is in. That's expected. Skip running `pnpm test` here — proceed to Task 17 which provides the missing piece.

- [ ] **Step 6: Commit**

```bash
git add src/components/machines src/views/Machines.vue src/__tests__/MachineDetail.spec.ts src/__tests__/Machines-view.spec.ts
git commit -m "feat(frontend): split Machines view into list + detail panel with refresh"
```

---

## Task 17: DiscoveryWizard modal

**Files:**
- Create: `src/components/modals/DiscoveryWizard.vue`
- Create: `src/__tests__/DiscoveryWizard.spec.ts`

- [ ] **Step 1: Write `src/__tests__/DiscoveryWizard.spec.ts`**

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    scanNetwork: vi.fn(),
    addDiscoveredMachine: vi.fn(),
    listMachines: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));

import DiscoveryWizard from "@/components/modals/DiscoveryWizard.vue";

describe("DiscoveryWizard", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((m: any) => m.mockReset());
    mockApi.listMachines.mockResolvedValue([]);
  });

  it("renders nothing when open=false", () => {
    const wrapper = mount(DiscoveryWizard, { props: { open: false } });
    expect(document.body.querySelector("[data-modal]")).toBeNull();
  });

  it("scan button calls scanNetwork with the CIDR input value", async () => {
    mockApi.scanNetwork.mockResolvedValue({ probed: [] });
    const wrapper = mount(DiscoveryWizard, { props: { open: true } });
    await flushPromises();
    const cidrInput = document.body.querySelector("[data-cidr-input]") as HTMLInputElement;
    cidrInput.value = "10.0.0.0/30";
    cidrInput.dispatchEvent(new Event("input"));
    const scanBtn = document.body.querySelector("[data-scan-btn]") as HTMLButtonElement;
    scanBtn.click();
    await flushPromises();
    expect(mockApi.scanNetwork).toHaveBeenCalledWith("10.0.0.0/30");
  });

  it("renders probed hosts after scan", async () => {
    mockApi.scanNetwork.mockResolvedValue({
      probed: [
        { ip: "192.168.10.21", winrm_open: true, smb_open: true },
        { ip: "192.168.10.22", winrm_open: false, smb_open: true },
      ],
    });
    const wrapper = mount(DiscoveryWizard, { props: { open: true } });
    await flushPromises();
    const scanBtn = document.body.querySelector("[data-scan-btn]") as HTMLButtonElement;
    scanBtn.click();
    await flushPromises();
    const rows = document.body.querySelectorAll("[data-probed-row]");
    expect(rows.length).toBe(2);
  });

  it("Add button calls addDiscoveredMachine for the row's IP", async () => {
    mockApi.scanNetwork.mockResolvedValue({
      probed: [{ ip: "192.168.10.21", winrm_open: true, smb_open: true }],
    });
    mockApi.addDiscoveredMachine.mockResolvedValue(7);
    const wrapper = mount(DiscoveryWizard, { props: { open: true } });
    await flushPromises();
    (document.body.querySelector("[data-scan-btn]") as HTMLButtonElement).click();
    await flushPromises();
    (document.body.querySelector("[data-add-btn]") as HTMLButtonElement).click();
    await flushPromises();
    expect(mockApi.addDiscoveredMachine).toHaveBeenCalledWith("192.168.10.21", null);
  });
});
```

- [ ] **Step 2: Write `src/components/modals/DiscoveryWizard.vue`**

```vue
<script setup lang="ts">
import { computed, ref, watch } from "vue";
import BaseModal from "./BaseModal.vue";
import { useDiscoveryStore } from "@/stores/discovery";
import { useMachinesStore } from "@/stores/machines";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ (e: "close"): void }>();

const discovery = useDiscoveryStore();
const machines = useMachinesStore();
const cidrInput = ref(discovery.cidr);

watch(() => props.open, (val) => {
  if (val) {
    cidrInput.value = discovery.cidr;
  }
});

async function onScan() {
  await discovery.scan(cidrInput.value);
}

async function onAdd(ip: string) {
  await discovery.addToInventory(ip, null);
  await machines.loadMachines();
}
</script>

<template>
  <BaseModal :open="props.open" title="Scan network" @close="emit('close')">
    <div>
      <label class="block text-sm mb-1">CIDR (max 1024 hosts)</label>
      <div class="flex gap-2">
        <input
          data-cidr-input
          v-model="cidrInput"
          placeholder="192.168.10.0/24"
          class="flex-1 border rounded px-2 py-1 text-sm"
        />
        <button
          data-scan-btn
          :disabled="discovery.isScanning"
          class="px-3 py-1 text-sm bg-gray-200 rounded hover:bg-gray-300 disabled:opacity-50"
          @click="onScan"
        >
          {{ discovery.isScanning ? "Scanning..." : "Scan" }}
        </button>
      </div>

      <p v-if="discovery.error" class="mt-2 text-xs text-red-600">
        {{ discovery.error.message }}
      </p>

      <div class="mt-4">
        <p v-if="!discovery.isScanning && discovery.probed.length === 0" class="text-sm text-gray-500">
          No hosts probed yet, or last scan returned no reachable hosts.
        </p>
        <table v-else class="w-full text-sm border">
          <thead class="bg-gray-50">
            <tr>
              <th class="text-left px-2 py-1">IP</th>
              <th class="text-left px-2 py-1">WinRM</th>
              <th class="text-left px-2 py-1">SMB</th>
              <th class="px-2 py-1"></th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="host in discovery.probed"
              :key="host.ip"
              data-probed-row
              class="border-t"
            >
              <td class="px-2 py-1 font-mono text-xs">{{ host.ip }}</td>
              <td class="px-2 py-1">{{ host.winrm_open ? "✓" : "—" }}</td>
              <td class="px-2 py-1">{{ host.smb_open ? "✓" : "—" }}</td>
              <td class="px-2 py-1 text-right">
                <button
                  data-add-btn
                  class="text-xs px-2 py-0.5 border rounded hover:bg-gray-100"
                  @click="onAdd(host.ip)"
                >
                  Add
                </button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
    <template #footer>
      <button class="px-3 py-1 text-sm border rounded hover:bg-gray-100" @click="emit('close')">
        Done
      </button>
    </template>
  </BaseModal>
</template>
```

- [ ] **Step 3: Run tests**

```bash
pnpm test
```

Expected: all earlier tests + 4 new DiscoveryWizard tests + the deferred Machines view tests now pass. Total ~42 frontend tests.

- [ ] **Step 4: Commit**

```bash
git add src/components/modals/DiscoveryWizard.vue src/__tests__/DiscoveryWizard.spec.ts
git commit -m "feat(frontend): add DiscoveryWizard modal (CIDR scan + add to inventory)"
```

---

## Task 18: CredentialDialog modal

**Files:**
- Create: `src/components/modals/CredentialDialog.vue`
- Create: `src/__tests__/CredentialDialog.spec.ts`

The dialog manages credentials globally (not tied to a single machine). User opens it from anywhere; sees existing aliases, can add or delete. Password is write-only — never displayed.

- [ ] **Step 1: Write `src/__tests__/CredentialDialog.spec.ts`**

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    listCredentials: vi.fn(),
    saveCredential: vi.fn(),
    deleteCredential: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));

import CredentialDialog from "@/components/modals/CredentialDialog.vue";

describe("CredentialDialog", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((m: any) => m.mockReset());
  });

  it("loads credentials when opened", async () => {
    mockApi.listCredentials.mockResolvedValue([
      { id: 1, alias: "UECM:winrm:HOST-A", kind: "winrm", username: "admin" },
    ]);
    mount(CredentialDialog, { props: { open: true } });
    await flushPromises();
    expect(mockApi.listCredentials).toHaveBeenCalled();
    const rows = document.body.querySelectorAll("[data-cred-row]");
    expect(rows.length).toBe(1);
  });

  it("save form submits alias/kind/username/password", async () => {
    mockApi.listCredentials.mockResolvedValue([]);
    mockApi.saveCredential.mockResolvedValue(1);
    mount(CredentialDialog, { props: { open: true } });
    await flushPromises();

    (document.body.querySelector("[data-cred-alias]") as HTMLInputElement).value = "UECM:winrm:HOST-A";
    document.body.querySelector("[data-cred-alias]")!.dispatchEvent(new Event("input"));
    (document.body.querySelector("[data-cred-username]") as HTMLInputElement).value = "admin";
    document.body.querySelector("[data-cred-username]")!.dispatchEvent(new Event("input"));
    (document.body.querySelector("[data-cred-password]") as HTMLInputElement).value = "p@ss";
    document.body.querySelector("[data-cred-password]")!.dispatchEvent(new Event("input"));
    (document.body.querySelector("[data-cred-save-btn]") as HTMLButtonElement).click();
    await flushPromises();

    expect(mockApi.saveCredential).toHaveBeenCalledWith("UECM:winrm:HOST-A", "winrm", "admin", "p@ss");
  });

  it("delete button calls deleteCredential", async () => {
    mockApi.listCredentials.mockResolvedValue([
      { id: 1, alias: "UECM:winrm:HOST-A", kind: "winrm", username: "admin" },
    ]);
    mockApi.deleteCredential.mockResolvedValue(undefined);
    mount(CredentialDialog, { props: { open: true } });
    await flushPromises();

    (document.body.querySelector("[data-cred-delete-btn]") as HTMLButtonElement).click();
    await flushPromises();

    expect(mockApi.deleteCredential).toHaveBeenCalledWith("UECM:winrm:HOST-A");
  });
});
```

- [ ] **Step 2: Write `src/components/modals/CredentialDialog.vue`**

```vue
<script setup lang="ts">
import { ref, watch } from "vue";
import BaseModal from "./BaseModal.vue";
import { useCredentialsStore } from "@/stores/credentials";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ (e: "close"): void }>();

const store = useCredentialsStore();
const alias = ref("");
const kind = ref("winrm");
const username = ref("");
const password = ref("");

watch(() => props.open, async (val) => {
  if (val) {
    await store.load();
    alias.value = "";
    kind.value = "winrm";
    username.value = "";
    password.value = "";
  }
});

async function onSave() {
  if (!alias.value || !username.value || !password.value) return;
  try {
    await store.save(alias.value, kind.value, username.value, password.value);
    alias.value = "";
    username.value = "";
    password.value = "";
  } catch {
    /* error captured in store */
  }
}

async function onDelete(a: string) {
  await store.remove(a);
}
</script>

<template>
  <BaseModal :open="props.open" title="Credentials" @close="emit('close')">
    <section>
      <h3 class="text-sm font-medium mb-2">Stored credentials</h3>
      <p v-if="store.credentials.length === 0" class="text-sm text-gray-500">
        No credentials saved yet.
      </p>
      <table v-else class="w-full text-sm border">
        <thead class="bg-gray-50">
          <tr>
            <th class="text-left px-2 py-1">Alias</th>
            <th class="text-left px-2 py-1">Kind</th>
            <th class="text-left px-2 py-1">User</th>
            <th class="px-2 py-1"></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="c in store.credentials" :key="c.alias" data-cred-row class="border-t">
            <td class="px-2 py-1 font-mono text-xs">{{ c.alias }}</td>
            <td class="px-2 py-1">{{ c.kind }}</td>
            <td class="px-2 py-1">{{ c.username }}</td>
            <td class="px-2 py-1 text-right">
              <button
                data-cred-delete-btn
                class="text-xs text-red-600 hover:underline"
                @click="onDelete(c.alias)"
              >
                Delete
              </button>
            </td>
          </tr>
        </tbody>
      </table>
    </section>

    <section class="mt-6">
      <h3 class="text-sm font-medium mb-2">Add credential</h3>
      <div class="space-y-2">
        <input
          data-cred-alias
          v-model="alias"
          placeholder="alias (e.g. UECM:winrm:RENDER-01)"
          class="w-full border rounded px-2 py-1 text-sm font-mono"
        />
        <select v-model="kind" class="w-full border rounded px-2 py-1 text-sm">
          <option value="winrm">winrm</option>
          <option value="share">share</option>
        </select>
        <input
          data-cred-username
          v-model="username"
          placeholder="username"
          class="w-full border rounded px-2 py-1 text-sm"
        />
        <input
          data-cred-password
          v-model="password"
          type="password"
          placeholder="password (write-only, never displayed)"
          class="w-full border rounded px-2 py-1 text-sm"
        />
        <button
          data-cred-save-btn
          class="w-full px-3 py-1 text-sm bg-gray-200 rounded hover:bg-gray-300"
          @click="onSave"
        >
          Save
        </button>
      </div>
      <p v-if="store.error" class="mt-2 text-xs text-red-600">
        {{ store.error.message }}
      </p>
    </section>
  </BaseModal>
</template>
```

- [ ] **Step 3: Run tests + commit**

```bash
pnpm test
```

Expected: 3 new tests pass.

```bash
git add src/components/modals/CredentialDialog.vue src/__tests__/CredentialDialog.spec.ts
git commit -m "feat(frontend): add CredentialDialog modal (list/add/delete cmdkey aliases)"
```

---

## Task 19: EnvVarConfigModal + IniEditModal + wire up Machines view

**Files:**
- Create: `src/components/modals/EnvVarConfigModal.vue`
- Create: `src/components/modals/IniEditModal.vue`
- Create: `src/__tests__/EnvVarConfigModal.spec.ts`
- Create: `src/__tests__/IniEditModal.spec.ts`
- Modify: `src/views/Machines.vue` (add modal state + emit handlers)

- [ ] **Step 1: Write `src/__tests__/EnvVarConfigModal.spec.ts`**

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    getMachineEnvVar: vi.fn(),
    setMachineEnvVar: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));

import EnvVarConfigModal from "@/components/modals/EnvVarConfigModal.vue";

describe("EnvVarConfigModal", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((m: any) => m.mockReset());
  });

  it("reads current value when opened", async () => {
    mockApi.getMachineEnvVar.mockResolvedValue("\\\\HOST\\DDC");
    mount(EnvVarConfigModal, { props: { open: true, machineId: 5, varName: "UE-SharedDataCachePath" } });
    await flushPromises();
    expect(mockApi.getMachineEnvVar).toHaveBeenCalledWith(5, "UE-SharedDataCachePath");
    const currentEl = document.body.querySelector("[data-current-value]");
    expect(currentEl?.textContent).toContain("\\\\HOST\\DDC");
  });

  it("apply submits the new value via setMachineEnvVar", async () => {
    mockApi.getMachineEnvVar.mockResolvedValue(null);
    mockApi.setMachineEnvVar.mockResolvedValue(undefined);
    mount(EnvVarConfigModal, { props: { open: true, machineId: 5, varName: "UE-SharedDataCachePath" } });
    await flushPromises();

    const input = document.body.querySelector("[data-new-value]") as HTMLInputElement;
    input.value = "\\\\HOST\\NewDDC";
    input.dispatchEvent(new Event("input"));
    (document.body.querySelector("[data-apply-btn]") as HTMLButtonElement).click();
    await flushPromises();

    expect(mockApi.setMachineEnvVar).toHaveBeenCalledWith(5, "UE-SharedDataCachePath", "\\\\HOST\\NewDDC");
  });
});
```

- [ ] **Step 2: Write `src/components/modals/EnvVarConfigModal.vue`**

```vue
<script setup lang="ts">
import { ref, watch } from "vue";
import BaseModal from "./BaseModal.vue";
import { tauriApi, type UecmError } from "@/services/tauri";

const props = defineProps<{
  open: boolean;
  machineId: number | null;
  varName: string;
}>();
const emit = defineEmits<{ (e: "close"): void }>();

const currentValue = ref<string | null>(null);
const newValue = ref("");
const loading = ref(false);
const applying = ref(false);
const error = ref<UecmError | null>(null);
const applied = ref(false);

watch(
  () => [props.open, props.machineId],
  async ([open, id]) => {
    if (!open || id === null) return;
    loading.value = true;
    applied.value = false;
    error.value = null;
    try {
      currentValue.value = await tauriApi.getMachineEnvVar(id as number, props.varName);
      newValue.value = currentValue.value ?? "";
    } catch (e) {
      error.value = e as UecmError;
      currentValue.value = null;
    } finally {
      loading.value = false;
    }
  },
);

async function onApply() {
  if (props.machineId === null) return;
  applying.value = true;
  error.value = null;
  try {
    await tauriApi.setMachineEnvVar(props.machineId, props.varName, newValue.value);
    currentValue.value = newValue.value;
    applied.value = true;
  } catch (e) {
    error.value = e as UecmError;
  } finally {
    applying.value = false;
  }
}
</script>

<template>
  <BaseModal :open="props.open" :title="`Set env var: ${props.varName}`" @close="emit('close')">
    <div>
      <p class="text-xs text-gray-500 mb-2">
        Sets the system-level env var on the remote machine. Requires WinRM admin.
      </p>

      <p class="text-sm mb-1">Current value</p>
      <p data-current-value class="text-sm font-mono bg-gray-50 border rounded px-2 py-1 mb-3 break-all">
        <span v-if="loading">loading...</span>
        <span v-else>{{ currentValue ?? "(not set)" }}</span>
      </p>

      <p class="text-sm mb-1">New value</p>
      <input
        data-new-value
        v-model="newValue"
        placeholder="\\HOST\DDC"
        class="w-full border rounded px-2 py-1 text-sm font-mono"
      />

      <p v-if="error" class="mt-2 text-xs text-red-600">{{ error.message }}</p>
      <p v-if="applied" class="mt-2 text-xs text-green-700">Applied + verified.</p>
    </div>
    <template #footer>
      <button class="px-3 py-1 text-sm border rounded hover:bg-gray-100" @click="emit('close')">
        Cancel
      </button>
      <button
        data-apply-btn
        :disabled="applying || loading"
        class="px-3 py-1 text-sm bg-gray-200 rounded hover:bg-gray-300 disabled:opacity-50"
        @click="onApply"
      >
        {{ applying ? "Applying..." : "Apply" }}
      </button>
    </template>
  </BaseModal>
</template>
```

- [ ] **Step 3: Write `src/__tests__/IniEditModal.spec.ts`**

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    readIniSection: vi.fn(),
    setIniKey: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));

import IniEditModal from "@/components/modals/IniEditModal.vue";

describe("IniEditModal", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((m: any) => m.mockReset());
  });

  it("loads keys when Read button clicked", async () => {
    mockApi.readIniSection.mockResolvedValue([{ name: "Path", value: "../Content" }]);
    mount(IniEditModal, { props: { open: true, machineId: 5 } });
    await flushPromises();

    (document.body.querySelector("[data-ini-path]") as HTMLInputElement).value = "C:\\Proj\\Config\\DefaultEngine.ini";
    document.body.querySelector("[data-ini-path]")!.dispatchEvent(new Event("input"));
    (document.body.querySelector("[data-ini-section]") as HTMLInputElement).value = "Core.System";
    document.body.querySelector("[data-ini-section]")!.dispatchEvent(new Event("input"));
    (document.body.querySelector("[data-ini-read-btn]") as HTMLButtonElement).click();
    await flushPromises();

    expect(mockApi.readIniSection).toHaveBeenCalledWith(5, "C:\\Proj\\Config\\DefaultEngine.ini", "Core.System");
    const rows = document.body.querySelectorAll("[data-ini-row]");
    expect(rows.length).toBe(1);
  });

  it("apply calls setIniKey and shows backup path", async () => {
    mockApi.readIniSection.mockResolvedValue([]);
    mockApi.setIniKey.mockResolvedValue({ backup_path: "C:\\Proj\\Config\\DefaultEngine.ini.uecm-bak-1700000000" });
    mount(IniEditModal, { props: { open: true, machineId: 5 } });
    await flushPromises();

    (document.body.querySelector("[data-ini-path]") as HTMLInputElement).value = "C:\\Proj\\Config\\DefaultEngine.ini";
    document.body.querySelector("[data-ini-path]")!.dispatchEvent(new Event("input"));
    (document.body.querySelector("[data-ini-section]") as HTMLInputElement).value = "Core.System";
    document.body.querySelector("[data-ini-section]")!.dispatchEvent(new Event("input"));
    (document.body.querySelector("[data-ini-key]") as HTMLInputElement).value = "Paths";
    document.body.querySelector("[data-ini-key]")!.dispatchEvent(new Event("input"));
    (document.body.querySelector("[data-ini-value]") as HTMLInputElement).value = "../Content/NewPath";
    document.body.querySelector("[data-ini-value]")!.dispatchEvent(new Event("input"));
    (document.body.querySelector("[data-ini-apply-btn]") as HTMLButtonElement).click();
    await flushPromises();

    expect(mockApi.setIniKey).toHaveBeenCalledWith(
      5,
      "C:\\Proj\\Config\\DefaultEngine.ini",
      "Core.System",
      "Paths",
      "../Content/NewPath",
    );
    expect(document.body.innerHTML).toContain("uecm-bak-1700000000");
  });
});
```

- [ ] **Step 4: Write `src/components/modals/IniEditModal.vue`**

```vue
<script setup lang="ts">
import { ref, watch } from "vue";
import BaseModal from "./BaseModal.vue";
import { tauriApi, type IniKey, type UecmError } from "@/services/tauri";

const props = defineProps<{
  open: boolean;
  machineId: number | null;
}>();
const emit = defineEmits<{ (e: "close"): void }>();

const filePath = ref("");
const section = ref("");
const keyName = ref("");
const keyValue = ref("");
const loadedKeys = ref<IniKey[]>([]);
const lastBackup = ref<string | null>(null);
const reading = ref(false);
const applying = ref(false);
const error = ref<UecmError | null>(null);

watch(
  () => props.open,
  (val) => {
    if (val) {
      filePath.value = "";
      section.value = "";
      keyName.value = "";
      keyValue.value = "";
      loadedKeys.value = [];
      lastBackup.value = null;
      error.value = null;
    }
  },
);

async function onRead() {
  if (props.machineId === null || !filePath.value || !section.value) return;
  reading.value = true;
  error.value = null;
  try {
    loadedKeys.value = await tauriApi.readIniSection(props.machineId, filePath.value, section.value);
  } catch (e) {
    error.value = e as UecmError;
    loadedKeys.value = [];
  } finally {
    reading.value = false;
  }
}

async function onApply() {
  if (props.machineId === null || !filePath.value || !section.value || !keyName.value) return;
  applying.value = true;
  error.value = null;
  try {
    const result = await tauriApi.setIniKey(
      props.machineId,
      filePath.value,
      section.value,
      keyName.value,
      keyValue.value,
    );
    lastBackup.value = result.backup_path;
    // Re-read to confirm
    await onRead();
  } catch (e) {
    error.value = e as UecmError;
  } finally {
    applying.value = false;
  }
}
</script>

<template>
  <BaseModal :open="props.open" title="Edit INI key" @close="emit('close')">
    <div>
      <p class="text-xs text-gray-500 mb-2">
        Reads/writes a single key in an [section] of an INI file on the remote machine.
        Auto-backs up the file before writing.
      </p>

      <label class="block text-sm mb-1">File path (remote)</label>
      <input
        data-ini-path
        v-model="filePath"
        placeholder="C:\\Path\\To\\Project\\Config\\DefaultEngine.ini"
        class="w-full border rounded px-2 py-1 text-sm font-mono mb-2"
      />

      <label class="block text-sm mb-1">Section (without brackets)</label>
      <input
        data-ini-section
        v-model="section"
        placeholder="Core.System"
        class="w-full border rounded px-2 py-1 text-sm font-mono mb-2"
      />

      <button
        data-ini-read-btn
        :disabled="reading"
        class="px-3 py-1 text-sm border rounded hover:bg-gray-100 disabled:opacity-50 mb-3"
        @click="onRead"
      >
        {{ reading ? "Reading..." : "Read section" }}
      </button>

      <div v-if="loadedKeys.length > 0" class="mb-3">
        <p class="text-sm font-medium mb-1">Existing keys</p>
        <table class="w-full text-xs border">
          <thead class="bg-gray-50">
            <tr><th class="text-left px-2 py-1">Name</th><th class="text-left px-2 py-1">Value</th></tr>
          </thead>
          <tbody>
            <tr v-for="k in loadedKeys" :key="k.name" data-ini-row class="border-t">
              <td class="px-2 py-1 font-mono">{{ k.name }}</td>
              <td class="px-2 py-1 font-mono break-all">{{ k.value }}</td>
            </tr>
          </tbody>
        </table>
      </div>

      <hr class="my-3" />

      <label class="block text-sm mb-1">Key name</label>
      <input
        data-ini-key
        v-model="keyName"
        placeholder="DDCStrategy"
        class="w-full border rounded px-2 py-1 text-sm font-mono mb-2"
      />

      <label class="block text-sm mb-1">New value</label>
      <input
        data-ini-value
        v-model="keyValue"
        placeholder="Filesystem"
        class="w-full border rounded px-2 py-1 text-sm font-mono mb-2"
      />

      <p v-if="lastBackup" class="text-xs text-green-700">
        Applied. Backup saved to <span class="font-mono">{{ lastBackup }}</span>
      </p>
      <p v-if="error" class="text-xs text-red-600">{{ error.message }}</p>
    </div>
    <template #footer>
      <button class="px-3 py-1 text-sm border rounded hover:bg-gray-100" @click="emit('close')">
        Cancel
      </button>
      <button
        data-ini-apply-btn
        :disabled="applying"
        class="px-3 py-1 text-sm bg-gray-200 rounded hover:bg-gray-300 disabled:opacity-50"
        @click="onApply"
      >
        {{ applying ? "Applying..." : "Apply" }}
      </button>
    </template>
  </BaseModal>
</template>
```

- [ ] **Step 5: Wire all three modals into `src/views/Machines.vue`**

Replace the file:

```vue
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useMachinesStore } from "@/stores/machines";
import MachineDetail from "@/components/machines/MachineDetail.vue";
import DiscoveryWizard from "@/components/modals/DiscoveryWizard.vue";
import CredentialDialog from "@/components/modals/CredentialDialog.vue";
import EnvVarConfigModal from "@/components/modals/EnvVarConfigModal.vue";
import IniEditModal from "@/components/modals/IniEditModal.vue";

const store = useMachinesStore();
const showDiscovery = ref(false);
const showCredentials = ref(false);
const showEnvVar = ref(false);
const showIniEdit = ref(false);

const selectedId = computed(() => store.selectedDetail?.machine.id ?? null);

onMounted(() => {
  store.loadMachines();
});

async function onSelect(id: number | null) {
  if (id === null) return;
  await store.selectMachine(id);
}

async function onDelete(id: number | null) {
  if (id === null) return;
  await store.deleteMachine(id);
}
</script>

<template>
  <div class="h-full flex">
    <aside class="w-80 border-r overflow-auto p-4">
      <header class="flex items-center justify-between mb-3">
        <h1 class="text-lg font-semibold">Machines</h1>
        <button
          data-discover-btn
          class="px-3 py-1 text-sm bg-gray-200 rounded hover:bg-gray-300"
          @click="showDiscovery = true"
        >
          Scan
        </button>
      </header>

      <p v-if="store.isLoading" class="text-sm text-gray-500">Loading...</p>
      <p v-else-if="store.machines.length === 0" class="text-sm text-gray-500">
        No machines yet. Click Scan to discover.
      </p>
      <ul v-else class="space-y-1">
        <li
          v-for="m in store.machines"
          :key="m.id ?? m.ip"
          data-machine-row
          class="px-2 py-2 rounded cursor-pointer hover:bg-gray-100 flex items-center justify-between"
          :class="store.selectedDetail?.machine.id === m.id ? 'bg-gray-200 font-medium' : ''"
          @click="onSelect(m.id)"
        >
          <span class="truncate">
            {{ m.hostname }}<br />
            <span class="text-xs text-gray-500 font-normal">{{ m.ip }}</span>
          </span>
          <button
            class="text-xs text-red-600 hover:underline ml-2"
            @click.stop="onDelete(m.id)"
          >
            ×
          </button>
        </li>
      </ul>

      <p v-if="store.error" class="mt-3 text-xs text-red-600">
        {{ store.error.message }}
      </p>
    </aside>

    <main class="flex-1 overflow-auto">
      <MachineDetail
        @open-credential-modal="showCredentials = true"
        @open-env-var-modal="showEnvVar = true"
        @open-ini-edit-modal="showIniEdit = true"
      />
    </main>

    <DiscoveryWizard :open="showDiscovery" @close="showDiscovery = false" />
    <CredentialDialog :open="showCredentials" @close="showCredentials = false" />
    <EnvVarConfigModal
      :open="showEnvVar"
      :machine-id="selectedId"
      var-name="UE-SharedDataCachePath"
      @close="showEnvVar = false"
    />
    <IniEditModal
      :open="showIniEdit"
      :machine-id="selectedId"
      @close="showIniEdit = false"
    />
  </div>
</template>
```

- [ ] **Step 6: Run all tests**

```bash
pnpm test
```

Expected: ~50 frontend tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/components/modals src/views/Machines.vue src/__tests__/EnvVarConfigModal.spec.ts src/__tests__/IniEditModal.spec.ts
git commit -m "feat(frontend): add EnvVar + INI edit modals and wire up Machines view"
```

---

## Task 20: Final integration — README + production build smoke

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Run the full test suite**

```bash
export PATH="/Users/bip.lan/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
pnpm test
cd src-tauri && cargo test && cd ..
```

Expected:
- Frontend: ~50 tests pass.
- Backend: ~44 tests pass.

If anything fails, report BLOCKED with the failing test name.

- [ ] **Step 2: Production build smoke**

```bash
pnpm tauri build
```

Expected: completes; bundle written to `src-tauri/target/release/bundle/`. May take 5-10 min due to LTO release profile.

- [ ] **Step 3: Update README.md to reflect Plan 2 completion**

Replace `README.md` with:

```
# UE Cache Manager (UECM)

Cross-machine Unreal Engine cache management tool for VP/XR render clusters.

**Status:** Plan 2 (Discovery & Single-Machine Config) complete. See `docs/superpowers/plans/`.

## What's working

- Tauri 2.x app shell with 7 navigable views
- SQLite persistence: machines + machine_ue_installs + machine_gpus + credentials
- **Network discovery**: scan a CIDR, probe ports 5985 (WinRM) and 445 (SMB), add reachable hosts
- **Per-machine refresh**: probe WinRM, then read installed UE versions (registry) + GPU model + driver version (WMI)
- **Credential management**: store WinRM credentials via cmdkey + Windows Credential Manager; aliases tracked in SQLite
- **Single-machine env var config**: read/write system-level env vars on a remote host (e.g. `UE-SharedDataCachePath`)
- **Single-machine INI editing**: read [section] keys + set a single key with auto-backup (`<file>.uecm-bak-<unix-ts>`)
- Builds to a single .exe / .dmg / .AppImage

## What's NOT yet implemented (next plans)

- SMB share creation wizard (Mode A / Mode B), SYSTEM credential injection (Plan 3)
- Cluster batch configuration pushes (Plan 3)
- INI conflict scanner + auto-fix (Plan 4)
- Cluster health check matrix (Plan 4)
- DDC Pak generation + distribution (Plan 5)
- PSO Cache operations + visual polish (Plan 6)

## Development

Prerequisites:
- Rust 1.75+ (https://rustup.rs/)
- Node.js 20+, pnpm
- OS-specific Tauri prerequisites: see https://v2.tauri.app/start/prerequisites/

```bash
pnpm install
pnpm tauri dev
pnpm test
cd src-tauri
cargo test
cd ..
pnpm tauri build
```

End-to-end verification of WinRM-dependent features (discovery refresh, env var, INI edit) requires a Windows test host with `Enable-PSRemoting` enabled. macOS dev builds + tests work, but PowerShell-dependent unit tests are gated `#[cfg(windows)]`.

## Architecture

- `src/`: Vue 3 frontend (TypeScript, Tailwind, Pinia, Vue Router)
- `src-tauri/`: Rust backend (rusqlite, serde, tokio, ipnet)
- `ps-scripts/`: PowerShell sidecar scripts (Windows-only execution)
- `docs/superpowers/specs/`: design docs
- `docs/superpowers/plans/`: implementation plans

Frontend ↔ backend communication via Tauri commands. All command handlers in
`src-tauri/src/commands/` are thin wrappers that delegate to logic in
`src-tauri/src/core/` and data in `src-tauri/src/data/`. All cross-machine
operations route through PowerShell sidecar via `core/winrm.rs` →
`ps-scripts/invoke-remote.ps1`.

## Platform support

Target: Windows 10/11. Foundation builds + runs on Mac/Linux for development;
PowerShell sidecar (and all WinRM/registry/credential features) only work
on Windows. Tests gated appropriately with `#[cfg(windows)]`.
```

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs: update README with Plan 2 completion status"
```

- [ ] **Step 5: Verify clean repo state**

```bash
git status
git log --oneline | head -25
```

Expected: clean (only `.claude/` + `CLAUDE.md` untracked); ~20+ new commits since Plan 1.

---

## Summary

At the end of Plan 2:

1. ✅ User can scan a LAN CIDR for reachable Windows machines
2. ✅ User can store WinRM credentials per machine via cmdkey
3. ✅ User can refresh a machine to populate UE installs + GPU info
4. ✅ Machines view shows a detail panel with full per-machine info + actions
5. ✅ User can set a DDC env var on a single remote machine
6. ✅ User can edit a single INI key on a single remote machine with auto-backup
7. ✅ Production build still succeeds; all tests green

**Plan 3** will build on this foundation: multi-machine SMB share creation wizard (Mode A + Mode B), SYSTEM-level credential injection via PsExec, and cluster batch configuration pushes (apply env var / INI changes to multiple machines in one operation).
