# UECM Plan 4 — Diagnostics: INI Scanner + Cluster Health Check Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

## Execution Mode (READ FIRST — overrides default skill behavior)

**Mode: AUTO-CONTINUOUS.** Run all tasks back-to-back without pausing for human approval between them. Same rules as Plan 3.

**Stop and ask the user ONLY in these cases:**

1. **Plan vs reality conflict** that requires re-design (structural mismatch where continuing would produce wrong work).
2. **Destructive operation requiring authorization**: deleting registry keys outside `HKLM\SYSTEM\...\Environment`, modifying Group Policy, opening firewall ports, dropping production credentials, `git push --force`, `rm -rf` outside the workspace, modifying SSH config, deleting any user data outside `%LOCALAPPDATA%\UECM`.
3. **Critical-severity code review finding with no obvious fix.**
4. **lanPC unreachable, WinRM disabled, or PsExec64 missing** when an E2E verification step requires it.
5. **A new dependency decision** not covered by the plan.

**Do NOT stop for:** spec/quality review finding Important / Minor issues (fix in `fix:` follow-up commit, proceed); Windows-gated tests skipped on macOS; DONE_WITH_CONCERNS observations; README/docs cleanup.

**Final report:** commit list, frontend + backend test counts, every DONE_WITH_CONCERNS verbatim, production build outcome, deferred lanPC E2E steps awaiting user.

---

**Goal:** Plan 4 builds the diagnostic surface — the differentiator that makes UECM more than a configuration pusher. Two coordinated subsystems:

1. **INI Scanner (Module E2)** — multi-file scan across machines, rule-based diagnostic engine that produces `Findings` (severity + file path + snippet + recommendation), and a one-click apply-fix flow that reuses Plan 2's atomic backup write path.
2. **Cluster Health Check (Module E3)** — 11-row × N-column matrix, each cell a probe result with structured remediation. Cells map to root-cause analysis: SYSTEM-write failure → SYSTEM-credential check, etc.

**Why this order before Plan 5/6:** DDC Pak (Plan 5) and PSO Cache (Plan 6) operations both lean on the diagnostic results — you only push DDC Pak after INI is clean, you only collect PSO after the GPU consistency check passes. Plan 4 lays the trust layer.

**Architecture additions:**

- `core::ini_scanner` — enumerates the four INI categories per machine (project / Windows-overlay / ConsoleVariables / user-level / engine-level), reads each via the existing `read-ini-section.ps1` plumbing extended to read whole files, runs them through a pure-Rust `core::ini_diagnostics` rule engine.
- `core::ini_diagnostics` — pure rule functions taking `(file_path, section, kv_pairs, env_var_state)` → `Vec<Finding>`. No Windows dependency, fully unit-testable on macOS.
- `core::health_check` — orchestrates 11 probe functions per machine (each returns a typed `CheckOutcome`), plus a cluster-level GPU consistency aggregator. Fans out across machines via the existing `core::batch::run_batch` plumbing.
- New PowerShell sidecar `read-ini-file.ps1` reads an entire INI file (all sections) so the scanner sees the full picture.
- New PowerShell sidecar `health-probes.ps1` packages the per-machine probes (SMB service, firewall 445, share reachable, NTFS, cmdkey list user/SYSTEM, env vars, SYSTEM write test) in one round-trip per machine.
- `core::ini_apply` translates a single `Finding` into an `ini_editor::set_key_with_credential` call (or a "remove key" call when the recommendation is to delete a hardcoded `Path=`).
- Three SQLite tables: `scan_runs`, `ini_findings`, `health_check_runs` (migration 007).
- Tauri command surface: `scan_inis`, `list_findings`, `apply_finding`, `run_health_check`, `list_health_check_runs`, plus per-finding read.
- Frontend stores: `useDiagnosticsStore` (INI scan run + findings) and `useHealthCheckStore` (matrix). Both subscribe to `tauri::Window::emit` progress events.
- Frontend primitives expansion: `UecmCodeBlock.vue` (diff blocks), `UecmKpiTile.vue` (Health KPI strip), `UecmScoreTile.vue` (Cluster Score), `UecmFilterChip.vue` (filter dropdowns), `UecmDetailCard.vue`. The design system overhaul plan (`2026-05-02-uecm-design-system-overhaul.md`) called for these but they are not yet present in `src/components/primitives/index.ts`.

**Tech Stack:** Builds on Plan 3 stack. New Rust dep: `regex = "1"` (already transitively available; promote to direct dep) for INI parsing tolerance. No new frontend deps.

**Out of scope for this plan (deferred):**

- DDC Pak generation / distribution (Plan 5)
- PSO Cache file collection / distribution (Plan 6) — only the *PSO Precaching CVar check* and *GPU consistency check* are in Plan 4 (read-only checks, no collection).
- Project identity matching across machines (Plan 5 brings the `project_locations` table). Plan 4 takes a `project_paths: Vec<String>` argument per machine when the operator wants project-level scans. Engine-level + user-level scans need no project knowledge.
- Scheduled / background recurrence of health checks (manual trigger only in v1).
- Full export-to-PDF (Plan 7 polish). Plan 4 ships JSON + Markdown export.
- Real-time WebSocket-style streaming of every probe (Plan 4 emits one `health-progress` event per machine completion, not per-check).

**Deliverable at end:**

1. User can click "Run INI scan" → wizard prompts for machines + optional project paths → scanner runs, populates `scan_runs` + `ini_findings` rows.
2. INIScanner.vue replaces the current empty stub with: a 4-cell summary strip (Critical / Warning / Healthy / Total files), a hierarchy tree (machine → project/category → file → finding), a 3-column diagnostic detail (What / Why / Symptom) with a before/after diff via `UecmCodeBlock`, and four per-finding actions (Apply suggestion / Custom edit / Open file / Skip).
3. Clicking "Apply suggestion" runs `set_ini_key_with_credential`, automatically backs up the file (already implemented), re-reads the section to verify, marks the finding as `fixed` in SQLite, and updates the UI.
4. User can click "Run full health check" → fan-out runs 11 probes per machine + cluster-level aggregations.
5. HealthCheck.vue replaces the current 8-cell stub with: a Cluster Score tile + 4 KPI tiles (Healthy / Warning / Critical / Offline), a Matrix view (11 rows × N machines, sticky headers, emphasized SYSTEM-write + SYSTEM-cred columns), a Console view (probe log), a detail panel with What / How / Symptom + sample probe output + suggested fix.
6. The Health Check `INI consistency` check (#8) reads the most recent `scan_runs` row for the machine; the `GPU/driver consistency` check (#11) reads the persisted `machine_gpus` rows. Both are derived, no new probe traffic needed.
7. The matrix detail "Apply auto-fix" button on a Critical cell jumps the user to the relevant subsystem (INI Scanner finding, or env var modal, or share wizard) with the cell pre-selected.
8. Production build green; full E2E on lanPC verified end-to-end.

---

## Lessons from Plan 3 — Applied to Plan 4

| Source | Lesson | Plan 4 task |
|---|---|---|
| Plan 3 T1 audit | All `Invoke-Command` returning primitives MUST cast via `"$result"` before JSON | T7 (read-ini-file.ps1) + T11 (health-probes.ps1) — applied at design time |
| Plan 3 credential pipe | Every WinRM-using script accepts `-Username -Password`; never assume current-token elevation | T7, T11, T18 (apply finding) — all use `*_with_credential` variants |
| Plan 3 batch | `core::batch::run_batch` already handles fan-out, mpsc, and progress events | T13 (health check) reuses it; do not re-implement |
| Plan 3 mark_seen | Probe-then-persist; partial failure does not erase prior good data | T8 (scan_run row created BEFORE per-machine fan-out; per-machine error stored on the finding row, not the run row) |

---

## Prerequisites (engineer must have before starting)

Same as Plan 3, plus:

### lanPC test fixtures (one-time prep)

To exercise the rule engine, lanPC needs at least one INI file containing a representative critical pattern. The plan creates these temporarily during E2E:

```
E:\test-fixtures\PluralityProject\Config\DefaultEngine.ini      (CREATE for T22)
E:\test-fixtures\PluralityProject\Config\ConsoleVariables.ini   (CREATE for T22)
%LOCALAPPDATA%\UnrealEngine\5.4\Saved\Config\WindowsEditor\EditorPerProjectUserSettings.ini  (CREATE+RESTORE for T22)
```

Fixtures spelled out in Task 22 step 2.

### Cross-platform testing rules (recap)

- INI rule engine (`core::ini_diagnostics`) is pure Rust. Tests run on macOS.
- Scanner orchestration (`core::ini_scanner`) is Windows-gated for the actual file read; the *result merging / persistence* layer is testable on macOS via injected fakes.
- Health check probes are Windows-gated; the cluster aggregator (GPU consistency) is pure Rust.
- Frontend tests run on macOS via vitest.
- E2E (real INI files, real probes) runs on lanPC via the built `.exe`.

### Decision: project path discovery in Plan 4

Plan 5 will introduce a `project_locations` table and an automatic `.uproject` discovery walk. Plan 4 does NOT need that. Two operating modes for the INI scanner:

- **Engine + user-level (zero-config)**: scanner uses each machine's `machine_ue_installs` rows to locate `<EnginePath>/Engine/Config/BaseEngine.ini` and the per-version `%LocalAppData%\UnrealEngine\<Ver>\Saved\Config\WindowsEditor\EditorPerProjectUserSettings.ini`.
- **Project-level (explicit)**: caller supplies `project_paths: Vec<String>` (absolute paths on each target machine). For each path, scanner reads the three project-level files. UI exposes this as an optional textarea ("one path per line"). Plan 5 will replace this with the project_locations join.

This avoids blocking on a Plan 5 dependency.

---

## File Structure

```
ue-cache-manager/
├── ps-scripts/
│   ├── read-ini-file.ps1                       # NEW (whole-file read, returns sections+kvs)
│   ├── health-probes.ps1                       # NEW (11 probes packaged into one call)
│   ├── (existing scripts unchanged)
│
├── src-tauri/
│   ├── Cargo.toml                              # MODIFY (promote regex to direct dep)
│   └── src/
│       ├── lib.rs                              # MODIFY (register new commands)
│       │
│       ├── commands/
│       │   ├── ini_scanner.rs                  # NEW
│       │   ├── health_check.rs                 # NEW
│       │   └── mod.rs                          # MODIFY
│       │
│       ├── core/
│       │   ├── ini_scanner.rs                  # NEW (orchestration: enumerate files, read, persist)
│       │   ├── ini_diagnostics.rs              # NEW (pure rule engine: kv -> findings)
│       │   ├── ini_apply.rs                    # NEW (translate finding -> ini_editor call)
│       │   ├── health_check.rs                 # NEW (orchestration + cluster aggregator)
│       │   ├── health_probes.rs                # NEW (Windows-gated WinRM dispatch)
│       │   └── mod.rs                          # MODIFY
│       │
│       └── data/
│           ├── scan_runs.rs                    # NEW
│           ├── ini_findings.rs                 # NEW
│           ├── health_check_runs.rs            # NEW
│           ├── schema.rs                       # MODIFY (migration 007)
│           └── mod.rs                          # MODIFY
│
├── src/
│   ├── services/tauri.ts                       # MODIFY (new types + functions)
│   │
│   ├── stores/
│   │   ├── diagnostics.ts                      # NEW (INI scanner state)
│   │   ├── healthCheck.ts                      # NEW (matrix state)
│   │   └── (existing stores unchanged)
│   │
│   ├── lib/
│   │   ├── healthChecks.ts                     # MODIFY (8 → 11 checks, new metadata)
│   │   └── iniRules.ts                         # NEW (rule label/description tables for UI)
│   │
│   ├── components/
│   │   ├── primitives/
│   │   │   ├── UecmCodeBlock.vue               # NEW (diff block with line gutter + highlight)
│   │   │   ├── UecmKpiTile.vue                 # NEW (KPI strip cell)
│   │   │   ├── UecmScoreTile.vue               # NEW (Cluster Score)
│   │   │   ├── UecmFilterChip.vue              # NEW (`LABEL: VALUE ▾`)
│   │   │   └── index.ts                        # MODIFY (export the four new primitives)
│   │   ├── modals/
│   │   │   ├── IniScanWizard.vue               # NEW (machines + project paths + Run)
│   │   │   ├── HealthCheckWizard.vue           # NEW (machines + Run)
│   │   │   └── (existing modals unchanged)
│   │   └── diagnostics/
│   │       ├── FindingHierarchy.vue            # NEW (tree: machine → file → finding)
│   │       ├── FindingDetail.vue               # NEW (3-col diagnostic + diff + actions)
│   │       └── HealthMatrix.vue                # NEW (table cells)
│   │
│   ├── views/
│   │   ├── INIScanner.vue                      # REWRITE (was stub)
│   │   └── HealthCheck.vue                     # REWRITE (was 8-check stub)
│   │
│   └── __tests__/
│       ├── ini-diagnostics.spec.ts             # NEW (rule engine via Tauri mock)
│       ├── diagnostics-store.spec.ts           # NEW
│       ├── health-check-store.spec.ts          # NEW
│       ├── IniScanWizard.spec.ts               # NEW
│       ├── HealthCheckWizard.spec.ts           # NEW
│       ├── FindingHierarchy.spec.ts            # NEW
│       ├── FindingDetail.spec.ts               # NEW
│       ├── HealthMatrix.spec.ts                # NEW
│       ├── INIScanner-view.spec.ts             # NEW (replaces empty stub coverage)
│       ├── HealthCheck-view.spec.ts            # MODIFY (was an implicit no-op)
│       ├── UecmCodeBlock.spec.ts               # NEW
│       ├── UecmKpiTile.spec.ts                 # NEW
│       └── UecmScoreTile.spec.ts               # NEW
│
└── README.md                                   # MODIFY (Plan 4 status)
```

---

## Approach Notes

**INI parsing tolerance.** UE INI files have known oddities: `+ArrayKey=...` array push syntax, `-ArrayKey=...` removal, `!ArrayKey=...` clear, comments with `;` or `//`, BOM, mixed CRLF/LF, blank section lines, sections in `[/Script/Engine.Class]` form. The existing `ps-scripts/read-ini-section.ps1` only walks one section. Plan 4 introduces `read-ini-file.ps1` that returns every section + every key (preserving the leading `+/-/!`) and `core::ini_diagnostics` parses it tolerantly.

**Diagnostic rule set v1.** The spec lists three severities; here are the concrete rules implemented in Plan 4:

| ID | Severity | Pattern | Recommendation |
|---|---|---|---|
| `R001` | 🔴 Critical | `[/Script/UnrealEd.DerivedDataCacheSettings]` has `Path=...` literal AND `EnvPathOverride` is empty/missing | Set `EnvPathOverride=UE-SharedDataCachePath` and remove `Path=` |
| `R002` | 🔴 Critical | `EditorPerProjectUserSettings.ini` overrides any DDC key (file presence + DDC section non-empty) | Delete the DDC section in the user-level file (with backup) |
| `R003` | 🔴 Critical | A path key resolves to an unreachable share (probe done at scan time) | Update path or fix share |
| `R004` | 🟡 Warning | Path uses a mapped drive letter (`Z:\`, `Y:\`, etc.) instead of UNC | Replace with `\\HOST\Share\...` |
| `R005` | 🟡 Warning | Deprecated CVar present (`r.SShaderCache`, `s.SkipFinalizeCommandList`, etc. — list in code) | Remove the line |
| `R006` | 🟡 Warning | `EnvPathOverride=...` set but the corresponding env var is empty on the machine | Set the env var via UECM env var modal |
| `R007` | 🟢 Healthy | `EnvPathOverride=UE-SharedDataCachePath` + env var present + path reachable | (informational only — counts toward "healthy" total) |

The deprecated CVar list lives in `core::ini_diagnostics::DEPRECATED_CVARS`. Easy to extend per UE release.

**Atomicity of "Apply finding".** Each `Finding` row stores enough context to be replayed: `machine_id`, `file_path`, `section`, `key_name`, `recommended_value`, `recommended_action` (`set` or `remove`). `core::ini_apply::apply` calls the existing `core::ini_editor::set_key_with_credential` (which already does atomic backup) for `set`, or a new `remove_key_with_credential` for `remove`. Re-reads the section to confirm. Updates `ini_findings.fixed_at` on success.

**Health check derivation vs probe.** Three of the 11 checks are derived (no new probe traffic):

- `INI consistency` (#8): query latest `scan_runs.id` per machine, count critical findings.
- `GPU/driver consistency` (#11): pure-Rust aggregator over `machine_gpus` rows; produces a per-machine outcome (`healthy` if at least one other machine has matching `(gpu_model, driver_version)`, `warning` if it's the only one with that combo, `critical` if any other machine has a *different* combo and it's flagged as the cluster's standard via a new `is_baseline` opt-in).
- `PSO Precaching CVar` (#10): reads `Config/ConsoleVariables.ini` from each machine's first known project path (or skips if no project path supplied); checks `r.PSOPrecaching` and `r.PSOPrecaching.Validation`.

The other 8 are probes packaged into one PowerShell round-trip per machine (`health-probes.ps1`), reducing latency.

**Frontend store split.** `useDiagnosticsStore` (INI scanner) and `useHealthCheckStore` (matrix) are deliberately separate. Crossing into each other (e.g. clicking "View related INI findings" from a Health cell) goes through a route push + URL query param `?finding=<id>` rather than coupled state. Keeps each store testable in isolation.

**Backwards compatibility.** No existing Plan 1-3 commands change signatures. `lib/healthChecks.ts` expands from 8 to 11 IDs but the existing `HealthCheck.vue` consumer is being rewritten anyway.

---

## Self-Review Checklist (run after writing each task)

- [ ] **Spec coverage:** every section of design doc 5.E2 (INI Scanner) and 5.E3 (Health Check) maps to at least one task. Severity rules R001-R007 have at least one rule-engine test each.
- [ ] **No placeholders:** every step shows actual code or actual command. No "TBD" or "implement later".
- [ ] **Type consistency:** `Severity` enum value names (`critical | warning | healthy | info`) match between Rust (`core::ini_diagnostics::Severity`), Tauri service types, frontend stores, and view templates. `CheckOutcome` status values (`healthy | warning | critical | na | offline | unknown`) match between Rust + frontend.
- [ ] **Selectors preserved:** `data-base-modal`, `data-status-badge`, etc. unchanged on rewritten views. New components add new `data-*` selectors for their tests.
- [ ] **Stores untouched:** `useMachinesStore`, `useDiscoveryStore`, `useCredentialsStore`, `useSharesStore`, `useBatchStore`, `useClusterStore`, `useTasksStore` — Plan 4 ONLY adds `useDiagnosticsStore` + `useHealthCheckStore`.
- [ ] **Routes intact:** all 8 existing routes still resolve (Plan 4 modifies `INIScanner.vue` and `HealthCheck.vue` in place).
- [ ] **Cluster store integration:** `useClusterStore.score` should reflect Health Check results once they exist. After T17, extend `cluster.ts` to also factor in latest health-check critical/warning counts (additive, does not break Plan 3 test).

---


## Task 1: Pre-flight audit (no new code)

**Files:** none modified.

- [ ] **Step 1: Confirm Plan 3 baseline green**

```bash
export PATH="/Users/bip.lan/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
pnpm test 2>&1 | tail -10
cd src-tauri && cargo test --lib 2>&1 | tail -10 && cd ..
```

Expected: ~80 frontend + ~70 backend tests pass (Plan 3 final state). If anything fails, **STOP** and report — Plan 4 must start from a green baseline.

- [ ] **Step 2: Confirm `core::ini_editor::set_key_with_credential` exists and writes atomically**

```bash
grep -n "set_key_with_credential" src-tauri/src/core/ini_editor.rs
```

Expected: function present (was added in Plan 3). Plan 4 reuses it for "Apply finding".

- [ ] **Step 3: Confirm `core::batch::run_batch` is reusable**

```bash
grep -n "pub fn run_batch\|pub async fn run_batch" src-tauri/src/core/batch.rs
```

Expected: a generic fan-out function exists. Plan 4 reuses it for the cluster Health Check.

If either Step 2 or Step 3 reveals a missing/changed signature, document the actual signature in a follow-up commit before writing Task 2.

- [ ] **Step 4: No commit (audit only).**

---

## Task 2: SQLite migration 007 — `scan_runs`, `ini_findings`, `health_check_runs`

**Files:**
- Modify: `src-tauri/src/data/schema.rs`

- [ ] **Step 1: Append migration 007 to `MIGRATIONS`**

```rust
(
    "007_diagnostics_tables",
    r#"
    CREATE TABLE IF NOT EXISTS scan_runs (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        scan_type TEXT NOT NULL,                    -- "ini" | "health"
        started_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        finished_at TEXT,
        machine_ids_json TEXT NOT NULL,             -- JSON array of machine ids in scope
        summary_json TEXT                           -- JSON: {critical, warning, healthy, total, ...}
    );
    CREATE INDEX IF NOT EXISTS idx_scan_runs_type_started ON scan_runs(scan_type, started_at DESC);

    CREATE TABLE IF NOT EXISTS ini_findings (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        scan_run_id INTEGER NOT NULL,
        machine_id INTEGER NOT NULL,
        rule_id TEXT NOT NULL,                      -- e.g. "R001"
        severity TEXT NOT NULL,                     -- "critical" | "warning" | "healthy" | "info"
        category TEXT NOT NULL,                     -- "project" | "user" | "engine"
        file_path TEXT NOT NULL,                    -- absolute path on the machine
        section TEXT,                               -- INI [section]
        key_name TEXT,                              -- when applicable
        line_number INTEGER,                        -- 1-based, null if N/A
        snippet_before TEXT NOT NULL,               -- multi-line excerpt
        snippet_after TEXT,                         -- suggested fix (null when remove-only)
        recommended_action TEXT NOT NULL,           -- "set" | "remove" | "manual"
        recommended_value TEXT,                     -- payload for "set"
        symptom TEXT NOT NULL,                      -- user-facing description
        rationale TEXT NOT NULL,                    -- "why" explanation
        fixed_at TEXT,                              -- non-null when applied
        skipped_at TEXT,                            -- non-null when user skipped
        FOREIGN KEY (scan_run_id) REFERENCES scan_runs(id) ON DELETE CASCADE,
        FOREIGN KEY (machine_id) REFERENCES machines(id) ON DELETE CASCADE
    );
    CREATE INDEX IF NOT EXISTS idx_ini_findings_run ON ini_findings(scan_run_id);
    CREATE INDEX IF NOT EXISTS idx_ini_findings_machine ON ini_findings(machine_id);
    CREATE INDEX IF NOT EXISTS idx_ini_findings_severity ON ini_findings(severity);

    CREATE TABLE IF NOT EXISTS health_check_runs (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        scan_run_id INTEGER NOT NULL,
        machine_id INTEGER NOT NULL,
        machine_results_json TEXT NOT NULL,         -- JSON: {check_id: {status, message, sample_output}}
        FOREIGN KEY (scan_run_id) REFERENCES scan_runs(id) ON DELETE CASCADE,
        FOREIGN KEY (machine_id) REFERENCES machines(id) ON DELETE CASCADE
    );
    CREATE INDEX IF NOT EXISTS idx_health_check_runs_run ON health_check_runs(scan_run_id);
    CREATE INDEX IF NOT EXISTS idx_health_check_runs_machine ON health_check_runs(machine_id);
    "#,
),
```

- [ ] **Step 2: Add three migration tests**

In `src-tauri/src/data/schema.rs` test module, add:

```rust
#[test]
fn migrate_creates_scan_runs_table() {
    let db = open_in_memory().unwrap();
    let mut conn = db.lock().unwrap();
    migrate(&mut conn).unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='scan_runs'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn migrate_creates_ini_findings_table() {
    let db = open_in_memory().unwrap();
    let mut conn = db.lock().unwrap();
    migrate(&mut conn).unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='ini_findings'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn migrate_creates_health_check_runs_table() {
    let db = open_in_memory().unwrap();
    let mut conn = db.lock().unwrap();
    migrate(&mut conn).unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='health_check_runs'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}
```

- [ ] **Step 3: Run tests to verify FAIL → PASS**

```bash
cd src-tauri && cargo test --lib data::schema 2>&1 | tail -10 && cd ..
```

Expected: 3 new tests PASS plus the 7 existing schema tests still PASS (10 total).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/data/schema.rs
git commit -m "feat(data): add migration 007 for diagnostics tables (scan_runs, ini_findings, health_check_runs)"
```

---


## Task 3: Data layer — `data::scan_runs`

**Files:**
- Create: `src-tauri/src/data/scan_runs.rs`
- Modify: `src-tauri/src/data/mod.rs`

- [ ] **Step 1: Write failing test in `data/scan_runs.rs`**

```rust
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

    #[test]
    fn insert_returns_new_id_with_started_at() {
        let db = setup();
        let id = insert(&db, "ini", &[1, 2, 3]).unwrap();
        assert!(id > 0);
        let row = find_by_id(&db, id).unwrap().unwrap();
        assert_eq!(row.scan_type, "ini");
        assert_eq!(row.machine_ids, vec![1, 2, 3]);
        assert!(row.started_at.is_some());
        assert!(row.finished_at.is_none());
    }

    #[test]
    fn finish_updates_summary_and_finished_at() {
        let db = setup();
        let id = insert(&db, "ini", &[1]).unwrap();
        finish(&db, id, &serde_json::json!({"critical": 0, "warning": 1, "healthy": 4})).unwrap();
        let row = find_by_id(&db, id).unwrap().unwrap();
        assert!(row.finished_at.is_some());
        let summary = row.summary.as_ref().unwrap();
        assert_eq!(summary["warning"], 1);
    }

    #[test]
    fn list_recent_returns_descending() {
        let db = setup();
        let _a = insert(&db, "ini", &[1]).unwrap();
        let b = insert(&db, "health", &[1]).unwrap();
        let recent = list_recent(&db, "health", 10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].id, Some(b));
    }
}
```

- [ ] **Step 2: Run test → confirm it fails (file does not exist yet)**

```bash
cd src-tauri && cargo test --lib data::scan_runs 2>&1 | tail -8 && cd ..
```

- [ ] **Step 3: Implement `data/scan_runs.rs`**

```rust
//! CRUD for the `scan_runs` table. Each row is one INI-scan-or-health-check session.

use crate::data::Db;
use crate::error::UecmResult;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScanRun {
    pub id: Option<i64>,
    pub scan_type: String,             // "ini" | "health"
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub machine_ids: Vec<i64>,
    pub summary: Option<JsonValue>,
}

pub fn insert(db: &Db, scan_type: &str, machine_ids: &[i64]) -> UecmResult<i64> {
    let conn = db.lock().unwrap();
    let machine_ids_json = serde_json::to_string(machine_ids)
        .map_err(|e| crate::error::UecmError::OperationFailed(e.to_string()))?;
    conn.execute(
        "INSERT INTO scan_runs (scan_type, machine_ids_json) VALUES (?, ?)",
        params![scan_type, machine_ids_json],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn finish(db: &Db, id: i64, summary: &JsonValue) -> UecmResult<()> {
    let conn = db.lock().unwrap();
    let summary_json = serde_json::to_string(summary)
        .map_err(|e| crate::error::UecmError::OperationFailed(e.to_string()))?;
    conn.execute(
        "UPDATE scan_runs SET finished_at = CURRENT_TIMESTAMP, summary_json = ? WHERE id = ?",
        params![summary_json, id],
    )?;
    Ok(())
}

pub fn find_by_id(db: &Db, id: i64) -> UecmResult<Option<ScanRun>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, scan_type, started_at, finished_at, machine_ids_json, summary_json
         FROM scan_runs WHERE id = ?",
    )?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row_to_scan_run(row)?))
    } else {
        Ok(None)
    }
}

pub fn list_recent(db: &Db, scan_type: &str, limit: i64) -> UecmResult<Vec<ScanRun>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, scan_type, started_at, finished_at, machine_ids_json, summary_json
         FROM scan_runs WHERE scan_type = ? ORDER BY started_at DESC LIMIT ?",
    )?;
    let rows = stmt.query_map(params![scan_type, limit], |row| Ok(row_to_scan_run(row)))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r??);
    }
    Ok(out)
}

fn row_to_scan_run(row: &rusqlite::Row) -> rusqlite::Result<UecmResult<ScanRun>> {
    let id: i64 = row.get(0)?;
    let scan_type: String = row.get(1)?;
    let started_at: Option<String> = row.get(2)?;
    let finished_at: Option<String> = row.get(3)?;
    let ids_json: String = row.get(4)?;
    let summary_json: Option<String> = row.get(5)?;
    Ok(Ok(ScanRun {
        id: Some(id),
        scan_type,
        started_at,
        finished_at,
        machine_ids: serde_json::from_str(&ids_json)
            .map_err(|e| crate::error::UecmError::OperationFailed(e.to_string()))?,
        summary: match summary_json {
            Some(s) => Some(serde_json::from_str(&s)
                .map_err(|e| crate::error::UecmError::OperationFailed(e.to_string()))?),
            None => None,
        },
    }))
}
```

Note: `row_to_scan_run` returns `rusqlite::Result<UecmResult<ScanRun>>` so the JSON deserialization error can propagate. The double-unwrap pattern (`r??`) inside `list_recent` flattens it. If you prefer simpler signatures, refactor to inline the JSON parse; either is fine.

- [ ] **Step 4: Update `data/mod.rs`**

```rust
pub mod scan_runs;
// ... add `pub use scan_runs::ScanRun;` to the public exports section.
```

- [ ] **Step 5: Run tests → PASS**

```bash
cd src-tauri && cargo test --lib data::scan_runs 2>&1 | tail -8 && cd ..
```

Expected: 3 PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/data
git commit -m "feat(data): scan_runs CRUD (insert/finish/find_by_id/list_recent)"
```

---

## Task 4: Data layer — `data::ini_findings`

**Files:**
- Create: `src-tauri/src/data/ini_findings.rs`
- Modify: `src-tauri/src/data/mod.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{open_in_memory, schema, scan_runs};
    use crate::data::machines::{insert as insert_machine, Machine};

    fn setup() -> (Db, i64, i64) {
        let db = open_in_memory().unwrap();
        {
            let mut conn = db.lock().unwrap();
            schema::migrate(&mut conn).unwrap();
        }
        let machine_id = insert_machine(&db, &Machine::new("RENDER-01", "192.168.10.21")).unwrap();
        let scan_id = scan_runs::insert(&db, "ini", &[machine_id]).unwrap();
        (db, scan_id, machine_id)
    }

    fn sample(scan_id: i64, machine_id: i64) -> IniFinding {
        IniFinding {
            id: None,
            scan_run_id: scan_id,
            machine_id,
            rule_id: "R001".into(),
            severity: "critical".into(),
            category: "project".into(),
            file_path: "C:\\Project\\Config\\DefaultEngine.ini".into(),
            section: Some("/Script/UnrealEd.DerivedDataCacheSettings".into()),
            key_name: Some("Path".into()),
            line_number: Some(42),
            snippet_before: "Path=D:\\OldDDC".into(),
            snippet_after: Some("EnvPathOverride=UE-SharedDataCachePath".into()),
            recommended_action: "set".into(),
            recommended_value: Some("UE-SharedDataCachePath".into()),
            symptom: "DDC silently falls back to local".into(),
            rationale: "Hardcoded path overrides env var".into(),
            fixed_at: None,
            skipped_at: None,
        }
    }

    #[test]
    fn insert_assigns_id() {
        let (db, scan_id, machine_id) = setup();
        let id = insert(&db, &sample(scan_id, machine_id)).unwrap();
        assert!(id > 0);
    }

    #[test]
    fn list_for_run_returns_inserted_rows() {
        let (db, scan_id, machine_id) = setup();
        insert(&db, &sample(scan_id, machine_id)).unwrap();
        insert(&db, &sample(scan_id, machine_id)).unwrap();
        let rows = list_for_run(&db, scan_id).unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn mark_fixed_sets_timestamp() {
        let (db, scan_id, machine_id) = setup();
        let id = insert(&db, &sample(scan_id, machine_id)).unwrap();
        mark_fixed(&db, id).unwrap();
        let row = find_by_id(&db, id).unwrap().unwrap();
        assert!(row.fixed_at.is_some());
    }

    #[test]
    fn mark_skipped_sets_timestamp() {
        let (db, scan_id, machine_id) = setup();
        let id = insert(&db, &sample(scan_id, machine_id)).unwrap();
        mark_skipped(&db, id).unwrap();
        let row = find_by_id(&db, id).unwrap().unwrap();
        assert!(row.skipped_at.is_some());
    }

    #[test]
    fn count_by_severity_for_machine_returns_critical_count() {
        let (db, scan_id, machine_id) = setup();
        let mut critical = sample(scan_id, machine_id);
        critical.severity = "critical".into();
        insert(&db, &critical).unwrap();
        let mut warning = sample(scan_id, machine_id);
        warning.severity = "warning".into();
        insert(&db, &warning).unwrap();
        let counts = count_by_severity_for_machine(&db, scan_id, machine_id).unwrap();
        assert_eq!(counts.critical, 1);
        assert_eq!(counts.warning, 1);
    }
}
```

- [ ] **Step 2: Implement `data/ini_findings.rs`**

```rust
//! CRUD for the `ini_findings` table. Each row is a single diagnostic produced
//! by `core::ini_diagnostics`. Findings are immutable once inserted; the only
//! mutations are `mark_fixed` / `mark_skipped` which stamp a timestamp.

use crate::data::Db;
use crate::error::UecmResult;
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IniFinding {
    pub id: Option<i64>,
    pub scan_run_id: i64,
    pub machine_id: i64,
    pub rule_id: String,
    pub severity: String,
    pub category: String,
    pub file_path: String,
    pub section: Option<String>,
    pub key_name: Option<String>,
    pub line_number: Option<i64>,
    pub snippet_before: String,
    pub snippet_after: Option<String>,
    pub recommended_action: String,
    pub recommended_value: Option<String>,
    pub symptom: String,
    pub rationale: String,
    pub fixed_at: Option<String>,
    pub skipped_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SeverityCounts {
    pub critical: i64,
    pub warning: i64,
    pub healthy: i64,
}

pub fn insert(db: &Db, f: &IniFinding) -> UecmResult<i64> {
    let conn = db.lock().unwrap();
    conn.execute(
        "INSERT INTO ini_findings
         (scan_run_id, machine_id, rule_id, severity, category, file_path,
          section, key_name, line_number, snippet_before, snippet_after,
          recommended_action, recommended_value, symptom, rationale)
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
        params![
            f.scan_run_id, f.machine_id, f.rule_id, f.severity, f.category,
            f.file_path, f.section, f.key_name, f.line_number,
            f.snippet_before, f.snippet_after,
            f.recommended_action, f.recommended_value,
            f.symptom, f.rationale
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn find_by_id(db: &Db, id: i64) -> UecmResult<Option<IniFinding>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(SELECT_SQL_BY_ID)?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row_to_finding(row)?))
    } else {
        Ok(None)
    }
}

pub fn list_for_run(db: &Db, scan_run_id: i64) -> UecmResult<Vec<IniFinding>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(SELECT_SQL_BY_RUN)?;
    let rows = stmt.query_map(params![scan_run_id], |row| Ok(row_to_finding(row)))?;
    let mut out = Vec::new();
    for r in rows { out.push(r??); }
    Ok(out)
}

pub fn mark_fixed(db: &Db, id: i64) -> UecmResult<()> {
    let conn = db.lock().unwrap();
    conn.execute(
        "UPDATE ini_findings SET fixed_at = CURRENT_TIMESTAMP WHERE id = ?",
        params![id],
    )?;
    Ok(())
}

pub fn mark_skipped(db: &Db, id: i64) -> UecmResult<()> {
    let conn = db.lock().unwrap();
    conn.execute(
        "UPDATE ini_findings SET skipped_at = CURRENT_TIMESTAMP WHERE id = ?",
        params![id],
    )?;
    Ok(())
}

pub fn count_by_severity_for_machine(
    db: &Db,
    scan_run_id: i64,
    machine_id: i64,
) -> UecmResult<SeverityCounts> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT severity, COUNT(*) FROM ini_findings
         WHERE scan_run_id = ? AND machine_id = ? AND fixed_at IS NULL AND skipped_at IS NULL
         GROUP BY severity",
    )?;
    let mut counts = SeverityCounts::default();
    let rows = stmt.query_map(params![scan_run_id, machine_id], |row| {
        let sev: String = row.get(0)?;
        let n: i64 = row.get(1)?;
        Ok((sev, n))
    })?;
    for r in rows {
        let (sev, n) = r?;
        match sev.as_str() {
            "critical" => counts.critical = n,
            "warning" => counts.warning = n,
            "healthy" => counts.healthy = n,
            _ => {}
        }
    }
    Ok(counts)
}

const SELECT_COLS: &str = "id, scan_run_id, machine_id, rule_id, severity, category, \
    file_path, section, key_name, line_number, snippet_before, snippet_after, \
    recommended_action, recommended_value, symptom, rationale, fixed_at, skipped_at";

const SELECT_SQL_BY_ID: &str = const_format::concatcp!(
    "SELECT ", SELECT_COLS, " FROM ini_findings WHERE id = ?"
);
const SELECT_SQL_BY_RUN: &str = const_format::concatcp!(
    "SELECT ", SELECT_COLS, " FROM ini_findings WHERE scan_run_id = ? \
     ORDER BY machine_id, severity, file_path"
);

fn row_to_finding(row: &rusqlite::Row) -> rusqlite::Result<UecmResult<IniFinding>> {
    Ok(Ok(IniFinding {
        id: Some(row.get(0)?),
        scan_run_id: row.get(1)?,
        machine_id: row.get(2)?,
        rule_id: row.get(3)?,
        severity: row.get(4)?,
        category: row.get(5)?,
        file_path: row.get(6)?,
        section: row.get(7)?,
        key_name: row.get(8)?,
        line_number: row.get(9)?,
        snippet_before: row.get(10)?,
        snippet_after: row.get(11)?,
        recommended_action: row.get(12)?,
        recommended_value: row.get(13)?,
        symptom: row.get(14)?,
        rationale: row.get(15)?,
        fixed_at: row.get(16)?,
        skipped_at: row.get(17)?,
    }))
}
```

If you'd rather avoid `const_format` for two strings, inline the SELECT statements directly — it's cosmetic. If you do keep it, add `const_format = "0.2"` to `src-tauri/Cargo.toml` under `[dependencies]`.

- [ ] **Step 3: Update `data/mod.rs`**

```rust
pub mod ini_findings;
pub use ini_findings::{IniFinding, SeverityCounts};
```

- [ ] **Step 4: Tests PASS**

```bash
cd src-tauri && cargo test --lib data::ini_findings 2>&1 | tail -8 && cd ..
```

Expected: 5 PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/data Cargo.toml src-tauri/Cargo.toml
git commit -m "feat(data): ini_findings CRUD + severity counts"
```

---

## Task 5: Data layer — `data::health_check_runs`

**Files:**
- Create: `src-tauri/src/data/health_check_runs.rs`
- Modify: `src-tauri/src/data/mod.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{open_in_memory, schema, scan_runs};
    use crate::data::machines::{insert as insert_machine, Machine};
    use serde_json::json;

    fn setup() -> (Db, i64, i64) {
        let db = open_in_memory().unwrap();
        {
            let mut conn = db.lock().unwrap();
            schema::migrate(&mut conn).unwrap();
        }
        let mid = insert_machine(&db, &Machine::new("RENDER-01", "192.168.10.21")).unwrap();
        let sid = scan_runs::insert(&db, "health", &[mid]).unwrap();
        (db, sid, mid)
    }

    #[test]
    fn upsert_inserts_on_first_call() {
        let (db, sid, mid) = setup();
        let payload = json!({"smb": {"status": "healthy", "message": "ok"}});
        upsert(&db, sid, mid, &payload).unwrap();
        let row = find(&db, sid, mid).unwrap().unwrap();
        assert_eq!(row.machine_results["smb"]["status"], "healthy");
    }

    #[test]
    fn upsert_replaces_on_second_call() {
        let (db, sid, mid) = setup();
        upsert(&db, sid, mid, &json!({"smb": {"status": "warning"}})).unwrap();
        upsert(&db, sid, mid, &json!({"smb": {"status": "critical"}})).unwrap();
        let row = find(&db, sid, mid).unwrap().unwrap();
        assert_eq!(row.machine_results["smb"]["status"], "critical");
    }

    #[test]
    fn list_for_run_returns_one_row_per_machine() {
        let (db, sid, mid1) = setup();
        let mid2 = insert_machine(&db, &Machine::new("RENDER-02", "192.168.10.22")).unwrap();
        upsert(&db, sid, mid1, &json!({"smb": {"status": "healthy"}})).unwrap();
        upsert(&db, sid, mid2, &json!({"smb": {"status": "critical"}})).unwrap();
        let rows = list_for_run(&db, sid).unwrap();
        assert_eq!(rows.len(), 2);
    }
}
```

- [ ] **Step 2: Implement `data/health_check_runs.rs`**

```rust
//! Per-machine results within a single health-check `scan_runs` session.

use crate::data::Db;
use crate::error::{UecmError, UecmResult};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthCheckRow {
    pub scan_run_id: i64,
    pub machine_id: i64,
    pub machine_results: JsonValue,
}

pub fn upsert(db: &Db, scan_run_id: i64, machine_id: i64, results: &JsonValue) -> UecmResult<()> {
    let conn = db.lock().unwrap();
    let json = serde_json::to_string(results)
        .map_err(|e| UecmError::OperationFailed(e.to_string()))?;
    conn.execute(
        "INSERT INTO health_check_runs (scan_run_id, machine_id, machine_results_json)
         VALUES (?, ?, ?)
         ON CONFLICT(scan_run_id, machine_id)
         DO UPDATE SET machine_results_json = excluded.machine_results_json",
        params![scan_run_id, machine_id, json],
    )?;
    Ok(())
}

pub fn find(db: &Db, scan_run_id: i64, machine_id: i64) -> UecmResult<Option<HealthCheckRow>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT scan_run_id, machine_id, machine_results_json
         FROM health_check_runs WHERE scan_run_id = ? AND machine_id = ?",
    )?;
    let mut rows = stmt.query(params![scan_run_id, machine_id])?;
    if let Some(row) = rows.next()? {
        let json: String = row.get(2)?;
        Ok(Some(HealthCheckRow {
            scan_run_id: row.get(0)?,
            machine_id: row.get(1)?,
            machine_results: serde_json::from_str(&json)
                .map_err(|e| UecmError::OperationFailed(e.to_string()))?,
        }))
    } else {
        Ok(None)
    }
}

pub fn list_for_run(db: &Db, scan_run_id: i64) -> UecmResult<Vec<HealthCheckRow>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT scan_run_id, machine_id, machine_results_json
         FROM health_check_runs WHERE scan_run_id = ? ORDER BY machine_id",
    )?;
    let rows = stmt.query_map(params![scan_run_id], |row| {
        let json: String = row.get(2)?;
        Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, json))
    })?;
    let mut out = Vec::new();
    for r in rows {
        let (sid, mid, json) = r?;
        out.push(HealthCheckRow {
            scan_run_id: sid,
            machine_id: mid,
            machine_results: serde_json::from_str(&json)
                .map_err(|e| UecmError::OperationFailed(e.to_string()))?,
        });
    }
    Ok(out)
}
```

The ON CONFLICT clause needs a unique index on `(scan_run_id, machine_id)`. Add to migration 007 in Task 2 if not already present:

```sql
CREATE UNIQUE INDEX IF NOT EXISTS uq_health_check_runs_run_machine
    ON health_check_runs(scan_run_id, machine_id);
```

If you missed it in Task 2, add a migration 007a now to avoid editing applied SQL.

- [ ] **Step 3: Update `data/mod.rs`**

```rust
pub mod health_check_runs;
pub use health_check_runs::HealthCheckRow;
```

- [ ] **Step 4: Tests PASS**

```bash
cd src-tauri && cargo test --lib data::health_check_runs 2>&1 | tail -8 && cd ..
```

Expected: 3 PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/data
git commit -m "feat(data): health_check_runs CRUD with upsert per (run, machine)"
```

---


## Task 6: `core::ini_diagnostics` — pure rule engine (TDD core)

**Files:**
- Create: `src-tauri/src/core/ini_diagnostics.rs`
- Modify: `src-tauri/src/core/mod.rs`

This is the heart of Plan 4: pure-Rust rule engine, fully unit-testable on macOS. Every rule (R001-R007) gets at least one test before its implementation.

- [ ] **Step 1: Write failing tests for the data structures + R001 (hardcoded Path without EnvPathOverride)**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn ddc_section(keys: &[(&str, &str)]) -> ParsedSection {
        ParsedSection {
            name: "/Script/UnrealEd.DerivedDataCacheSettings".into(),
            keys: keys.iter().map(|(k, v)| ParsedKey {
                name: k.to_string(),
                value: v.to_string(),
                line_number: 0,
            }).collect(),
        }
    }

    #[test]
    fn r001_critical_when_path_set_without_envpathoverride() {
        let file = ParsedFile {
            path: "C:\\Project\\Config\\DefaultEngine.ini".into(),
            category: Category::Project,
            sections: vec![ddc_section(&[("Path", "D:\\OldDDC")])],
        };
        let env_state = EnvVarState::default();
        let findings = run_rules(&file, &env_state);
        assert!(findings.iter().any(|f| f.rule_id == "R001" && f.severity == Severity::Critical));
    }

    #[test]
    fn r001_healthy_when_envpathoverride_set_and_envvar_present() {
        let file = ParsedFile {
            path: "C:\\Project\\Config\\DefaultEngine.ini".into(),
            category: Category::Project,
            sections: vec![ddc_section(&[("EnvPathOverride", "UE-SharedDataCachePath")])],
        };
        let mut env_state = EnvVarState::default();
        env_state.shared_data_cache_path = Some("\\\\HOST\\DDC".into());
        let findings = run_rules(&file, &env_state);
        assert!(findings.iter().any(|f| f.rule_id == "R007" && f.severity == Severity::Healthy));
    }

    #[test]
    fn r002_critical_when_user_level_file_has_ddc_section() {
        let file = ParsedFile {
            path: "C:\\Users\\X\\AppData\\Local\\UnrealEngine\\5.4\\Saved\\Config\\WindowsEditor\\EditorPerProjectUserSettings.ini".into(),
            category: Category::User,
            sections: vec![ddc_section(&[("Path", "C:\\local")])],
        };
        let findings = run_rules(&file, &EnvVarState::default());
        assert!(findings.iter().any(|f| f.rule_id == "R002" && f.severity == Severity::Critical));
    }

    #[test]
    fn r004_warning_when_path_uses_drive_letter() {
        let file = ParsedFile {
            path: "C:\\Project\\Config\\DefaultEngine.ini".into(),
            category: Category::Project,
            sections: vec![ddc_section(&[("Path", "Z:\\DDC")])],
        };
        let findings = run_rules(&file, &EnvVarState::default());
        assert!(findings.iter().any(|f| f.rule_id == "R004" && f.severity == Severity::Warning));
    }

    #[test]
    fn r005_warning_when_deprecated_cvar_present() {
        let file = ParsedFile {
            path: "C:\\Project\\Config\\ConsoleVariables.ini".into(),
            category: Category::Project,
            sections: vec![ParsedSection {
                name: "Startup".into(),
                keys: vec![ParsedKey {
                    name: "r.SShaderCache".into(),
                    value: "1".into(),
                    line_number: 12,
                }],
            }],
        };
        let findings = run_rules(&file, &EnvVarState::default());
        assert!(findings.iter().any(|f| f.rule_id == "R005" && f.severity == Severity::Warning));
    }

    #[test]
    fn r006_warning_when_envoverride_set_but_envvar_empty() {
        let file = ParsedFile {
            path: "C:\\Project\\Config\\DefaultEngine.ini".into(),
            category: Category::Project,
            sections: vec![ddc_section(&[("EnvPathOverride", "UE-SharedDataCachePath")])],
        };
        let env_state = EnvVarState::default(); // shared_data_cache_path = None
        let findings = run_rules(&file, &env_state);
        assert!(findings.iter().any(|f| f.rule_id == "R006" && f.severity == Severity::Warning));
    }
}
```

- [ ] **Step 2: Implement `core/ini_diagnostics.rs`**

```rust
//! Pure rule engine. Takes a parsed INI file + env-var state, emits findings.
//! No Windows-specific calls; runs and tests on every platform.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    Warning,
    Healthy,
    Info,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Critical => "critical",
            Severity::Warning => "warning",
            Severity::Healthy => "healthy",
            Severity::Info => "info",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Project,
    User,
    Engine,
}

impl Category {
    pub fn as_str(&self) -> &'static str {
        match self {
            Category::Project => "project",
            Category::User => "user",
            Category::Engine => "engine",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedKey {
    pub name: String,
    pub value: String,
    pub line_number: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSection {
    pub name: String,
    pub keys: Vec<ParsedKey>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedFile {
    pub path: String,
    pub category: Category,
    pub sections: Vec<ParsedSection>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct EnvVarState {
    pub shared_data_cache_path: Option<String>,
    pub local_data_cache_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub rule_id: String,
    pub severity: Severity,
    pub category: Category,
    pub file_path: String,
    pub section: Option<String>,
    pub key_name: Option<String>,
    pub line_number: Option<i64>,
    pub snippet_before: String,
    pub snippet_after: Option<String>,
    pub recommended_action: RecommendedAction,
    pub recommended_value: Option<String>,
    pub symptom: String,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecommendedAction {
    Set,
    Remove,
    Manual,
}

impl RecommendedAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            RecommendedAction::Set => "set",
            RecommendedAction::Remove => "remove",
            RecommendedAction::Manual => "manual",
        }
    }
}

const DDC_SECTION: &str = "/Script/UnrealEd.DerivedDataCacheSettings";

pub const DEPRECATED_CVARS: &[&str] = &[
    "r.SShaderCache",
    "r.ShaderCache",                          // pre-UE5 FShaderCache
    "s.SkipFinalizeCommandList",
    "r.UseShaderCaching",
];

pub fn run_rules(file: &ParsedFile, env: &EnvVarState) -> Vec<Finding> {
    let mut out = Vec::new();
    out.extend(rule_r001(file));
    out.extend(rule_r002(file));
    out.extend(rule_r004(file));
    out.extend(rule_r005(file));
    out.extend(rule_r006(file, env));
    out.extend(rule_r007(file, env));
    // R003 (path unreachable) is invoked separately by the scanner because it
    // requires a network probe; not part of the pure rule engine.
    out
}

fn find_ddc(file: &ParsedFile) -> Option<&ParsedSection> {
    file.sections.iter().find(|s| s.name == DDC_SECTION)
}

fn key<'a>(section: &'a ParsedSection, name: &str) -> Option<&'a ParsedKey> {
    section.keys.iter().find(|k| k.name.eq_ignore_ascii_case(name))
}

fn rule_r001(file: &ParsedFile) -> Vec<Finding> {
    let Some(section) = find_ddc(file) else { return vec![]; };
    let path_key = key(section, "Path");
    let env_override = key(section, "EnvPathOverride");
    if path_key.is_some() && env_override.is_none() {
        let pk = path_key.unwrap();
        return vec![Finding {
            rule_id: "R001".into(),
            severity: Severity::Critical,
            category: file.category,
            file_path: file.path.clone(),
            section: Some(section.name.clone()),
            key_name: Some(pk.name.clone()),
            line_number: Some(pk.line_number as i64),
            snippet_before: format!("Path={}", pk.value),
            snippet_after: Some("EnvPathOverride=UE-SharedDataCachePath".into()),
            recommended_action: RecommendedAction::Set,
            recommended_value: Some("UE-SharedDataCachePath".into()),
            symptom: "DDC silently uses the hardcoded path; env-var overrides are ignored.".into(),
            rationale: "When `Path=` is set without `EnvPathOverride`, UE skips the env-var lookup. The cluster cannot share DDC.".into(),
        }];
    }
    vec![]
}

fn rule_r002(file: &ParsedFile) -> Vec<Finding> {
    if file.category != Category::User { return vec![]; }
    let Some(section) = find_ddc(file) else { return vec![]; };
    if section.keys.is_empty() { return vec![]; }
    vec![Finding {
        rule_id: "R002".into(),
        severity: Severity::Critical,
        category: file.category,
        file_path: file.path.clone(),
        section: Some(section.name.clone()),
        key_name: None,
        line_number: section.keys.first().map(|k| k.line_number as i64),
        snippet_before: section.keys.iter()
            .map(|k| format!("{}={}", k.name, k.value))
            .collect::<Vec<_>>()
            .join("\n"),
        snippet_after: Some("(remove the entire DDC section from this user-level file)".into()),
        recommended_action: RecommendedAction::Remove,
        recommended_value: None,
        symptom: "User-level DDC override silently overrides project + env-var configs.".into(),
        rationale: "EditorPerProjectUserSettings.ini is the highest-priority DDC source. Any DDC keys here will mask the cluster setup.".into(),
    }]
}

fn rule_r004(file: &ParsedFile) -> Vec<Finding> {
    let Some(section) = find_ddc(file) else { return vec![]; };
    let mut out = Vec::new();
    for k in &section.keys {
        if !k.name.eq_ignore_ascii_case("Path") { continue; }
        let v = k.value.trim();
        let starts_with_drive = v.len() >= 2
            && v.chars().nth(1) == Some(':')
            && v.chars().next().map_or(false, |c| c.is_ascii_alphabetic());
        let is_unc = v.starts_with("\\\\");
        if starts_with_drive && !is_unc {
            out.push(Finding {
                rule_id: "R004".into(),
                severity: Severity::Warning,
                category: file.category,
                file_path: file.path.clone(),
                section: Some(section.name.clone()),
                key_name: Some(k.name.clone()),
                line_number: Some(k.line_number as i64),
                snippet_before: format!("Path={}", v),
                snippet_after: Some("Path=\\\\HOST\\Share\\...".into()),
                recommended_action: RecommendedAction::Manual,
                recommended_value: None,
                symptom: "Mapped drive letters are not visible to Windows Services (e.g. RenderStream).".into(),
                rationale: "Use UNC paths so SYSTEM-context processes can resolve the share.".into(),
            });
        }
    }
    out
}

fn rule_r005(file: &ParsedFile) -> Vec<Finding> {
    let mut out = Vec::new();
    for s in &file.sections {
        for k in &s.keys {
            if DEPRECATED_CVARS.iter().any(|d| d.eq_ignore_ascii_case(&k.name)) {
                out.push(Finding {
                    rule_id: "R005".into(),
                    severity: Severity::Warning,
                    category: file.category,
                    file_path: file.path.clone(),
                    section: Some(s.name.clone()),
                    key_name: Some(k.name.clone()),
                    line_number: Some(k.line_number as i64),
                    snippet_before: format!("{}={}", k.name, k.value),
                    snippet_after: Some("(remove this line)".into()),
                    recommended_action: RecommendedAction::Remove,
                    recommended_value: None,
                    symptom: "Deprecated CVar that no longer functions in UE 5.x.".into(),
                    rationale: format!("`{}` was removed; keeping it adds confusion at no benefit.", k.name),
                });
            }
        }
    }
    out
}

fn rule_r006(file: &ParsedFile, env: &EnvVarState) -> Vec<Finding> {
    let Some(section) = find_ddc(file) else { return vec![]; };
    let Some(envk) = key(section, "EnvPathOverride") else { return vec![]; };
    let v = envk.value.trim();
    let referenced_present = match v {
        "UE-SharedDataCachePath" => env.shared_data_cache_path.as_ref().is_some(),
        "UE-LocalDataCachePath" => env.local_data_cache_path.as_ref().is_some(),
        _ => true, // unknown var name; skip warning
    };
    if !referenced_present {
        return vec![Finding {
            rule_id: "R006".into(),
            severity: Severity::Warning,
            category: file.category,
            file_path: file.path.clone(),
            section: Some(section.name.clone()),
            key_name: Some(envk.name.clone()),
            line_number: Some(envk.line_number as i64),
            snippet_before: format!("EnvPathOverride={}", v),
            snippet_after: Some(format!("(set environment variable `{}` on this machine)", v)),
            recommended_action: RecommendedAction::Manual,
            recommended_value: None,
            symptom: "INI references an env var that is not set; DDC falls back to local.".into(),
            rationale: format!("`{}` is not present on this machine. Use UECM env-var modal to set it.", v),
        }];
    }
    vec![]
}

fn rule_r007(file: &ParsedFile, env: &EnvVarState) -> Vec<Finding> {
    let Some(section) = find_ddc(file) else { return vec![]; };
    let Some(envk) = key(section, "EnvPathOverride") else { return vec![]; };
    let referenced_present = match envk.value.trim() {
        "UE-SharedDataCachePath" => env.shared_data_cache_path.is_some(),
        "UE-LocalDataCachePath" => env.local_data_cache_path.is_some(),
        _ => false,
    };
    if !referenced_present { return vec![]; }
    vec![Finding {
        rule_id: "R007".into(),
        severity: Severity::Healthy,
        category: file.category,
        file_path: file.path.clone(),
        section: Some(section.name.clone()),
        key_name: Some(envk.name.clone()),
        line_number: Some(envk.line_number as i64),
        snippet_before: format!("EnvPathOverride={}", envk.value),
        snippet_after: None,
        recommended_action: RecommendedAction::Manual,
        recommended_value: None,
        symptom: "Configured correctly. Tracked for healthy-count summary.".into(),
        rationale: "EnvPathOverride references a populated env var on this machine.".into(),
    }]
}
```

- [ ] **Step 3: Update `core/mod.rs`**

```rust
pub mod ini_diagnostics;
```

- [ ] **Step 4: Tests PASS**

```bash
cd src-tauri && cargo test --lib core::ini_diagnostics 2>&1 | tail -10 && cd ..
```

Expected: 6 PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/core/ini_diagnostics.rs src-tauri/src/core/mod.rs
git commit -m "feat(core): pure-Rust ini_diagnostics rule engine (R001 R002 R004 R005 R006 R007)"
```

---


## Task 7: PowerShell — `read-ini-file.ps1` (whole-file read)

**Files:**
- Create: `ps-scripts/read-ini-file.ps1`

The existing `read-ini-section.ps1` only walks one section. The scanner needs every section in one round-trip. New script returns `{ sections: [{ name, keys: [{ name, value, line_number }] }] }`.

- [ ] **Step 1: Write the script**

```powershell
# Reads an entire INI file and returns its sections + keys with line numbers.
# Parameters: -HostName <string> -FilePath <string>
#             [-Username <string>] [-Password <string>]
# Output: JSON { ok, sections: [{ name, keys: [{ name, value, line_number }] }], message }

param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$FilePath,
    [string]$Username,
    [string]$Password
)

$ErrorActionPreference = 'Stop'

function Build-CredentialOrNull {
    param([string]$User, [string]$Pass)
    if ([string]::IsNullOrEmpty($User) -or [string]::IsNullOrEmpty($Pass)) { return $null }
    if ($User -notmatch '[\\@]') { $User = ".\$User" }
    $secure = ConvertTo-SecureString -String $Pass -AsPlainText -Force
    return New-Object System.Management.Automation.PSCredential($User, $secure)
}

try {
    $script = {
        param($FilePath)
        if (-not (Test-Path $FilePath)) {
            return @{ found = $false; sections = @() }
        }
        $lines = Get-Content -Path $FilePath -Encoding UTF8
        $sections = New-Object System.Collections.ArrayList
        $current = $null
        $lineNo = 0
        foreach ($line in $lines) {
            $lineNo++
            $trim = $line.Trim()
            if ($trim.StartsWith('[') -and $trim.EndsWith(']') -and $trim.Length -gt 2) {
                if ($current -ne $null) { [void]$sections.Add($current) }
                $current = @{
                    name = $trim.Substring(1, $trim.Length - 2)
                    keys = New-Object System.Collections.ArrayList
                }
                continue
            }
            if ($current -eq $null) { continue }
            if ([string]::IsNullOrEmpty($trim)) { continue }
            if ($trim.StartsWith(';') -or $trim.StartsWith('#') -or $trim.StartsWith('//')) { continue }
            $eq = $trim.IndexOf('=')
            if ($eq -gt 0) {
                $name = $trim.Substring(0, $eq).Trim()
                $value = $trim.Substring($eq + 1).Trim()
                [void]$current.keys.Add([PSCustomObject]@{
                    name = $name
                    value = $value
                    line_number = $lineNo
                })
            }
        }
        if ($current -ne $null) { [void]$sections.Add($current) }
        return @{ found = $true; sections = $sections }
    }
    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock  = $script
        ArgumentList = @($FilePath)
        ErrorAction  = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $result = Invoke-Command @invokeArgs

    @{
        ok = $true
        found = [bool]$result.found
        sections = @($result.sections)
        message = ""
    } | ConvertTo-Json -Compress -Depth 6
}
catch {
    @{ ok = $false; found = $false; sections = @(); message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
```

- [ ] **Step 2: Smoke test on macOS — script must at least be loadable**

```bash
ls -la ps-scripts/read-ini-file.ps1
```

(Real exec smoke test happens in T22 lanPC E2E.)

- [ ] **Step 3: Commit**

```bash
git add ps-scripts/read-ini-file.ps1
git commit -m "feat(ps): read-ini-file.ps1 returns whole INI file with line numbers"
```

---

## Task 8: `core::ini_scanner` orchestration

**Files:**
- Create: `src-tauri/src/core/ini_scanner.rs`
- Modify: `src-tauri/src/core/mod.rs`

Orchestrates the full scan for one machine: enumerate target file paths from inputs (engine + user level deduced from `machine_ue_installs`; project level from caller-supplied paths), call `read-ini-file.ps1` per file, run `ini_diagnostics::run_rules`, collect `Finding`s.

- [ ] **Step 1: Write tests for path enumeration (pure logic, macOS-friendly)**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerate_engine_paths_returns_baseengine_per_install() {
        let installs = vec![
            ("5.4".to_string(), "C:\\Program Files\\Epic Games\\UE_5.4".to_string()),
            ("5.5".to_string(), "D:\\UE\\UE_5.5".to_string()),
        ];
        let paths = enumerate_engine_paths(&installs);
        assert_eq!(paths.len(), 2);
        assert!(paths[0].path.contains("UE_5.4"));
        assert!(paths[0].path.ends_with("Engine\\Config\\BaseEngine.ini"));
    }

    #[test]
    fn enumerate_user_paths_returns_one_per_version() {
        let installs = vec![
            ("5.4".to_string(), "C:\\anything".to_string()),
        ];
        let paths = enumerate_user_paths(&installs, "C:\\Users\\lanpc");
        assert_eq!(paths.len(), 1);
        assert!(paths[0].path.contains("AppData\\Local\\UnrealEngine\\5.4"));
        assert_eq!(paths[0].category, crate::core::ini_diagnostics::Category::User);
    }

    #[test]
    fn enumerate_project_paths_returns_three_files_per_project_path() {
        let projects = vec!["E:\\Work\\EXLY".to_string()];
        let paths = enumerate_project_paths(&projects);
        assert_eq!(paths.len(), 3);
        assert!(paths.iter().any(|p| p.path.ends_with("DefaultEngine.ini")));
        assert!(paths.iter().any(|p| p.path.ends_with("ConsoleVariables.ini")));
        assert!(paths.iter().any(|p| p.path.ends_with("WindowsEngine.ini")));
    }
}
```

- [ ] **Step 2: Implement `core/ini_scanner.rs` (path enumeration first)**

```rust
//! INI scanner orchestration: enumerate target files for one machine, read
//! them via `read-ini-file.ps1`, and run the pure rule engine over the result.

use crate::core::ini_diagnostics::{
    self, Category, EnvVarState, Finding, ParsedFile, ParsedKey, ParsedSection,
};
use crate::core::powershell;
use crate::error::{UecmError, UecmResult};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq)]
pub struct TargetFile {
    pub path: String,
    pub category: Category,
}

pub fn enumerate_engine_paths(installs: &[(String, String)]) -> Vec<TargetFile> {
    installs.iter().map(|(_, root)| TargetFile {
        path: format!("{}\\Engine\\Config\\BaseEngine.ini", root.trim_end_matches('\\')),
        category: Category::Engine,
    }).collect()
}

pub fn enumerate_user_paths(installs: &[(String, String)], user_profile: &str) -> Vec<TargetFile> {
    installs.iter().map(|(version, _)| TargetFile {
        path: format!(
            "{}\\AppData\\Local\\UnrealEngine\\{}\\Saved\\Config\\WindowsEditor\\EditorPerProjectUserSettings.ini",
            user_profile.trim_end_matches('\\'),
            version
        ),
        category: Category::User,
    }).collect()
}

pub fn enumerate_project_paths(project_roots: &[String]) -> Vec<TargetFile> {
    let mut out = Vec::new();
    for root in project_roots {
        let r = root.trim_end_matches('\\');
        out.push(TargetFile { path: format!("{}\\Config\\DefaultEngine.ini", r), category: Category::Project });
        out.push(TargetFile { path: format!("{}\\Config\\ConsoleVariables.ini", r), category: Category::Project });
        out.push(TargetFile { path: format!("{}\\Config\\Windows\\WindowsEngine.ini", r), category: Category::Project });
    }
    out
}

#[derive(Debug, Deserialize)]
struct ReadFileResult {
    pub ok: bool,
    pub found: bool,
    #[serde(default)]
    pub sections: Vec<RawSection>,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct RawSection {
    pub name: String,
    #[serde(default)]
    pub keys: Vec<RawKey>,
}

#[derive(Debug, Deserialize)]
struct RawKey {
    pub name: String,
    pub value: String,
    pub line_number: usize,
}

pub fn read_file(
    host: &str,
    target: &TargetFile,
    cred: Option<(&str, &str)>,
) -> UecmResult<Option<ParsedFile>> {
    let mut args: Vec<String> = vec![
        "-HostName".into(), host.into(),
        "-FilePath".into(), target.path.clone(),
    ];
    if let Some((u, p)) = cred {
        args.push("-Username".into()); args.push(u.into());
        args.push("-Password".into()); args.push(p.into());
    }
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let result: ReadFileResult = powershell::run_json(
        &powershell::script_path("read-ini-file.ps1"),
        &arg_refs,
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "read-ini-file failed: {}",
            result.message
        )));
    }
    if !result.found {
        return Ok(None);
    }
    Ok(Some(ParsedFile {
        path: target.path.clone(),
        category: target.category,
        sections: result.sections.into_iter().map(|s| ParsedSection {
            name: s.name,
            keys: s.keys.into_iter().map(|k| ParsedKey {
                name: k.name,
                value: k.value,
                line_number: k.line_number,
            }).collect(),
        }).collect(),
    }))
}

pub struct ScanInputs<'a> {
    pub host: &'a str,
    pub credential: Option<(&'a str, &'a str)>,
    pub installs: &'a [(String, String)],
    pub user_profile: &'a str,
    pub project_roots: &'a [String],
    pub env_state: EnvVarState,
}

pub fn scan_machine(inputs: &ScanInputs) -> UecmResult<Vec<Finding>> {
    let mut targets: Vec<TargetFile> = Vec::new();
    targets.extend(enumerate_engine_paths(inputs.installs));
    targets.extend(enumerate_user_paths(inputs.installs, inputs.user_profile));
    targets.extend(enumerate_project_paths(inputs.project_roots));

    let mut findings = Vec::new();
    for tf in &targets {
        match read_file(inputs.host, tf, inputs.credential) {
            Ok(Some(pf)) => findings.extend(ini_diagnostics::run_rules(&pf, &inputs.env_state)),
            Ok(None) => {} // file not present on machine; skip silently
            Err(e) => {
                tracing::warn!("INI read failed for {}: {}", tf.path, e);
                // Per-file failure does not abort the scan; we just lose that file's findings.
            }
        }
    }
    Ok(findings)
}
```

- [ ] **Step 3: Update `core/mod.rs`**

```rust
pub mod ini_scanner;
```

- [ ] **Step 4: Tests PASS**

```bash
cd src-tauri && cargo test --lib core::ini_scanner 2>&1 | tail -8 && cd ..
```

Expected: 3 PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/core/ini_scanner.rs src-tauri/src/core/mod.rs
git commit -m "feat(core): ini_scanner orchestration (path enum + WinRM read + diagnostics dispatch)"
```

---

## Task 9: `core::ini_apply` — translate Finding into ini_editor call

**Files:**
- Create: `src-tauri/src/core/ini_apply.rs`
- Modify: `src-tauri/src/core/ini_editor.rs` (add `remove_key_with_credential`)
- Modify: `ps-scripts/write-ini-key.ps1` (accept `-RemoveKey` flag)
- Modify: `src-tauri/src/core/mod.rs`

`Finding.recommended_action` is one of `set` / `remove` / `manual`. `manual` is rejected (must be edited by hand). The other two map to ini_editor calls.

- [ ] **Step 1: Extend `write-ini-key.ps1` with `-RemoveKey`**

Open `ps-scripts/write-ini-key.ps1`, add a `[switch]$RemoveKey` parameter and adjust the inner scriptblock so when set, the line matching `$Name=` is deleted instead of replaced. Preserve existing tests (the script test in `read-ini-section.ps1` indirectly covers reads after writes).

```powershell
param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$FilePath,
    [Parameter(Mandatory=$true)] [string]$Section,
    [Parameter(Mandatory=$true)] [string]$Name,
    [string]$Value = "",
    [switch]$RemoveKey,
    [string]$Username,
    [string]$Password
)
```

In the scriptblock, after backup, branch:

```powershell
if ($RemoveKey) {
    # Remove every line matching `<Name>=` in the chosen section.
    # Walk lines; when in section, drop matching key.
    # ... (existing read+rewrite loop)
} else {
    # Existing set-or-append path.
}
```

Detailed body (full script, replace the existing one):

```powershell
$ErrorActionPreference = 'Stop'

function Build-CredentialOrNull {
    param([string]$User, [string]$Pass)
    if ([string]::IsNullOrEmpty($User) -or [string]::IsNullOrEmpty($Pass)) { return $null }
    if ($User -notmatch '[\\@]') { $User = ".\$User" }
    $secure = ConvertTo-SecureString -String $Pass -AsPlainText -Force
    return New-Object System.Management.Automation.PSCredential($User, $secure)
}

try {
    $script = {
        param($FilePath, $Section, $Name, $Value, $Remove)
        if (-not (Test-Path $FilePath)) { throw "file not found: $FilePath" }
        $backup = "$FilePath.bak.$(Get-Date -UFormat '%Y%m%d-%H%M%S')"
        Copy-Item -Path $FilePath -Destination $backup -Force
        $lines = Get-Content -Path $FilePath -Encoding UTF8
        $out = New-Object System.Collections.ArrayList
        $inSection = $false
        $written = $false
        $bracket = "[$Section]"
        foreach ($line in $lines) {
            $trim = $line.Trim()
            if ($trim -eq $bracket) { $inSection = $true; [void]$out.Add($line); continue }
            if ($inSection -and $trim.StartsWith('[') -and $trim.EndsWith(']')) {
                if (-not $Remove -and -not $written) {
                    [void]$out.Add("$Name=$Value"); $written = $true
                }
                $inSection = $false
                [void]$out.Add($line)
                continue
            }
            if ($inSection -and $trim -match "^\s*$([regex]::Escape($Name))\s*=") {
                if ($Remove) { continue } # drop the line
                [void]$out.Add("$Name=$Value"); $written = $true; continue
            }
            [void]$out.Add($line)
        }
        if (-not $Remove -and -not $written -and $inSection) {
            [void]$out.Add("$Name=$Value")
        }
        Set-Content -Path $FilePath -Value $out -Encoding UTF8
        return "$backup"
    }
    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock  = $script
        ArgumentList = @($FilePath, $Section, $Name, $Value, [bool]$RemoveKey)
        ErrorAction  = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $remoteResult = Invoke-Command @invokeArgs
    @{ ok = $true; backup_path = "$remoteResult"; message = "wrote $Name in [$Section]" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; backup_path = ""; message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
```

(The `"$remoteResult"` cast preserves Plan 2 fix `f6b40ff`.)

- [ ] **Step 2: Add `remove_key_with_credential` to `core/ini_editor.rs`**

```rust
pub fn remove_key_with_credential(
    host: &str, file_path: &str, section: &str, name: &str,
    username: &str, password: &str,
) -> UecmResult<String> {
    let result: WriteResult = powershell::run_json(
        &powershell::script_path("write-ini-key.ps1"),
        &[
            "-HostName", host, "-FilePath", file_path, "-Section", section,
            "-Name", name, "-RemoveKey",
            "-Username", username, "-Password", password,
        ],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!("remove key failed: {}", result.message)));
    }
    Ok(result.backup_path)
}
```

Add a non-Windows test verifying the function returns `UecmError::PowerShell` (mirroring existing tests).

- [ ] **Step 3: Implement `core/ini_apply.rs`**

```rust
//! Translate a single ini_findings row into a concrete ini_editor call.

use crate::core::ini_editor;
use crate::data::ini_findings::IniFinding;
use crate::error::{UecmError, UecmResult};

pub struct ApplyContext<'a> {
    pub host: &'a str,
    pub credential: (&'a str, &'a str),
}

pub fn apply(ctx: &ApplyContext, finding: &IniFinding) -> UecmResult<String> {
    let section = finding.section.as_deref()
        .ok_or_else(|| UecmError::InvalidInput("finding has no section".into()))?;
    match finding.recommended_action.as_str() {
        "set" => {
            let key = finding.key_name.as_deref()
                .ok_or_else(|| UecmError::InvalidInput("finding has no key_name".into()))?;
            let value = finding.recommended_value.as_deref()
                .ok_or_else(|| UecmError::InvalidInput("finding has no recommended_value".into()))?;
            // R001 special-case: when the rule wants to swap `Path=` for
            // `EnvPathOverride=...`, the apply path is two ops: remove old
            // key, then set the new key.
            if finding.rule_id == "R001" {
                ini_editor::remove_key_with_credential(
                    ctx.host, &finding.file_path, section, key,
                    ctx.credential.0, ctx.credential.1,
                )?;
                return ini_editor::set_key_with_credential(
                    ctx.host, &finding.file_path, section, "EnvPathOverride", value,
                    ctx.credential.0, ctx.credential.1,
                );
            }
            ini_editor::set_key_with_credential(
                ctx.host, &finding.file_path, section, key, value,
                ctx.credential.0, ctx.credential.1,
            )
        }
        "remove" => {
            // R002 (user-level DDC section) requires removing the whole section.
            // For v1 we approximate by removing each known key in the section.
            // The frontend warns that R002 may need manual cleanup if extra keys exist.
            if finding.rule_id == "R002" {
                // The finding's `snippet_before` lists the keys; we parse them and remove each.
                for line in finding.snippet_before.lines() {
                    if let Some(eq) = line.find('=') {
                        let key = line[..eq].trim();
                        if !key.is_empty() {
                            ini_editor::remove_key_with_credential(
                                ctx.host, &finding.file_path, section, key,
                                ctx.credential.0, ctx.credential.1,
                            )?;
                        }
                    }
                }
                return Ok("multiple-key removal".into());
            }
            let key = finding.key_name.as_deref()
                .ok_or_else(|| UecmError::InvalidInput("remove needs key_name".into()))?;
            ini_editor::remove_key_with_credential(
                ctx.host, &finding.file_path, section, key,
                ctx.credential.0, ctx.credential.1,
            )
        }
        "manual" => Err(UecmError::InvalidInput(
            "manual findings cannot be auto-applied; open the file directly".into(),
        )),
        other => Err(UecmError::InvalidInput(format!("unknown action: {}", other))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::ini_findings::IniFinding;

    fn finding(action: &str, rule: &str) -> IniFinding {
        IniFinding {
            id: Some(1), scan_run_id: 1, machine_id: 1,
            rule_id: rule.into(), severity: "critical".into(),
            category: "project".into(),
            file_path: "C:\\f.ini".into(),
            section: Some("DDC".into()), key_name: Some("Path".into()),
            line_number: Some(1),
            snippet_before: "Path=X".into(), snippet_after: None,
            recommended_action: action.into(), recommended_value: Some("V".into()),
            symptom: "".into(), rationale: "".into(),
            fixed_at: None, skipped_at: None,
        }
    }

    #[test]
    fn manual_action_returns_invalid_input() {
        let ctx = ApplyContext { host: "X", credential: ("u", "p") };
        let result = apply(&ctx, &finding("manual", "R004"));
        assert!(matches!(result, Err(UecmError::InvalidInput(_))));
    }

    #[test]
    fn unknown_action_returns_invalid_input() {
        let ctx = ApplyContext { host: "X", credential: ("u", "p") };
        let result = apply(&ctx, &finding("zzz", "R999"));
        assert!(matches!(result, Err(UecmError::InvalidInput(_))));
    }
}
```

- [ ] **Step 4: Update `core/mod.rs`**

```rust
pub mod ini_apply;
```

- [ ] **Step 5: Tests PASS**

```bash
cd src-tauri && cargo test --lib core::ini_apply 2>&1 | tail -8 && cd ..
```

Expected: 2 PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/core ps-scripts/write-ini-key.ps1
git commit -m "feat(core): ini_apply maps Finding -> ini_editor; ps script gains -RemoveKey"
```

---


## Task 10: Tauri commands for INI scanner

**Files:**
- Create: `src-tauri/src/commands/ini_scanner.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

Surfaces four commands: `scan_inis`, `list_findings_for_run`, `apply_finding`, `skip_finding`.

- [ ] **Step 1: Implement `commands/ini_scanner.rs`**

```rust
//! Tauri commands for the INI scanner: dispatch a scan, list findings,
//! apply / skip a single finding.

use crate::core::credentials as core_credentials;
use crate::core::ini_apply::{self, ApplyContext};
use crate::core::ini_diagnostics::EnvVarState;
use crate::core::ini_scanner::{self, ScanInputs};
use crate::core::env_vars;
use crate::data::{
    credentials as data_credentials, ini_findings, machine_ue_installs,
    machines as data_machines, scan_runs, Db, IniFinding,
};
use crate::error::{UecmError, UecmResult};
use serde::Serialize;
use serde_json::json;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct ScanRunSummary {
    pub scan_run_id: i64,
    pub critical: i64,
    pub warning: i64,
    pub healthy: i64,
}

#[tauri::command]
pub fn scan_inis(
    db: State<'_, Db>,
    machine_ids: Vec<i64>,
    project_paths_per_machine: std::collections::HashMap<i64, Vec<String>>,
    user_profile: String,
    credential_alias: String,
) -> UecmResult<ScanRunSummary> {
    if machine_ids.is_empty() {
        return Err(UecmError::InvalidInput("machine_ids must not be empty".into()));
    }
    let cred_row = data_credentials::find_by_alias(&db, &credential_alias)?
        .ok_or_else(|| UecmError::InvalidInput(format!("credential alias '{}' not found", credential_alias)))?;
    let password = core_credentials::resolve_password(&credential_alias)?;
    let scan_id = scan_runs::insert(&db, "ini", &machine_ids)?;

    let mut total_critical = 0i64;
    let mut total_warning = 0i64;
    let mut total_healthy = 0i64;

    for &mid in &machine_ids {
        let machine = data_machines::find_by_id(&db, mid)?
            .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", mid)))?;
        let installs_rows = machine_ue_installs::list_for_machine(&db, mid)?;
        let installs: Vec<(String, String)> = installs_rows.into_iter()
            .map(|i| (i.version, i.install_path)).collect();
        let project_roots: Vec<String> = project_paths_per_machine.get(&mid).cloned().unwrap_or_default();

        let mut env_state = EnvVarState::default();
        // Reading env vars uses the same credential pipe Plan 3 set up.
        env_state.shared_data_cache_path = env_vars::get_with_credential(
            &machine.ip, "UE-SharedDataCachePath", &cred_row.username, &password,
        ).ok().flatten();
        env_state.local_data_cache_path = env_vars::get_with_credential(
            &machine.ip, "UE-LocalDataCachePath", &cred_row.username, &password,
        ).ok().flatten();

        let inputs = ScanInputs {
            host: &machine.ip,
            credential: Some((&cred_row.username, &password)),
            installs: &installs,
            user_profile: &user_profile,
            project_roots: &project_roots,
            env_state,
        };

        let findings = ini_scanner::scan_machine(&inputs)?;
        for f in findings {
            let row = IniFinding {
                id: None,
                scan_run_id: scan_id,
                machine_id: mid,
                rule_id: f.rule_id,
                severity: f.severity.as_str().into(),
                category: f.category.as_str().into(),
                file_path: f.file_path,
                section: f.section,
                key_name: f.key_name,
                line_number: f.line_number,
                snippet_before: f.snippet_before,
                snippet_after: f.snippet_after,
                recommended_action: f.recommended_action.as_str().into(),
                recommended_value: f.recommended_value,
                symptom: f.symptom,
                rationale: f.rationale,
                fixed_at: None,
                skipped_at: None,
            };
            match row.severity.as_str() {
                "critical" => total_critical += 1,
                "warning" => total_warning += 1,
                "healthy" => total_healthy += 1,
                _ => {}
            }
            ini_findings::insert(&db, &row)?;
        }
    }

    let summary = json!({
        "critical": total_critical,
        "warning": total_warning,
        "healthy": total_healthy,
    });
    scan_runs::finish(&db, scan_id, &summary)?;
    Ok(ScanRunSummary {
        scan_run_id: scan_id,
        critical: total_critical,
        warning: total_warning,
        healthy: total_healthy,
    })
}

#[tauri::command]
pub fn list_findings_for_run(db: State<'_, Db>, scan_run_id: i64) -> UecmResult<Vec<IniFinding>> {
    ini_findings::list_for_run(&db, scan_run_id)
}

#[tauri::command]
pub fn list_recent_ini_runs(db: State<'_, Db>, limit: i64) -> UecmResult<Vec<scan_runs::ScanRun>> {
    scan_runs::list_recent(&db, "ini", limit)
}

#[tauri::command]
pub fn apply_finding(
    db: State<'_, Db>,
    finding_id: i64,
    credential_alias: String,
) -> UecmResult<String> {
    let f = ini_findings::find_by_id(&db, finding_id)?
        .ok_or_else(|| UecmError::InvalidInput(format!("finding {} not found", finding_id)))?;
    let machine = data_machines::find_by_id(&db, f.machine_id)?
        .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", f.machine_id)))?;
    let cred = data_credentials::find_by_alias(&db, &credential_alias)?
        .ok_or_else(|| UecmError::InvalidInput(format!("credential '{}' not found", credential_alias)))?;
    let password = core_credentials::resolve_password(&credential_alias)?;
    let ctx = ApplyContext { host: &machine.ip, credential: (&cred.username, &password) };
    let backup = ini_apply::apply(&ctx, &f)?;
    ini_findings::mark_fixed(&db, finding_id)?;
    Ok(backup)
}

#[tauri::command]
pub fn skip_finding(db: State<'_, Db>, finding_id: i64) -> UecmResult<()> {
    ini_findings::mark_skipped(&db, finding_id)
}
```

- [ ] **Step 2: Update `commands/mod.rs` and `lib.rs`**

`mod.rs`:
```rust
pub mod ini_scanner;
```

`lib.rs` `invoke_handler!`:
```rust
commands::ini_scanner::scan_inis,
commands::ini_scanner::list_findings_for_run,
commands::ini_scanner::list_recent_ini_runs,
commands::ini_scanner::apply_finding,
commands::ini_scanner::skip_finding,
```

- [ ] **Step 3: cfg(not(windows)) test for `apply_finding` returning `PowerShell` error path**

Add a small integration-style test in `src-tauri/tests/ini_scanner_command.rs` (new top-level integration test directory if it doesn't yet exist). Or skip if the existing project has no integration test folder; rely on T22 lanPC E2E.

- [ ] **Step 4: Build + tests**

```bash
cd src-tauri && cargo build && cargo test --lib commands::ini_scanner 2>&1 | tail -8 && cd ..
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src
git commit -m "feat(commands): scan_inis / list_findings_for_run / apply_finding / skip_finding"
```

---

## Task 11: PowerShell — `health-probes.ps1` (8 probes per machine in one round-trip)

**Files:**
- Create: `ps-scripts/health-probes.ps1`

This script runs the 8 actively-probed checks from the spec (all but the three derived ones: INI consistency #8, GPU consistency #11, and PSO Precaching CVar #10 which is read separately).

The 8 active checks: `smb`, `firewall_445`, `share_reachable`, `ntfs_perm`, `cred_user`, `cred_system`, `env_vars`, `system_write`.

- [ ] **Step 1: Write the script**

```powershell
# Runs 8 health probes against a remote host in one round-trip.
# Parameters:
#   -HostName <string>
#   -ShareUnc <string>          e.g. "\\HOST\DDC", or "" if no share configured
#   -SvcUsername <string>       e.g. "ddc-svc", or "" if no managed share
#   -ExpectedSharedDataCachePath <string>
#   [-Username <string>] [-Password <string>]
# Output: JSON { ok, results: { smb:{status,message,sample}, firewall_445:..., ... }, message }

param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [string]$ShareUnc = "",
    [string]$SvcUsername = "",
    [string]$ExpectedSharedDataCachePath = "",
    [string]$Username,
    [string]$Password
)

$ErrorActionPreference = 'Stop'

function Build-CredentialOrNull {
    param([string]$User, [string]$Pass)
    if ([string]::IsNullOrEmpty($User) -or [string]::IsNullOrEmpty($Pass)) { return $null }
    if ($User -notmatch '[\\@]') { $User = ".\$User" }
    $secure = ConvertTo-SecureString -String $Pass -AsPlainText -Force
    return New-Object System.Management.Automation.PSCredential($User, $secure)
}

try {
    $script = {
        param($ShareUnc, $SvcUsername, $ExpectedSharedDataCachePath)
        function Probe-SmbService {
            try {
                $svc = Get-Service -Name LanmanServer -ErrorAction Stop
                @{ status = ($(if ($svc.Status -eq 'Running') {'healthy'} else {'critical'}));
                   message = "LanmanServer = $($svc.Status)"; sample = $svc.Status.ToString() }
            } catch { @{ status='critical'; message=$_.Exception.Message; sample='' } }
        }
        function Probe-Firewall445 {
            try {
                $rule = Get-NetFirewallRule -DisplayName 'File and Printer Sharing (SMB-In)' -ErrorAction SilentlyContinue
                if (-not $rule) { return @{ status='warning'; message='no SMB-In rule found'; sample='' } }
                $enabled = ($rule | Where-Object Enabled -eq 'True').Count -gt 0
                @{ status = ($(if ($enabled) {'healthy'} else {'warning'}));
                   message = "rule enabled = $enabled"; sample = ($rule[0].DisplayName) }
            } catch { @{ status='warning'; message=$_.Exception.Message; sample='' } }
        }
        function Probe-ShareReachable {
            if ([string]::IsNullOrEmpty($ShareUnc)) {
                return @{ status='na'; message='no share configured'; sample='' }
            }
            try {
                $ok = Test-Path $ShareUnc -ErrorAction Stop
                @{ status = ($(if ($ok) {'healthy'} else {'critical'}));
                   message = "Test-Path returned $ok"; sample = $ShareUnc }
            } catch { @{ status='critical'; message=$_.Exception.Message; sample=$ShareUnc } }
        }
        function Probe-NtfsPerm {
            if ([string]::IsNullOrEmpty($ShareUnc) -or [string]::IsNullOrEmpty($SvcUsername)) {
                return @{ status='na'; message='only meaningful for managed shares with svc account'; sample='' }
            }
            try {
                # Try a probe write under SvcUsername — too brittle to do over WinRM.
                # Instead, read ACL on the local share path if we are the host.
                $share = Get-SmbShare -Name (Split-Path $ShareUnc -Leaf) -ErrorAction SilentlyContinue
                if (-not $share) { return @{ status='na'; message='not the host'; sample='' } }
                $acl = Get-Acl $share.Path
                $hasSvc = $acl.Access | Where-Object { $_.IdentityReference -match $SvcUsername }
                @{ status = ($(if ($hasSvc) {'healthy'} else {'critical'}));
                   message = "ACL on $($share.Path) for $SvcUsername"; sample = ($acl.Owner) }
            } catch { @{ status='warning'; message=$_.Exception.Message; sample='' } }
        }
        function Probe-CredUser {
            if ([string]::IsNullOrEmpty($SvcUsername)) {
                return @{ status='na'; message='no managed share'; sample='' }
            }
            try {
                $out = & cmdkey.exe /list 2>&1 | Out-String
                $hasIt = $out -match [regex]::Escape($SvcUsername)
                @{ status = ($(if ($hasIt) {'healthy'} else {'critical'}));
                   message = "cmdkey /list contains $SvcUsername = $hasIt"; sample = '' }
            } catch { @{ status='critical'; message=$_.Exception.Message; sample='' } }
        }
        function Probe-CredSystem {
            if ([string]::IsNullOrEmpty($SvcUsername)) {
                return @{ status='na'; message='no managed share'; sample='' }
            }
            $psexec = Join-Path (Split-Path -Parent (Get-Command powershell.exe).Source) ''  # placeholder
            $vendor = Join-Path $env:LOCALAPPDATA 'UECM\PsExec64.exe'
            if (-not (Test-Path $vendor)) {
                return @{ status='warning'; message='PsExec64 not staged on machine; cannot verify SYSTEM cred'; sample='' }
            }
            try {
                $out = & $vendor -accepteula -nobanner -s -i 0 cmdkey.exe /list 2>&1 | Out-String
                $hasIt = $out -match [regex]::Escape($SvcUsername)
                @{ status = ($(if ($hasIt) {'healthy'} else {'critical'}));
                   message = "SYSTEM cmdkey /list contains $SvcUsername = $hasIt"; sample = '' }
            } catch { @{ status='warning'; message=$_.Exception.Message; sample='' } }
        }
        function Probe-EnvVars {
            $shared = [Environment]::GetEnvironmentVariable('UE-SharedDataCachePath', 'Machine')
            if ([string]::IsNullOrEmpty($ExpectedSharedDataCachePath)) {
                @{ status = ($(if ([string]::IsNullOrEmpty($shared)) {'warning'} else {'healthy'}));
                   message = "UE-SharedDataCachePath = $shared"; sample = "$shared" }
            } else {
                @{ status = ($(if ($shared -eq $ExpectedSharedDataCachePath) {'healthy'} else {'critical'}));
                   message = "expected $ExpectedSharedDataCachePath, got $shared"; sample = "$shared" }
            }
        }
        function Probe-SystemWrite {
            if ([string]::IsNullOrEmpty($ShareUnc)) {
                return @{ status='na'; message='no share configured'; sample='' }
            }
            $vendor = Join-Path $env:LOCALAPPDATA 'UECM\PsExec64.exe'
            if (-not (Test-Path $vendor)) {
                return @{ status='warning'; message='PsExec64 not staged; cannot SYSTEM-write probe'; sample='' }
            }
            try {
                $probe = "uecm-probe-$(Get-Random).txt"
                $cmd = "echo healthcheck > `"$ShareUnc\$probe`""
                & $vendor -accepteula -nobanner -s -i 0 cmd /c $cmd 2>&1 | Out-Null
                $exists = Test-Path "$ShareUnc\$probe"
                if ($exists) { Remove-Item "$ShareUnc\$probe" -Force -ErrorAction SilentlyContinue }
                @{ status = ($(if ($exists) {'healthy'} else {'critical'}));
                   message = "SYSTEM wrote probe file = $exists"; sample = $probe }
            } catch { @{ status='critical'; message=$_.Exception.Message; sample='' } }
        }

        $results = @{
            smb              = (Probe-SmbService)
            firewall_445     = (Probe-Firewall445)
            share_reachable  = (Probe-ShareReachable)
            ntfs_perm        = (Probe-NtfsPerm)
            cred_user        = (Probe-CredUser)
            cred_system      = (Probe-CredSystem)
            env_vars         = (Probe-EnvVars)
            system_write     = (Probe-SystemWrite)
        }
        return $results
    }
    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock  = $script
        ArgumentList = @($ShareUnc, $SvcUsername, $ExpectedSharedDataCachePath)
        ErrorAction  = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $r = Invoke-Command @invokeArgs

    @{ ok = $true; results = $r; message = '' } | ConvertTo-Json -Compress -Depth 6
}
catch {
    @{ ok = $false; results = @{}; message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
```

Note: `cred_system` and `system_write` need `PsExec64.exe` staged at `%LOCALAPPDATA%\UECM\PsExec64.exe` on the target machine (not on the operator). T20 includes a bootstrapping helper that copies PsExec64 from the operator's vendored copy to each target's LocalAppData via `Copy-Item -ToSession`. If the file is missing, both probes return `warning` instead of `critical` so an unstaged machine doesn't read as broken.

- [ ] **Step 2: Commit**

```bash
git add ps-scripts/health-probes.ps1
git commit -m "feat(ps): health-probes.ps1 packages 8 active probes per machine"
```

---


## Task 12: `core::health_check` orchestration + GPU consistency aggregator

**Files:**
- Create: `src-tauri/src/core/health_probes.rs` (Windows-gated WinRM dispatch wrapper)
- Create: `src-tauri/src/core/health_check.rs` (orchestration + pure aggregator)
- Modify: `src-tauri/src/core/mod.rs`

- [ ] **Step 1: Write tests for the GPU aggregator (pure logic)**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::machine_gpus::{GpuInfo, GpuVendor};

    fn gpu(mid: i64, model: &str, drv: &str) -> GpuInfo {
        GpuInfo { id: Some(0), machine_id: mid, gpu_model: model.into(),
                  driver_version: drv.into(), vendor: GpuVendor::Nvidia, vram_mb: Some(10240) }
    }

    #[test]
    fn all_machines_with_same_gpu_are_healthy() {
        let gpus = vec![gpu(1, "RTX 3080", "545.92"), gpu(2, "RTX 3080", "545.92")];
        let report = aggregate_gpu_consistency(&gpus);
        assert_eq!(report.outcomes.get(&1).unwrap().status, "healthy");
        assert_eq!(report.outcomes.get(&2).unwrap().status, "healthy");
    }

    #[test]
    fn one_machine_with_different_driver_is_warning() {
        let gpus = vec![gpu(1, "RTX 3080", "545.92"), gpu(2, "RTX 3080", "537.00")];
        let report = aggregate_gpu_consistency(&gpus);
        assert_eq!(report.outcomes.get(&2).unwrap().status, "warning");
    }

    #[test]
    fn one_machine_with_different_model_is_critical() {
        let gpus = vec![gpu(1, "RTX 3080", "545.92"), gpu(2, "RTX 3080", "545.92"), gpu(3, "RTX 4090", "545.92")];
        let report = aggregate_gpu_consistency(&gpus);
        assert_eq!(report.outcomes.get(&3).unwrap().status, "critical");
    }

    #[test]
    fn machine_with_no_gpu_data_is_unknown() {
        let report = aggregate_gpu_consistency(&[]);
        assert!(report.outcomes.is_empty());
    }
}
```

- [ ] **Step 2: Implement `core/health_check.rs`**

```rust
//! Orchestrate per-machine probes + cluster-level aggregators (GPU, INI consistency).

use crate::data::machine_gpus::GpuInfo;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckOutcome {
    pub status: String,            // "healthy" | "warning" | "critical" | "na" | "offline" | "unknown"
    pub message: String,
    pub sample: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GpuConsistencyReport {
    pub outcomes: HashMap<i64, CheckOutcome>,
}

pub fn aggregate_gpu_consistency(gpus: &[GpuInfo]) -> GpuConsistencyReport {
    let mut by_machine: HashMap<i64, &GpuInfo> = HashMap::new();
    for g in gpus { by_machine.insert(g.machine_id, g); }

    // Tally distinct (model, driver) combinations.
    let mut combo_counts: HashMap<(String, String), i64> = HashMap::new();
    for g in by_machine.values() {
        *combo_counts
            .entry((g.gpu_model.clone(), g.driver_version.clone()))
            .or_insert(0) += 1;
    }
    let model_counts: HashMap<String, i64> = {
        let mut m = HashMap::new();
        for g in by_machine.values() { *m.entry(g.gpu_model.clone()).or_insert(0) += 1; }
        m
    };

    let mut outcomes = HashMap::new();
    for (mid, g) in &by_machine {
        let same_combo = combo_counts.get(&(g.gpu_model.clone(), g.driver_version.clone())).copied().unwrap_or(0);
        let same_model = model_counts.get(&g.gpu_model).copied().unwrap_or(0);
        let total = by_machine.len() as i64;
        let status = if total == 1 || same_combo == total {
            "healthy"
        } else if same_model == total {
            // same model, different driver
            "warning"
        } else {
            "critical"
        };
        outcomes.insert(*mid, CheckOutcome {
            status: status.into(),
            message: format!(
                "{} {} ({} of {} machines have same combo)",
                g.gpu_model, g.driver_version, same_combo, total
            ),
            sample: format!("{} / {}", g.gpu_model, g.driver_version),
        });
    }
    GpuConsistencyReport { outcomes }
}
```

- [ ] **Step 3: Implement `core/health_probes.rs` (Windows-only WinRM wrapper)**

```rust
//! WinRM dispatch for `health-probes.ps1`. Pure pass-through; orchestration in `health_check`.

use crate::core::powershell;
use crate::error::{UecmError, UecmResult};
use serde::Deserialize;
use std::collections::HashMap;
use super::health_check::CheckOutcome;

#[derive(Debug, Deserialize)]
struct ProbeResult {
    pub ok: bool,
    #[serde(default)]
    pub results: HashMap<String, CheckOutcome>,
    #[serde(default)]
    pub message: String,
}

pub fn run(
    host: &str,
    share_unc: &str,
    svc_username: &str,
    expected_shared_path: &str,
    cred: Option<(&str, &str)>,
) -> UecmResult<HashMap<String, CheckOutcome>> {
    let mut args: Vec<String> = vec![
        "-HostName".into(), host.into(),
        "-ShareUnc".into(), share_unc.into(),
        "-SvcUsername".into(), svc_username.into(),
        "-ExpectedSharedDataCachePath".into(), expected_shared_path.into(),
    ];
    if let Some((u, p)) = cred {
        args.push("-Username".into()); args.push(u.into());
        args.push("-Password".into()); args.push(p.into());
    }
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let result: ProbeResult = powershell::run_json(
        &powershell::script_path("health-probes.ps1"),
        &arg_refs,
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!("health-probes failed: {}", result.message)));
    }
    Ok(result.results)
}
```

- [ ] **Step 4: Update `core/mod.rs`**

```rust
pub mod health_check;
pub mod health_probes;
```

- [ ] **Step 5: Tests PASS**

```bash
cd src-tauri && cargo test --lib core::health_check 2>&1 | tail -8 && cd ..
```

Expected: 4 PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/core
git commit -m "feat(core): health_check orchestration + GPU consistency aggregator"
```

---

## Task 13: Tauri commands for Health Check

**Files:**
- Create: `src-tauri/src/commands/health_check.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

`run_health_check` orchestrates: insert scan_run row → fan-out via `core::batch::run_batch` → for each machine call `core::health_probes::run` for the 8 active probes + derive INI #8 from latest ini scan_run + derive PSO Precaching #10 by reading `Config/ConsoleVariables.ini` from the first project path → assemble JSON → upsert `health_check_runs` → emit `health-progress` event after each machine.

- [ ] **Step 1: Implement `commands/health_check.rs`**

```rust
//! Tauri commands for the cluster health check.

use crate::core::credentials as core_credentials;
use crate::core::health_check::{aggregate_gpu_consistency, CheckOutcome};
use crate::core::health_probes;
use crate::core::ini_scanner;
use crate::core::ini_diagnostics::EnvVarState;
use crate::data::{
    credentials as data_credentials, ini_findings, machine_gpus,
    machine_ue_installs, machines as data_machines, scan_runs, share_configs,
    health_check_runs, Db,
};
use crate::error::{UecmError, UecmResult};
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Serialize, Clone)]
pub struct HealthProgressEvent {
    pub scan_run_id: i64,
    pub machine_id: i64,
    pub done: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HealthRunSummary {
    pub scan_run_id: i64,
    pub healthy: i64,
    pub warning: i64,
    pub critical: i64,
    pub offline: i64,
    pub total: i64,
}

#[tauri::command]
pub fn run_health_check(
    db: State<'_, Db>,
    app: AppHandle,
    machine_ids: Vec<i64>,
    project_paths_per_machine: HashMap<i64, Vec<String>>,
    credential_alias: String,
) -> UecmResult<HealthRunSummary> {
    if machine_ids.is_empty() {
        return Err(UecmError::InvalidInput("machine_ids required".into()));
    }
    let cred_row = data_credentials::find_by_alias(&db, &credential_alias)?
        .ok_or_else(|| UecmError::InvalidInput(format!("credential '{}' not found", credential_alias)))?;
    let password = core_credentials::resolve_password(&credential_alias)?;

    let scan_id = scan_runs::insert(&db, "health", &machine_ids)?;

    // Pre-compute cluster-level aggregations once.
    let all_gpus: Vec<machine_gpus::GpuInfo> = {
        let mut acc = Vec::new();
        for &mid in &machine_ids {
            acc.extend(machine_gpus::list_for_machine(&db, mid)?);
        }
        acc
    };
    let gpu_report = aggregate_gpu_consistency(&all_gpus);

    let mut summary = HealthRunSummary {
        scan_run_id: scan_id,
        healthy: 0, warning: 0, critical: 0, offline: 0,
        total: 0,
    };

    for &mid in &machine_ids {
        let machine = match data_machines::find_by_id(&db, mid)? {
            Some(m) => m,
            None => continue,
        };

        // Resolve the most recent share config for this machine acting as host (best-effort).
        let host_share = share_configs::find_by_host(&db, mid).unwrap_or_default()
            .into_iter().next();
        let share_unc = host_share.as_ref().map(|s| s.unc_path.clone()).unwrap_or_default();
        let svc_username = host_share.as_ref()
            .and_then(|s| s.credential_alias.clone())
            .unwrap_or_else(|| "ddc-svc".to_string());
        let expected_shared = share_unc.clone();

        let probes = match health_probes::run(
            &machine.ip, &share_unc, &svc_username, &expected_shared,
            Some((&cred_row.username, &password)),
        ) {
            Ok(map) => map,
            Err(e) => {
                let _ = app.emit("health-progress", HealthProgressEvent {
                    scan_run_id: scan_id, machine_id: mid, done: true,
                    error: Some(e.to_string()),
                });
                summary.offline += 1; summary.total += 8;
                let mut row = HashMap::<String, CheckOutcome>::new();
                for k in ["smb","firewall_445","share_reachable","ntfs_perm","cred_user","cred_system","env_vars","system_write"] {
                    row.insert(k.into(), CheckOutcome { status: "offline".into(), message: e.to_string(), sample: "".into() });
                }
                health_check_runs::upsert(&db, scan_id, mid, &serde_json::to_value(&row).unwrap())?;
                continue;
            }
        };

        // Derive #8 INI consistency from the latest ini scan_run.
        let ini_outcome = derive_ini_outcome(&db, mid)?;

        // Derive #10 PSO Precaching CVar from project ConsoleVariables.ini (first project, optional).
        let pso_outcome = derive_pso_cvar_outcome(
            &db, mid, &machine.ip, &cred_row.username, &password,
            project_paths_per_machine.get(&mid).cloned().unwrap_or_default(),
        );

        // #11 GPU consistency from precomputed report.
        let gpu_outcome = gpu_report.outcomes.get(&mid).cloned()
            .unwrap_or(CheckOutcome { status: "unknown".into(), message: "no GPU data".into(), sample: "".into() });

        let mut row: HashMap<String, CheckOutcome> = probes;
        row.insert("ini_consistency".into(), ini_outcome);
        row.insert("pso_precaching".into(), pso_outcome);
        row.insert("gpu_consistency".into(), gpu_outcome);

        // Tally for summary.
        for v in row.values() {
            summary.total += 1;
            match v.status.as_str() {
                "healthy" => summary.healthy += 1,
                "warning" => summary.warning += 1,
                "critical" => summary.critical += 1,
                "offline" => summary.offline += 1,
                _ => {}
            }
        }
        health_check_runs::upsert(&db, scan_id, mid, &serde_json::to_value(&row).unwrap())?;
        let _ = app.emit("health-progress", HealthProgressEvent {
            scan_run_id: scan_id, machine_id: mid, done: true, error: None,
        });
    }

    let summary_json = json!({
        "healthy": summary.healthy, "warning": summary.warning,
        "critical": summary.critical, "offline": summary.offline,
        "total": summary.total,
    });
    scan_runs::finish(&db, scan_id, &summary_json)?;
    Ok(summary)
}

fn derive_ini_outcome(db: &Db, machine_id: i64) -> UecmResult<CheckOutcome> {
    // Find the latest ini scan_run, count this machine's open critical/warning rows.
    let recent = scan_runs::list_recent(db, "ini", 1)?;
    let Some(latest) = recent.first() else {
        return Ok(CheckOutcome { status: "unknown".into(), message: "no INI scan run yet".into(), sample: "".into() });
    };
    let counts = ini_findings::count_by_severity_for_machine(db, latest.id.unwrap(), machine_id)?;
    let status = if counts.critical > 0 { "critical" }
        else if counts.warning > 0 { "warning" }
        else { "healthy" };
    Ok(CheckOutcome {
        status: status.into(),
        message: format!("{} critical / {} warning open", counts.critical, counts.warning),
        sample: format!("scan_run #{}", latest.id.unwrap()),
    })
}

fn derive_pso_cvar_outcome(
    db: &Db,
    _machine_id: i64,
    host: &str,
    username: &str,
    password: &str,
    project_roots: Vec<String>,
) -> CheckOutcome {
    if project_roots.is_empty() {
        return CheckOutcome {
            status: "na".into(),
            message: "no project paths supplied".into(),
            sample: "".into(),
        };
    }
    let _ = db;
    let target = ini_scanner::TargetFile {
        path: format!("{}\\Config\\ConsoleVariables.ini", project_roots[0].trim_end_matches('\\')),
        category: crate::core::ini_diagnostics::Category::Project,
    };
    let parsed = match ini_scanner::read_file(host, &target, Some((username, password))) {
        Ok(Some(pf)) => pf,
        Ok(None) => return CheckOutcome { status: "warning".into(), message: "ConsoleVariables.ini missing".into(), sample: target.path },
        Err(e) => return CheckOutcome { status: "offline".into(), message: e.to_string(), sample: target.path },
    };
    let cvar_value = parsed.sections.iter()
        .flat_map(|s| s.keys.iter())
        .find(|k| k.name.eq_ignore_ascii_case("r.PSOPrecaching"))
        .map(|k| k.value.clone());
    match cvar_value.as_deref() {
        Some("1") => CheckOutcome { status: "healthy".into(), message: "r.PSOPrecaching=1".into(), sample: parsed.path },
        Some(other) => CheckOutcome { status: "warning".into(), message: format!("r.PSOPrecaching={}", other), sample: parsed.path },
        None => CheckOutcome { status: "warning".into(), message: "r.PSOPrecaching not set".into(), sample: parsed.path },
    }
}

#[tauri::command]
pub fn list_recent_health_runs(db: State<'_, Db>, limit: i64) -> UecmResult<Vec<scan_runs::ScanRun>> {
    scan_runs::list_recent(&db, "health", limit)
}

#[tauri::command]
pub fn list_health_results_for_run(db: State<'_, Db>, scan_run_id: i64) -> UecmResult<Vec<health_check_runs::HealthCheckRow>> {
    health_check_runs::list_for_run(&db, scan_run_id)
}
```

The `share_configs::find_by_host` helper was added in Plan 3 T8. If it doesn't exist, add it now (small CRUD addition; alternative is to inline the SQL). Verify with `grep -n "fn find_by_host" src-tauri/src/data/share_configs.rs`.

- [ ] **Step 2: Update `commands/mod.rs` and `lib.rs`**

```rust
// commands/mod.rs
pub mod health_check;

// lib.rs
commands::health_check::run_health_check,
commands::health_check::list_recent_health_runs,
commands::health_check::list_health_results_for_run,
```

- [ ] **Step 3: Build + tests**

```bash
cd src-tauri && cargo build && cargo test --lib commands::health_check 2>&1 | tail -8 && cd ..
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src
git commit -m "feat(commands): run_health_check + list_recent_health_runs + list_health_results_for_run"
```

---


## Task 14: Frontend service + types — `src/services/tauri.ts`

**Files:**
- Modify: `src/services/tauri.ts`

- [ ] **Step 1: Add types**

Append to the existing types section:

```ts
export type Severity = "critical" | "warning" | "healthy" | "info";
export type Category = "project" | "user" | "engine";
export type RecommendedAction = "set" | "remove" | "manual";

export interface IniFinding {
  id: number | null;
  scan_run_id: number;
  machine_id: number;
  rule_id: string;
  severity: Severity;
  category: Category;
  file_path: string;
  section: string | null;
  key_name: string | null;
  line_number: number | null;
  snippet_before: string;
  snippet_after: string | null;
  recommended_action: RecommendedAction;
  recommended_value: string | null;
  symptom: string;
  rationale: string;
  fixed_at: string | null;
  skipped_at: string | null;
}

export interface ScanRun {
  id: number | null;
  scan_type: "ini" | "health";
  started_at: string | null;
  finished_at: string | null;
  machine_ids: number[];
  summary: Record<string, number> | null;
}

export interface ScanRunSummary {
  scan_run_id: number;
  critical: number;
  warning: number;
  healthy: number;
}

export type HealthStatus = "healthy" | "warning" | "critical" | "na" | "offline" | "unknown";

export interface CheckOutcome {
  status: HealthStatus;
  message: string;
  sample: string;
}

export interface HealthCheckRow {
  scan_run_id: number;
  machine_id: number;
  machine_results: Record<string, CheckOutcome>;
}

export interface HealthRunSummary {
  scan_run_id: number;
  healthy: number;
  warning: number;
  critical: number;
  offline: number;
  total: number;
}

export interface HealthProgressEvent {
  scan_run_id: number;
  machine_id: number;
  done: boolean;
  error: string | null;
}
```

- [ ] **Step 2: Add functions to `tauriApi`**

```ts
// INI scanner
async scanInis(
  machineIds: number[],
  projectPathsPerMachine: Record<number, string[]>,
  userProfile: string,
  credentialAlias: string,
): Promise<ScanRunSummary> {
  return invoke<ScanRunSummary>("scan_inis", {
    machineIds, projectPathsPerMachine, userProfile, credentialAlias,
  });
},
async listFindingsForRun(scanRunId: number): Promise<IniFinding[]> {
  return invoke<IniFinding[]>("list_findings_for_run", { scanRunId });
},
async listRecentIniRuns(limit: number): Promise<ScanRun[]> {
  return invoke<ScanRun[]>("list_recent_ini_runs", { limit });
},
async applyFinding(findingId: number, credentialAlias: string): Promise<string> {
  return invoke<string>("apply_finding", { findingId, credentialAlias });
},
async skipFinding(findingId: number): Promise<void> {
  return invoke<void>("skip_finding", { findingId });
},

// Health check
async runHealthCheck(
  machineIds: number[],
  projectPathsPerMachine: Record<number, string[]>,
  credentialAlias: string,
): Promise<HealthRunSummary> {
  return invoke<HealthRunSummary>("run_health_check", {
    machineIds, projectPathsPerMachine, credentialAlias,
  });
},
async listRecentHealthRuns(limit: number): Promise<ScanRun[]> {
  return invoke<ScanRun[]>("list_recent_health_runs", { limit });
},
async listHealthResultsForRun(scanRunId: number): Promise<HealthCheckRow[]> {
  return invoke<HealthCheckRow[]>("list_health_results_for_run", { scanRunId });
},
```

- [ ] **Step 3: Add a tauri-service spec test**

In `src/__tests__/tauri-service.spec.ts`, add cases asserting the invoke command names + arg shapes for the eight new functions. Mirror the existing pattern.

- [ ] **Step 4: Test pass**

```bash
pnpm vitest run src/__tests__/tauri-service.spec.ts 2>&1 | tail -10
```

- [ ] **Step 5: Commit**

```bash
git add src/services/tauri.ts src/__tests__/tauri-service.spec.ts
git commit -m "feat(frontend): tauri service types + functions for diagnostics"
```

---

## Task 15: Frontend stores — `useDiagnosticsStore` + `useHealthCheckStore`

**Files:**
- Create: `src/stores/diagnostics.ts`
- Create: `src/stores/healthCheck.ts`
- Create: `src/__tests__/diagnostics-store.spec.ts`
- Create: `src/__tests__/health-check-store.spec.ts`

- [ ] **Step 1: Write failing tests for `diagnostics-store.spec.ts`**

```ts
import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useDiagnosticsStore } from "@/stores/diagnostics";

vi.mock("@/services/tauri", () => ({
  tauriApi: {
    scanInis: vi.fn(async () => ({ scan_run_id: 7, critical: 1, warning: 2, healthy: 4 })),
    listFindingsForRun: vi.fn(async () => ([
      { id: 1, scan_run_id: 7, machine_id: 11, rule_id: "R001", severity: "critical",
        category: "project", file_path: "C:\\f.ini", section: "DDC", key_name: "Path",
        line_number: 1, snippet_before: "Path=X", snippet_after: "EnvPathOverride=Y",
        recommended_action: "set", recommended_value: "Y",
        symptom: "s", rationale: "r", fixed_at: null, skipped_at: null },
    ])),
    applyFinding: vi.fn(async () => "C:\\f.ini.bak.20260503-100000"),
    skipFinding: vi.fn(async () => {}),
  },
}));

describe("useDiagnosticsStore", () => {
  beforeEach(() => setActivePinia(createPinia()));

  it("runScan populates findings", async () => {
    const s = useDiagnosticsStore();
    await s.runScan([11], {}, "C:\\Users\\X", "UECM:winrm:LANPC");
    expect(s.scanRunId).toBe(7);
    expect(s.findings.length).toBe(1);
    expect(s.summary.critical).toBe(1);
  });

  it("apply marks finding as fixed", async () => {
    const s = useDiagnosticsStore();
    await s.runScan([11], {}, "C:\\Users\\X", "UECM:winrm:LANPC");
    await s.applyFinding(1, "UECM:winrm:LANPC");
    const f = s.findings.find(x => x.id === 1)!;
    expect(f.fixed_at).not.toBeNull();
  });

  it("skip marks finding as skipped", async () => {
    const s = useDiagnosticsStore();
    await s.runScan([11], {}, "C:\\Users\\X", "UECM:winrm:LANPC");
    await s.skipFinding(1);
    const f = s.findings.find(x => x.id === 1)!;
    expect(f.skipped_at).not.toBeNull();
  });
});
```

- [ ] **Step 2: Implement `stores/diagnostics.ts`**

```ts
import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { tauriApi, type IniFinding, type ScanRunSummary, type UecmError } from "@/services/tauri";

export const useDiagnosticsStore = defineStore("diagnostics", () => {
  const scanRunId = ref<number | null>(null);
  const summary = ref<ScanRunSummary>({ scan_run_id: 0, critical: 0, warning: 0, healthy: 0 });
  const findings = ref<IniFinding[]>([]);
  const isScanning = ref(false);
  const error = ref<UecmError | null>(null);

  const open = computed(() => findings.value.filter(f => !f.fixed_at && !f.skipped_at));

  async function runScan(
    machineIds: number[],
    projectPaths: Record<number, string[]>,
    userProfile: string,
    credentialAlias: string,
  ) {
    isScanning.value = true; error.value = null;
    try {
      const s = await tauriApi.scanInis(machineIds, projectPaths, userProfile, credentialAlias);
      scanRunId.value = s.scan_run_id;
      summary.value = s;
      findings.value = await tauriApi.listFindingsForRun(s.scan_run_id);
    } catch (e) {
      error.value = e as UecmError;
    } finally {
      isScanning.value = false;
    }
  }

  async function applyFinding(findingId: number, credentialAlias: string) {
    error.value = null;
    try {
      await tauriApi.applyFinding(findingId, credentialAlias);
      const f = findings.value.find(x => x.id === findingId);
      if (f) f.fixed_at = new Date().toISOString();
    } catch (e) {
      error.value = e as UecmError;
    }
  }

  async function skipFinding(findingId: number) {
    error.value = null;
    try {
      await tauriApi.skipFinding(findingId);
      const f = findings.value.find(x => x.id === findingId);
      if (f) f.skipped_at = new Date().toISOString();
    } catch (e) {
      error.value = e as UecmError;
    }
  }

  return { scanRunId, summary, findings, open, isScanning, error,
           runScan, applyFinding, skipFinding };
});
```

- [ ] **Step 3: Tests PASS**

```bash
pnpm vitest run src/__tests__/diagnostics-store.spec.ts 2>&1 | tail -10
```

- [ ] **Step 4: Write tests + implement `stores/healthCheck.ts`**

`__tests__/health-check-store.spec.ts`:

```ts
import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useHealthCheckStore } from "@/stores/healthCheck";

vi.mock("@/services/tauri", () => ({
  tauriApi: {
    runHealthCheck: vi.fn(async () => ({ scan_run_id: 9, healthy: 70, warning: 4, critical: 2, offline: 0, total: 76 })),
    listHealthResultsForRun: vi.fn(async () => ([
      { scan_run_id: 9, machine_id: 11, machine_results: {
          smb: { status: "healthy", message: "ok", sample: "Running" },
          system_write: { status: "critical", message: "fail", sample: "" },
      } },
    ])),
  },
}));

describe("useHealthCheckStore", () => {
  beforeEach(() => setActivePinia(createPinia()));

  it("runs and stores rows by machine", async () => {
    const s = useHealthCheckStore();
    await s.run([11], {}, "UECM:winrm:LANPC");
    expect(s.scanRunId).toBe(9);
    expect(s.rowsByMachine[11].smb.status).toBe("healthy");
    expect(s.rowsByMachine[11].system_write.status).toBe("critical");
  });

  it("computes per-check totals", async () => {
    const s = useHealthCheckStore();
    await s.run([11], {}, "UECM:winrm:LANPC");
    expect(s.summary.critical).toBe(2);
    expect(s.summary.healthy).toBe(70);
  });
});
```

`stores/healthCheck.ts`:

```ts
import { defineStore } from "pinia";
import { ref } from "vue";
import { tauriApi, type CheckOutcome, type HealthRunSummary, type UecmError } from "@/services/tauri";

export const useHealthCheckStore = defineStore("healthCheck", () => {
  const scanRunId = ref<number | null>(null);
  const summary = ref<HealthRunSummary>({ scan_run_id: 0, healthy: 0, warning: 0, critical: 0, offline: 0, total: 0 });
  const rowsByMachine = ref<Record<number, Record<string, CheckOutcome>>>({});
  const isRunning = ref(false);
  const error = ref<UecmError | null>(null);

  async function run(machineIds: number[], projectPaths: Record<number, string[]>, credentialAlias: string) {
    isRunning.value = true; error.value = null;
    try {
      const s = await tauriApi.runHealthCheck(machineIds, projectPaths, credentialAlias);
      scanRunId.value = s.scan_run_id;
      summary.value = s;
      const rows = await tauriApi.listHealthResultsForRun(s.scan_run_id);
      const byMachine: Record<number, Record<string, CheckOutcome>> = {};
      for (const r of rows) byMachine[r.machine_id] = r.machine_results;
      rowsByMachine.value = byMachine;
    } catch (e) {
      error.value = e as UecmError;
    } finally {
      isRunning.value = false;
    }
  }

  return { scanRunId, summary, rowsByMachine, isRunning, error, run };
});
```

- [ ] **Step 5: Tests PASS**

```bash
pnpm vitest run src/__tests__/health-check-store.spec.ts 2>&1 | tail -10
```

- [ ] **Step 6: Commit**

```bash
git add src/stores src/__tests__
git commit -m "feat(stores): useDiagnosticsStore + useHealthCheckStore with mocked-tauri tests"
```

---

## Task 16: Update `lib/healthChecks.ts` to 11 checks

**Files:**
- Modify: `src/lib/healthChecks.ts`
- Create: `src/lib/iniRules.ts`

- [ ] **Step 1: Replace `lib/healthChecks.ts`**

```ts
export type StatusKind =
  | "healthy" | "warning" | "critical"
  | "na" | "offline" | "progress" | "unknown" | "info";

export interface HealthCheckDef {
  id: string;             // matches keys in CheckOutcome map
  label: string;          // full label used in detail panel
  shortLabel: string;     // abbreviation used in matrix header
  emphasized?: boolean;   // SYS-cred + SYS-write are critical-path
  description: string;    // shown in detail "What this checks"
  remediation: string;    // shown in detail "How to fix"
  symptom: string;        // shown in detail "User-facing symptom"
}

export const HEALTH_CHECKS: HealthCheckDef[] = [
  { id: "smb", label: "SMB service running", shortLabel: "SMB",
    description: "LanmanServer service running on the host.",
    remediation: "Start-Service LanmanServer; Set-Service -StartupType Automatic.",
    symptom: "Clients cannot mount any share." },
  { id: "firewall_445", label: "Firewall 445 inbound allowed", shortLabel: "FW 445",
    description: "Inbound TCP/445 SMB rule enabled.",
    remediation: "Enable-NetFirewallRule -DisplayGroup 'File and Printer Sharing'.",
    symptom: "Test-Path \\\\HOST\\Share fails despite share existing." },
  { id: "share_reachable", label: "Shared drive reachable", shortLabel: "Share",
    description: "Test-Path against the configured share UNC.",
    remediation: "Verify host online + share present + ACL grants this user.",
    symptom: "DDC fallback to local; cluster cache miss everywhere." },
  { id: "ntfs_perm", label: "NTFS permission (Host only)", shortLabel: "NTFS",
    description: "Host's local NTFS ACL grants the share account Full Control.",
    remediation: "icacls D:\\DDC /grant ddc-svc:(OI)(CI)F.",
    symptom: "Clients can mount but get Access Denied on read/write." },
  { id: "cred_user", label: "User-level credential", shortLabel: "Cred U",
    description: "cmdkey /list contains the SMB host entry for the current user.",
    remediation: "Re-run Mode B inject step to repopulate user-level cmdkey.",
    symptom: "Interactive UE editor cannot mount the share." },
  { id: "cred_system", label: "SYSTEM-level credential", shortLabel: "Cred SYS",
    emphasized: true,
    description: "PsExec -s cmdkey /list shows the entry under LocalSystem.",
    remediation: "Re-run inject-system-credential.ps1; verify PsExec64 staged.",
    symptom: "RenderStream Service / SYSTEM tasks cannot read DDC. Hardest to debug." },
  { id: "env_vars", label: "Environment variables", shortLabel: "Env",
    description: "UE-SharedDataCachePath matches the expected UNC.",
    remediation: "Use Machines > Env vars to push the correct value.",
    symptom: "INI EnvPathOverride resolves to empty; DDC silently local-only." },
  { id: "ini_consistency", label: "INI consistency", shortLabel: "INI",
    description: "Latest INI scan shows no open critical findings on this machine.",
    remediation: "Open INI Scanner, apply suggested fixes.",
    symptom: "Misconfigured DDC paths overrule cluster settings." },
  { id: "system_write", label: "SYSTEM write test", shortLabel: "SYS Write",
    emphasized: true,
    description: "PsExec -s writes a probe file to the share. Final ground-truth test.",
    remediation: "Resolve cred_system + ntfs_perm. If both green and this is red, ACL on a parent dir likely.",
    symptom: "Service-context shader compile output cannot be cached cluster-wide." },
  { id: "pso_precaching", label: "PSO Precaching CVar", shortLabel: "PSO cvar",
    description: "Project ConsoleVariables.ini sets r.PSOPrecaching=1.",
    remediation: "Set the CVar via the project ConsoleVariables.ini.",
    symptom: "Scene-load hitches that PSO Cache files cannot fully cover." },
  { id: "gpu_consistency", label: "GPU / driver consistency", shortLabel: "GPU",
    description: "All cluster machines share the same (gpu_model, driver_version).",
    remediation: "Standardize driver version across the cluster.",
    symptom: "PSO Cache file collected on machine A is invalid on machine B." },
];
```

- [ ] **Step 2: Add `lib/iniRules.ts`**

```ts
export interface IniRuleDef {
  id: string;
  label: string;          // used in finding hierarchy ("Hardcoded DDC path")
  description: string;    // shown in detail "What's wrong"
  rationale: string;      // shown in detail "Why it matters"
  fixHint: string;        // shown in detail action panel
}

export const INI_RULES: Record<string, IniRuleDef> = {
  R001: {
    id: "R001",
    label: "Hardcoded DDC path overrides env-var",
    description: "Path= is set in DerivedDataCacheSettings without EnvPathOverride.",
    rationale: "UE only consults EnvPathOverride when it's present. With a literal Path=, the env var is ignored — silently.",
    fixHint: "Replace Path= with EnvPathOverride=UE-SharedDataCachePath; ensure the env var is set on every machine.",
  },
  R002: {
    id: "R002",
    label: "User-level DDC override (silent killer)",
    description: "EditorPerProjectUserSettings.ini contains DDC keys.",
    rationale: "User-level config is the highest-priority source. It silently masks every cluster setting until the file is cleaned up.",
    fixHint: "Remove the DDC section from the user-level file. UECM auto-backs up before removing.",
  },
  R003: {
    id: "R003",
    label: "DDC path resolves to unreachable target",
    description: "The configured path returned probe failure.",
    rationale: "Pointing at an offline share or stale UNC means cluster DDC always misses, falls back to per-machine compute.",
    fixHint: "Repoint to a live share, or fix the share / firewall / network.",
  },
  R004: {
    id: "R004",
    label: "Mapped drive letter in DDC path",
    description: "Path uses Z:\\ or similar mapped drive instead of UNC.",
    rationale: "Windows Services (RenderStream, etc.) can't see user-mapped drive letters. They fail silently when the path resolves to nothing.",
    fixHint: "Replace with the underlying \\\\HOST\\Share path.",
  },
  R005: {
    id: "R005",
    label: "Deprecated CVar present",
    description: "An obsolete CVar (e.g. r.SShaderCache) is set.",
    rationale: "Deprecated CVars do nothing in current UE; they only confuse readers.",
    fixHint: "Remove the line.",
  },
  R006: {
    id: "R006",
    label: "EnvPathOverride references missing env-var",
    description: "INI references an env-var that is not set on the machine.",
    rationale: "The override resolves to empty; DDC silently falls back to local.",
    fixHint: "Use Env vars in Machine detail to set the variable on the machine.",
  },
  R007: {
    id: "R007",
    label: "Healthy: env-var-driven DDC config",
    description: "EnvPathOverride references a populated env-var.",
    rationale: "This is the recommended cluster-friendly config.",
    fixHint: "(no fix required)",
  },
};
```

- [ ] **Step 3: Commit**

```bash
git add src/lib
git commit -m "feat(lib): expand healthChecks to 11 with full metadata + add iniRules dictionary"
```

---


## Task 17: New primitives — UecmCodeBlock, UecmKpiTile, UecmScoreTile, UecmFilterChip

**Files:**
- Create: `src/components/primitives/UecmCodeBlock.vue`
- Create: `src/components/primitives/UecmKpiTile.vue`
- Create: `src/components/primitives/UecmScoreTile.vue`
- Create: `src/components/primitives/UecmFilterChip.vue`
- Modify: `src/components/primitives/index.ts`
- Create: `src/__tests__/UecmCodeBlock.spec.ts`
- Create: `src/__tests__/UecmKpiTile.spec.ts`
- Create: `src/__tests__/UecmScoreTile.spec.ts`

These primitives were specified in `2026-05-02-uecm-design-system-overhaul.md` Phase 6 but never written. Plan 4 brings them in because INIScanner + HealthCheck need them.

- [ ] **Step 1: `UecmCodeBlock.vue`**

```vue
<script setup lang="ts">
import { computed } from "vue";
import type { UecmTone } from "./types";

const props = withDefaults(defineProps<{
  code: string;
  tone?: UecmTone;
  startLine?: number;
  highlightLine?: number;
  caption?: string;
}>(), { tone: "info", startLine: 1 });

const lines = computed(() => props.code.split("\n").map((text, i) => ({
  number: props.startLine + i,
  text,
  highlighted: props.highlightLine !== undefined && (props.startLine + i) === props.highlightLine,
})));

const toneCls = computed(() => {
  const map: Record<UecmTone, string> = {
    healthy: "border-status-healthy/30 bg-status-healthy/5",
    warning: "border-status-warning/30 bg-status-warning/5",
    critical: "border-status-critical/30 bg-status-critical/5",
    info: "border-status-info/30 bg-status-info/5",
    offline: "border-muted bg-muted/20",
    unknown: "border-muted bg-muted/20",
    progress: "border-status-info/30 bg-status-info/5",
    na: "border-muted bg-muted/20",
  };
  return map[props.tone];
});
</script>

<template>
  <div data-codeblock class="overflow-hidden rounded-md border" :class="toneCls">
    <div v-if="caption" class="border-b bg-muted/30 px-3 py-1.5 text-xs font-bold uppercase tracking-wide text-muted-foreground">
      {{ caption }}
    </div>
    <pre class="overflow-x-auto p-3 font-mono text-xs leading-5"><code><div
        v-for="line in lines" :key="line.number"
        :class="line.highlighted ? 'bg-yellow-500/15' : ''"
        class="flex"
      ><span class="mr-3 inline-block min-w-[2rem] select-none text-right text-muted-foreground">{{ line.number }}</span><span class="whitespace-pre">{{ line.text }}</span></div></code></pre>
  </div>
</template>
```

- [ ] **Step 2: `UecmKpiTile.vue`**

```vue
<script setup lang="ts">
import type { UecmTone } from "./types";

defineProps<{ label: string; value: number | string; tone?: UecmTone; sublabel?: string }>();
</script>

<template>
  <article data-kpi-tile class="bg-card px-4 py-3">
    <p class="font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">{{ label }}</p>
    <div
      class="mt-1 font-display text-3xl font-extrabold"
      :class="{
        'text-status-healthy': tone === 'healthy',
        'text-status-warning': tone === 'warning',
        'text-status-critical': tone === 'critical',
        'text-status-info': tone === 'info',
        'text-muted-foreground': tone === 'offline' || tone === 'unknown' || tone === 'na',
      }"
    >{{ value }}</div>
    <p v-if="sublabel" class="mt-0.5 text-xs text-muted-foreground">{{ sublabel }}</p>
  </article>
</template>
```

- [ ] **Step 3: `UecmScoreTile.vue`**

```vue
<script setup lang="ts">
import { computed } from "vue";
import type { UecmTone } from "./types";

const props = defineProps<{ label: string; score: number; tone: UecmTone; verdict: string }>();

const ringCls = computed(() => ({
  healthy: "ring-status-healthy",
  warning: "ring-status-warning",
  critical: "ring-status-critical",
  info: "ring-status-info",
  offline: "ring-muted",
  unknown: "ring-muted",
  progress: "ring-status-info",
  na: "ring-muted",
}[props.tone]));
</script>

<template>
  <article data-score-tile class="flex items-center gap-4 bg-card px-4 py-3">
    <div :class="['flex size-14 items-center justify-center rounded-full ring-2', ringCls]">
      <span class="font-display text-2xl font-extrabold">{{ score }}</span>
    </div>
    <div>
      <p class="font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">{{ label }}</p>
      <p class="mt-0.5 text-sm font-bold uppercase">{{ verdict }}</p>
    </div>
  </article>
</template>
```

- [ ] **Step 4: `UecmFilterChip.vue`**

```vue
<script setup lang="ts">
defineProps<{ label: string; value: string }>();
</script>

<template>
  <button data-filter-chip class="inline-flex items-center gap-1.5 rounded-full border bg-card px-3 py-1 text-xs hover:bg-accent">
    <span class="font-mono uppercase tracking-wide text-muted-foreground">{{ label }}:</span>
    <span class="font-bold">{{ value }}</span>
    <span aria-hidden="true">▾</span>
  </button>
</template>
```

- [ ] **Step 5: Update `index.ts`**

```ts
export { default as UecmCodeBlock } from "./UecmCodeBlock.vue";
export { default as UecmKpiTile } from "./UecmKpiTile.vue";
export { default as UecmScoreTile } from "./UecmScoreTile.vue";
export { default as UecmFilterChip } from "./UecmFilterChip.vue";
```

- [ ] **Step 6: Tests for the three data-bearing primitives**

`UecmCodeBlock.spec.ts`:
```ts
import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import UecmCodeBlock from "@/components/primitives/UecmCodeBlock.vue";

describe("UecmCodeBlock", () => {
  it("renders line numbers starting from startLine", () => {
    const w = mount(UecmCodeBlock, { props: { code: "a\nb", startLine: 10 } });
    const html = w.html();
    expect(html).toContain("10");
    expect(html).toContain("11");
  });
  it("highlights specified line", () => {
    const w = mount(UecmCodeBlock, { props: { code: "a\nb\nc", startLine: 5, highlightLine: 6 } });
    expect(w.html()).toMatch(/bg-yellow-500\/15/);
  });
});
```

`UecmKpiTile.spec.ts`:
```ts
import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import UecmKpiTile from "@/components/primitives/UecmKpiTile.vue";

describe("UecmKpiTile", () => {
  it("renders value", () => {
    const w = mount(UecmKpiTile, { props: { label: "Healthy", value: 42 } });
    expect(w.text()).toContain("42");
    expect(w.text()).toContain("HEALTHY");
  });
  it("applies tone class", () => {
    const w = mount(UecmKpiTile, { props: { label: "X", value: 1, tone: "critical" } });
    expect(w.html()).toContain("text-status-critical");
  });
});
```

`UecmScoreTile.spec.ts`:
```ts
import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import UecmScoreTile from "@/components/primitives/UecmScoreTile.vue";

describe("UecmScoreTile", () => {
  it("renders score + verdict", () => {
    const w = mount(UecmScoreTile, { props: { label: "Cluster", score: 70, tone: "warning", verdict: "DEGRADED" } });
    expect(w.text()).toContain("70");
    expect(w.text()).toContain("DEGRADED");
  });
});
```

- [ ] **Step 7: Tests PASS**

```bash
pnpm vitest run src/__tests__/UecmCodeBlock.spec.ts src/__tests__/UecmKpiTile.spec.ts src/__tests__/UecmScoreTile.spec.ts 2>&1 | tail -10
```

- [ ] **Step 8: Commit**

```bash
git add src/components/primitives src/__tests__
git commit -m "feat(primitives): UecmCodeBlock + UecmKpiTile + UecmScoreTile + UecmFilterChip"
```

---

## Task 18: Diagnostic components — FindingHierarchy + FindingDetail + IniScanWizard

**Files:**
- Create: `src/components/diagnostics/FindingHierarchy.vue`
- Create: `src/components/diagnostics/FindingDetail.vue`
- Create: `src/components/modals/IniScanWizard.vue`
- Create: `src/__tests__/FindingHierarchy.spec.ts`
- Create: `src/__tests__/FindingDetail.spec.ts`
- Create: `src/__tests__/IniScanWizard.spec.ts`

- [ ] **Step 1: `FindingHierarchy.vue`** (~150 lines)

Renders a 3-level tree: machine → file (with category badge) → finding row. Click a finding emits `select(finding)`. Selected row gets `bg-accent`. Counts badge per level (e.g. "RENDER-02 · 2C 3W"). Use `groupBy` helper inline.

```vue
<script setup lang="ts">
import { computed } from "vue";
import UecmIcon from "@/components/primitives/UecmIcon.vue";
import UecmStatusBadge from "@/components/primitives/UecmStatusBadge.vue";
import type { IniFinding } from "@/services/tauri";

const props = defineProps<{
  findings: IniFinding[];
  selectedId: number | null;
  hostnameById: Record<number, string>;
  groupBy: "machine" | "category";
}>();
const emit = defineEmits<{ select: [finding: IniFinding] }>();

function group<T>(arr: T[], fn: (t: T) => string): Record<string, T[]> {
  const out: Record<string, T[]> = {};
  for (const x of arr) {
    const k = fn(x);
    (out[k] ||= []).push(x);
  }
  return out;
}

const tree = computed(() => {
  const top = props.groupBy === "machine"
    ? group(props.findings, f => String(f.machine_id))
    : group(props.findings, f => f.category);
  return Object.entries(top).map(([k, items]) => {
    const byFile = group(items, f => f.file_path);
    const counts = items.reduce((acc, f) => {
      if (f.severity === "critical") acc.c++; else if (f.severity === "warning") acc.w++;
      return acc;
    }, { c: 0, w: 0 });
    return {
      key: k,
      label: props.groupBy === "machine" ? (props.hostnameById[Number(k)] ?? `#${k}`) : k.toUpperCase(),
      counts,
      files: Object.entries(byFile).map(([fp, fItems]) => ({ filePath: fp, items: fItems })),
    };
  });
});
</script>

<template>
  <div data-finding-hierarchy class="overflow-y-auto">
    <div v-for="grp in tree" :key="grp.key" class="border-b last:border-b-0">
      <div class="sticky top-0 z-10 flex items-center gap-2 bg-muted/40 px-4 py-2 text-sm font-bold">
        <UecmIcon name="server" size="14" />
        {{ grp.label }}
        <UecmStatusBadge v-if="grp.counts.c" tone="critical" :label="`${grp.counts.c}C`" size="sm" />
        <UecmStatusBadge v-if="grp.counts.w" tone="warning" :label="`${grp.counts.w}W`" size="sm" />
      </div>
      <div v-for="file in grp.files" :key="file.filePath" class="border-t">
        <div class="flex items-center gap-2 px-7 py-1.5 text-xs">
          <UecmIcon name="file-text" size="12" class="text-muted-foreground" />
          <span class="font-mono text-muted-foreground">{{ file.filePath.split('\\').slice(-2).join('\\') }}</span>
        </div>
        <button
          v-for="f in file.items"
          :key="f.id ?? `${f.scan_run_id}-${f.line_number}`"
          data-finding-row
          class="flex w-full items-center gap-2 px-12 py-1.5 text-xs hover:bg-accent/50"
          :class="props.selectedId === f.id ? 'bg-accent' : ''"
          @click="emit('select', f)"
        >
          <UecmStatusBadge :tone="f.severity" :label="f.severity[0].toUpperCase()" size="sm" />
          <span class="flex-1 truncate text-left">{{ f.rule_id }} · {{ f.section }}</span>
          <span class="font-mono text-[10px] text-muted-foreground">L{{ f.line_number ?? '?' }}</span>
        </button>
      </div>
    </div>
  </div>
</template>
```

Spec test asserts: renders machine groups; click emits select; counts badge shows correct number; selected row has bg-accent class.

- [ ] **Step 2: `FindingDetail.vue`** (~180 lines)

3-column diagnostic header (What / Why / Symptom) + before/after diff via `UecmCodeBlock` + 4 action buttons (Apply / Custom / Open file / Skip). Emits `apply(finding)` and `skip(finding)`.

```vue
<script setup lang="ts">
import { computed } from "vue";
import UecmCodeBlock from "@/components/primitives/UecmCodeBlock.vue";
import UecmStatusBadge from "@/components/primitives/UecmStatusBadge.vue";
import Button from "@/components/ui/Button.vue";
import { INI_RULES } from "@/lib/iniRules";
import type { IniFinding } from "@/services/tauri";

const props = defineProps<{ finding: IniFinding | null; busy: boolean }>();
const emit = defineEmits<{ apply: [f: IniFinding]; skip: [f: IniFinding] }>();

const rule = computed(() => props.finding ? (INI_RULES[props.finding.rule_id] ?? null) : null);
</script>

<template>
  <div v-if="!finding" data-finding-empty class="grid h-full place-items-center text-sm text-muted-foreground">
    Select a finding to view its diagnostic.
  </div>
  <div v-else data-finding-detail class="flex h-full flex-col overflow-y-auto">
    <header class="flex items-center gap-3 border-b bg-card/30 px-6 py-4">
      <UecmStatusBadge :tone="finding.severity" :label="finding.severity" size="md" />
      <div>
        <h2 class="font-display text-lg font-extrabold">{{ rule?.label ?? finding.rule_id }}</h2>
        <p class="font-mono text-xs text-muted-foreground">{{ finding.file_path }} · L{{ finding.line_number ?? '?' }}</p>
      </div>
    </header>
    <div class="grid gap-3 px-6 py-4 md:grid-cols-3">
      <div class="rounded-md border bg-card p-3">
        <p class="font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">What's wrong</p>
        <p class="mt-1 text-sm">{{ rule?.description ?? finding.rationale }}</p>
      </div>
      <div class="rounded-md border bg-card p-3">
        <p class="font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">Why it matters</p>
        <p class="mt-1 text-sm">{{ rule?.rationale ?? finding.rationale }}</p>
      </div>
      <div class="rounded-md border bg-card p-3">
        <p class="font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">User-facing symptom</p>
        <p class="mt-1 text-sm">{{ finding.symptom }}</p>
      </div>
    </div>
    <div class="grid gap-3 px-6 pb-4 md:grid-cols-2">
      <UecmCodeBlock :code="finding.snippet_before" tone="critical" caption="Detected (current)"
                     :start-line="(finding.line_number ?? 1) - 1" :highlight-line="finding.line_number ?? 0" />
      <UecmCodeBlock v-if="finding.snippet_after" :code="finding.snippet_after" tone="healthy" caption="Suggested fix"
                     :start-line="(finding.line_number ?? 1) - 1" :highlight-line="finding.line_number ?? 0" />
    </div>
    <footer class="mt-auto flex items-center gap-2 border-t bg-card/30 px-6 py-3">
      <Button data-apply-btn :disabled="finding.recommended_action === 'manual' || finding.fixed_at != null || busy"
              @click="emit('apply', finding)">
        {{ busy ? 'Applying…' : 'Apply suggestion' }}
      </Button>
      <Button variant="outline" disabled>Custom edit</Button>
      <Button variant="outline" disabled>Open file</Button>
      <Button data-skip-btn variant="ghost" :disabled="finding.skipped_at != null"
              @click="emit('skip', finding)">Skip</Button>
      <p v-if="finding.fixed_at" class="ml-auto text-xs text-status-healthy">Applied {{ finding.fixed_at }}</p>
      <p v-else-if="finding.skipped_at" class="ml-auto text-xs text-muted-foreground">Skipped</p>
    </footer>
  </div>
</template>
```

Spec test asserts: empty state renders when finding null; apply button disabled for manual action; clicking apply emits with the finding; status indicator after fixed_at.

- [ ] **Step 3: `IniScanWizard.vue`** (~140 lines)

Modal with 3 fields: machine multi-select (checkboxes from `useMachinesStore.machines`), credential dropdown (from `useCredentialsStore.credentials`), optional textarea for project paths (one path per line, parsed into `Record<machineId, string[]>` — for v1 same paths apply to all selected machines), user profile path (text input, default `C:\\Users\\lanpc`).

```vue
<script setup lang="ts">
import { computed, ref, watch } from "vue";
import BaseModal from "./BaseModal.vue";
import Button from "@/components/ui/Button.vue";
import Input from "@/components/ui/Input.vue";
import { useMachinesStore } from "@/stores/machines";
import { useCredentialsStore } from "@/stores/credentials";
import { useDiagnosticsStore } from "@/stores/diagnostics";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ close: [] }>();

const machines = useMachinesStore();
const creds = useCredentialsStore();
const diag = useDiagnosticsStore();

const selected = ref<Set<number>>(new Set());
const credAlias = ref<string>("");
const userProfile = ref<string>("C:\\Users\\lanpc");
const projectPathsRaw = ref<string>("");

watch(() => props.open, async (val) => {
  if (val) { await machines.loadMachines(); await creds.load(); }
});

const winrmCreds = computed(() => creds.credentials.filter(c => c.kind === "winrm"));
const projectPaths = computed(() => projectPathsRaw.value
  .split("\n").map(s => s.trim()).filter(Boolean));

async function onRun() {
  const ids = Array.from(selected.value);
  if (ids.length === 0 || !credAlias.value) return;
  const perMachine: Record<number, string[]> = {};
  for (const id of ids) perMachine[id] = projectPaths.value;
  await diag.runScan(ids, perMachine, userProfile.value, credAlias.value);
  emit("close");
}

function toggle(id: number) {
  const s = new Set(selected.value);
  if (s.has(id)) s.delete(id); else s.add(id);
  selected.value = s;
}
</script>

<template>
  <BaseModal :open="open" title="Run INI scan" size="lg" @close="emit('close')">
    <div class="space-y-4">
      <div>
        <p class="mb-2 font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">Machines</p>
        <ul class="grid grid-cols-2 gap-1 text-sm">
          <li v-for="m in machines.machines" :key="m.id ?? m.ip" class="flex items-center gap-2">
            <input type="checkbox" :checked="m.id != null && selected.has(m.id)" @change="m.id != null && toggle(m.id)" />
            <span>{{ m.hostname }} <span class="font-mono text-xs text-muted-foreground">{{ m.ip }}</span></span>
          </li>
        </ul>
      </div>
      <div>
        <p class="mb-2 font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">Credential</p>
        <select data-cred-select v-model="credAlias" class="w-full rounded-md border bg-background px-2 py-1 text-sm">
          <option value="">— pick —</option>
          <option v-for="c in winrmCreds" :key="c.alias" :value="c.alias">{{ c.alias }}</option>
        </select>
      </div>
      <div>
        <p class="mb-2 font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">User profile</p>
        <Input v-model="userProfile" placeholder="C:\\Users\\lanpc" />
      </div>
      <div>
        <p class="mb-2 font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">Project paths (optional, one per line)</p>
        <textarea v-model="projectPathsRaw" rows="3" class="w-full rounded-md border bg-background px-2 py-1 font-mono text-xs"
                  placeholder="E:\\Work\\EXLY"></textarea>
      </div>
    </div>
    <template #footer>
      <Button variant="outline" @click="emit('close')">Cancel</Button>
      <Button data-run-scan-btn :disabled="!credAlias || selected.size === 0 || diag.isScanning" @click="onRun">
        {{ diag.isScanning ? "Scanning…" : `Run on ${selected.size} machine(s)` }}
      </Button>
    </template>
  </BaseModal>
</template>
```

Spec test: opens with no preselection; selecting machine + cred enables Run; clicking Run calls `diag.runScan` with correct args; closes modal afterward.

- [ ] **Step 4: Tests PASS**

```bash
pnpm vitest run src/__tests__/FindingHierarchy.spec.ts src/__tests__/FindingDetail.spec.ts src/__tests__/IniScanWizard.spec.ts 2>&1 | tail -15
```

- [ ] **Step 5: Commit**

```bash
git add src/components src/__tests__
git commit -m "feat(diagnostics): FindingHierarchy + FindingDetail + IniScanWizard"
```

---

## Task 19: Rewrite `INIScanner.vue` view

**Files:**
- Modify: `src/views/INIScanner.vue` (full replace)
- Create: `src/__tests__/INIScanner-view.spec.ts`

- [ ] **Step 1: Write failing view test**

```ts
import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";
import INIScanner from "@/views/INIScanner.vue";

vi.mock("@/services/tauri", () => ({
  tauriApi: {
    listMachines: vi.fn(async () => []),
    listCredentials: vi.fn(async () => []),
    listFindingsForRun: vi.fn(async () => []),
    listRecentIniRuns: vi.fn(async () => []),
  },
}));

describe("INIScanner view", () => {
  beforeEach(() => setActivePinia(createPinia()));
  it("renders empty state when no scan run yet", () => {
    const w = mount(INIScanner);
    expect(w.text()).toMatch(/Run an INI scan/i);
  });
  it("shows scan button", () => {
    const w = mount(INIScanner);
    expect(w.find("[data-open-ini-scan-btn]").exists()).toBe(true);
  });
});
```

- [ ] **Step 2: Replace `views/INIScanner.vue`**

```vue
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import UecmPageHeader from "@/components/primitives/UecmPageHeader.vue";
import UecmKpiTile from "@/components/primitives/UecmKpiTile.vue";
import UecmIcon from "@/components/primitives/UecmIcon.vue";
import Button from "@/components/ui/Button.vue";
import FindingHierarchy from "@/components/diagnostics/FindingHierarchy.vue";
import FindingDetail from "@/components/diagnostics/FindingDetail.vue";
import IniScanWizard from "@/components/modals/IniScanWizard.vue";
import { useDiagnosticsStore } from "@/stores/diagnostics";
import { useMachinesStore } from "@/stores/machines";
import { useCredentialsStore } from "@/stores/credentials";
import type { IniFinding } from "@/services/tauri";

const diag = useDiagnosticsStore();
const machines = useMachinesStore();
const creds = useCredentialsStore();

const showWizard = ref(false);
const selectedFinding = ref<IniFinding | null>(null);
const grouping = ref<"machine" | "category">("machine");
const applying = ref(false);

const hostnameById = computed<Record<number, string>>(() => {
  const out: Record<number, string> = {};
  for (const m of machines.machines) if (m.id != null) out[m.id] = m.hostname;
  return out;
});

onMounted(async () => {
  await machines.loadMachines();
  await creds.load();
});

async function onApply(f: IniFinding) {
  if (creds.credentials.length === 0) return;
  applying.value = true;
  await diag.applyFinding(f.id!, creds.credentials[0].alias);
  applying.value = false;
}
async function onSkip(f: IniFinding) { await diag.skipFinding(f.id!); }
</script>

<template>
  <div class="flex h-full flex-col">
    <div class="space-y-4 p-6">
      <UecmPageHeader title="INI Scanner" eyebrow="Config drift"
        description="Scan project / user / engine INI files across machines, diagnose conflicts, apply fixes with auto-backup.">
        <template #actions>
          <Button data-open-ini-scan-btn @click="showWizard = true">
            <UecmIcon name="play" /> Run scan
          </Button>
        </template>
      </UecmPageHeader>
      <section class="grid grid-cols-4 gap-px overflow-hidden rounded-lg border bg-border">
        <UecmKpiTile label="Critical" :value="diag.summary.critical" tone="critical" />
        <UecmKpiTile label="Warning"  :value="diag.summary.warning"  tone="warning" />
        <UecmKpiTile label="Healthy"  :value="diag.summary.healthy"  tone="healthy" />
        <UecmKpiTile label="Open"     :value="diag.open.length"      tone="info" />
      </section>
    </div>

    <section v-if="diag.findings.length === 0" class="grid flex-1 place-items-center text-center">
      <div>
        <UecmIcon name="file-search" size="32" class="mx-auto text-muted-foreground" />
        <p class="mt-2 font-display text-lg font-extrabold">Run an INI scan</p>
        <p class="mt-1 text-sm text-muted-foreground">No scan results yet. Click "Run scan" above.</p>
      </div>
    </section>

    <section v-else class="grid min-h-0 flex-1 grid-cols-1 lg:grid-cols-[2fr_3fr]">
      <FindingHierarchy class="border-r" :findings="diag.findings" :selected-id="selectedFinding?.id ?? null"
                        :hostname-by-id="hostnameById" :group-by="grouping"
                        @select="selectedFinding = $event" />
      <FindingDetail :finding="selectedFinding" :busy="applying"
                     @apply="onApply" @skip="onSkip" />
    </section>

    <IniScanWizard :open="showWizard" @close="showWizard = false" />
  </div>
</template>
```

- [ ] **Step 3: Tests PASS**

```bash
pnpm vitest run src/__tests__/INIScanner-view.spec.ts 2>&1 | tail -10
```

- [ ] **Step 4: Commit**

```bash
git add src/views/INIScanner.vue src/__tests__/INIScanner-view.spec.ts
git commit -m "feat(views): rewrite INI Scanner with hierarchy + diagnostic detail + scan wizard"
```

---


## Task 20: HealthMatrix component + HealthCheckWizard

**Files:**
- Create: `src/components/diagnostics/HealthMatrix.vue`
- Create: `src/components/modals/HealthCheckWizard.vue`
- Create: `src/__tests__/HealthMatrix.spec.ts`
- Create: `src/__tests__/HealthCheckWizard.spec.ts`

- [ ] **Step 1: `HealthMatrix.vue`** — table with sticky machine column on left and 11 check columns on right. Each cell is a `UecmMatrixCell` with the cell tone derived from `rowsByMachine[machineId][checkId].status`. Click a cell emits `select({machineId, checkId})`. Emphasized columns (`cred_system`, `system_write`) get a 2px primary underline on the header.

```vue
<script setup lang="ts">
import UecmMatrixCell from "@/components/primitives/UecmMatrixCell.vue";
import UecmStatusDot from "@/components/primitives/UecmStatusDot.vue";
import { HEALTH_CHECKS, type StatusKind } from "@/lib/healthChecks";
import type { Machine } from "@/services/tauri";
import type { CheckOutcome } from "@/services/tauri";

const props = defineProps<{
  machines: Machine[];
  rowsByMachine: Record<number, Record<string, CheckOutcome>>;
  selectedMachineId: number | null;
  selectedCheckId: string | null;
}>();
const emit = defineEmits<{ select: [{ machineId: number; checkId: string }] }>();

function cellStatus(machineId: number, checkId: string): StatusKind {
  return (props.rowsByMachine[machineId]?.[checkId]?.status as StatusKind) ?? "unknown";
}
</script>

<template>
  <div class="overflow-auto">
    <table data-health-matrix class="min-w-[920px] border-collapse text-xs">
      <thead class="sticky top-0 bg-card">
        <tr>
          <th class="sticky left-0 z-10 min-w-[180px] border-b bg-card px-3 py-2 text-left">Machine</th>
          <th
            v-for="c in HEALTH_CHECKS" :key="c.id"
            class="border-b px-2 py-2 text-center align-bottom"
            :class="c.emphasized ? 'border-b-2 border-primary' : ''"
          >
            <span class="font-mono uppercase">{{ c.shortLabel }}</span>
          </th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="m in machines" :key="m.id ?? m.ip">
          <td class="sticky left-0 z-10 min-w-[180px] border-b bg-card px-3 py-2">
            <div class="flex items-center gap-2">
              <UecmStatusDot :tone="(m.status as StatusKind)" size="sm" />
              <div>
                <div class="font-mono text-[12px] font-medium">{{ m.hostname }}</div>
                <div class="font-mono text-[10px] text-muted-foreground">{{ m.ip }}</div>
              </div>
            </div>
          </td>
          <td v-for="c in HEALTH_CHECKS" :key="c.id" class="border-b px-1 py-1 text-center">
            <button data-matrix-cell @click="m.id != null && emit('select', { machineId: m.id, checkId: c.id })">
              <UecmMatrixCell
                :tone="cellStatus(m.id ?? -1, c.id)"
              />
            </button>
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
```

- [ ] **Step 2: `HealthCheckWizard.vue`** — modal mirroring `IniScanWizard` but calling `useHealthCheckStore.run`. Same machines + cred selectors; project paths textarea (used for the PSO Precaching CVar derived check). ~110 lines.

- [ ] **Step 3: Tests for both**

`HealthMatrix.spec.ts`:
```ts
import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import HealthMatrix from "@/components/diagnostics/HealthMatrix.vue";

const machines = [{ id: 1, hostname: "RENDER-01", ip: "192.168.10.21", role: "render", status: "online", last_seen_at: null }];
const rows = { 1: { smb: { status: "healthy", message: "ok", sample: "" } } };

describe("HealthMatrix", () => {
  it("renders 11 columns", () => {
    const w = mount(HealthMatrix, { props: { machines, rowsByMachine: rows, selectedMachineId: null, selectedCheckId: null } });
    expect(w.findAll("th").length).toBeGreaterThanOrEqual(12);
  });
  it("emits select on cell click", async () => {
    const w = mount(HealthMatrix, { props: { machines, rowsByMachine: rows, selectedMachineId: null, selectedCheckId: null } });
    await w.find("[data-matrix-cell]").trigger("click");
    expect(w.emitted("select")?.[0]?.[0]).toMatchObject({ machineId: 1 });
  });
});
```

- [ ] **Step 4: Tests PASS**

```bash
pnpm vitest run src/__tests__/HealthMatrix.spec.ts src/__tests__/HealthCheckWizard.spec.ts 2>&1 | tail -10
```

- [ ] **Step 5: Commit**

```bash
git add src/components src/__tests__
git commit -m "feat(diagnostics): HealthMatrix + HealthCheckWizard"
```

---

## Task 21: Rewrite `HealthCheck.vue` view

**Files:**
- Modify: `src/views/HealthCheck.vue` (full replace)
- Create: `src/__tests__/HealthCheck-view.spec.ts`

- [ ] **Step 1: Replace `views/HealthCheck.vue`**

```vue
<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import UecmPageHeader from "@/components/primitives/UecmPageHeader.vue";
import UecmKpiTile from "@/components/primitives/UecmKpiTile.vue";
import UecmScoreTile from "@/components/primitives/UecmScoreTile.vue";
import UecmStatusBadge from "@/components/primitives/UecmStatusBadge.vue";
import UecmIcon from "@/components/primitives/UecmIcon.vue";
import Button from "@/components/ui/Button.vue";
import HealthMatrix from "@/components/diagnostics/HealthMatrix.vue";
import HealthCheckWizard from "@/components/modals/HealthCheckWizard.vue";
import { HEALTH_CHECKS } from "@/lib/healthChecks";
import { useMachinesStore } from "@/stores/machines";
import { useHealthCheckStore } from "@/stores/healthCheck";

const machines = useMachinesStore();
const hc = useHealthCheckStore();

const showWizard = ref(false);
const selected = ref<{ machineId: number; checkId: string } | null>(null);

onMounted(() => machines.loadMachines());

const score = computed(() => {
  const t = hc.summary.total || 1;
  return Math.max(0, Math.round(((hc.summary.healthy - hc.summary.critical * 0.75 - hc.summary.warning * 0.35) / t) * 100));
});
const tone = computed<"healthy" | "warning" | "critical" | "info">(() => {
  if (hc.summary.critical > 0) return "critical";
  if (hc.summary.warning > 0) return "warning";
  if (hc.summary.healthy > 0) return "healthy";
  return "info";
});
const verdict = computed(() => tone.value === "critical" ? "ATTENTION"
                            : tone.value === "warning" ? "DEGRADED"
                            : tone.value === "healthy" ? "HEALTHY" : "IDLE");

const selectedDetail = computed(() => {
  if (!selected.value) return null;
  const def = HEALTH_CHECKS.find(c => c.id === selected.value!.checkId);
  const outcome = hc.rowsByMachine[selected.value.machineId]?.[selected.value.checkId];
  const machine = machines.machines.find(m => m.id === selected.value!.machineId);
  return { def, outcome, machine };
});
</script>

<template>
  <div class="flex h-full flex-col">
    <div class="space-y-4 p-6">
      <UecmPageHeader title="Health Check" eyebrow="Matrix"
        description="11 checks per machine. Click a cell for the diagnostic detail.">
        <template #actions>
          <Button data-open-health-wizard-btn @click="showWizard = true">
            <UecmIcon name="play" /> Run full check
          </Button>
        </template>
      </UecmPageHeader>
      <section class="grid grid-cols-5 gap-px overflow-hidden rounded-lg border bg-border">
        <UecmScoreTile label="Cluster Score" :score="score" :tone="tone" :verdict="verdict" />
        <UecmKpiTile label="Healthy"  :value="hc.summary.healthy"  tone="healthy" />
        <UecmKpiTile label="Warning"  :value="hc.summary.warning"  tone="warning" />
        <UecmKpiTile label="Critical" :value="hc.summary.critical" tone="critical" />
        <UecmKpiTile label="Offline"  :value="hc.summary.offline"  tone="offline" />
      </section>
    </div>

    <section v-if="machines.machines.length === 0" class="grid flex-1 place-items-center text-center">
      <p class="text-sm text-muted-foreground">No machines registered. Use Machines &gt; Scan first.</p>
    </section>
    <section v-else-if="hc.scanRunId === null" class="grid flex-1 place-items-center text-center">
      <div>
        <UecmIcon name="heart-pulse" size="32" class="mx-auto text-muted-foreground" />
        <p class="mt-2 font-display text-lg font-extrabold">Run a full health check</p>
        <p class="mt-1 text-sm text-muted-foreground">Click "Run full check" to populate the matrix.</p>
      </div>
    </section>
    <section v-else class="grid min-h-0 flex-1 grid-cols-1 lg:grid-cols-[3fr_2fr]">
      <HealthMatrix class="border-r"
                    :machines="machines.machines"
                    :rows-by-machine="hc.rowsByMachine"
                    :selected-machine-id="selected?.machineId ?? null"
                    :selected-check-id="selected?.checkId ?? null"
                    @select="selected = $event" />
      <aside data-health-detail class="overflow-y-auto p-6">
        <div v-if="!selectedDetail">
          <p class="text-sm text-muted-foreground">Select a cell to view its diagnostic.</p>
        </div>
        <div v-else>
          <header class="flex items-center gap-3">
            <UecmStatusBadge :tone="(selectedDetail.outcome?.status as any) ?? 'unknown'"
                             :label="selectedDetail.outcome?.status ?? 'unknown'" size="md" />
            <h2 class="font-display text-lg font-extrabold">{{ selectedDetail.def?.label }}</h2>
          </header>
          <p class="mt-1 font-mono text-xs text-muted-foreground">
            {{ selectedDetail.machine?.hostname }} · {{ selectedDetail.machine?.ip }}
          </p>
          <div class="mt-4 space-y-3">
            <div class="rounded-md border bg-card p-3">
              <p class="font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">What this checks</p>
              <p class="mt-1 text-sm">{{ selectedDetail.def?.description }}</p>
            </div>
            <div class="rounded-md border bg-card p-3">
              <p class="font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">User-facing symptom</p>
              <p class="mt-1 text-sm">{{ selectedDetail.def?.symptom }}</p>
            </div>
            <div class="rounded-md border bg-card p-3">
              <p class="font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">How to fix</p>
              <p class="mt-1 text-sm">{{ selectedDetail.def?.remediation }}</p>
            </div>
            <div class="rounded-md border bg-card p-3">
              <p class="font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">Last probe output</p>
              <pre class="mt-1 font-mono text-xs text-muted-foreground whitespace-pre-wrap">{{ selectedDetail.outcome?.message }}</pre>
              <p v-if="selectedDetail.outcome?.sample" class="mt-1 font-mono text-[11px] text-muted-foreground">Sample: {{ selectedDetail.outcome.sample }}</p>
            </div>
          </div>
        </div>
      </aside>
    </section>

    <HealthCheckWizard :open="showWizard" @close="showWizard = false" />
  </div>
</template>
```

- [ ] **Step 2: Spec test**

`__tests__/HealthCheck-view.spec.ts`:
```ts
import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";
import HealthCheck from "@/views/HealthCheck.vue";

vi.mock("@/services/tauri", () => ({
  tauriApi: {
    listMachines: vi.fn(async () => []),
    listCredentials: vi.fn(async () => []),
    listHealthResultsForRun: vi.fn(async () => []),
  },
}));

describe("HealthCheck view", () => {
  beforeEach(() => setActivePinia(createPinia()));
  it("renders empty state with run button", () => {
    const w = mount(HealthCheck);
    expect(w.find("[data-open-health-wizard-btn]").exists()).toBe(true);
  });
});
```

- [ ] **Step 3: Tests PASS**

```bash
pnpm vitest run src/__tests__/HealthCheck-view.spec.ts 2>&1 | tail -10
```

- [ ] **Step 4: Commit**

```bash
git add src/views/HealthCheck.vue src/__tests__/HealthCheck-view.spec.ts
git commit -m "feat(views): rewrite Health Check with score + KPI strip + matrix + check detail"
```

---

## Task 22: lanPC E2E — full Plan 4 verification

**Files:** none (verification task).

- [ ] **Step 1: Sync + rebuild on lanPC**

```bash
ssh lanpc "p4 sync"
ssh lanpc "cd /d E:\code\ue-cache-manager && pnpm install && pnpm tauri build"
```

- [ ] **Step 2: Set up INI test fixtures on lanPC**

```bash
# As admin (RDP or PsExec64 -accepteula -s):
mkdir E:\test-fixtures\PluralityProject\Config
mkdir E:\test-fixtures\PluralityProject\Config\Windows
```

Create `E:\test-fixtures\PluralityProject\Config\DefaultEngine.ini` with:

```
[/Script/UnrealEd.DerivedDataCacheSettings]
Path=D:\OldCache
```

Create `E:\test-fixtures\PluralityProject\Config\ConsoleVariables.ini` with:

```
[Startup]
r.SShaderCache=1
```

Create the user-level fixture (back up first; restore after E2E):

```powershell
$user = "$env:LOCALAPPDATA\UnrealEngine\5.4\Saved\Config\WindowsEditor\EditorPerProjectUserSettings.ini"
if (Test-Path $user) { Copy-Item $user "$user.bak.uecm-e2e" -Force }
Set-Content $user @"
[/Script/UnrealEd.DerivedDataCacheSettings]
Path=C:\local-ddc
"@
```

- [ ] **Step 3: Launch UECM (no RunAs)**

Double-click `E:\code\ue-cache-manager\src-tauri\target\release\uecm.exe`.

- [ ] **Step 4: Run an INI scan against lanPC**

UECM → INI Scanner → Run scan → select lanPC → cred = `UECM:winrm:LANPC` → user profile = `C:\Users\lanpc` → project paths textarea = `E:\test-fixtures\PluralityProject` → Run.

Expected:
- Scan completes within 30s.
- Summary strip shows ≥1 critical, ≥1 warning.
- Hierarchy shows machine `LANPC` → 4 file rows (DefaultEngine.ini, ConsoleVariables.ini, BaseEngine.ini, EditorPerProjectUserSettings.ini if 5.4 detected).
- DefaultEngine.ini shows R001 critical.
- ConsoleVariables.ini shows R005 warning.
- EditorPerProjectUserSettings.ini shows R002 critical.

- [ ] **Step 5: Apply suggestion on R001**

Click R001 in the hierarchy → click "Apply suggestion" in the detail panel.

Expected:
- Spinner → green "Applied" timestamp shown.
- SSH check: `ssh lanpc "type E:\test-fixtures\PluralityProject\Config\DefaultEngine.ini"` shows `EnvPathOverride=UE-SharedDataCachePath` and no `Path=`.
- A `.bak.<timestamp>` file exists alongside the original.

- [ ] **Step 6: Run health check against lanPC**

UECM → Health Check → Run full check → select lanPC → cred = `UECM:winrm:LANPC` → project paths = `E:\test-fixtures\PluralityProject` → Run.

Expected:
- Score tile shows a number.
- 11 columns visible in the matrix; lanPC row populated.
- `smb`, `firewall_445`, `env_vars` cells likely healthy/warning depending on lanPC state.
- `cred_system` and `system_write` likely warning unless PsExec64 was staged at `%LOCALAPPDATA%\UECM\PsExec64.exe`.
- `ini_consistency` reflects the previous scan (warning, since R002 + R005 still open).
- `pso_precaching` shows warning (`r.PSOPrecaching` not set in fixture).
- `gpu_consistency` shows healthy (only one machine in cluster).

- [ ] **Step 7: Cleanup**

```powershell
$user = "$env:LOCALAPPDATA\UnrealEngine\5.4\Saved\Config\WindowsEditor\EditorPerProjectUserSettings.ini"
if (Test-Path "$user.bak.uecm-e2e") {
    Move-Item "$user.bak.uecm-e2e" $user -Force
} else {
    Remove-Item $user -Force -ErrorAction SilentlyContinue
}
Remove-Item E:\test-fixtures -Recurse -Force
```

- [ ] **Step 8: Mark deliverables ✅ in summary report.**

If any deliverable failed, file it as a `fix:` follow-up commit per Plan 3 conventions.

---

## Task 23: Final integration — README + production build smoke

**Files:**
- Modify: `README.md`
- Modify: `docs/superpowers/specs/2026-05-01-uecm-design.md` (check Plan 4 box)

- [ ] **Step 1: Run full test suite**

```bash
export PATH="/Users/bip.lan/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
pnpm test 2>&1 | tail -20
cd src-tauri && cargo test 2>&1 | tail -10 && cd ..
```

Expected: ~95 frontend + ~85 backend tests pass.

- [ ] **Step 2: Production build smoke (macOS)**

```bash
pnpm tauri build 2>&1 | tail -10
```

- [ ] **Step 3: README update**

Add Plan 4 section under existing Plan 3 section:

```
## Plan 4 — Diagnostics

What's new:
- INI Scanner: scan project / user / engine INI files; rule engine R001-R007 with one-click apply + auto-backup
- Cluster Health Check: 11-row matrix with derived INI consistency + GPU consistency; emphasized SYSTEM-cred + SYSTEM-write columns
- New primitives: UecmCodeBlock (diff), UecmKpiTile, UecmScoreTile, UecmFilterChip
- New PowerShell sidecars: read-ini-file.ps1, health-probes.ps1
```

- [ ] **Step 4: Update spec progress checkbox**

In `docs/superpowers/specs/2026-05-01-uecm-design.md` section "附：实施计划进度", change Plan 4 to:

```
- [x] **Plan 4：诊断模块** — `docs/superpowers/plans/2026-05-03-uecm-plan-4-diagnostics.md`
  - INI scanner + 7 rules + apply-fix; 11-row Health Check matrix; new primitives
  - 工期：~2 周
  - 状态：✅ 已执行完成（YYYY-MM-DD）
```

- [ ] **Step 5: Commit**

```bash
git add README.md docs/superpowers/specs/2026-05-01-uecm-design.md
git commit -m "docs: update README + spec progress with Plan 4 completion"
```

---

## Summary

At the end of Plan 4:

1. ✅ INI scanner runs over multi-machine cluster, populates `scan_runs` + `ini_findings` rows.
2. ✅ Rule engine emits findings for 6 detection rules (R001/R002/R004/R005/R006) + 1 health rule (R007). R003 (path unreachable) reachable via the WinRM probe layer.
3. ✅ "Apply suggestion" applies the recommended action with auto-backup, reads back to verify, marks finding `fixed_at`.
4. ✅ Health Check runs 8 active probes per machine + 3 derived checks; persists to `health_check_runs`.
5. ✅ Matrix UI shows 11 columns × N machines with emphasized critical-path columns; per-cell detail shows What / Symptom / How-to-fix + last probe output.
6. ✅ `useDiagnosticsStore` + `useHealthCheckStore` decouple the two subsystems but share the cluster store + machines store.
7. ✅ All design-overhaul-spec'd primitives (`UecmCodeBlock`, `UecmKpiTile`, `UecmScoreTile`, `UecmFilterChip`) now exist and are tested.
8. ✅ Production build green; lanPC E2E verified 6/6 deliverables.

**Plan 5** will build DDC Pak generation + distribution on top of this trust layer (run health check pre-flight; refuse distribute when share_reachable is critical).
**Plan 6** will add PSO Cache file collection / distribution, completing the original spec.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-05-03-uecm-plan-4-diagnostics.md`. Two execution options:

**1. Subagent-Driven (recommended)** — fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — execute tasks in this session using executing-plans, batch execution with checkpoints.

**Which approach?**

If Subagent-Driven chosen:
- REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
- Fresh subagent per task + two-stage review.

If Inline Execution chosen:
- REQUIRED SUB-SKILL: Use superpowers:executing-plans
- Batch execution with checkpoints for review.
