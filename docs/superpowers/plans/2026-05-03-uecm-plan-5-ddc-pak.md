# UECM Plan 5 — DDC Pak Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

## Execution Mode (READ FIRST — overrides default skill behavior)

**Mode: AUTO-CONTINUOUS.** Run all 23 tasks back-to-back without pausing for human approval between them. Same rules as Plan 4.

**Stop and ask the user ONLY in these cases:**

1. **Plan vs reality conflict** that requires re-design (structural mismatch where continuing would produce wrong work).
2. **Destructive operation requiring authorization**: deleting files outside `<ProjectDir>/DerivedDataCache/`, modifying source-controlled UE project files, deleting credentials, `git push --force`, `rm -rf` outside the workspace, modifying SSH config.
3. **Critical-severity code review finding with no obvious fix.**
4. **lanPC unreachable, WinRM disabled, UnrealEditor.exe absent, or test project missing** when an E2E verification step requires it.
5. **A new dependency decision** not covered by the plan.

**Do NOT stop for:** spec/quality review finding Important / Minor issues (fix in `fix:` follow-up commit, proceed); Windows-gated tests skipped on macOS; DONE_WITH_CONCERNS observations; README/docs cleanup.

**Final report:** commit list, frontend + backend test counts, every DONE_WITH_CONCERNS verbatim, production build outcome, deferred lanPC E2E steps awaiting user.

---

**Goal:** Plan 5 builds the DDC Pak workflow — operators can package a project's derived-data cache into a portable `.ddp` file (D1 generation), distribute it to N machines mapped to the same logical project (D2 distribution), and orchestrate the three real-world combinations through one wizard (D3). It also lays the **UE process runner** infrastructure that Plan 6 will reuse for PSO Cache collection.

**Architecture additions:**

- `core::ue_runner` — Generic UE process orchestrator. Spawns `UnrealEditor.exe` (or `UnrealEditor-Cmd.exe`) on a remote machine via WinRM with a structured argument list, monitors the UE log file (`<ProjectDir>/Saved/Logs/<ProjectName>.log`) by streaming the tail, parses progress markers, and emits `UeRunnerEvent` over an mpsc channel. Returns final exit code + last 200 log lines on completion. This is the foundation for both DDC Pak generation (Plan 5) and PSO Cache collection (Plan 6 — F2).
- `core::project_identity` — Pure-Rust three-level matcher: (1) match by `.uproject` filename, (2) accept manual mapping, (3) accept absolute path mapping. Operates on data persisted in two new tables `projects` (logical identity) + `project_locations` (per-machine path). No Windows dependency, fully unit-testable on macOS.
- `core::project_discovery` — Walks operator-supplied search roots on a target machine via WinRM, returns every `*.uproject` found. Persists results into `project_locations` keyed by `(project_id, machine_id)`.
- `core::ddc_pak::generate` — Calls `ue_runner` with `-run=DerivedDataCache -fill -DDC=CreatePak` against a chosen project on a chosen machine; final output verified via remote file existence check on `<ProjectDir>/DerivedDataCache/Compressed.ddp`.
- `core::pak_distribute` — Multi-target Robocopy fan-out reusing `core::batch::run_batch`. Source = UNC of the generation machine's `<ProjectDir>/DerivedDataCache/`. Target = each receiver's `<ProjectDir>/DerivedDataCache/` resolved through `project_locations`. Per-machine retry support.
- New PowerShell sidecars: `discover-uprojects.ps1`, `start-ue-process.ps1`, `tail-ue-log.ps1`, `generate-ddc-pak.ps1`, `distribute-pak-file.ps1`.
- Two SQLite migrations: `008_projects_table` + `009_project_locations_table`.
- Tauri command surface: `discover_projects`, `list_projects`, `list_project_locations`, `set_project_location`, `delete_project`, `generate_ddc_pak`, `distribute_ddc_pak`, `cancel_ue_process`.
- Frontend stores: `useProjectsStore` (logical projects + locations) and `useDdcPakStore` (generation + distribution job state). Both subscribe to `tauri::Window::emit` events: `ue-runner-progress` (per-machine), `pak-distribute-progress` (per-machine).
- Frontend primitives expansion: `UecmProgressBar.vue` (deterministic + indeterminate modes — drives both UE log progress and Robocopy bytes), `UecmTaskCard.vue` (job status card), `UecmPathInput.vue` (Windows path input with quick-validate). All called for in the design system overhaul plan but not yet present.

**Tech Stack:** Builds on Plan 4 stack. New Rust dep: `regex` (already promoted in Plan 4) for UE log parsing. No new frontend deps.

**Out of scope for this plan (deferred):**

- PSO Cache file collection / distribution (Plan 6 — F2/F3) — though Plan 5's `ue_runner` is the foundation Plan 6 builds on.
- PSO Precaching CVar verification (Plan 6 — F1) — included only as a deferred E3 health check in Plan 4.
- GPU/driver consistency cluster matrix UI (Plan 6 — F4) — Plan 4 already collected `machine_gpus`; Plan 5 doesn't touch this.
- ZenServer DDC mode (out of scope indefinitely; v1 is Filesystem-only).
- Auto-retry of failed Robocopy targets (manual retry button only in v1).
- Resumable / chunked .ddp transfer (`Robocopy /MIR` is the v1 transport; chunking only if needed).

**Deliverable at end:**

1. User clicks "Discover projects on machine" → enters one or more search roots (e.g. `D:\Work`, `E:\Projects`) → wizard runs WinRM walk, returns every `.uproject` found, persists to `project_locations`.
2. UI lists logical projects in a "Projects" view (Plan 1 stub view becomes real). Each row shows: `.uproject` filename, # of machines that have it, expand → table of `(machine, abs_path)` rows.
3. User clicks "Generate DDC Pak" → wizard prompts: pick logical project, pick which machine generates (could be the operator machine "self" or a remote dev box), pick output mode ("keep on source" / "auto-distribute to N machines"). Click Run.
4. Wizard shows real-time UE process status: spawning → log progress (lines parsed every 1s) → completion (.ddp file size + path).
5. After generation, if "auto-distribute" was chosen, Robocopy fans out to selected target machines with per-machine progress bar (running / ok / err / mb-transferred). Failures land in a "retry" panel.
6. Three real combinations from the spec all work end-to-end:
   - **C1 (本机生成 / no distribute)**: pick "self" + "keep on source" → .ddp lands in operator machine's project DDC folder.
   - **C2 (远程生成 + 远程分发, 核心场景)**: pick "RENDER-01" + auto-distribute to RENDER-02..08 → .ddp lands on RENDER-01, then Robocopy fans out to all targets without operator-machine middleman.
   - **C3 (本机生成 + 远程分发)**: pick "self" + auto-distribute → operator machine generates, then fans out.
7. Operator can cancel a running UE process; cancellation kills the remote UE.exe via `Stop-Process` and updates the job to `cancelled`.
8. Production build green; full E2E on lanPC verified end-to-end.

---

## Lessons from Plan 4 — Applied to Plan 5

| Source | Lesson | Plan 5 task |
|---|---|---|
| Plan 4 T7 audit | Whole-file PS reads must `"$result"` cast every Invoke-Command primitive return | T5 (`discover-uprojects.ps1`), T8 (`start-ue-process.ps1`), T13 (`distribute-pak-file.ps1`) — applied at design time |
| Plan 4 T15 store split | One Pinia store per logical surface; cross-store interaction goes through routes, not shared state | T17 — `useProjectsStore` and `useDdcPakStore` are decoupled |
| Plan 4 T13 batch | `core::batch::run_batch` gives per-machine fan-out with progress for free | T15 distribution uses `run_batch`; T9 `ue_runner` does NOT use `run_batch` (single machine, but emits structurally similar events for UI symmetry) |
| Plan 4 T8 scan_run row | Persist parent job row BEFORE fan-out; per-target rows reference it; partial failure does not erase prior good data | T11 generation persists a `pak_jobs`-style row idea — but Plan 5 reuses Plan 3's `operations` table with `action_type='ddc_pak.generate'` / `'ddc_pak.distribute'` rather than introducing yet another job table |
| Plan 4 T22 fixtures | Real lanPC fixtures need explicit creation + cleanup steps in the E2E task | T22 — fixtures spelled out: a real `.uproject` + tiny content folder + project Saved/ subtree |

---

## Prerequisites (engineer must have before starting)

Same as Plan 4, plus:

### lanPC test fixtures (one-time prep)

To exercise generation + distribution end-to-end, lanPC needs at least one **real** UE project. The plan does NOT depend on the operator having a content-heavy production project — a minimal blank project is fine because we measure orchestration, not DDC size.

Either:

- **Option A (preferred)**: a tiny Blank-template project at `E:\test-fixtures\PluralityProject\` (already used in Plan 4 fixtures; if its `.uproject` exists this plan reuses it). The project must be openable in UE 5.4 — i.e. `UnrealEditor.exe E:\test-fixtures\PluralityProject\PluralityProject.uproject -run=DerivedDataCache` must execute without prompting.
- **Option B (fallback)**: create a fresh project via UE 5.4 launcher to `E:\test-fixtures\PakTestProject\PakTestProject.uproject`, save once with no content modification, close UE.

Confirmation command in Task 22 will check existence and the engineer will pick A or B.

### UE engine path knowledge

`UnrealEditor.exe` lives under `<EngineRoot>\Engine\Binaries\Win64\UnrealEditor.exe`. Plan 1 already discovers `<EngineRoot>` per UE version into `machine_ue_installs.install_path`; Plan 5 just appends `Engine\Binaries\Win64\UnrealEditor.exe` and verifies existence at job-start time.

### Robocopy availability

Robocopy ships with Windows since Vista. Plan 5 assumes presence — no vendoring. The `start-process` invocation uses `robocopy.exe` literal. Robocopy exit codes 0–7 are non-failures (per MS docs); code ≥8 is a real error. The PS sidecar maps this.

### Cross-platform testing rules (recap)

- `core::project_identity` matcher and `core::pak_distribute` planning logic are pure Rust; tests run on macOS.
- `core::ue_runner` orchestration is Windows-gated; the *event emission / progress parsing* layer is testable on macOS via injected fakes (we ingest a canned UE log slice).
- `core::project_discovery` orchestration is Windows-gated.
- Frontend tests run on macOS via vitest.
- E2E (real UE.exe, real Robocopy) runs on lanPC via the built `.exe`.

---

## File Structure

```
ue-cache-manager/
├── ps-scripts/
│   ├── discover-uprojects.ps1                  # NEW (walk roots → uproject paths + GUIDs)
│   ├── start-ue-process.ps1                    # NEW (spawn UE.exe; returns PID + log path)
│   ├── tail-ue-log.ps1                         # NEW (incremental log tail; returns new lines + EOF flag)
│   ├── generate-ddc-pak.ps1                    # NEW (high-level wrapper: spawn + tail + verify)
│   ├── distribute-pak-file.ps1                 # NEW (Robocopy + structured progress)
│   ├── stop-ue-process.ps1                     # NEW (Stop-Process by PID, used for cancel)
│   └── (existing scripts unchanged)
│
├── src-tauri/
│   ├── Cargo.toml                              # MODIFY (no new deps; verify regex is direct)
│   └── src/
│       ├── lib.rs                              # MODIFY (register new commands; emit ue-runner-progress events)
│       │
│       ├── commands/
│       │   ├── projects.rs                     # NEW (discover_projects / list / set_location / delete)
│       │   ├── ddc_pak.rs                      # NEW (generate / distribute / cancel)
│       │   └── mod.rs                          # MODIFY
│       │
│       ├── core/
│       │   ├── ue_runner.rs                    # NEW (UE.exe orchestrator + log tailing + cancellation)
│       │   ├── project_identity.rs             # NEW (3-level matcher, pure)
│       │   ├── project_discovery.rs            # NEW (WinRM walk + persist)
│       │   ├── ddc_pak.rs                      # NEW (generate orchestration)
│       │   ├── pak_distribute.rs               # NEW (Robocopy fan-out, reuses core::batch)
│       │   └── mod.rs                          # MODIFY
│       │
│       └── data/
│           ├── projects.rs                     # NEW (CRUD on projects table)
│           ├── project_locations.rs            # NEW (CRUD on project_locations)
│           ├── schema.rs                       # MODIFY (migrations 008 + 009)
│           └── mod.rs                          # MODIFY
│
├── src/
│   ├── services/tauri.ts                       # MODIFY (new types + functions)
│   │
│   ├── stores/
│   │   ├── projects.ts                         # NEW (logical projects + locations)
│   │   ├── ddcPak.ts                           # NEW (generation + distribution job state)
│   │   └── (existing stores unchanged)
│   │
│   ├── components/
│   │   ├── primitives/
│   │   │   ├── UecmProgressBar.vue             # NEW (deterministic + indeterminate)
│   │   │   ├── UecmTaskCard.vue                # NEW (job status with cancel)
│   │   │   ├── UecmPathInput.vue               # NEW (Windows path input)
│   │   │   └── index.ts                        # MODIFY (export the three new primitives)
│   │   ├── modals/
│   │   │   ├── ProjectDiscoveryWizard.vue      # NEW
│   │   │   ├── ProjectMatchingModal.vue        # NEW (manual identity / path mapping)
│   │   │   ├── DdcPakWizard.vue                # NEW (3 combos in one modal)
│   │   │   └── (existing modals unchanged)
│   │   └── ddcpak/
│   │       ├── PakJobCard.vue                  # NEW (per-job status with progress + cancel)
│   │       └── DistributeProgressTable.vue     # NEW (per-target row with retry)
│   │
│   ├── views/
│   │   ├── Projects.vue                        # REWRITE (was stub)
│   │   └── DDCPak.vue                          # REWRITE (was stub)
│   │
│   └── __tests__/
│       ├── project-identity.spec.ts            # NEW (matcher rules via Tauri mock)
│       ├── projects-store.spec.ts              # NEW
│       ├── ddc-pak-store.spec.ts               # NEW
│       ├── DdcPakWizard.spec.ts                # NEW
│       ├── ProjectDiscoveryWizard.spec.ts      # NEW
│       ├── ProjectMatchingModal.spec.ts        # NEW
│       ├── DistributeProgressTable.spec.ts     # NEW
│       ├── PakJobCard.spec.ts                  # NEW
│       ├── Projects-view.spec.ts               # NEW
│       ├── DDCPak-view.spec.ts                 # NEW
│       ├── UecmProgressBar.spec.ts             # NEW
│       ├── UecmTaskCard.spec.ts                # NEW
│       └── UecmPathInput.spec.ts               # NEW
│
└── README.md                                   # MODIFY (Plan 5 status)
```

---

## Approach Notes

**UE process orchestration model.** UE.exe is a long-running interactive process. It does NOT cleanly emit progress to stdout — its useful signal is the per-project log at `<ProjectDir>/Saved/Logs/<ProjectName>.log`. Plan 5 spawns UE detached, captures its PID, tails the log file via repeated `Get-Content -Tail` PS calls every 1 second, parses recognised markers, and emits `UeRunnerEvent` enum values (`Spawned { pid }`, `LogLine { text, parsed_kind }`, `Progress { pct }`, `Completed { exit_code, log_tail }`, `Cancelled`). Cancellation closes the channel and issues `Stop-Process -Id <pid> -Force` over WinRM. There is no clean "in-process" UE termination protocol, so killing the process is the v1 contract — UE may leave a `<ProjectDir>/Saved/Logs/<ProjectName>-backup-*.log` artefact, which is acceptable.

**DDC Pak generation invocation.** UE command line:

```
UnrealEditor.exe <Project>.uproject -run=DerivedDataCache -fill -DDC=CreatePak -unattended -nopause -nosplash
```

`-fill` walks every asset and pushes to the configured DDC backends. `-DDC=CreatePak` sets the backend to a pak writer rather than the local filesystem cache. The product is `<ProjectDir>/DerivedDataCache/Compressed.ddp` (UE 5.0–5.3) or `<ProjectDir>/DerivedDataCache/DDC.ddp` (UE 5.4+, depending on engine config). Plan 5's verifier checks BOTH paths and returns whichever exists with non-zero size. Log progress markers we recognise:

- `LogDerivedDataCache: Display: Filling derived data cache for <Asset>` — counted as one unit
- `LogDerivedDataCache: Display: Done filling derived data cache.` — completion marker
- `LogInit: Engine exit requested` — clean shutdown
- `LogCore: Error: Critical fail` — abort

The percentage is approximate (asset count is not known up front), so the progress bar is **indeterminate after spawn** and only flips to determinate during the final "Saving pak" phase, which UE prints with `LogDerivedDataCache: Display: Saving pak (<n>/<total>)`.

**Project identity matcher (3-level).** Per spec §5.D2:

1. **Auto by `.uproject` filename** (`PluralityProject.uproject` on RENDER-01 == `PluralityProject.uproject` on RENDER-02). The matcher hashes the filename (case-insensitive, stripped of extension) into a normalisation key. If two machines yield the same key, they share a `project_id`.
2. **Manual mapping**. UI lets the operator override: "RENDER-02's `MyProj.uproject` is logically the same as the RENDER-01 project". This writes a `project_locations` row where `project_id` is borrowed from the canonical machine.
3. **Path-only manual**. For the rare case where the operator ONLY knows two paths (e.g. `D:\Work\X` on RENDER-01, `E:\Projects\X` on RENDER-02) and the discovery walk hasn't reached one of them. UI lets operator type the path; UECM stores it as a `project_locations` row with `discovery_status='manual'`.

The matcher logic is **pure Rust**, takes a `Vec<DiscoveredUproject>` as input, returns a `Vec<MatchedProject>` plus an `unmatched: Vec<DiscoveredUproject>` for the UI to ask the operator about.

**Project ID stability under content changes.** UE projects rarely change their `.uproject` filename. We use **filename (stem, lowercased)** as the canonical identity key for v1, NOT the `.uproject` GUID — the GUID can drift if the file is regenerated by template tools, and operators rename projects rarely. If filename collisions exist (two distinct logical projects sharing a filename — extremely unlikely in a single VP shop), the manual mapping path handles it.

**Distribution model.** D2 distribution is **strict 1-to-many Robocopy** — no chunking, no incremental sync logic. Source = source machine's UNC `\\<source_host>\<share_or_admin>\<abs_path>\DerivedDataCache\`. Target = each target machine's local `<abs_path>\DerivedDataCache\` resolved via the `project_locations` join. Robocopy invocation:

```
robocopy <source_unc> <target_local> *.ddp /MIR /R:3 /W:5 /NP /NDL /NJH /NJS /BYTES
```

`/MIR` mirrors (deletes pak files no longer in source). `/R:3 /W:5` retries 3 times with 5s wait. `/NP /NDL /NJH /NJS /BYTES` minimises noise so we can parse the per-line `bytes copied` summary easily. Exit codes 0–7 are success-flavoured (per MS); ≥8 is real error.

**Source UNC strategy.** Robocopy on RENDER-02 can't read `D:\Work\X` from RENDER-01 directly — we need a UNC path. Two options:

1. **Operator-supplied share**: the operator pre-creates a share on RENDER-01 (using Plan 3's wizard if needed) and supplies the UNC. Plan 5's wizard validates connectivity from each target.
2. **Admin share fallback**: every Windows machine has `C$` / `D$` admin shares, accessible via UNC `\\<host>\<drive>$\path` if the caller has admin credentials. Plan 5 uses **option 2 by default** (with the WinRM credential alias also acting as the SMB credential), and falls back to option 1 if the operator explicitly toggles "use named share".

The admin-share strategy keeps "C2 (远程生成 + 远程分发)" friction-free: no operator setup needed beyond the WinRM credential.

**Three-combo wizard mapping.** Combos C1/C2/C3 are NOT three separate wizards — they're three configurations of one wizard:

| | Source machine | Distribute? |
|---|---|---|
| C1 | Self (operator machine) | No |
| C2 | Remote dev box | Yes (to N selected targets) |
| C3 | Self | Yes (to N selected targets) |

The wizard's step 1 picks source (radio: self / remote-pick); step 2 picks targets (multi-select machine list; "no targets" toggles distribute=off); step 3 reviews + runs. Self-as-source uses a local `tokio::process::Command` invocation rather than WinRM (operator machine doesn't WinRM into itself).

**Cancellation semantics.** Cancelling a generation job mid-flight kills UE.exe on the source machine. The half-written `.ddp` is left as-is (UE may have partially flushed it; we don't auto-delete because operator forensics may want it). The job row is marked `cancelled`. Cancelling a distribution job uses Robocopy's natural "no resume" behaviour: we send Ctrl+C to the Robocopy process (via `Stop-Process`) on each in-flight target; partial files Robocopy was mid-write are lost (operator must re-run).

**State persistence pattern.** Plan 5 reuses Plan 3's `operations` table for job history rather than inventing a new one. New `action_type` values: `'ddc_pak.generate'` and `'ddc_pak.distribute'`. The `target_machines` JSON column stores the source for generate, target list for distribute. The `log_text` column stores the UE log tail (last 200 lines). The `snapshot_blob` column is unused for these actions (no rollback semantics — DDC pak generation/distribution doesn't change config state).

**Self-as-source local execution.** When source = operator machine, we don't go through WinRM — that requires the operator machine to be in `TrustedHosts` of itself, which it usually isn't. Instead we shell out via `tokio::process::Command::new("UnrealEditor.exe")` with stdin/stderr piping. The log-tail logic runs on the local filesystem. The runner abstraction has two backends: `UeRunnerBackend::Local` and `UeRunnerBackend::Remote(machine_id)`.

---

## Self-Review Checklist (run after writing each task)

- [ ] **Spec coverage:** every section of design doc 5.D (DDC Pak Operations) maps to at least one task. Three combos C1/C2/C3 each have an E2E sub-step.
- [ ] **No placeholders:** every step shows actual code or actual command. No "TBD" or "implement later".
- [ ] **Type consistency:** `UeRunnerEvent` enum value names match across Rust and TS (`spawned | log_line | progress | completed | cancelled | error`). `DistributionStatus` (`pending | running | ok | err | cancelled`) matches. `MatchKind` (`auto | manual_alias | manual_path`) matches.
- [ ] **Selectors preserved:** `data-base-modal`, `data-status-badge`, etc. unchanged on rewritten views. New components add new `data-*` selectors for their tests.
- [ ] **Stores untouched:** `useMachinesStore`, `useDiscoveryStore`, `useCredentialsStore`, `useSharesStore`, `useBatchStore`, `useClusterStore`, `useTasksStore`, `useDiagnosticsStore`, `useHealthCheckStore` — Plan 5 ONLY adds `useProjectsStore` + `useDdcPakStore`.
- [ ] **Routes intact:** all 8 existing routes still resolve (Plan 5 modifies `Projects.vue` and `DDCPak.vue` in place).
- [ ] **Cluster store integration:** `useClusterStore.score` is NOT affected — pak operations are jobs, not health.
- [ ] **Migration ordering:** 008 + 009 land after Plan 4's 007. They are append-only — no modification to 001-007.

---

## Task 1: Pre-flight audit (no new code)

**Files:** none modified.

- [ ] **Step 1: Confirm Plan 4 baseline green**

```bash
export PATH="/Users/bip.lan/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
pnpm test 2>&1 | tail -10
cd src-tauri && cargo test --lib 2>&1 | tail -10 && cd ..
```

Expected: ~120 frontend + ~95 backend tests pass (Plan 4 final state). If anything fails, **STOP** and report — Plan 5 must start from a green baseline.

- [ ] **Step 2: Confirm `core::batch::run_batch` signature still matches what Plan 5 will call**

```bash
grep -n "pub async fn run_batch" src-tauri/src/core/batch.rs
```

Expected: `pub async fn run_batch<F, Fut, T>(machine_ids: Vec<i64>, max_concurrency: usize, op: F) -> mpsc::UnboundedReceiver<BatchEvent>`. If the signature has drifted, update T15 distribution to match.

- [ ] **Step 3: Confirm `core::winrm::invoke_with_credential` exists and works for long output**

```bash
grep -n "pub fn invoke_with_credential" src-tauri/src/core/winrm.rs
```

Expected: function signature `pub fn invoke_with_credential(host: &str, script_body: &str, username: &str, password: &str) -> UecmResult<String>`. Plan 5 will *not* use this for log-tailing (too slow per round-trip) — it will use a dedicated `tail-ue-log.ps1` invoked once per second. But it IS used for one-shot operations like `discover-uprojects.ps1`.

- [ ] **Step 4: Confirm `data::open_in_memory` is available for tests**

```bash
grep -n "pub fn open_in_memory" src-tauri/src/data/mod.rs
```

Expected: helper exists. Plan 5 reuses it for Rust unit tests of `data::projects` and `data::project_locations`.

- [ ] **Step 5: Confirm Plan 4's `operations` table is present (for job history reuse)**

If Plan 4 didn't add it, Plan 5 needs to. Check:

```bash
grep -n "CREATE TABLE.*operations" src-tauri/src/data/schema.rs
```

If missing: Plan 5 task list adds migration `008_operations_table` BEFORE the `008_projects_table` migration; renumber projects → 009 and project_locations → 010 in all subsequent tasks. **Engineer note**: as of this plan's writing the `operations` table was deferred from Plan 3; if still absent, fold the migration into Task 2.

- [ ] **Step 6: Commit baseline note**

No code changes — just note state in commit:

```bash
git checkout -b feature/plan-5-ddc-pak
# (no commit needed; this branch tracking-only)
```

Expected: clean working tree, branch created.

---

## Task 2: SQLite migrations 008 + 009 — `projects` + `project_locations`

**Files:**
- Modify: `src-tauri/src/data/schema.rs` (append migrations 008 + 009; if Step 5 above flagged `operations` missing, also add `008_operations_table` and renumber)

- [ ] **Step 1: Open schema.rs and add the migrations**

Append at the end of the `MIGRATIONS` array, before the closing `];`:

```rust
    (
        "008_projects_table",
        r#"
        CREATE TABLE IF NOT EXISTS projects (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            uproject_name TEXT NOT NULL,
            uproject_stem_lower TEXT NOT NULL UNIQUE,
            uproject_guid TEXT,
            display_name TEXT,
            first_seen_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            last_seen_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_projects_stem ON projects(uproject_stem_lower);
        "#,
    ),
    (
        "009_project_locations_table",
        r#"
        CREATE TABLE IF NOT EXISTS project_locations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id INTEGER NOT NULL,
            machine_id INTEGER NOT NULL,
            abs_path TEXT NOT NULL,
            uproject_path TEXT NOT NULL,
            discovery_status TEXT NOT NULL DEFAULT 'auto',
            discovered_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(project_id, machine_id),
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
            FOREIGN KEY (machine_id) REFERENCES machines(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_project_locations_project ON project_locations(project_id);
        CREATE INDEX IF NOT EXISTS idx_project_locations_machine ON project_locations(machine_id);
        "#,
    ),
```

Notes on column choices:
- `uproject_stem_lower` is the matcher key (filename without `.uproject`, lowercased). Unique — one row per logical project.
- `uproject_guid` is read from the `.uproject` JSON's `EngineAssociation` field if available (NOT used as identity, just informational).
- `discovery_status` ∈ `auto | manual_alias | manual_path` — matches `MatchKind` enum on the Rust side.
- `abs_path` is the project root directory; `uproject_path` is the full path to the `.uproject` file. Slightly redundant, but each is independently useful — many command lines need the root, and the `.uproject` path is needed for the UE invocation.

- [ ] **Step 2: Run migration to verify**

```bash
cd src-tauri && cargo build --lib 2>&1 | tail -5 && cd ..
```

Expected: clean build. Migration only fires on app startup; we'll exercise it in T3.

- [ ] **Step 3: Add a quick migration smoke test**

In `src-tauri/src/data/schema.rs`, in the existing `#[cfg(test)] mod tests { ... }` block (or create one if absent), append:

```rust
    #[test]
    fn migration_008_creates_projects_table() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='projects'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn migration_009_creates_project_locations_table_with_fks() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        // FK violation should fail when project_id is invalid
        conn.execute("PRAGMA foreign_keys = ON;", []).unwrap();
        conn.execute(
            "INSERT INTO machines (hostname, ip) VALUES ('h', '1.1.1.1')",
            [],
        )
        .unwrap();
        let result = conn.execute(
            "INSERT INTO project_locations (project_id, machine_id, abs_path, uproject_path) \
             VALUES (999, 1, 'C:\\X', 'C:\\X\\Y.uproject')",
            [],
        );
        assert!(result.is_err(), "FK violation expected");
    }
```

- [ ] **Step 4: Run the new tests**

```bash
cd src-tauri && cargo test --lib data::schema::tests 2>&1 | tail -10 && cd ..
```

Expected: 2 new tests pass. Total schema test count is whatever it was + 2.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/data/schema.rs
git commit -m "feat: schema migrations 008-009 (projects + project_locations)"
```

---

## Task 3: Data layer — `data::projects` + `data::project_locations`

**Files:**
- Create: `src-tauri/src/data/projects.rs`
- Create: `src-tauri/src/data/project_locations.rs`
- Modify: `src-tauri/src/data/mod.rs`

- [ ] **Step 1: Write `data::projects`**

Create `src-tauri/src/data/projects.rs`:

```rust
//! CRUD for the `projects` table.

use crate::data::Db;
use crate::error::UecmResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: Option<i64>,
    pub uproject_name: String,
    pub uproject_stem_lower: String,
    pub uproject_guid: Option<String>,
    pub display_name: Option<String>,
    pub first_seen_at: Option<String>,
    pub last_seen_at: Option<String>,
}

pub fn upsert(db: &Db, p: &Project) -> UecmResult<i64> {
    let conn = db.lock().unwrap();
    conn.execute(
        "INSERT INTO projects (uproject_name, uproject_stem_lower, uproject_guid, display_name, last_seen_at)
         VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)
         ON CONFLICT(uproject_stem_lower) DO UPDATE SET
           uproject_name = excluded.uproject_name,
           uproject_guid = COALESCE(excluded.uproject_guid, projects.uproject_guid),
           last_seen_at = CURRENT_TIMESTAMP",
        rusqlite::params![p.uproject_name, p.uproject_stem_lower, p.uproject_guid, p.display_name],
    )?;
    let id: i64 = conn.query_row(
        "SELECT id FROM projects WHERE uproject_stem_lower = ?",
        [&p.uproject_stem_lower],
        |r| r.get(0),
    )?;
    Ok(id)
}

pub fn list(db: &Db) -> UecmResult<Vec<Project>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, uproject_name, uproject_stem_lower, uproject_guid, display_name, first_seen_at, last_seen_at
         FROM projects ORDER BY uproject_name",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Project {
                id: Some(r.get(0)?),
                uproject_name: r.get(1)?,
                uproject_stem_lower: r.get(2)?,
                uproject_guid: r.get(3)?,
                display_name: r.get(4)?,
                first_seen_at: r.get(5)?,
                last_seen_at: r.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn get(db: &Db, project_id: i64) -> UecmResult<Option<Project>> {
    let conn = db.lock().unwrap();
    let row = conn.query_row(
        "SELECT id, uproject_name, uproject_stem_lower, uproject_guid, display_name, first_seen_at, last_seen_at
         FROM projects WHERE id = ?",
        [project_id],
        |r| {
            Ok(Project {
                id: Some(r.get(0)?),
                uproject_name: r.get(1)?,
                uproject_stem_lower: r.get(2)?,
                uproject_guid: r.get(3)?,
                display_name: r.get(4)?,
                first_seen_at: r.get(5)?,
                last_seen_at: r.get(6)?,
            })
        },
    );
    match row {
        Ok(p) => Ok(Some(p)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn delete(db: &Db, project_id: i64) -> UecmResult<()> {
    let conn = db.lock().unwrap();
    conn.execute("DELETE FROM projects WHERE id = ?", [project_id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::open_in_memory;

    #[test]
    fn upsert_creates_then_updates() {
        let db = open_in_memory().unwrap();
        let p = Project {
            id: None,
            uproject_name: "Plurality.uproject".into(),
            uproject_stem_lower: "plurality".into(),
            uproject_guid: None,
            display_name: None,
            first_seen_at: None,
            last_seen_at: None,
        };
        let id1 = upsert(&db, &p).unwrap();
        let id2 = upsert(&db, &p).unwrap();
        assert_eq!(id1, id2, "upsert must be idempotent on stem_lower");
    }

    #[test]
    fn list_orders_by_name() {
        let db = open_in_memory().unwrap();
        upsert(&db, &Project {
            id: None,
            uproject_name: "B.uproject".into(),
            uproject_stem_lower: "b".into(),
            uproject_guid: None,
            display_name: None,
            first_seen_at: None,
            last_seen_at: None,
        }).unwrap();
        upsert(&db, &Project {
            id: None,
            uproject_name: "A.uproject".into(),
            uproject_stem_lower: "a".into(),
            uproject_guid: None,
            display_name: None,
            first_seen_at: None,
            last_seen_at: None,
        }).unwrap();
        let rows = list(&db).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].uproject_name, "A.uproject");
    }
}
```

- [ ] **Step 2: Write `data::project_locations`**

Create `src-tauri/src/data/project_locations.rs`:

```rust
//! CRUD for the `project_locations` table.

use crate::data::Db;
use crate::error::UecmResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryStatus {
    Auto,
    ManualAlias,
    ManualPath,
}

impl DiscoveryStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            DiscoveryStatus::Auto => "auto",
            DiscoveryStatus::ManualAlias => "manual_alias",
            DiscoveryStatus::ManualPath => "manual_path",
        }
    }
    pub fn parse(s: &str) -> UecmResult<Self> {
        match s {
            "auto" => Ok(Self::Auto),
            "manual_alias" => Ok(Self::ManualAlias),
            "manual_path" => Ok(Self::ManualPath),
            _ => Err(crate::error::UecmError::InvalidInput(format!(
                "unknown discovery_status: {}",
                s
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectLocation {
    pub id: Option<i64>,
    pub project_id: i64,
    pub machine_id: i64,
    pub abs_path: String,
    pub uproject_path: String,
    pub discovery_status: DiscoveryStatus,
    pub discovered_at: Option<String>,
}

pub fn upsert(db: &Db, loc: &ProjectLocation) -> UecmResult<i64> {
    let conn = db.lock().unwrap();
    conn.execute(
        "INSERT INTO project_locations (project_id, machine_id, abs_path, uproject_path, discovery_status)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(project_id, machine_id) DO UPDATE SET
           abs_path = excluded.abs_path,
           uproject_path = excluded.uproject_path,
           discovery_status = excluded.discovery_status,
           discovered_at = CURRENT_TIMESTAMP",
        rusqlite::params![
            loc.project_id,
            loc.machine_id,
            loc.abs_path,
            loc.uproject_path,
            loc.discovery_status.as_str(),
        ],
    )?;
    let id: i64 = conn.query_row(
        "SELECT id FROM project_locations WHERE project_id = ? AND machine_id = ?",
        rusqlite::params![loc.project_id, loc.machine_id],
        |r| r.get(0),
    )?;
    Ok(id)
}

pub fn list_by_project(db: &Db, project_id: i64) -> UecmResult<Vec<ProjectLocation>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, project_id, machine_id, abs_path, uproject_path, discovery_status, discovered_at
         FROM project_locations WHERE project_id = ? ORDER BY machine_id",
    )?;
    let rows = stmt
        .query_map([project_id], |r| {
            let status_str: String = r.get(5)?;
            Ok(ProjectLocation {
                id: Some(r.get(0)?),
                project_id: r.get(1)?,
                machine_id: r.get(2)?,
                abs_path: r.get(3)?,
                uproject_path: r.get(4)?,
                discovery_status: DiscoveryStatus::parse(&status_str)
                    .unwrap_or(DiscoveryStatus::Auto),
                discovered_at: r.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn list_by_machine(db: &Db, machine_id: i64) -> UecmResult<Vec<ProjectLocation>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, project_id, machine_id, abs_path, uproject_path, discovery_status, discovered_at
         FROM project_locations WHERE machine_id = ? ORDER BY project_id",
    )?;
    let rows = stmt
        .query_map([machine_id], |r| {
            let status_str: String = r.get(5)?;
            Ok(ProjectLocation {
                id: Some(r.get(0)?),
                project_id: r.get(1)?,
                machine_id: r.get(2)?,
                abs_path: r.get(3)?,
                uproject_path: r.get(4)?,
                discovery_status: DiscoveryStatus::parse(&status_str)
                    .unwrap_or(DiscoveryStatus::Auto),
                discovered_at: r.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn get_for_project_machine(
    db: &Db,
    project_id: i64,
    machine_id: i64,
) -> UecmResult<Option<ProjectLocation>> {
    let conn = db.lock().unwrap();
    let row = conn.query_row(
        "SELECT id, project_id, machine_id, abs_path, uproject_path, discovery_status, discovered_at
         FROM project_locations WHERE project_id = ? AND machine_id = ?",
        rusqlite::params![project_id, machine_id],
        |r| {
            let status_str: String = r.get(5)?;
            Ok(ProjectLocation {
                id: Some(r.get(0)?),
                project_id: r.get(1)?,
                machine_id: r.get(2)?,
                abs_path: r.get(3)?,
                uproject_path: r.get(4)?,
                discovery_status: DiscoveryStatus::parse(&status_str)
                    .unwrap_or(DiscoveryStatus::Auto),
                discovered_at: r.get(6)?,
            })
        },
    );
    match row {
        Ok(loc) => Ok(Some(loc)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn delete(db: &Db, location_id: i64) -> UecmResult<()> {
    let conn = db.lock().unwrap();
    conn.execute("DELETE FROM project_locations WHERE id = ?", [location_id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{open_in_memory, projects::{self, Project}};

    fn seed_machine(db: &Db, ip: &str) -> i64 {
        let conn = db.lock().unwrap();
        conn.execute(
            "INSERT INTO machines (hostname, ip) VALUES (?, ?)",
            rusqlite::params![format!("h-{}", ip), ip],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn seed_project(db: &Db, stem: &str) -> i64 {
        projects::upsert(
            db,
            &Project {
                id: None,
                uproject_name: format!("{}.uproject", stem),
                uproject_stem_lower: stem.to_lowercase(),
                uproject_guid: None,
                display_name: None,
                first_seen_at: None,
                last_seen_at: None,
            },
        )
        .unwrap()
    }

    #[test]
    fn upsert_is_idempotent_on_project_machine_pair() {
        let db = open_in_memory().unwrap();
        let mid = seed_machine(&db, "1.1.1.1");
        let pid = seed_project(&db, "demo");
        let loc = ProjectLocation {
            id: None,
            project_id: pid,
            machine_id: mid,
            abs_path: "D:\\Work\\demo".into(),
            uproject_path: "D:\\Work\\demo\\demo.uproject".into(),
            discovery_status: DiscoveryStatus::Auto,
            discovered_at: None,
        };
        let id1 = upsert(&db, &loc).unwrap();
        let mut loc2 = loc.clone();
        loc2.abs_path = "E:\\Work\\demo".into();
        let id2 = upsert(&db, &loc2).unwrap();
        assert_eq!(id1, id2);
        let got = get_for_project_machine(&db, pid, mid).unwrap().unwrap();
        assert_eq!(got.abs_path, "E:\\Work\\demo");
    }

    #[test]
    fn list_by_project_returns_only_that_project() {
        let db = open_in_memory().unwrap();
        let m1 = seed_machine(&db, "1.1.1.1");
        let m2 = seed_machine(&db, "2.2.2.2");
        let pid = seed_project(&db, "demo");
        for mid in [m1, m2] {
            upsert(
                &db,
                &ProjectLocation {
                    id: None,
                    project_id: pid,
                    machine_id: mid,
                    abs_path: format!("D:\\m{}", mid),
                    uproject_path: format!("D:\\m{}\\demo.uproject", mid),
                    discovery_status: DiscoveryStatus::Auto,
                    discovered_at: None,
                },
            )
            .unwrap();
        }
        let locs = list_by_project(&db, pid).unwrap();
        assert_eq!(locs.len(), 2);
    }
}
```

- [ ] **Step 3: Wire into `data/mod.rs`**

Open `src-tauri/src/data/mod.rs`. Add module declarations alongside existing ones:

```rust
pub mod projects;
pub mod project_locations;
```

And in the re-export block (where `Machine`, `ShareConfig`, etc. are re-exported) add:

```rust
pub use projects::Project;
pub use project_locations::{ProjectLocation, DiscoveryStatus};
```

- [ ] **Step 4: Run the tests**

```bash
cd src-tauri && cargo test --lib data::projects::tests data::project_locations::tests 2>&1 | tail -10 && cd ..
```

Expected: 5 new tests pass (2 for projects, 3 for project_locations).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/data/projects.rs src-tauri/src/data/project_locations.rs src-tauri/src/data/mod.rs
git commit -m "feat: data layer for projects + project_locations"
```

---

## Task 4: Pure-Rust project identity matcher (TDD)

**Files:**
- Create: `src-tauri/src/core/project_identity.rs`
- Modify: `src-tauri/src/core/mod.rs`

- [ ] **Step 1: Write failing tests first**

Create `src-tauri/src/core/project_identity.rs` with the test module skeleton:

```rust
//! Three-level project identity matcher: by-name → manual-alias → manual-path.
//! Pure functions; no Windows or DB dependency. The DB layer (`data::projects`,
//! `data::project_locations`) is the persistence side; this module is the
//! stateless reasoning that takes raw discovery results and produces grouping
//! decisions for the UI to either commit or ask the operator about.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiscoveredUproject {
    pub machine_id: i64,
    pub abs_path: String,
    pub uproject_path: String,
    pub uproject_filename: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MatchKind {
    Auto,
    ManualAlias,
    ManualPath,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MatchedProject {
    pub stem_lower: String,
    pub canonical_filename: String,
    pub locations: Vec<DiscoveredUproject>,
    pub match_kind: MatchKind,
}

pub fn stem_lower(filename: &str) -> String {
    let stem = filename
        .strip_suffix(".uproject")
        .or_else(|| filename.strip_suffix(".UPROJECT"))
        .unwrap_or(filename);
    stem.to_lowercase()
}

#[derive(Debug, Default)]
pub struct MatchOutcome {
    pub matched: Vec<MatchedProject>,
    pub ambiguous: Vec<DiscoveredUproject>,
}

pub fn match_by_filename(items: Vec<DiscoveredUproject>) -> MatchOutcome {
    use std::collections::HashMap;
    let mut groups: HashMap<String, Vec<DiscoveredUproject>> = HashMap::new();
    for it in items {
        let key = stem_lower(&it.uproject_filename);
        groups.entry(key).or_default().push(it);
    }

    let mut matched: Vec<MatchedProject> = groups
        .into_iter()
        .map(|(stem, locs)| MatchedProject {
            stem_lower: stem,
            canonical_filename: locs[0].uproject_filename.clone(),
            locations: locs,
            match_kind: MatchKind::Auto,
        })
        .collect();

    matched.sort_by(|a, b| a.canonical_filename.cmp(&b.canonical_filename));
    MatchOutcome {
        matched,
        ambiguous: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn discovered(machine_id: i64, abs_path: &str, filename: &str) -> DiscoveredUproject {
        DiscoveredUproject {
            machine_id,
            abs_path: abs_path.into(),
            uproject_path: format!("{}\\{}", abs_path, filename),
            uproject_filename: filename.into(),
        }
    }

    #[test]
    fn stem_lower_strips_extension_and_lowercases() {
        assert_eq!(stem_lower("Plurality.uproject"), "plurality");
        assert_eq!(stem_lower("MyProj.UPROJECT"), "myproj");
        assert_eq!(stem_lower("Already_lower.uproject"), "already_lower");
    }

    #[test]
    fn matches_two_machines_with_same_filename() {
        let items = vec![
            discovered(1, "D:\\Work\\Plurality", "Plurality.uproject"),
            discovered(2, "E:\\Projects\\Plurality", "Plurality.uproject"),
        ];
        let out = match_by_filename(items);
        assert_eq!(out.matched.len(), 1);
        assert_eq!(out.matched[0].locations.len(), 2);
        assert_eq!(out.matched[0].stem_lower, "plurality");
        assert_eq!(out.matched[0].match_kind, MatchKind::Auto);
    }

    #[test]
    fn separates_distinct_filenames() {
        let items = vec![
            discovered(1, "D:\\X", "X.uproject"),
            discovered(2, "D:\\Y", "Y.uproject"),
        ];
        let out = match_by_filename(items);
        assert_eq!(out.matched.len(), 2);
    }

    #[test]
    fn case_insensitive_grouping() {
        let items = vec![
            discovered(1, "D:\\X", "MyProj.uproject"),
            discovered(2, "E:\\Y", "myproj.uproject"),
        ];
        let out = match_by_filename(items);
        assert_eq!(out.matched.len(), 1);
        assert_eq!(out.matched[0].locations.len(), 2);
    }

    #[test]
    fn empty_input_yields_empty_outcome() {
        let out = match_by_filename(vec![]);
        assert_eq!(out.matched.len(), 0);
        assert_eq!(out.ambiguous.len(), 0);
    }
}
```

- [ ] **Step 2: Wire into `core/mod.rs`**

Open `src-tauri/src/core/mod.rs` and add to the module list:

```rust
pub mod project_identity;
```

- [ ] **Step 3: Run the tests**

```bash
cd src-tauri && cargo test --lib core::project_identity::tests 2>&1 | tail -10 && cd ..
```

Expected: 5 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/core/project_identity.rs src-tauri/src/core/mod.rs
git commit -m "feat: pure-rust project identity matcher (3-level v1: filename)"
```

---

## Task 5: PowerShell — `discover-uprojects.ps1`

**Files:**
- Create: `ps-scripts/discover-uprojects.ps1`

- [ ] **Step 1: Write the PS sidecar**

```powershell
param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$SearchRoots,   # comma-separated absolute paths
    [int]$MaxDepth = 6,
    [string]$Username,
    [string]$Password
)

$ErrorActionPreference = 'Stop'

function Build-CredentialOrNull {
    param([string]$User, [string]$Pass)
    if ([string]::IsNullOrEmpty($User) -or [string]::IsNullOrEmpty($Pass)) { return $null }
    $secure = ConvertTo-SecureString -String $Pass -AsPlainText -Force
    return New-Object System.Management.Automation.PSCredential($User, $secure)
}

try {
    $script = {
        param($Roots, $MaxDepth)
        $found = @()
        foreach ($root in $Roots) {
            if (-not (Test-Path -LiteralPath $root)) { continue }
            try {
                $ups = Get-ChildItem -LiteralPath $root -Filter '*.uproject' -Recurse -Depth $MaxDepth -File -ErrorAction SilentlyContinue
                foreach ($u in $ups) {
                    $abs = Split-Path -LiteralPath $u.FullName -Parent
                    $guid = $null
                    try {
                        $json = Get-Content -LiteralPath $u.FullName -Raw -ErrorAction SilentlyContinue | ConvertFrom-Json -ErrorAction SilentlyContinue
                        if ($json -and $json.EngineAssociation) { $guid = "$($json.EngineAssociation)" }
                    } catch {}
                    $found += @{
                        uproject_filename = "$($u.Name)"
                        uproject_path     = "$($u.FullName)"
                        abs_path          = "$abs"
                        engine_association = $guid
                    }
                }
            } catch {}
        }
        return ,$found
    }

    $rootsArr = $SearchRoots -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ -ne '' }
    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock  = $script
        ArgumentList = @($rootsArr, $MaxDepth)
        ErrorAction  = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $remoteResult = Invoke-Command @invokeArgs

    # Force array even if single element
    $list = @($remoteResult)
    @{ ok = $true; items = $list; count = $list.Count } | ConvertTo-Json -Depth 6 -Compress
}
catch {
    @{ ok = $false; items = @(); count = 0; message = "$($_.Exception.Message)" } | ConvertTo-Json -Depth 6 -Compress
    exit 1
}
```

- [ ] **Step 2: Verify the script parses on macOS**

```bash
pwsh -File ps-scripts/discover-uprojects.ps1 -HostName "127.0.0.1" -SearchRoots "/tmp/non-existent" 2>&1 | head -5
```

Expected: emits `{ ok: false, ... }` JSON or fails Invoke-Command with a connectivity error — script syntax is fine. Skip if `pwsh` unavailable.

- [ ] **Step 3: Commit**

```bash
git add ps-scripts/discover-uprojects.ps1
git commit -m "feat(ps): add discover-uprojects.ps1"
```

---

## Task 6: Rust `core::project_discovery` — WinRM walk + persist

**Files:**
- Create: `src-tauri/src/core/project_discovery.rs`
- Modify: `src-tauri/src/core/mod.rs`

- [ ] **Step 1: Write the module**

```rust
//! Project discovery — walks search roots on a target machine via WinRM,
//! returns every .uproject found, and persists into the projects +
//! project_locations tables.

use crate::core::powershell;
use crate::core::project_identity::{stem_lower, DiscoveredUproject};
use crate::data::{
    project_locations::{self, DiscoveryStatus, ProjectLocation},
    projects::{self, Project},
    Db,
};
use crate::error::{UecmError, UecmResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct DiscoveryItemRaw {
    uproject_filename: String,
    uproject_path: String,
    abs_path: String,
    engine_association: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DiscoveryScriptResult {
    ok: bool,
    items: Vec<DiscoveryItemRaw>,
    #[serde(default)]
    count: i64,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoveryResult {
    pub project_id: i64,
    pub location_id: i64,
    pub uproject_filename: String,
    pub abs_path: String,
}

pub fn run_discovery(
    db: &Db,
    machine_id: i64,
    host: &str,
    search_roots: &[String],
    operator_user: Option<&str>,
    operator_pass: Option<&str>,
) -> UecmResult<Vec<DiscoveryResult>> {
    if search_roots.is_empty() {
        return Err(UecmError::InvalidInput("search_roots is empty".into()));
    }
    let roots_csv = search_roots.join(",");
    let mut args: Vec<String> = vec![
        "-HostName".into(),
        host.into(),
        "-SearchRoots".into(),
        roots_csv,
    ];
    if let (Some(u), Some(p)) = (operator_user, operator_pass) {
        args.push("-Username".into());
        args.push(u.into());
        args.push("-Password".into());
        args.push(p.into());
    }
    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let result: DiscoveryScriptResult = powershell::run_json(
        &powershell::script_path("discover-uprojects.ps1"),
        &args_ref,
    )?;

    if !result.ok {
        return Err(UecmError::OperationFailed(
            result.message.unwrap_or_else(|| "discovery failed".into()),
        ));
    }

    let mut out = Vec::with_capacity(result.items.len());
    for item in result.items {
        let stem = stem_lower(&item.uproject_filename);
        let project = Project {
            id: None,
            uproject_name: item.uproject_filename.clone(),
            uproject_stem_lower: stem.clone(),
            uproject_guid: item.engine_association.clone(),
            display_name: None,
            first_seen_at: None,
            last_seen_at: None,
        };
        let project_id = projects::upsert(db, &project)?;
        let location = ProjectLocation {
            id: None,
            project_id,
            machine_id,
            abs_path: item.abs_path.clone(),
            uproject_path: item.uproject_path.clone(),
            discovery_status: DiscoveryStatus::Auto,
            discovered_at: None,
        };
        let location_id = project_locations::upsert(db, &location)?;
        out.push(DiscoveryResult {
            project_id,
            location_id,
            uproject_filename: item.uproject_filename,
            abs_path: item.abs_path,
        });
    }
    Ok(out)
}

/// Convert the Rust DiscoveredUproject collection into the matcher input form.
/// Used by command-layer "preview before commit" flows.
pub fn to_discovered(
    machine_id: i64,
    items: &[DiscoveryItemRaw],
) -> Vec<DiscoveredUproject> {
    items
        .iter()
        .map(|i| DiscoveredUproject {
            machine_id,
            abs_path: i.abs_path.clone(),
            uproject_path: i.uproject_path.clone(),
            uproject_filename: i.uproject_filename.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn run_discovery_returns_powershell_error_on_non_windows() {
        let db = crate::data::open_in_memory().unwrap();
        // seed a machine row so FK is valid
        {
            let conn = db.lock().unwrap();
            conn.execute(
                "INSERT INTO machines (hostname, ip) VALUES ('h', '1.1.1.1')",
                [],
            )
            .unwrap();
        }
        let result = run_discovery(
            &db,
            1,
            "RENDER-01",
            &["D:\\Work".into()],
            Some("user"),
            Some("pass"),
        );
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
    }

    #[test]
    fn empty_search_roots_returns_invalid_input() {
        let db = crate::data::open_in_memory().unwrap();
        let result = run_discovery(&db, 1, "h", &[], None, None);
        assert!(matches!(result, Err(UecmError::InvalidInput(_))));
    }
}
```

- [ ] **Step 2: Wire into `core/mod.rs`**

Append:

```rust
pub mod project_discovery;
```

- [ ] **Step 3: Run the tests**

```bash
cd src-tauri && cargo test --lib core::project_discovery 2>&1 | tail -10 && cd ..
```

Expected: 2 tests pass on macOS (1 PowerShell error guard + 1 input validation).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/core/project_discovery.rs src-tauri/src/core/mod.rs
git commit -m "feat: core::project_discovery — WinRM walk + persist"
```

---

## Task 7: Tauri commands for projects

**Files:**
- Create: `src-tauri/src/commands/projects.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write the commands module**

```rust
//! Project management commands: discover, list, manual-set, delete.

use crate::core::project_discovery::{self, DiscoveryResult};
use crate::core::project_identity::{stem_lower, MatchKind};
use crate::data::{
    machines as data_machines,
    project_locations::{self, DiscoveryStatus, ProjectLocation},
    projects::{self, Project},
    Db,
};
use crate::error::{UecmError, UecmResult};
use serde::Serialize;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct ProjectSummary {
    pub id: i64,
    pub uproject_name: String,
    pub display_name: Option<String>,
    pub uproject_guid: Option<String>,
    pub location_count: i64,
}

fn resolve_operator_creds(
    db: &Db,
    alias: Option<&str>,
) -> UecmResult<(Option<String>, Option<String>)> {
    let Some(alias) = alias else { return Ok((None, None)); };
    let cred = crate::data::credentials::get_by_alias(db, alias)?
        .ok_or_else(|| UecmError::InvalidInput(format!("credential '{}' not found", alias)))?;
    let pass = crate::core::credentials::read_password(&cred)?;
    Ok((Some(cred.username), Some(pass)))
}

#[tauri::command]
pub fn list_projects(db: State<'_, Db>) -> UecmResult<Vec<ProjectSummary>> {
    let projects_list = projects::list(&db)?;
    let mut out = Vec::with_capacity(projects_list.len());
    for p in projects_list {
        let id = p.id.unwrap_or(0);
        let locs = project_locations::list_by_project(&db, id)?;
        out.push(ProjectSummary {
            id,
            uproject_name: p.uproject_name,
            display_name: p.display_name,
            uproject_guid: p.uproject_guid,
            location_count: locs.len() as i64,
        });
    }
    Ok(out)
}

#[tauri::command]
pub fn list_project_locations(
    db: State<'_, Db>,
    project_id: i64,
) -> UecmResult<Vec<ProjectLocation>> {
    project_locations::list_by_project(&db, project_id)
}

#[tauri::command]
pub fn discover_projects(
    db: State<'_, Db>,
    machine_id: i64,
    search_roots: Vec<String>,
    operator_credential_alias: Option<String>,
) -> UecmResult<Vec<DiscoveryResult>> {
    let machine = data_machines::get(&db, machine_id)?
        .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", machine_id)))?;
    let (op_user, op_pass) = resolve_operator_creds(&db, operator_credential_alias.as_deref())?;
    project_discovery::run_discovery(
        &db,
        machine_id,
        &machine.ip,
        &search_roots,
        op_user.as_deref(),
        op_pass.as_deref(),
    )
}

#[tauri::command]
pub fn set_project_location(
    db: State<'_, Db>,
    project_id: i64,
    machine_id: i64,
    abs_path: String,
    uproject_path: String,
    manual: bool,
) -> UecmResult<i64> {
    let kind = if manual {
        DiscoveryStatus::ManualPath
    } else {
        DiscoveryStatus::ManualAlias
    };
    let loc = ProjectLocation {
        id: None,
        project_id,
        machine_id,
        abs_path,
        uproject_path,
        discovery_status: kind,
        discovered_at: None,
    };
    project_locations::upsert(&db, &loc)
}

#[tauri::command]
pub fn delete_project(db: State<'_, Db>, project_id: i64) -> UecmResult<()> {
    projects::delete(&db, project_id)
}

#[tauri::command]
pub fn delete_project_location(
    db: State<'_, Db>,
    location_id: i64,
) -> UecmResult<()> {
    project_locations::delete(&db, location_id)
}

#[tauri::command]
pub fn create_project_manual(
    db: State<'_, Db>,
    uproject_name: String,
    display_name: Option<String>,
) -> UecmResult<i64> {
    let stem = stem_lower(&uproject_name);
    let p = Project {
        id: None,
        uproject_name,
        uproject_stem_lower: stem,
        uproject_guid: None,
        display_name,
        first_seen_at: None,
        last_seen_at: None,
    };
    projects::upsert(&db, &p)
}
```

- [ ] **Step 2: Wire into `commands/mod.rs`**

```rust
pub mod projects;
```

- [ ] **Step 3: Register in `lib.rs`**

In the `invoke_handler!` macro, add:

```rust
            commands::projects::list_projects,
            commands::projects::list_project_locations,
            commands::projects::discover_projects,
            commands::projects::set_project_location,
            commands::projects::delete_project,
            commands::projects::delete_project_location,
            commands::projects::create_project_manual,
```

- [ ] **Step 4: Build to verify**

```bash
cd src-tauri && cargo build --lib 2>&1 | tail -5 && cd ..
```

Expected: clean build. (Note: `data::machines::get` and `core::credentials::read_password` are assumed to exist from Plan 1/3. If a name has drifted, adjust.)

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/projects.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat: tauri commands for projects (discover/list/set/delete)"
```

---

## Task 8: PowerShell — `start-ue-process.ps1` + `tail-ue-log.ps1` + `stop-ue-process.ps1`

**Files:**
- Create: `ps-scripts/start-ue-process.ps1`
- Create: `ps-scripts/tail-ue-log.ps1`
- Create: `ps-scripts/stop-ue-process.ps1`

- [ ] **Step 1: Write `start-ue-process.ps1`**

```powershell
param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$EnginePath,    # e.g. C:\UnrealEngine\UE_5.4
    [Parameter(Mandatory=$true)] [string]$ProjectPath,   # absolute .uproject
    [Parameter(Mandatory=$true)] [string[]]$ExtraArgs,   # NEVER joined to a string — each arg stays atomic
    [string]$Username,
    [string]$Password
)

$ErrorActionPreference = 'Stop'

function Build-CredentialOrNull {
    param([string]$User, [string]$Pass)
    if ([string]::IsNullOrEmpty($User) -or [string]::IsNullOrEmpty($Pass)) { return $null }
    $secure = ConvertTo-SecureString -String $Pass -AsPlainText -Force
    return New-Object System.Management.Automation.PSCredential($User, $secure)
}

try {
    $script = {
        param($EnginePath, $ProjectPath, [string[]]$ExtraArgs)
        $exe = Join-Path -Path $EnginePath -ChildPath 'Engine\Binaries\Win64\UnrealEditor.exe'
        if (-not (Test-Path -LiteralPath $exe)) {
            throw "UnrealEditor.exe not found at $exe"
        }
        if (-not (Test-Path -LiteralPath $ProjectPath)) {
            throw "uproject not found at $ProjectPath"
        }
        # Start-Process -ArgumentList accepts a string array; each element stays
        # one argv entry. Args containing spaces (e.g. -ExecCmds="r.X 1; r.Y 1")
        # MUST be passed as a single element of ExtraArgs.
        $argList = @("`"$ProjectPath`"") + $ExtraArgs
        $proc = Start-Process -FilePath $exe -ArgumentList $argList -PassThru -WindowStyle Hidden
        $projDir = Split-Path -LiteralPath $ProjectPath -Parent
        $projName = [System.IO.Path]::GetFileNameWithoutExtension($ProjectPath)
        $logPath = Join-Path -Path $projDir -ChildPath ("Saved\Logs\$projName.log")
        return @{
            pid       = "$($proc.Id)"
            log_path  = "$logPath"
            project_dir = "$projDir"
            project_name = "$projName"
        }
    }

    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock  = $script
        # Note: $ExtraArgs is wrapped via comma-prefix to preserve array shape
        # across the Invoke-Command boundary (PS unwraps single-element arrays).
        ArgumentList = @($EnginePath, $ProjectPath, ,$ExtraArgs)
        ErrorAction  = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $r = Invoke-Command @invokeArgs

    @{ ok = $true; pid = "$($r.pid)"; log_path = "$($r.log_path)"; project_dir = "$($r.project_dir)"; project_name = "$($r.project_name)" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; pid = ""; log_path = ""; message = "$($_.Exception.Message)" } | ConvertTo-Json -Compress
    exit 1
}
```

**Calling convention from Rust**: `core::powershell::run_json` already passes each Rust `&str` as a separate argv entry. To pass an array to a PowerShell `[string[]]` param, repeat the param name once per element:

```
-ExtraArgs "-run=DerivedDataCache" -ExtraArgs "-fill" -ExtraArgs "-DDC=CreatePak" -ExtraArgs "-unattended" ...
```

The Rust caller in T9 builds this multi-occurrence form. **Never** concatenate the args into one string — that's the bug the adversarial review caught (PSO `-ExecCmds="r.X 1; r.Y 1"` would be split on the inner space).

- [ ] **Step 2: Write `tail-ue-log.ps1`**

```powershell
param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$LogPath,
    [Parameter(Mandatory=$true)] [int]$LastReadOffset,   # bytes
    [int]$MaxBytes = 65536,
    [string]$Username,
    [string]$Password
)

$ErrorActionPreference = 'Stop'

function Build-CredentialOrNull {
    param([string]$User, [string]$Pass)
    if ([string]::IsNullOrEmpty($User) -or [string]::IsNullOrEmpty($Pass)) { return $null }
    $secure = ConvertTo-SecureString -String $Pass -AsPlainText -Force
    return New-Object System.Management.Automation.PSCredential($User, $secure)
}

try {
    $script = {
        param($LogPath, $LastReadOffset, $MaxBytes)
        if (-not (Test-Path -LiteralPath $LogPath)) {
            return @{ exists = $false; new_offset = 0; new_text = "" }
        }
        $size = (Get-Item -LiteralPath $LogPath).Length
        if ($size -le $LastReadOffset) {
            return @{ exists = $true; new_offset = "$size"; new_text = "" }
        }
        $start = $LastReadOffset
        $count = [Math]::Min($MaxBytes, ($size - $start))
        $stream = [System.IO.File]::Open($LogPath, 'Open', 'Read', 'ReadWrite')
        try {
            $stream.Seek($start, 'Begin') | Out-Null
            $buf = New-Object byte[] $count
            $read = $stream.Read($buf, 0, $count)
            $text = [System.Text.Encoding]::UTF8.GetString($buf, 0, $read)
        } finally {
            $stream.Dispose()
        }
        return @{ exists = $true; new_offset = "$($start + $read)"; new_text = "$text" }
    }

    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock  = $script
        ArgumentList = @($LogPath, $LastReadOffset, $MaxBytes)
        ErrorAction  = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $r = Invoke-Command @invokeArgs

    @{ ok = $true; exists = $r.exists; new_offset = "$($r.new_offset)"; new_text = "$($r.new_text)" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; message = "$($_.Exception.Message)" } | ConvertTo-Json -Compress
    exit 1
}
```

- [ ] **Step 3: Write `stop-ue-process.ps1`**

```powershell
param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [int]$Pid,
    [string]$Username,
    [string]$Password
)

$ErrorActionPreference = 'Stop'

function Build-CredentialOrNull {
    param([string]$User, [string]$Pass)
    if ([string]::IsNullOrEmpty($User) -or [string]::IsNullOrEmpty($Pass)) { return $null }
    $secure = ConvertTo-SecureString -String $Pass -AsPlainText -Force
    return New-Object System.Management.Automation.PSCredential($User, $secure)
}

try {
    $script = {
        param($TargetPid)
        try {
            Stop-Process -Id $TargetPid -Force -ErrorAction Stop
            return @{ killed = $true; message = "stopped pid $TargetPid" }
        } catch {
            return @{ killed = $false; message = "$($_.Exception.Message)" }
        }
    }
    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock  = $script
        ArgumentList = @($Pid)
        ErrorAction  = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $r = Invoke-Command @invokeArgs

    @{ ok = $true; killed = "$($r.killed)" -eq "True"; message = "$($r.message)" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; killed = $false; message = "$($_.Exception.Message)" } | ConvertTo-Json -Compress
    exit 1
}
```

- [ ] **Step 4: Commit**

```bash
git add ps-scripts/start-ue-process.ps1 ps-scripts/tail-ue-log.ps1 ps-scripts/stop-ue-process.ps1
git commit -m "feat(ps): UE process orchestration sidecars (start/tail/stop)"
```

---

## Task 9: Rust `core::ue_runner` — async UE process orchestrator

**Files:**
- Create: `src-tauri/src/core/ue_runner.rs`
- Modify: `src-tauri/src/core/mod.rs`

This task implements the foundation that BOTH Plan 5 (DDC Pak) and Plan 6 (PSO Cache) build on. Take care with the `UeRunnerEvent` enum — its values are part of the contract with the frontend.

- [ ] **Step 1: Write the runner**

```rust
//! Async UE process orchestrator. Spawns UE.exe (remotely or locally),
//! tails its log every 1s, parses progress markers, and emits events
//! over an mpsc channel until completion or cancellation.

use crate::core::powershell;
use crate::error::{UecmError, UecmResult};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{sleep, Duration};

const TAIL_INTERVAL: Duration = Duration::from_millis(1000);
const MAX_LOG_TAIL_LINES: usize = 200;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UeRunnerBackend {
    Local,
    Remote,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UeRunnerEvent {
    Spawned { pid: i64, log_path: String },
    LogLine { text: String, parsed_kind: Option<String> },
    Progress { pct: Option<f32>, label: String },
    Completed { exit_code: i32, log_tail: Vec<String> },
    Cancelled,
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UeRunSpec {
    pub backend: UeRunnerBackend,
    pub host: String,            // unused for Local
    pub engine_path: String,     // engine root
    pub project_path: String,    // .uproject absolute
    pub extra_args: Vec<String>, // each element is one argv entry — never joined
    pub credential_user: Option<String>,
    pub credential_pass: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StartScriptResult {
    ok: bool,
    pid: String,
    log_path: String,
    #[serde(default)]
    project_dir: String,
    #[serde(default)]
    project_name: String,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TailScriptResult {
    ok: bool,
    #[serde(default)]
    exists: bool,
    #[serde(default)]
    new_offset: String,
    #[serde(default)]
    new_text: String,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StopScriptResult {
    ok: bool,
    #[serde(default)]
    killed: bool,
    #[serde(default)]
    message: String,
}

pub struct RunnerHandle {
    pub events: mpsc::UnboundedReceiver<UeRunnerEvent>,
    pub cancel: Arc<Mutex<RunnerCancel>>,
}

#[derive(Debug, Default)]
pub struct RunnerCancel {
    pub requested: bool,
    pub host: Option<String>,
    pub pid: Option<i64>,
    pub credential_user: Option<String>,
    pub credential_pass: Option<String>,
}

pub fn run(spec: UeRunSpec) -> RunnerHandle {
    let (tx, rx) = mpsc::unbounded_channel();
    let cancel = Arc::new(Mutex::new(RunnerCancel::default()));
    let cancel_handle = cancel.clone();

    let user = spec.credential_user.clone();
    let pass = spec.credential_pass.clone();

    tokio::spawn(async move {
        // 1. Start the UE process
        let start = match start_process(&spec).await {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.send(UeRunnerEvent::Error {
                    message: format!("spawn failed: {}", e),
                });
                return;
            }
        };

        let pid: i64 = start.pid.parse().unwrap_or(-1);
        {
            let mut c = cancel_handle.lock().await;
            c.host = Some(spec.host.clone());
            c.pid = Some(pid);
            c.credential_user = user.clone();
            c.credential_pass = pass.clone();
        }
        let _ = tx.send(UeRunnerEvent::Spawned {
            pid,
            log_path: start.log_path.clone(),
        });

        // 2. Tail loop
        let mut offset: i64 = 0;
        let mut total_lines: Vec<String> = Vec::new();
        loop {
            // Check cancellation
            {
                let c = cancel_handle.lock().await;
                if c.requested {
                    drop(c);
                    let stop = stop_process(
                        &spec.backend,
                        &spec.host,
                        pid,
                        user.as_deref(),
                        pass.as_deref(),
                    )
                    .await;
                    if let Err(e) = stop {
                        let _ = tx.send(UeRunnerEvent::Error {
                            message: format!("cancel failed: {}", e),
                        });
                    }
                    let _ = tx.send(UeRunnerEvent::Cancelled);
                    return;
                }
            }

            sleep(TAIL_INTERVAL).await;

            let tail = match read_tail(
                &spec.backend,
                &spec.host,
                &start.log_path,
                offset,
                user.as_deref(),
                pass.as_deref(),
            )
            .await
            {
                Ok(t) => t,
                Err(_) => continue,
            };
            if !tail.exists {
                continue;
            }
            let new_offset: i64 = tail.new_offset.parse().unwrap_or(offset);
            offset = new_offset;
            if tail.new_text.is_empty() {
                continue;
            }

            let mut completed_exit: Option<i32> = None;
            for raw_line in tail.new_text.lines() {
                let line = raw_line.to_string();
                total_lines.push(line.clone());
                if total_lines.len() > MAX_LOG_TAIL_LINES {
                    let drop_n = total_lines.len() - MAX_LOG_TAIL_LINES;
                    total_lines.drain(0..drop_n);
                }

                let parsed = parse_line(&line);
                if let Some(p) = parsed.progress {
                    let _ = tx.send(UeRunnerEvent::Progress {
                        pct: p.pct,
                        label: p.label.clone(),
                    });
                }
                if parsed.completed_exit.is_some() {
                    completed_exit = parsed.completed_exit;
                }
                let _ = tx.send(UeRunnerEvent::LogLine {
                    text: line,
                    parsed_kind: parsed.kind.map(|k| k.to_string()),
                });
            }

            if let Some(code) = completed_exit {
                let _ = tx.send(UeRunnerEvent::Completed {
                    exit_code: code,
                    log_tail: total_lines.clone(),
                });
                return;
            }
        }
    });

    RunnerHandle { events: rx, cancel }
}

#[derive(Debug, Default)]
struct ParsedLine {
    kind: Option<&'static str>,
    progress: Option<ProgressInfo>,
    completed_exit: Option<i32>,
}

#[derive(Debug, Clone)]
struct ProgressInfo {
    pct: Option<f32>,
    label: String,
}

fn parse_line(line: &str) -> ParsedLine {
    let mut p = ParsedLine::default();
    if line.contains("LogDerivedDataCache: Display: Filling derived data cache for") {
        p.kind = Some("ddc_fill");
        p.progress = Some(ProgressInfo {
            pct: None,
            label: "Filling DDC".into(),
        });
    } else if line.contains("LogDerivedDataCache: Display: Saving pak") {
        p.kind = Some("ddc_pak_save");
        if let Some(pct) = extract_pct_in_parens(line) {
            p.progress = Some(ProgressInfo {
                pct: Some(pct),
                label: "Saving pak".into(),
            });
        } else {
            p.progress = Some(ProgressInfo {
                pct: None,
                label: "Saving pak".into(),
            });
        }
    } else if line.contains("LogDerivedDataCache: Display: Done filling derived data cache.") {
        p.kind = Some("ddc_done");
        p.progress = Some(ProgressInfo {
            pct: Some(0.95),
            label: "DDC fill complete".into(),
        });
    } else if line.contains("LogInit: Engine exit requested") || line.contains("LogExit: Exiting.") {
        p.kind = Some("exit_clean");
        p.completed_exit = Some(0);
    } else if line.contains("LogCore: Error: Critical fail") || line.contains("LogOutputDevice: Error: Assertion failed") {
        p.kind = Some("exit_critical");
        p.completed_exit = Some(1);
    }
    p
}

fn extract_pct_in_parens(line: &str) -> Option<f32> {
    // matches things like "Saving pak (3/10)" -> 0.30
    let open = line.rfind('(')?;
    let close = line.rfind(')')?;
    if close <= open {
        return None;
    }
    let inner = &line[open + 1..close];
    let parts: Vec<&str> = inner.split('/').collect();
    if parts.len() != 2 {
        return None;
    }
    let n: f32 = parts[0].trim().parse().ok()?;
    let total: f32 = parts[1].trim().parse().ok()?;
    if total <= 0.0 {
        return None;
    }
    Some(n / total)
}

async fn start_process(spec: &UeRunSpec) -> UecmResult<StartScriptResult> {
    match spec.backend {
        UeRunnerBackend::Remote => {
            let mut args: Vec<String> = vec![
                "-HostName".into(),
                spec.host.clone(),
                "-EnginePath".into(),
                spec.engine_path.clone(),
                "-ProjectPath".into(),
                spec.project_path.clone(),
            ];
            // Repeat -ExtraArgs once per element so PowerShell binds them
            // as a [string[]] without splitting any single element.
            for a in &spec.extra_args {
                args.push("-ExtraArgs".into());
                args.push(a.clone());
            }
            if let (Some(u), Some(p)) = (spec.credential_user.as_deref(), spec.credential_pass.as_deref()) {
                args.push("-Username".into());
                args.push(u.into());
                args.push("-Password".into());
                args.push(p.into());
            }
            let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            let r: StartScriptResult = powershell::run_json(
                &powershell::script_path("start-ue-process.ps1"),
                &args_ref,
            )?;
            if !r.ok {
                return Err(UecmError::OperationFailed(
                    r.message.unwrap_or_else(|| "spawn failed".into()),
                ));
            }
            Ok(r)
        }
        UeRunnerBackend::Local => {
            // Local: shell out directly.
            #[cfg(windows)]
            {
                use std::path::PathBuf;
                let exe = PathBuf::from(&spec.engine_path)
                    .join("Engine")
                    .join("Binaries")
                    .join("Win64")
                    .join("UnrealEditor.exe");
                if !exe.exists() {
                    return Err(UecmError::InvalidInput(format!(
                        "UnrealEditor.exe not found at {}",
                        exe.display()
                    )));
                }
                let proj = PathBuf::from(&spec.project_path);
                let mut cmd = tokio::process::Command::new(&exe);
                cmd.arg(&proj);
                // Each spec.extra_args element is one argv entry; do NOT split.
                for a in &spec.extra_args {
                    cmd.arg(a);
                }
                let child = cmd.spawn().map_err(UecmError::Io)?;
                let pid = child.id().unwrap_or(0) as i64;
                let project_dir = proj.parent()
                    .ok_or_else(|| UecmError::InvalidInput("project parent missing".into()))?
                    .to_string_lossy().to_string();
                let project_name = proj.file_stem()
                    .ok_or_else(|| UecmError::InvalidInput("project stem missing".into()))?
                    .to_string_lossy().to_string();
                let log_path = format!("{}\\Saved\\Logs\\{}.log", project_dir, project_name);
                Ok(StartScriptResult {
                    ok: true,
                    pid: pid.to_string(),
                    log_path,
                    project_dir,
                    project_name,
                    message: None,
                })
            }
            #[cfg(not(windows))]
            {
                let _ = spec;
                Err(UecmError::OperationFailed(
                    "local UE backend requires Windows".into(),
                ))
            }
        }
    }
}

async fn read_tail(
    backend: &UeRunnerBackend,
    host: &str,
    log_path: &str,
    offset: i64,
    user: Option<&str>,
    pass: Option<&str>,
) -> UecmResult<TailScriptResult> {
    match backend {
        UeRunnerBackend::Remote => {
            let mut args: Vec<String> = vec![
                "-HostName".into(),
                host.into(),
                "-LogPath".into(),
                log_path.into(),
                "-LastReadOffset".into(),
                offset.to_string(),
            ];
            if let (Some(u), Some(p)) = (user, pass) {
                args.push("-Username".into());
                args.push(u.into());
                args.push("-Password".into());
                args.push(p.into());
            }
            let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            let r: TailScriptResult = powershell::run_json(
                &powershell::script_path("tail-ue-log.ps1"),
                &args_ref,
            )?;
            Ok(r)
        }
        UeRunnerBackend::Local => {
            use std::io::{Read, Seek, SeekFrom};
            let path = std::path::Path::new(log_path);
            if !path.exists() {
                return Ok(TailScriptResult { ok: true, exists: false, new_offset: "0".into(), new_text: String::new(), message: None });
            }
            let mut file = std::fs::File::open(path).map_err(UecmError::Io)?;
            let size = file.metadata().map_err(UecmError::Io)?.len() as i64;
            if size <= offset {
                return Ok(TailScriptResult { ok: true, exists: true, new_offset: size.to_string(), new_text: String::new(), message: None });
            }
            file.seek(SeekFrom::Start(offset as u64)).map_err(UecmError::Io)?;
            let to_read = std::cmp::min(65536, (size - offset) as usize);
            let mut buf = vec![0u8; to_read];
            let read = file.read(&mut buf).map_err(UecmError::Io)?;
            let text = String::from_utf8_lossy(&buf[..read]).to_string();
            Ok(TailScriptResult { ok: true, exists: true, new_offset: (offset + read as i64).to_string(), new_text: text, message: None })
        }
    }
}

async fn stop_process(
    backend: &UeRunnerBackend,
    host: &str,
    pid: i64,
    user: Option<&str>,
    pass: Option<&str>,
) -> UecmResult<()> {
    match backend {
        UeRunnerBackend::Remote => {
            let mut args: Vec<String> = vec![
                "-HostName".into(),
                host.into(),
                "-Pid".into(),
                pid.to_string(),
            ];
            if let (Some(u), Some(p)) = (user, pass) {
                args.push("-Username".into());
                args.push(u.into());
                args.push("-Password".into());
                args.push(p.into());
            }
            let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            let r: StopScriptResult = powershell::run_json(
                &powershell::script_path("stop-ue-process.ps1"),
                &args_ref,
            )?;
            if !r.ok || !r.killed {
                return Err(UecmError::OperationFailed(r.message));
            }
            Ok(())
        }
        UeRunnerBackend::Local => {
            #[cfg(windows)]
            {
                use std::process::Command;
                let _ = Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/F"])
                    .output()
                    .map_err(UecmError::Io)?;
                Ok(())
            }
            #[cfg(not(windows))]
            {
                let _ = pid;
                Err(UecmError::OperationFailed("local stop requires Windows".into()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_line_recognises_filling_progress() {
        let line = "[2026.05.03-12.00.00] LogDerivedDataCache: Display: Filling derived data cache for /Game/Foo";
        let p = parse_line(line);
        assert_eq!(p.kind, Some("ddc_fill"));
        assert!(p.progress.is_some());
    }

    #[test]
    fn parse_line_recognises_saving_pak_with_pct() {
        let line = "[2026.05.03-12.00.05] LogDerivedDataCache: Display: Saving pak (3/10)";
        let p = parse_line(line);
        assert_eq!(p.kind, Some("ddc_pak_save"));
        let pct = p.progress.unwrap().pct.unwrap();
        assert!((pct - 0.3).abs() < 1e-3);
    }

    #[test]
    fn parse_line_recognises_clean_exit() {
        let line = "[2026.05.03-12.10.00] LogInit: Engine exit requested. Reason: -run=DerivedDataCache complete";
        let p = parse_line(line);
        assert_eq!(p.completed_exit, Some(0));
    }

    #[test]
    fn parse_line_recognises_critical_fail() {
        let line = "[2026.05.03-12.00.10] LogCore: Error: Critical fail in shader compile";
        let p = parse_line(line);
        assert_eq!(p.completed_exit, Some(1));
    }

    #[test]
    fn parse_line_ignores_unrelated() {
        let line = "[noise] LogTemp: nothing useful";
        let p = parse_line(line);
        assert!(p.kind.is_none());
        assert!(p.progress.is_none());
        assert!(p.completed_exit.is_none());
    }

    #[test]
    fn extract_pct_handles_garbage() {
        assert!(extract_pct_in_parens("no parens here").is_none());
        assert!(extract_pct_in_parens("(abc/def)").is_none());
        assert!(extract_pct_in_parens("(0/0)").is_none());
    }
}
```

- [ ] **Step 2: Wire into `core/mod.rs`**

```rust
pub mod ue_runner;
```

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test --lib core::ue_runner 2>&1 | tail -10 && cd ..
```

Expected: 6 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/core/ue_runner.rs src-tauri/src/core/mod.rs
git commit -m "feat: core::ue_runner async UE process orchestrator (foundation for D1 + F2)"
```

---

## Task 10: PowerShell — `generate-ddc-pak.ps1` + Rust `core::ddc_pak`

**Files:**
- Create: `ps-scripts/generate-ddc-pak.ps1`
- Create: `src-tauri/src/core/ddc_pak.rs`
- Modify: `src-tauri/src/core/mod.rs`

- [ ] **Step 1: Write `generate-ddc-pak.ps1`**

This is a thin pre-flight verifier — it does NOT spawn UE itself (the runner does). It checks that the engine + project paths are valid before we burn time on a 30-minute pak generation.

```powershell
param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$EnginePath,
    [Parameter(Mandatory=$true)] [string]$ProjectPath,
    [string]$Username,
    [string]$Password
)

$ErrorActionPreference = 'Stop'

function Build-CredentialOrNull {
    param([string]$User, [string]$Pass)
    if ([string]::IsNullOrEmpty($User) -or [string]::IsNullOrEmpty($Pass)) { return $null }
    $secure = ConvertTo-SecureString -String $Pass -AsPlainText -Force
    return New-Object System.Management.Automation.PSCredential($User, $secure)
}

try {
    $script = {
        param($EnginePath, $ProjectPath)
        $exe = Join-Path -Path $EnginePath -ChildPath 'Engine\Binaries\Win64\UnrealEditor.exe'
        $exists_exe = Test-Path -LiteralPath $exe
        $exists_proj = Test-Path -LiteralPath $ProjectPath
        $projDir = Split-Path -LiteralPath $ProjectPath -Parent
        $ddcDir = Join-Path -Path $projDir -ChildPath 'DerivedDataCache'
        $hasDdcDir = Test-Path -LiteralPath $ddcDir
        return @{
            exe_exists  = "$exists_exe" -eq "True"
            proj_exists = "$exists_proj" -eq "True"
            ddc_dir_exists = "$hasDdcDir" -eq "True"
            ddc_dir = "$ddcDir"
        }
    }
    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock  = $script
        ArgumentList = @($EnginePath, $ProjectPath)
        ErrorAction  = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $r = Invoke-Command @invokeArgs

    @{ ok = $true; exe_exists = $r.exe_exists; proj_exists = $r.proj_exists; ddc_dir_exists = $r.ddc_dir_exists; ddc_dir = "$($r.ddc_dir)" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; message = "$($_.Exception.Message)" } | ConvertTo-Json -Compress
    exit 1
}
```

Plus a `verify-pak-output.ps1` for AFTER generation. Append to the same task:

```powershell
# ps-scripts/verify-pak-output.ps1
param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$ProjectDir,
    [string]$Username,
    [string]$Password
)

$ErrorActionPreference = 'Stop'

function Build-CredentialOrNull {
    param([string]$User, [string]$Pass)
    if ([string]::IsNullOrEmpty($User) -or [string]::IsNullOrEmpty($Pass)) { return $null }
    $secure = ConvertTo-SecureString -String $Pass -AsPlainText -Force
    return New-Object System.Management.Automation.PSCredential($User, $secure)
}

try {
    $script = {
        param($ProjectDir)
        $candidates = @('Compressed.ddp', 'DDC.ddp')
        $found = $null
        foreach ($name in $candidates) {
            $p = Join-Path -Path $ProjectDir -ChildPath "DerivedDataCache\$name"
            if (Test-Path -LiteralPath $p) {
                $size = (Get-Item -LiteralPath $p).Length
                $found = @{ path = "$p"; size = "$size"; name = "$name" }
                break
            }
        }
        if ($null -eq $found) { return @{ found = $false; path = ""; size = "0" } }
        return @{ found = $true; path = $found.path; size = $found.size }
    }
    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock  = $script
        ArgumentList = @($ProjectDir)
        ErrorAction  = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $r = Invoke-Command @invokeArgs
    @{ ok = $true; found = "$($r.found)" -eq "True"; path = "$($r.path)"; size = "$($r.size)" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; message = "$($_.Exception.Message)" } | ConvertTo-Json -Compress
    exit 1
}
```

- [ ] **Step 2: Write `core::ddc_pak`**

```rust
//! High-level DDC Pak generation: pre-flight verify, run UE, post-flight verify.

use crate::core::powershell;
use crate::core::ue_runner::{self, UeRunSpec, UeRunnerBackend, UeRunnerEvent};
use crate::error::{UecmError, UecmResult};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

#[derive(Debug, Deserialize)]
struct PreflightRaw {
    ok: bool,
    #[serde(default)]
    exe_exists: bool,
    #[serde(default)]
    proj_exists: bool,
    #[serde(default)]
    ddc_dir_exists: bool,
    #[serde(default)]
    ddc_dir: String,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VerifyRaw {
    ok: bool,
    #[serde(default)]
    found: bool,
    #[serde(default)]
    path: String,
    #[serde(default)]
    size: String,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PakOutput {
    pub path: String,
    pub size_bytes: i64,
}

fn default_extra_args() -> Vec<String> {
    vec![
        "-run=DerivedDataCache".into(),
        "-fill".into(),
        "-DDC=CreatePak".into(),
        "-unattended".into(),
        "-nopause".into(),
        "-nosplash".into(),
    ]
}

pub fn preflight(
    host: &str,
    engine_path: &str,
    project_path: &str,
    user: Option<&str>,
    pass: Option<&str>,
) -> UecmResult<()> {
    let mut args: Vec<String> = vec![
        "-HostName".into(),
        host.into(),
        "-EnginePath".into(),
        engine_path.into(),
        "-ProjectPath".into(),
        project_path.into(),
    ];
    if let (Some(u), Some(p)) = (user, pass) {
        args.push("-Username".into());
        args.push(u.into());
        args.push("-Password".into());
        args.push(p.into());
    }
    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let r: PreflightRaw = powershell::run_json(
        &powershell::script_path("generate-ddc-pak.ps1"),
        &args_ref,
    )?;
    if !r.ok {
        return Err(UecmError::OperationFailed(
            r.message.unwrap_or_else(|| "preflight failed".into()),
        ));
    }
    if !r.exe_exists {
        return Err(UecmError::InvalidInput(
            "UnrealEditor.exe not found at engine_path".into(),
        ));
    }
    if !r.proj_exists {
        return Err(UecmError::InvalidInput(
            ".uproject not found at project_path".into(),
        ));
    }
    Ok(())
}

pub fn verify_output(
    host: &str,
    project_dir: &str,
    user: Option<&str>,
    pass: Option<&str>,
) -> UecmResult<PakOutput> {
    let mut args: Vec<String> = vec![
        "-HostName".into(),
        host.into(),
        "-ProjectDir".into(),
        project_dir.into(),
    ];
    if let (Some(u), Some(p)) = (user, pass) {
        args.push("-Username".into());
        args.push(u.into());
        args.push("-Password".into());
        args.push(p.into());
    }
    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let r: VerifyRaw = powershell::run_json(
        &powershell::script_path("verify-pak-output.ps1"),
        &args_ref,
    )?;
    if !r.ok {
        return Err(UecmError::OperationFailed(
            r.message.unwrap_or_else(|| "verify failed".into()),
        ));
    }
    if !r.found {
        return Err(UecmError::OperationFailed(
            ".ddp not found after generation".into(),
        ));
    }
    let size: i64 = r.size.parse().unwrap_or(0);
    Ok(PakOutput {
        path: r.path,
        size_bytes: size,
    })
}

pub fn launch_generation(
    backend: UeRunnerBackend,
    host: &str,
    engine_path: &str,
    project_path: &str,
    user: Option<&str>,
    pass: Option<&str>,
) -> ue_runner::RunnerHandle {
    let spec = UeRunSpec {
        backend,
        host: host.to_string(),
        engine_path: engine_path.to_string(),
        project_path: project_path.to_string(),
        extra_args: default_extra_args(),
        credential_user: user.map(String::from),
        credential_pass: pass.map(String::from),
    };
    ue_runner::run(spec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn preflight_returns_powershell_error_on_non_windows() {
        let r = preflight("h", "C:\\UE", "C:\\X.uproject", Some("u"), Some("p"));
        assert!(matches!(r, Err(UecmError::PowerShell(_)) | Err(UecmError::OperationFailed(_))));
    }

    #[cfg(not(windows))]
    #[test]
    fn verify_returns_powershell_error_on_non_windows() {
        let r = verify_output("h", "C:\\X", Some("u"), Some("p"));
        assert!(matches!(r, Err(UecmError::PowerShell(_)) | Err(UecmError::OperationFailed(_))));
    }
}
```

- [ ] **Step 3: Wire into `core/mod.rs`**

```rust
pub mod ddc_pak;
```

- [ ] **Step 4: Run tests + build**

```bash
cd src-tauri && cargo test --lib core::ddc_pak 2>&1 | tail -10 && cargo build --lib 2>&1 | tail -3 && cd ..
```

Expected: 2 tests pass on macOS, build green.

- [ ] **Step 5: Commit**

```bash
git add ps-scripts/generate-ddc-pak.ps1 ps-scripts/verify-pak-output.ps1 src-tauri/src/core/ddc_pak.rs src-tauri/src/core/mod.rs
git commit -m "feat: ddc-pak preflight + post-flight + UE launch wrapper"
```

---

## Task 11: Tauri commands for DDC Pak generation

**Files:**
- Create: `src-tauri/src/commands/ddc_pak.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write the commands**

```rust
//! Generation + cancellation commands. Distribution lives in the same module
//! (added in T15).

use crate::core::ddc_pak;
use crate::core::ue_runner::{RunnerCancel, UeRunnerBackend, UeRunnerEvent};
use crate::data::{
    machine_ue_installs, machines as data_machines,
    project_locations::{self},
    Db,
};
use crate::error::{UecmError, UecmResult};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{Emitter, State};
use tokio::sync::Mutex;

/// In-memory registry of running UE jobs, keyed by an opaque `job_id`.
/// The frontend gets the job_id back from `generate_ddc_pak` and uses it to
/// cancel later via `cancel_ue_job`.
#[derive(Default)]
pub struct UeJobRegistry {
    jobs: Mutex<HashMap<String, Arc<Mutex<RunnerCancel>>>>,
}

impl UeJobRegistry {
    pub async fn insert(&self, job_id: &str, cancel: Arc<Mutex<RunnerCancel>>) {
        self.jobs.lock().await.insert(job_id.to_string(), cancel);
    }
    pub async fn remove(&self, job_id: &str) {
        self.jobs.lock().await.remove(job_id);
    }
    pub async fn cancel(&self, job_id: &str) -> bool {
        let map = self.jobs.lock().await;
        if let Some(c) = map.get(job_id) {
            let mut g = c.lock().await;
            g.requested = true;
            return true;
        }
        false
    }
}

#[derive(Debug, Serialize)]
pub struct GenerateJobResponse {
    pub job_id: String,
    pub source_machine_id: i64,
    pub project_id: i64,
    pub backend: String,
}

fn resolve_engine_path(
    db: &Db,
    machine_id: i64,
    preferred_version: Option<&str>,
) -> UecmResult<String> {
    let installs = machine_ue_installs::list_for_machine(db, machine_id)?;
    if installs.is_empty() {
        return Err(UecmError::InvalidInput(format!(
            "machine {} has no detected UE installs",
            machine_id
        )));
    }
    let chosen = if let Some(v) = preferred_version {
        installs
            .into_iter()
            .find(|i| i.version == v)
            .ok_or_else(|| UecmError::InvalidInput(format!("UE {} not on machine", v)))?
    } else {
        installs
            .iter()
            .find(|i| i.is_primary)
            .cloned()
            .unwrap_or_else(|| installs.into_iter().next().unwrap())
    };
    Ok(chosen.install_path)
}

fn resolve_operator_creds(
    db: &Db,
    alias: Option<&str>,
) -> UecmResult<(Option<String>, Option<String>)> {
    let Some(alias) = alias else { return Ok((None, None)); };
    let cred = crate::data::credentials::get_by_alias(db, alias)?
        .ok_or_else(|| UecmError::InvalidInput(format!("credential '{}' not found", alias)))?;
    let pass = crate::core::credentials::read_password(&cred)?;
    Ok((Some(cred.username), Some(pass)))
}

/// Backend selector. The frontend wizard sends "remote" for an inventoried
/// render box and "local" for "self" mode (operator machine). For "local",
/// the engine path comes from a per-app config (see below) — NOT from the
/// machines table — so the operator does NOT have to register their own box
/// in the inventory. The previous design that demanded a self-row violated
/// the C1/C3 contract.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BackendChoice {
    Remote,
    Local,
}

#[tauri::command]
pub async fn generate_ddc_pak(
    app: tauri::AppHandle,
    db: State<'_, Db>,
    registry: State<'_, UeJobRegistry>,
    backend: BackendChoice,
    source_machine_id: Option<i64>,         // required for Remote, ignored for Local
    project_id: i64,
    /// For Local backend: absolute path to the .uproject ON the operator
    /// machine. For Remote: ignored (resolved via project_locations).
    local_uproject_path: Option<String>,
    /// For Local backend: engine root on the operator machine, e.g.
    /// `C:\UnrealEngine\UE_5.4`. Reused as-is when supplied; otherwise the
    /// app falls back to a per-machine config file (`%LOCALAPPDATA%\UECM\
    /// operator-config.json`) or the highest-priority `machine_ue_installs`
    /// row tagged with `is_operator=1` (see Plan 1 schema notes).
    local_engine_path: Option<String>,
    ue_version: Option<String>,
    operator_credential_alias: Option<String>,
} -> UecmResult<GenerateJobResponse> {
    // Resolve the four pieces — engine_path, uproject_path, host, runtime backend —
    // for both branches. The branches converge on a single launch.
    let (host, engine_path, uproject_path, runtime_backend) = match backend {
        BackendChoice::Remote => {
            let mid = source_machine_id.ok_or_else(|| {
                UecmError::InvalidInput("source_machine_id required for remote backend".into())
            })?;
            let machine = data_machines::get(&db, mid)?
                .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", mid)))?;
            let location = project_locations::get_for_project_machine(&db, project_id, mid)?
                .ok_or_else(|| UecmError::InvalidInput(format!(
                    "project {} not located on machine {}",
                    project_id, mid
                )))?;
            let engine = resolve_engine_path(&db, mid, ue_version.as_deref())?;
            (machine.ip, engine, location.uproject_path, UeRunnerBackend::Remote)
        }
        BackendChoice::Local => {
            let upath = local_uproject_path.ok_or_else(|| {
                UecmError::InvalidInput("local_uproject_path required for local backend".into())
            })?;
            let epath = local_engine_path
                .or_else(|| resolve_operator_engine_path(ue_version.as_deref()).ok())
                .ok_or_else(|| UecmError::InvalidInput(
                    "could not resolve local engine path; pass local_engine_path or configure %LOCALAPPDATA%\\UECM\\operator-config.json".into()
                ))?;
            // host is unused for Local; pass empty string.
            (String::new(), epath, upath, UeRunnerBackend::Local)
        }
    };

    let (op_user, op_pass) = resolve_operator_creds(&db, operator_credential_alias.as_deref())?;

    // Pre-flight only meaningful for Remote (verifies remote file existence
    // via WinRM). For Local, the runner's spawn step already does the same
    // check via std::fs::metadata.
    if matches!(runtime_backend, UeRunnerBackend::Remote) {
        ddc_pak::preflight(
            &host,
            &engine_path,
            &uproject_path,
            op_user.as_deref(),
            op_pass.as_deref(),
        )?;
    }

    let handle = ddc_pak::launch_generation(
        runtime_backend,
        &host,
        &engine_path,
        &uproject_path,
        op_user.as_deref(),
        op_pass.as_deref(),
    );

    let resolved_source_id = source_machine_id.unwrap_or(-1);
    let job_id = format!("ddc-pak-gen-{}-{}", resolved_source_id, chrono::Utc::now().timestamp_millis());
    registry.insert(&job_id, handle.cancel.clone()).await;

    let app_clone = app.clone();
    let job_id_clone = job_id.clone();
    let mid = resolved_source_id;
    let pid = project_id;
    let project_dir_for_verify = std::path::Path::new(&uproject_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let host_for_verify = host.clone();
    let user_for_verify = op_user.clone();
    let pass_for_verify = op_pass.clone();
    let backend_for_verify = runtime_backend;

    let mut events = handle.events;
    tokio::spawn(async move {
        while let Some(ev) = events.recv().await {
            #[derive(Serialize)]
            struct Payload<'a> {
                job_id: &'a str,
                source_machine_id: i64,
                project_id: i64,
                event: &'a UeRunnerEvent,
            }
            let _ = app_clone.emit(
                "ue-runner-progress",
                Payload {
                    job_id: &job_id_clone,
                    source_machine_id: mid,
                    project_id: pid,
                    event: &ev,
                },
            );

            // On clean completion: verify the .ddp landed (size > 0). The frontend
            // is told whether verify passed via a separate `pak-verified` event.
            // The store keys distribute auto-chain off this event — never off
            // `Completed` directly — so we never robocopy a non-existent or
            // partially-flushed pak.
            if let UeRunnerEvent::Completed { exit_code, .. } = &ev {
                let verified: Option<ddc_pak::PakOutput> = if *exit_code == 0 {
                    match backend_for_verify {
                        UeRunnerBackend::Remote => ddc_pak::verify_output(
                            &host_for_verify,
                            &project_dir_for_verify,
                            user_for_verify.as_deref(),
                            pass_for_verify.as_deref(),
                        ).ok(),
                        UeRunnerBackend::Local => verify_output_local(&project_dir_for_verify).ok(),
                    }
                } else {
                    None
                };
                #[derive(Serialize)]
                struct VerifyPayload<'a> {
                    job_id: &'a str,
                    project_id: i64,
                    verified: bool,
                    output: Option<ddc_pak::PakOutput>,
                }
                let _ = app_clone.emit(
                    "pak-verified",
                    VerifyPayload {
                        job_id: &job_id_clone,
                        project_id: pid,
                        verified: verified.is_some(),
                        output: verified,
                    },
                );
            }

            if matches!(
                ev,
                UeRunnerEvent::Completed { .. } | UeRunnerEvent::Cancelled | UeRunnerEvent::Error { .. }
            ) {
                let registry_in_task: tauri::State<'_, UeJobRegistry> =
                    app_clone.state::<UeJobRegistry>();
                registry_in_task.remove(&job_id_clone).await;
                break;
            }
        }
    });

    Ok(GenerateJobResponse {
        job_id,
        source_machine_id: resolved_source_id,
        project_id,
        backend: match runtime_backend { UeRunnerBackend::Remote => "remote".into(), UeRunnerBackend::Local => "local".into() },
    })
}

/// Walk the operator-machine UE installs table for an `is_operator=1` row.
/// Falls back to reading `%LOCALAPPDATA%\UECM\operator-config.json`.
fn resolve_operator_engine_path(_preferred_version: Option<&str>) -> UecmResult<String> {
    if let Some(local_app) = std::env::var_os("LOCALAPPDATA") {
        let cfg_path = std::path::Path::new(&local_app).join("UECM").join("operator-config.json");
        if cfg_path.exists() {
            #[derive(Deserialize)]
            struct OpCfg { engine_path: String }
            let txt = std::fs::read_to_string(&cfg_path).map_err(UecmError::Io)?;
            let cfg: OpCfg = serde_json::from_str(&txt)
                .map_err(|e| UecmError::OperationFailed(format!("operator-config.json parse: {}", e)))?;
            return Ok(cfg.engine_path);
        }
    }
    Err(UecmError::InvalidInput(
        "no operator-config.json found; create one with {\"engine_path\":\"C:\\\\UnrealEngine\\\\UE_5.4\"}".into(),
    ))
}

/// Local-side equivalent of the remote verify-pak-output.ps1.
fn verify_output_local(project_dir: &str) -> UecmResult<ddc_pak::PakOutput> {
    let candidates = ["Compressed.ddp", "DDC.ddp"];
    for name in candidates {
        let p = std::path::Path::new(project_dir).join("DerivedDataCache").join(name);
        if p.exists() {
            let size = std::fs::metadata(&p).map_err(UecmError::Io)?.len() as i64;
            if size > 0 {
                return Ok(ddc_pak::PakOutput {
                    path: p.to_string_lossy().to_string(),
                    size_bytes: size,
                });
            }
        }
    }
    Err(UecmError::OperationFailed(".ddp not found locally".into()))
}

#[tauri::command]
pub async fn cancel_ue_job(
    registry: State<'_, UeJobRegistry>,
    job_id: String,
) -> UecmResult<bool> {
    Ok(registry.cancel(&job_id).await)
}

#[tauri::command]
pub fn verify_pak_output(
    db: State<'_, Db>,
    machine_id: i64,
    project_id: i64,
    operator_credential_alias: Option<String>,
) -> UecmResult<crate::core::ddc_pak::PakOutput> {
    let machine = data_machines::get(&db, machine_id)?
        .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", machine_id)))?;
    let location = project_locations::get_for_project_machine(&db, project_id, machine_id)?
        .ok_or_else(|| {
            UecmError::InvalidInput(format!(
                "project {} not located on machine {}",
                project_id, machine_id
            ))
        })?;
    let (op_user, op_pass) = resolve_operator_creds(&db, operator_credential_alias.as_deref())?;
    crate::core::ddc_pak::verify_output(
        &machine.ip,
        &location.abs_path,
        op_user.as_deref(),
        op_pass.as_deref(),
    )
}
```

- [ ] **Step 2: Wire into `commands/mod.rs`**

```rust
pub mod ddc_pak;
```

- [ ] **Step 3: Register in `lib.rs`**

In the setup block, after `app.manage(db)`, add:

```rust
            app.manage(commands::ddc_pak::UeJobRegistry::default());
```

In `invoke_handler!`, add:

```rust
            commands::ddc_pak::generate_ddc_pak,
            commands::ddc_pak::cancel_ue_job,
            commands::ddc_pak::verify_pak_output,
```

Also at top of `lib.rs` add `chrono` if not already imported. Check `Cargo.toml` for `chrono`; if missing, add `chrono = "0.4"`.

- [ ] **Step 4: Build to verify**

```bash
cd src-tauri && cargo build --lib 2>&1 | tail -10 && cd ..
```

Expected: clean build. If `chrono` is missing, install:

```bash
cd src-tauri && cargo add chrono && cd ..
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/ddc_pak.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat: tauri commands for DDC pak generate + cancel + verify"
```

---

## Task 12: Add `data::machine_ue_installs::list_for_machine` if missing

**Files:**
- Modify: `src-tauri/src/data/machine_ue_installs.rs` (if function absent)

This is a small support task: T11's `resolve_engine_path` calls `machine_ue_installs::list_for_machine`. If Plan 1 already exposes it, skip; otherwise add.

- [ ] **Step 1: Check**

```bash
grep -n "pub fn list_for_machine" src-tauri/src/data/machine_ue_installs.rs
```

If present: skip the rest of this task and just commit a no-op note. If absent, continue.

- [ ] **Step 2: Add the function**

```rust
pub fn list_for_machine(db: &Db, machine_id: i64) -> UecmResult<Vec<UeInstall>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, machine_id, version, install_path, is_primary, detected_at
         FROM machine_ue_installs WHERE machine_id = ? ORDER BY is_primary DESC, version",
    )?;
    let rows = stmt
        .query_map([machine_id], |r| {
            Ok(UeInstall {
                id: Some(r.get(0)?),
                machine_id: r.get(1)?,
                version: r.get(2)?,
                install_path: r.get(3)?,
                is_primary: r.get::<_, i64>(4)? != 0,
                detected_at: r.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
```

(Adapt struct name to match the existing `UeInstall` definition.)

- [ ] **Step 3: Test compile + commit**

```bash
cd src-tauri && cargo build --lib 2>&1 | tail -3 && cd ..
git add src-tauri/src/data/machine_ue_installs.rs
git commit -m "feat: data::machine_ue_installs::list_for_machine helper"
```

If skipped: `git commit --allow-empty -m "chore: no-op (list_for_machine already exists)"` — keeps task numbering aligned in git log.

---

## Task 13: PowerShell — `distribute-pak-file.ps1`

**Files:**
- Create: `ps-scripts/distribute-pak-file.ps1`

- [ ] **Step 1: Write the PS sidecar**

**Why this script accepts SOURCE credentials separately**: WinRM second-hop. `Invoke-Command` to the target lands a session whose token cannot reach a *different* machine's SMB share by default (no CredSSP / Kerberos delegation). Inside the target session we therefore explicitly map the source UNC with credentials before robocopy reads it. Pass `-SourceSmbUser` / `-SourceSmbPass` matching whatever account has read access on `<source>` (the WinRM operator credential works in most workgroup setups, but allow override).

```powershell
param(
    [Parameter(Mandatory=$true)] [string]$HostName,        # target machine (where Robocopy runs)
    [Parameter(Mandatory=$true)] [string]$SourceUnc,       # \\source-host\drive$\projDir\DerivedDataCache
    [Parameter(Mandatory=$true)] [string]$TargetLocal,     # local path on target, e.g. D:\Work\Proj\DerivedDataCache
    [string]$Username,                                      # WinRM (target) credential
    [string]$Password,
    [string]$SourceSmbUser,                                 # SMB credential for SourceUnc (read on source machine)
    [string]$SourceSmbPass,
    [switch]$PreflightOnly                                  # if set: only probe, do not robocopy
)

$ErrorActionPreference = 'Stop'

function Build-CredentialOrNull {
    param([string]$User, [string]$Pass)
    if ([string]::IsNullOrEmpty($User) -or [string]::IsNullOrEmpty($Pass)) { return $null }
    $secure = ConvertTo-SecureString -String $Pass -AsPlainText -Force
    return New-Object System.Management.Automation.PSCredential($User, $secure)
}

try {
    $script = {
        param($SourceUnc, $TargetLocal, $SmbUser, $SmbPass, $PreflightOnly)
        if (-not (Test-Path -LiteralPath $TargetLocal)) {
            New-Item -Path $TargetLocal -ItemType Directory -Force | Out-Null
        }

        # --- Second-hop SMB mapping. Mounting a temp PSDrive with explicit
        # credentials gives this session (and the robocopy process it spawns)
        # read access to the source UNC on a DIFFERENT machine.
        $driveName = "uecmsrc$([System.Diagnostics.Process]::GetCurrentProcess().Id)"
        $cred = $null
        if (-not [string]::IsNullOrEmpty($SmbUser) -and -not [string]::IsNullOrEmpty($SmbPass)) {
            $secure = ConvertTo-SecureString -String $SmbPass -AsPlainText -Force
            $cred = New-Object System.Management.Automation.PSCredential($SmbUser, $secure)
        }
        $mounted = $false
        try {
            if ($cred) {
                # New-PSDrive only persists for this session; -Persist would put it on cmdkey list.
                New-PSDrive -Name $driveName -PSProvider FileSystem -Root $SourceUnc -Credential $cred -ErrorAction Stop | Out-Null
                $mounted = $true
            }

            # Preflight: confirm source is actually reachable AFTER mapping.
            if (-not (Test-Path -LiteralPath $SourceUnc)) {
                throw "source UNC unreachable from target session: $SourceUnc"
            }
            if ($PreflightOnly) {
                return @{
                    ok = $true
                    exit_code = "0"
                    bytes_copied = "0"
                    stdout_tail = "preflight ok"
                    preflight = $true
                }
            }

            $args = @(
                "$SourceUnc",
                "$TargetLocal",
                "*.ddp",
                '/MIR',
                '/R:3',
                '/W:5',
                '/NP',
                '/NDL',
                '/NJH',
                '/NJS',
                '/BYTES'
            )
            $proc = Start-Process -FilePath 'robocopy.exe' -ArgumentList $args -PassThru -Wait -NoNewWindow -RedirectStandardOutput "$env:TEMP\robocopy-stdout-$PID.log" -RedirectStandardError "$env:TEMP\robocopy-stderr-$PID.log"
            $code = $proc.ExitCode
            $stdout = Get-Content -LiteralPath "$env:TEMP\robocopy-stdout-$PID.log" -Raw -ErrorAction SilentlyContinue
            Remove-Item -LiteralPath "$env:TEMP\robocopy-stdout-$PID.log" -ErrorAction SilentlyContinue
            Remove-Item -LiteralPath "$env:TEMP\robocopy-stderr-$PID.log" -ErrorAction SilentlyContinue
            # Robocopy exit codes: 0-7 are success-flavoured; >=8 is failure.
            $bytesCopied = 0
            try {
                $m = [regex]::Matches($stdout, 'Bytes\s*:\s*(\d+)')
                if ($m.Count -gt 0) { $bytesCopied = [long]$m[0].Groups[1].Value }
            } catch {}
            return @{
                exit_code = "$code"
                ok = ($code -lt 8)
                bytes_copied = "$bytesCopied"
                stdout_tail = if ($stdout) { ($stdout -split "`n" | Select-Object -Last 30) -join "`n" } else { "" }
            }
        }
        finally {
            if ($mounted) {
                Remove-PSDrive -Name $driveName -Force -ErrorAction SilentlyContinue
            }
        }
    }
    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock  = $script
        ArgumentList = @($SourceUnc, $TargetLocal, $SourceSmbUser, $SourceSmbPass, [bool]$PreflightOnly)
        ErrorAction  = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $r = Invoke-Command @invokeArgs

    @{
        ok = "$($r.ok)" -eq "True"
        exit_code = "$($r.exit_code)"
        bytes_copied = "$($r.bytes_copied)"
        stdout_tail = "$($r.stdout_tail)"
        preflight = $PreflightOnly.IsPresent
    } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; exit_code = "-1"; bytes_copied = "0"; message = "$($_.Exception.Message)" } | ConvertTo-Json -Compress
    exit 1
}
```

- [ ] **Step 2: Commit**

```bash
git add ps-scripts/distribute-pak-file.ps1
git commit -m "feat(ps): distribute-pak-file.ps1 via Robocopy"
```

---

## Task 14: Rust `core::pak_distribute` — multi-target Robocopy fan-out

**Files:**
- Create: `src-tauri/src/core/pak_distribute.rs`
- Modify: `src-tauri/src/core/mod.rs`

- [ ] **Step 1: Write the module**

```rust
//! Robocopy fan-out for DDC pak distribution. Reuses core::batch::run_batch
//! for concurrency + per-machine progress events.

use crate::core::powershell;
use crate::data::{
    project_locations::{self, ProjectLocation},
    Db,
};
use crate::error::{UecmError, UecmResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct DistributeRaw {
    ok: bool,
    #[serde(default)]
    exit_code: String,
    #[serde(default)]
    bytes_copied: String,
    #[serde(default)]
    stdout_tail: String,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributeOutcome {
    pub target_machine_id: i64,
    pub ok: bool,
    pub exit_code: i32,
    pub bytes_copied: i64,
    pub stdout_tail: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DistributePlanItem {
    pub target_machine_id: i64,
    pub target_host: String,
    pub source_unc: String,
    pub target_local: String,
    /// WinRM credential (target session). `None` = current process token.
    pub credential_user: Option<String>,
    pub credential_pass: Option<String>,
    /// SMB credential the *target* uses to read `source_unc`. Required for the
    /// remote-to-remote case (WinRM second hop). For "self" / loopback this
    /// can be `None` because the target session already runs under an account
    /// with read access to its own admin share.
    pub source_smb_user: Option<String>,
    pub source_smb_pass: Option<String>,
}

/// Build the per-target plan. Caller owns running it (typically through
/// core::batch::run_batch). The source UNC defaults to the source machine's
/// admin share (`\\<source>\<drive>$\<path>\DerivedDataCache`); operator can
/// override by passing `named_share_unc`.
#[allow(clippy::too_many_arguments)]
pub fn plan(
    db: &Db,
    source_machine_id: i64,
    source_host: &str,
    source_location: &ProjectLocation,
    target_machine_ids: &[i64],
    project_id: i64,
    named_share_unc: Option<&str>,
    credential_user: Option<String>,
    credential_pass: Option<String>,
    source_smb_user: Option<String>,
    source_smb_pass: Option<String>,
) -> UecmResult<Vec<DistributePlanItem>> {
    if target_machine_ids.is_empty() {
        return Err(UecmError::InvalidInput("no target machines".into()));
    }
    let source_unc = if let Some(s) = named_share_unc {
        format!("{}\\DerivedDataCache", s.trim_end_matches('\\'))
    } else {
        // admin share: \\source\D$\Work\Proj\DerivedDataCache
        let abs = source_location.abs_path.replace('/', "\\");
        if abs.len() < 2 || !abs.chars().nth(1).map_or(false, |c| c == ':') {
            return Err(UecmError::InvalidInput(format!(
                "abs_path not a drive-rooted Windows path: {}",
                abs
            )));
        }
        let drive = abs.chars().next().unwrap();
        let rest = &abs[2..]; // skip "X:"
        format!("\\\\{}\\{}$\\{}\\DerivedDataCache", source_host, drive, rest.trim_start_matches('\\'))
    };

    let mut out = Vec::with_capacity(target_machine_ids.len());
    for tid in target_machine_ids {
        if *tid == source_machine_id {
            continue; // skip self-distribute
        }
        let loc = project_locations::get_for_project_machine(db, project_id, *tid)?
            .ok_or_else(|| UecmError::InvalidInput(format!(
                "project {} has no location on machine {}",
                project_id, tid
            )))?;
        let target_machine = crate::data::machines::get(db, *tid)?
            .ok_or_else(|| UecmError::InvalidInput(format!("target machine {} not found", tid)))?;
        let target_local = format!("{}\\DerivedDataCache", loc.abs_path.trim_end_matches('\\'));
        out.push(DistributePlanItem {
            target_machine_id: *tid,
            target_host: target_machine.ip,
            source_unc: source_unc.clone(),
            target_local,
            credential_user: credential_user.clone(),
            credential_pass: credential_pass.clone(),
            source_smb_user: source_smb_user.clone(),
            source_smb_pass: source_smb_pass.clone(),
        });
    }
    Ok(out)
}

fn build_distribute_args(item: &DistributePlanItem, preflight: bool) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "-HostName".into(),
        item.target_host.clone(),
        "-SourceUnc".into(),
        item.source_unc.clone(),
        "-TargetLocal".into(),
        item.target_local.clone(),
    ];
    if let (Some(u), Some(p)) = (item.credential_user.as_deref(), item.credential_pass.as_deref()) {
        args.push("-Username".into());
        args.push(u.into());
        args.push("-Password".into());
        args.push(p.into());
    }
    if let (Some(u), Some(p)) = (item.source_smb_user.as_deref(), item.source_smb_pass.as_deref()) {
        args.push("-SourceSmbUser".into());
        args.push(u.into());
        args.push("-SourceSmbPass".into());
        args.push(p.into());
    }
    if preflight {
        args.push("-PreflightOnly".into());
    }
    args
}

/// Probe target → source UNC reachability without doing the actual robocopy.
/// Returns Ok(()) if the source mapping succeeds inside the target session.
pub async fn preflight_one(item: &DistributePlanItem) -> UecmResult<()> {
    let args = build_distribute_args(item, true);
    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let r: DistributeRaw = powershell::run_json(
        &powershell::script_path("distribute-pak-file.ps1"),
        &args_ref,
    )?;
    if !r.ok {
        return Err(UecmError::OperationFailed(format!(
            "preflight failed for target {}: {}",
            item.target_machine_id,
            r.message.unwrap_or_else(|| r.stdout_tail.clone())
        )));
    }
    Ok(())
}

pub async fn run_one(item: DistributePlanItem) -> UecmResult<DistributeOutcome> {
    let args = build_distribute_args(&item, false);
    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let r: DistributeRaw = powershell::run_json(
        &powershell::script_path("distribute-pak-file.ps1"),
        &args_ref,
    )?;
    let exit: i32 = r.exit_code.parse().unwrap_or(-1);
    let bytes: i64 = r.bytes_copied.parse().unwrap_or(0);
    Ok(DistributeOutcome {
        target_machine_id: item.target_machine_id,
        ok: r.ok,
        exit_code: exit,
        bytes_copied: bytes,
        stdout_tail: r.stdout_tail,
        message: r.message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_rejects_empty_targets() {
        let db = crate::data::open_in_memory().unwrap();
        let loc = ProjectLocation {
            id: Some(1), project_id: 1, machine_id: 1,
            abs_path: "D:\\X".into(), uproject_path: "D:\\X\\X.uproject".into(),
            discovery_status: crate::data::DiscoveryStatus::Auto, discovered_at: None,
        };
        let r = plan(&db, 1, "h", &loc, &[], 1, None, None, None, None, None);
        assert!(matches!(r, Err(UecmError::InvalidInput(_))));
    }

    #[test]
    fn plan_skips_source_in_targets() {
        let db = crate::data::open_in_memory().unwrap();
        // seed a target machine + location
        {
            let conn = db.lock().unwrap();
            conn.execute("INSERT INTO machines (hostname, ip) VALUES ('s', '1.1.1.1')", []).unwrap();
            conn.execute("INSERT INTO machines (hostname, ip) VALUES ('t', '2.2.2.2')", []).unwrap();
            conn.execute(
                "INSERT INTO projects (uproject_name, uproject_stem_lower) VALUES ('X.uproject', 'x')",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO project_locations (project_id, machine_id, abs_path, uproject_path) \
                 VALUES (1, 2, 'E:\\Y', 'E:\\Y\\X.uproject')",
                [],
            ).unwrap();
        }
        let source_loc = ProjectLocation {
            id: Some(0), project_id: 1, machine_id: 1,
            abs_path: "D:\\X".into(), uproject_path: "D:\\X\\X.uproject".into(),
            discovery_status: crate::data::DiscoveryStatus::Auto, discovered_at: None,
        };
        let plan_items = plan(&db, 1, "1.1.1.1", &source_loc, &[1, 2], 1, None, None, None, None, None).unwrap();
        assert_eq!(plan_items.len(), 1, "source should be skipped");
        assert_eq!(plan_items[0].target_machine_id, 2);
        assert_eq!(plan_items[0].source_unc, "\\\\1.1.1.1\\D$\\X\\DerivedDataCache");
        assert_eq!(plan_items[0].target_local, "E:\\Y\\DerivedDataCache");
    }

    #[test]
    fn plan_uses_named_share_when_provided() {
        let db = crate::data::open_in_memory().unwrap();
        {
            let conn = db.lock().unwrap();
            conn.execute("INSERT INTO machines (hostname, ip) VALUES ('s', '1.1.1.1')", []).unwrap();
            conn.execute("INSERT INTO machines (hostname, ip) VALUES ('t', '2.2.2.2')", []).unwrap();
            conn.execute("INSERT INTO projects (uproject_name, uproject_stem_lower) VALUES ('X.uproject', 'x')", []).unwrap();
            conn.execute(
                "INSERT INTO project_locations (project_id, machine_id, abs_path, uproject_path) \
                 VALUES (1, 2, 'E:\\Y', 'E:\\Y\\X.uproject')",
                [],
            ).unwrap();
        }
        let source_loc = ProjectLocation {
            id: Some(0), project_id: 1, machine_id: 1,
            abs_path: "D:\\X".into(), uproject_path: "D:\\X\\X.uproject".into(),
            discovery_status: crate::data::DiscoveryStatus::Auto, discovered_at: None,
        };
        let plan_items = plan(&db, 1, "1.1.1.1", &source_loc, &[2], 1, Some("\\\\HOST\\DDC"), None, None, None, None).unwrap();
        assert_eq!(plan_items[0].source_unc, "\\\\HOST\\DDC\\DerivedDataCache");
    }
}
```

- [ ] **Step 2: Wire into `core/mod.rs`**

```rust
pub mod pak_distribute;
```

- [ ] **Step 3: Test + commit**

```bash
cd src-tauri && cargo test --lib core::pak_distribute 2>&1 | tail -10 && cd ..
git add src-tauri/src/core/pak_distribute.rs src-tauri/src/core/mod.rs
git commit -m "feat: core::pak_distribute Robocopy fan-out planning + run_one"
```

Expected: 3 tests pass.

---

## Task 15: Tauri command — `distribute_ddc_pak` (with batch fan-out)

**Files:**
- Modify: `src-tauri/src/commands/ddc_pak.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Append the distribute command to `commands::ddc_pak`**

```rust
use crate::core::batch;
use crate::core::pak_distribute::{self, DistributeOutcome, DistributePlanItem};
use std::sync::Arc;

#[derive(Debug, Serialize)]
pub struct DistributeJobResponse {
    pub job_id: String,
    pub project_id: i64,
    pub source_machine_id: i64,
    pub plan: Vec<pak_distribute::DistributePlanItem>,
}

#[tauri::command]
pub async fn distribute_ddc_pak(
    app: tauri::AppHandle,
    db: State<'_, Db>,
    source_machine_id: i64,
    project_id: i64,
    target_machine_ids: Vec<i64>,
    named_share_unc: Option<String>,
    operator_credential_alias: Option<String>,
    /// Optional separate SMB credential for the source UNC. When `None`,
    /// reuses the operator credential (works in single-domain or workgroup
    /// setups where the same account has WinRM admin + SMB read on the source).
    source_smb_credential_alias: Option<String>,
) -> UecmResult<DistributeJobResponse> {
    let source_machine = data_machines::get(&db, source_machine_id)?
        .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", source_machine_id)))?;
    let source_location = project_locations::get_for_project_machine(&db, project_id, source_machine_id)?
        .ok_or_else(|| {
            UecmError::InvalidInput(format!(
                "project {} not located on machine {}",
                project_id, source_machine_id
            ))
        })?;
    let (op_user, op_pass) = resolve_operator_creds(&db, operator_credential_alias.as_deref())?;
    // Pick the SMB credential explicitly if one was supplied; otherwise reuse operator.
    let (smb_user, smb_pass) = if source_smb_credential_alias.is_some() {
        resolve_operator_creds(&db, source_smb_credential_alias.as_deref())?
    } else {
        (op_user.clone(), op_pass.clone())
    };

    let plan = pak_distribute::plan(
        &db,
        source_machine_id,
        &source_machine.ip,
        &source_location,
        &target_machine_ids,
        project_id,
        named_share_unc.as_deref(),
        op_user.clone(),
        op_pass.clone(),
        smb_user,
        smb_pass,
    )?;

    // Preflight every target before kicking off the actual robocopy fan-out.
    // If any target cannot reach the source UNC, fail fast with a structured
    // error rather than letting the operator watch N robocopy 'access denied' lines.
    for item in &plan {
        pak_distribute::preflight_one(item).await.map_err(|e| {
            UecmError::OperationFailed(format!(
                "target {} cannot reach source UNC: {}",
                item.target_machine_id, e
            ))
        })?;
    }

    let job_id = format!("ddc-pak-dist-{}-{}", source_machine_id, chrono::Utc::now().timestamp_millis());
    let plan_clone = plan.clone();
    let job_id_clone = job_id.clone();
    let pid = project_id;
    let smid = source_machine_id;
    let app_clone = app.clone();

    tokio::spawn(async move {
        let plan_arc = Arc::new(plan_clone);
        let machine_ids: Vec<i64> = plan_arc.iter().map(|i| i.target_machine_id).collect();
        let plan_for_op = plan_arc.clone();
        let mut rx = batch::run_batch(machine_ids.clone(), batch::DEFAULT_MAX_CONCURRENCY, move |mid| {
            let plan_local = plan_for_op.clone();
            async move {
                let item = plan_local
                    .iter()
                    .find(|i| i.target_machine_id == mid)
                    .ok_or_else(|| UecmError::InvalidInput(format!("plan missing for {}", mid)))?
                    .clone();
                let outcome = pak_distribute::run_one(item).await?;
                if !outcome.ok {
                    return Err(UecmError::OperationFailed(format!(
                        "robocopy exit {}: {}",
                        outcome.exit_code,
                        outcome.message.unwrap_or_else(|| outcome.stdout_tail.clone())
                    )));
                }
                Ok::<_, UecmError>(outcome)
            }
        })
        .await;

        while let Some(ev) = rx.recv().await {
            #[derive(Serialize)]
            struct Payload<'a> {
                job_id: &'a str,
                project_id: i64,
                source_machine_id: i64,
                event: &'a batch::BatchEvent,
            }
            let _ = app_clone.emit(
                "pak-distribute-progress",
                Payload {
                    job_id: &job_id_clone,
                    project_id: pid,
                    source_machine_id: smid,
                    event: &ev,
                },
            );
        }
    });

    Ok(DistributeJobResponse {
        job_id,
        project_id,
        source_machine_id,
        plan,
    })
}
```

- [ ] **Step 2: Register in `lib.rs`**

```rust
            commands::ddc_pak::distribute_ddc_pak,
```

- [ ] **Step 3: Build to verify**

```bash
cd src-tauri && cargo build --lib 2>&1 | tail -10 && cd ..
```

Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/ddc_pak.rs src-tauri/src/lib.rs
git commit -m "feat: tauri command distribute_ddc_pak with batch fan-out"
```

---

## Task 16: Frontend service + types — `src/services/tauri.ts`

**Files:**
- Modify: `src/services/tauri.ts`

- [ ] **Step 1: Add type definitions**

In `src/services/tauri.ts`, after the existing types (after `EchoResult`), insert:

```typescript
// Projects
export type DiscoveryStatus = "auto" | "manual_alias" | "manual_path";

export interface ProjectSummary {
  id: number;
  uproject_name: string;
  display_name: string | null;
  uproject_guid: string | null;
  location_count: number;
}

export interface ProjectLocation {
  id: number | null;
  project_id: number;
  machine_id: number;
  abs_path: string;
  uproject_path: string;
  discovery_status: DiscoveryStatus;
  discovered_at: string | null;
}

export interface DiscoveryResult {
  project_id: number;
  location_id: number;
  uproject_filename: string;
  abs_path: string;
}

// UE Runner events
export type UeRunnerEventKind =
  | "spawned"
  | "log_line"
  | "progress"
  | "completed"
  | "cancelled"
  | "error";

export interface UeRunnerEvent {
  kind: UeRunnerEventKind;
  pid?: number;
  log_path?: string;
  text?: string;
  parsed_kind?: string | null;
  pct?: number | null;
  label?: string;
  exit_code?: number;
  log_tail?: string[];
  message?: string;
}

export interface UeRunnerProgressPayload {
  job_id: string;
  source_machine_id: number;
  project_id: number;
  event: UeRunnerEvent;
}

// DDC pak generate
export type BackendChoice = "remote" | "local";

export interface GenerateJobResponse {
  job_id: string;
  source_machine_id: number;
  project_id: number;
  backend: string;
}

export interface PakOutput {
  path: string;
  size_bytes: number;
}

/// Emitted on the "pak-verified" channel after every Completed UE run.
/// Distribute auto-chain MUST gate off this event (verified=true), never off
/// the raw UeRunnerEvent.Completed — otherwise robocopy may run before the
/// .ddp has flushed to disk or when the UE run failed but exited 0.
export interface PakVerifiedPayload {
  job_id: string;
  project_id: number;
  verified: boolean;
  output: PakOutput | null;
}

// DDC pak distribute
export interface DistributePlanItem {
  target_machine_id: number;
  target_host: string;
  source_unc: string;
  target_local: string;
  credential_user: string | null;
  credential_pass: string | null;
  source_smb_user: string | null;
  source_smb_pass: string | null;
}

export interface DistributeJobResponse {
  job_id: string;
  project_id: number;
  source_machine_id: number;
  plan: DistributePlanItem[];
}

export interface PakDistributeProgressPayload {
  job_id: string;
  project_id: number;
  source_machine_id: number;
  event: BatchEvent;
}
```

- [ ] **Step 2: Add API methods to `tauriApi`**

Inside the `tauriApi = { ... }` object (before the closing `};`), append:

```typescript
  // Projects
  async listProjects(): Promise<ProjectSummary[]> {
    return invoke<ProjectSummary[]>("list_projects");
  },
  async listProjectLocations(projectId: number): Promise<ProjectLocation[]> {
    return invoke<ProjectLocation[]>("list_project_locations", { projectId });
  },
  async discoverProjects(
    machineId: number,
    searchRoots: string[],
    operatorCredentialAlias: string | null,
  ): Promise<DiscoveryResult[]> {
    return invoke<DiscoveryResult[]>("discover_projects", {
      machineId,
      searchRoots,
      operatorCredentialAlias,
    });
  },
  async setProjectLocation(
    projectId: number,
    machineId: number,
    absPath: string,
    uprojectPath: string,
    manual: boolean,
  ): Promise<number> {
    return invoke<number>("set_project_location", {
      projectId,
      machineId,
      absPath,
      uprojectPath,
      manual,
    });
  },
  async deleteProject(projectId: number): Promise<void> {
    return invoke<void>("delete_project", { projectId });
  },
  async deleteProjectLocation(locationId: number): Promise<void> {
    return invoke<void>("delete_project_location", { locationId });
  },
  async createProjectManual(
    uprojectName: string,
    displayName: string | null,
  ): Promise<number> {
    return invoke<number>("create_project_manual", { uprojectName, displayName });
  },

  // DDC Pak
  async generateDdcPak(args: {
    backend: BackendChoice;
    sourceMachineId: number | null;          // null when backend === "local"
    projectId: number;
    localUprojectPath: string | null;        // required when backend === "local"
    localEnginePath: string | null;          // optional; falls back to operator-config.json
    ueVersion: string | null;
    operatorCredentialAlias: string | null;
  }): Promise<GenerateJobResponse> {
    return invoke<GenerateJobResponse>("generate_ddc_pak", {
      backend: args.backend,
      sourceMachineId: args.sourceMachineId,
      projectId: args.projectId,
      localUprojectPath: args.localUprojectPath,
      localEnginePath: args.localEnginePath,
      ueVersion: args.ueVersion,
      operatorCredentialAlias: args.operatorCredentialAlias,
    });
  },
  async cancelUeJob(jobId: string): Promise<boolean> {
    return invoke<boolean>("cancel_ue_job", { jobId });
  },
  async verifyPakOutput(
    machineId: number,
    projectId: number,
    operatorCredentialAlias: string | null,
  ): Promise<PakOutput> {
    return invoke<PakOutput>("verify_pak_output", {
      machineId,
      projectId,
      operatorCredentialAlias,
    });
  },
  async distributeDdcPak(args: {
    sourceMachineId: number;
    projectId: number;
    targetMachineIds: number[];
    namedShareUnc: string | null;
    operatorCredentialAlias: string | null;
    sourceSmbCredentialAlias: string | null;
  }): Promise<DistributeJobResponse> {
    return invoke<DistributeJobResponse>("distribute_ddc_pak", {
      sourceMachineId: args.sourceMachineId,
      projectId: args.projectId,
      targetMachineIds: args.targetMachineIds,
      namedShareUnc: args.namedShareUnc,
      operatorCredentialAlias: args.operatorCredentialAlias,
      sourceSmbCredentialAlias: args.sourceSmbCredentialAlias,
    });
  },
```

- [ ] **Step 3: Verify type-check**

```bash
pnpm vue-tsc --noEmit 2>&1 | tail -10
```

Expected: clean. (If `pnpm vue-tsc` is not in package scripts, run `pnpm exec tsc -p tsconfig.app.json --noEmit`.)

- [ ] **Step 4: Commit**

```bash
git add src/services/tauri.ts
git commit -m "feat(frontend): tauri service additions for projects + ddc-pak"
```

---

## Task 17: Frontend stores — `useProjectsStore` + `useDdcPakStore`

**Files:**
- Create: `src/stores/projects.ts`
- Create: `src/stores/ddcPak.ts`
- Create: `src/__tests__/projects-store.spec.ts`
- Create: `src/__tests__/ddc-pak-store.spec.ts`

- [ ] **Step 1: Write `useProjectsStore`**

```typescript
import { defineStore } from "pinia";
import { computed, ref } from "vue";
import {
  tauriApi,
  type DiscoveryResult,
  type ProjectLocation,
  type ProjectSummary,
  type UecmError,
} from "@/services/tauri";

export const useProjectsStore = defineStore("projects", () => {
  const projects = ref<ProjectSummary[]>([]);
  const locations = ref<Record<number, ProjectLocation[]>>({});
  const isLoading = ref(false);
  const error = ref<UecmError | null>(null);

  async function load() {
    isLoading.value = true;
    error.value = null;
    try {
      projects.value = await tauriApi.listProjects();
    } catch (e) {
      error.value = e as UecmError;
    } finally {
      isLoading.value = false;
    }
  }

  async function loadLocations(projectId: number) {
    error.value = null;
    try {
      const locs = await tauriApi.listProjectLocations(projectId);
      locations.value = { ...locations.value, [projectId]: locs };
    } catch (e) {
      error.value = e as UecmError;
    }
  }

  async function discover(
    machineId: number,
    searchRoots: string[],
    credAlias: string | null,
  ): Promise<DiscoveryResult[]> {
    error.value = null;
    const r = await tauriApi.discoverProjects(machineId, searchRoots, credAlias);
    await load();
    return r;
  }

  async function setLocation(
    projectId: number,
    machineId: number,
    absPath: string,
    uprojectPath: string,
    manual: boolean,
  ) {
    error.value = null;
    await tauriApi.setProjectLocation(projectId, machineId, absPath, uprojectPath, manual);
    await loadLocations(projectId);
    await load();
  }

  async function removeProject(projectId: number) {
    error.value = null;
    await tauriApi.deleteProject(projectId);
    delete locations.value[projectId];
    await load();
  }

  async function removeLocation(projectId: number, locationId: number) {
    error.value = null;
    await tauriApi.deleteProjectLocation(locationId);
    await loadLocations(projectId);
    await load();
  }

  async function createManual(uprojectName: string, displayName: string | null) {
    error.value = null;
    const id = await tauriApi.createProjectManual(uprojectName, displayName);
    await load();
    return id;
  }

  const projectsByName = computed(() => {
    const map = new Map<string, ProjectSummary>();
    for (const p of projects.value) map.set(p.uproject_name, p);
    return map;
  });

  return {
    projects,
    locations,
    isLoading,
    error,
    load,
    loadLocations,
    discover,
    setLocation,
    removeProject,
    removeLocation,
    createManual,
    projectsByName,
  };
});
```

- [ ] **Step 2: Write `useDdcPakStore`**

```typescript
import { defineStore } from "pinia";
import { ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  tauriApi,
  type BackendChoice,
  type BatchEvent,
  type DistributeJobResponse,
  type DistributePlanItem,
  type GenerateJobResponse,
  type PakDistributeProgressPayload,
  type PakOutput,
  type PakVerifiedPayload,
  type UecmError,
  type UeRunnerEvent,
  type UeRunnerProgressPayload,
} from "@/services/tauri";

/// Pending distribute settings attached to a generate job. When the generate
/// job's pak-verified event fires with verified=true, the store auto-fires
/// distribute. If verified=false, the targets stay attached and the operator
/// can retry manually.
export interface PendingDistribute {
  source_machine_id: number;
  target_machine_ids: number[];
  named_share_unc: string | null;
  operator_credential_alias: string | null;
  source_smb_credential_alias: string | null;
}

export interface GenerateJobState {
  job_id: string;
  source_machine_id: number;
  project_id: number;
  backend: BackendChoice;
  status:
    | "queued"
    | "spawning"
    | "running"
    | "verifying"
    | "completed"
    | "verify_failed"
    | "cancelled"
    | "error";
  log_lines: string[];
  progress_pct: number | null;
  progress_label: string | null;
  exit_code: number | null;
  error_message: string | null;
  output: PakOutput | null;
  pending_distribute: PendingDistribute | null;
  started_at: string;
  finished_at: string | null;
}

export interface DistributeTargetState {
  target_machine_id: number;
  target_host: string;
  status: "pending" | "running" | "ok" | "err";
  message: string | null;
}

export interface DistributeJobState {
  job_id: string;
  project_id: number;
  source_machine_id: number;
  status: "queued" | "running" | "completed";
  targets: DistributeTargetState[];
  started_at: string;
  finished_at: string | null;
}

export const useDdcPakStore = defineStore("ddcPak", () => {
  const generateJobs = ref<GenerateJobState[]>([]);
  const distributeJobs = ref<DistributeJobState[]>([]);
  const error = ref<UecmError | null>(null);

  let unlistenGen: UnlistenFn | null = null;
  let unlistenVerified: UnlistenFn | null = null;
  let unlistenDist: UnlistenFn | null = null;

  async function attach() {
    if (unlistenGen) return;
    unlistenGen = await listen<UeRunnerProgressPayload>("ue-runner-progress", (e) => {
      onUeRunnerEvent(e.payload);
    });
    unlistenVerified = await listen<PakVerifiedPayload>("pak-verified", (e) => {
      onPakVerified(e.payload);
    });
    unlistenDist = await listen<PakDistributeProgressPayload>("pak-distribute-progress", (e) => {
      onDistributeEvent(e.payload);
    });
  }

  async function detach() {
    unlistenGen?.();
    unlistenVerified?.();
    unlistenDist?.();
    unlistenGen = null;
    unlistenVerified = null;
    unlistenDist = null;
  }

  function onUeRunnerEvent(p: UeRunnerProgressPayload) {
    const job = generateJobs.value.find((j) => j.job_id === p.job_id);
    if (!job) return;
    const ev = p.event;
    switch (ev.kind) {
      case "spawned":
        job.status = "running";
        break;
      case "log_line":
        if (ev.text) {
          job.log_lines.push(ev.text);
          if (job.log_lines.length > 200) job.log_lines.splice(0, job.log_lines.length - 200);
        }
        break;
      case "progress":
        if (ev.pct != null) job.progress_pct = ev.pct;
        if (ev.label) job.progress_label = ev.label;
        break;
      case "completed":
        // Don't flip to "completed" until pak-verified arrives — the runtime
        // emits Completed at UE process exit, but the .ddp may still be
        // flushing. Distribute auto-chain MUST gate off pak-verified.
        job.status = "verifying";
        job.exit_code = ev.exit_code ?? null;
        break;
      case "cancelled":
        job.status = "cancelled";
        job.finished_at = new Date().toISOString();
        // Drop any pending distribute — won't fire on cancel.
        job.pending_distribute = null;
        break;
      case "error":
        job.status = "error";
        job.error_message = ev.message ?? "unknown error";
        job.finished_at = new Date().toISOString();
        job.pending_distribute = null;
        break;
    }
  }

  async function onPakVerified(p: PakVerifiedPayload) {
    const job = generateJobs.value.find((j) => j.job_id === p.job_id);
    if (!job) return;
    if (p.verified) {
      job.status = "completed";
      job.output = p.output;
    } else {
      job.status = "verify_failed";
      job.error_message = "pak verification failed (.ddp missing or empty)";
      job.pending_distribute = null;
    }
    job.finished_at = new Date().toISOString();

    // Auto-chain distribute IF a pending plan was attached AND verify passed.
    if (p.verified && job.pending_distribute) {
      const pd = job.pending_distribute;
      job.pending_distribute = null;
      try {
        await tauriApi.distributeDdcPak({
          sourceMachineId: pd.source_machine_id,
          projectId: job.project_id,
          targetMachineIds: pd.target_machine_ids,
          namedShareUnc: pd.named_share_unc,
          operatorCredentialAlias: pd.operator_credential_alias,
          sourceSmbCredentialAlias: pd.source_smb_credential_alias,
        }).then((resp) => {
          distributeJobs.value.unshift({
            job_id: resp.job_id,
            project_id: resp.project_id,
            source_machine_id: resp.source_machine_id,
            status: "running",
            targets: resp.plan.map((pl) => ({
              target_machine_id: pl.target_machine_id,
              target_host: pl.target_host,
              status: "pending",
              message: null,
            })),
            started_at: new Date().toISOString(),
            finished_at: null,
          });
        });
      } catch (e) {
        error.value = e as UecmError;
      }
    }
  }

  function onDistributeEvent(p: PakDistributeProgressPayload) {
    const job = distributeJobs.value.find((j) => j.job_id === p.job_id);
    if (!job) return;
    const t = job.targets.find((x) => x.target_machine_id === p.event.machine_id);
    if (!t) return;
    if (p.event.status === "running") t.status = "running";
    else if (p.event.status === "ok") t.status = "ok";
    else if (p.event.status === "err") {
      t.status = "err";
      t.message = p.event.message ?? null;
    }
    if (job.targets.every((x) => x.status === "ok" || x.status === "err")) {
      job.status = "completed";
      job.finished_at = new Date().toISOString();
    }
  }

  async function startGenerate(args: {
    backend: BackendChoice;
    sourceMachineId: number | null;
    projectId: number;
    localUprojectPath: string | null;
    localEnginePath: string | null;
    ueVersion: string | null;
    operatorCredentialAlias: string | null;
    /// If supplied, distribute auto-chains after pak-verified=true. Distribute
    /// will NEVER fire while UE is still running or while verify is pending.
    pendingDistribute: PendingDistribute | null;
  }): Promise<GenerateJobResponse> {
    await attach();
    error.value = null;
    const resp = await tauriApi.generateDdcPak({
      backend: args.backend,
      sourceMachineId: args.sourceMachineId,
      projectId: args.projectId,
      localUprojectPath: args.localUprojectPath,
      localEnginePath: args.localEnginePath,
      ueVersion: args.ueVersion,
      operatorCredentialAlias: args.operatorCredentialAlias,
    });
    generateJobs.value.unshift({
      job_id: resp.job_id,
      source_machine_id: resp.source_machine_id,
      project_id: resp.project_id,
      backend: args.backend,
      status: "spawning",
      log_lines: [],
      progress_pct: null,
      progress_label: null,
      exit_code: null,
      error_message: null,
      output: null,
      pending_distribute: args.pendingDistribute,
      started_at: new Date().toISOString(),
      finished_at: null,
    });
    return resp;
  }

  async function cancelGenerate(jobId: string): Promise<boolean> {
    return tauriApi.cancelUeJob(jobId);
  }

  /// Manual distribute (no generate dependency). Operator must have already
  /// produced and verified a .ddp.
  async function startDistribute(args: {
    sourceMachineId: number;
    projectId: number;
    targetMachineIds: number[];
    namedShareUnc: string | null;
    operatorCredentialAlias: string | null;
    sourceSmbCredentialAlias: string | null;
  }): Promise<DistributeJobResponse> {
    await attach();
    error.value = null;
    const resp = await tauriApi.distributeDdcPak({
      sourceMachineId: args.sourceMachineId,
      projectId: args.projectId,
      targetMachineIds: args.targetMachineIds,
      namedShareUnc: args.namedShareUnc,
      operatorCredentialAlias: args.operatorCredentialAlias,
      sourceSmbCredentialAlias: args.sourceSmbCredentialAlias,
    });
    distributeJobs.value.unshift({
      job_id: resp.job_id,
      project_id: resp.project_id,
      source_machine_id: resp.source_machine_id,
      status: "running",
      targets: resp.plan.map((p) => ({
        target_machine_id: p.target_machine_id,
        target_host: p.target_host,
        status: "pending",
        message: null,
      })),
      started_at: new Date().toISOString(),
      finished_at: null,
    });
    return resp;
  }

  async function verifyOutput(
    machineId: number,
    projectId: number,
    credAlias: string | null,
  ): Promise<PakOutput> {
    return tauriApi.verifyPakOutput(machineId, projectId, credAlias);
  }

  return {
    generateJobs,
    distributeJobs,
    error,
    attach,
    detach,
    startGenerate,
    cancelGenerate,
    startDistribute,
    verifyOutput,
  };
});
```

- [ ] **Step 3: Write `projects-store.spec.ts`**

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    listProjects: vi.fn(),
    listProjectLocations: vi.fn(),
    discoverProjects: vi.fn(),
    setProjectLocation: vi.fn(),
    deleteProject: vi.fn(),
    deleteProjectLocation: vi.fn(),
    createProjectManual: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));

import { useProjectsStore } from "@/stores/projects";

describe("projects store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((m) => m.mockReset());
  });

  it("starts empty", () => {
    const s = useProjectsStore();
    expect(s.projects).toEqual([]);
  });

  it("load fills the list", async () => {
    mockApi.listProjects.mockResolvedValue([
      { id: 1, uproject_name: "X.uproject", display_name: null, uproject_guid: null, location_count: 2 },
    ]);
    const s = useProjectsStore();
    await s.load();
    expect(s.projects).toHaveLength(1);
  });

  it("loadLocations populates per-project map", async () => {
    mockApi.listProjectLocations.mockResolvedValue([
      { id: 1, project_id: 1, machine_id: 10, abs_path: "D:\\X", uproject_path: "D:\\X\\X.uproject", discovery_status: "auto", discovered_at: null },
    ]);
    const s = useProjectsStore();
    await s.loadLocations(1);
    expect(s.locations[1]).toHaveLength(1);
  });

  it("discover then load refreshes list", async () => {
    mockApi.discoverProjects.mockResolvedValue([
      { project_id: 1, location_id: 1, uproject_filename: "X.uproject", abs_path: "D:\\X" },
    ]);
    mockApi.listProjects.mockResolvedValue([
      { id: 1, uproject_name: "X.uproject", display_name: null, uproject_guid: null, location_count: 1 },
    ]);
    const s = useProjectsStore();
    const r = await s.discover(10, ["D:\\Work"], null);
    expect(r).toHaveLength(1);
    expect(s.projects).toHaveLength(1);
  });

  it("removeProject deletes then reloads", async () => {
    mockApi.deleteProject.mockResolvedValue(undefined);
    mockApi.listProjects.mockResolvedValue([]);
    const s = useProjectsStore();
    await s.removeProject(1);
    expect(mockApi.deleteProject).toHaveBeenCalledWith(1);
    expect(s.projects).toHaveLength(0);
  });
});
```

- [ ] **Step 4: Write `ddc-pak-store.spec.ts`**

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

const { mockApi, mockListen } = vi.hoisted(() => ({
  mockApi: {
    generateDdcPak: vi.fn(),
    cancelUeJob: vi.fn(),
    distributeDdcPak: vi.fn(),
    verifyPakOutput: vi.fn(),
  },
  mockListen: vi.fn().mockResolvedValue(() => undefined),
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: mockListen,
}));

import { useDdcPakStore } from "@/stores/ddcPak";

describe("ddc-pak store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((m) => m.mockReset());
    mockListen.mockClear();
  });

  it("startGenerate appends a spawning job", async () => {
    mockApi.generateDdcPak.mockResolvedValue({
      job_id: "job-1",
      source_machine_id: 10,
      project_id: 1,
      backend: "remote",
    });
    const s = useDdcPakStore();
    await s.startGenerate(10, 1, null, null);
    expect(s.generateJobs).toHaveLength(1);
    expect(s.generateJobs[0].status).toBe("spawning");
  });

  it("cancelGenerate calls api", async () => {
    mockApi.cancelUeJob.mockResolvedValue(true);
    const s = useDdcPakStore();
    const ok = await s.cancelGenerate("job-1");
    expect(ok).toBe(true);
    expect(mockApi.cancelUeJob).toHaveBeenCalledWith("job-1");
  });

  it("startDistribute initialises target rows", async () => {
    mockApi.distributeDdcPak.mockResolvedValue({
      job_id: "dist-1",
      project_id: 1,
      source_machine_id: 10,
      plan: [
        { target_machine_id: 11, target_host: "1.1.1.1", source_unc: "\\\\h\\D$\\X", target_local: "D:\\X", credential_user: null, credential_pass: null },
        { target_machine_id: 12, target_host: "2.2.2.2", source_unc: "\\\\h\\D$\\X", target_local: "D:\\X", credential_user: null, credential_pass: null },
      ],
    });
    const s = useDdcPakStore();
    await s.startDistribute(10, 1, [11, 12], null, null);
    expect(s.distributeJobs[0].targets).toHaveLength(2);
    expect(s.distributeJobs[0].targets[0].status).toBe("pending");
  });
});
```

- [ ] **Step 5: Run + commit**

```bash
pnpm test src/__tests__/projects-store.spec.ts src/__tests__/ddc-pak-store.spec.ts 2>&1 | tail -10
```

Expected: 8 tests pass.

```bash
git add src/stores/projects.ts src/stores/ddcPak.ts src/__tests__/projects-store.spec.ts src/__tests__/ddc-pak-store.spec.ts
git commit -m "feat(frontend): useProjectsStore + useDdcPakStore + tests"
```

---

## Task 18: New primitives — UecmProgressBar, UecmTaskCard, UecmPathInput

**Files:**
- Create: `src/components/primitives/UecmProgressBar.vue`
- Create: `src/components/primitives/UecmTaskCard.vue`
- Create: `src/components/primitives/UecmPathInput.vue`
- Modify: `src/components/primitives/index.ts`
- Create: `src/__tests__/UecmProgressBar.spec.ts`
- Create: `src/__tests__/UecmTaskCard.spec.ts`
- Create: `src/__tests__/UecmPathInput.spec.ts`

- [ ] **Step 1: `UecmProgressBar.vue`**

```vue
<script setup lang="ts">
import { computed } from "vue";

const props = defineProps<{
  pct?: number | null;       // 0..1
  indeterminate?: boolean;
  label?: string;
  variant?: "default" | "success" | "danger";
}>();

const widthStyle = computed(() => {
  if (props.indeterminate || props.pct == null) return "100%";
  const p = Math.max(0, Math.min(1, props.pct));
  return `${(p * 100).toFixed(1)}%`;
});

const variantClass = computed(() => {
  switch (props.variant) {
    case "success": return "bg-emerald-500";
    case "danger":  return "bg-rose-500";
    default:        return "bg-primary";
  }
});
</script>

<template>
  <div data-progress-bar class="w-full">
    <div v-if="label" class="mb-1 flex justify-between text-xs text-muted-foreground">
      <span>{{ label }}</span>
      <span v-if="!indeterminate && pct != null">{{ Math.round((pct ?? 0) * 100) }}%</span>
    </div>
    <div class="relative h-2 w-full overflow-hidden rounded-full bg-muted">
      <div
        :class="[variantClass, indeterminate ? 'animate-pulse' : '']"
        class="h-full rounded-full transition-all"
        :style="{ width: widthStyle }"
      />
    </div>
  </div>
</template>
```

- [ ] **Step 2: `UecmTaskCard.vue`**

```vue
<script setup lang="ts">
import { computed } from "vue";
import UecmProgressBar from "./UecmProgressBar.vue";

const props = defineProps<{
  title: string;
  subtitle?: string;
  status: "queued" | "spawning" | "running" | "completed" | "cancelled" | "error";
  progressPct?: number | null;
  progressLabel?: string;
  errorMessage?: string | null;
}>();
const emit = defineEmits<{ (e: "cancel"): void }>();

const statusColor = computed(() => {
  switch (props.status) {
    case "completed": return "text-emerald-500";
    case "cancelled": return "text-amber-500";
    case "error":     return "text-rose-500";
    default:          return "text-primary";
  }
});

const statusLabel = computed(() => props.status.toUpperCase());

const showProgress = computed(() =>
  props.status === "spawning" || props.status === "running",
);
const variant = computed(() => {
  if (props.status === "error") return "danger" as const;
  if (props.status === "completed") return "success" as const;
  return "default" as const;
});
const showCancel = computed(() => props.status === "running" || props.status === "spawning");
</script>

<template>
  <div data-task-card class="rounded-lg border bg-card p-4">
    <div class="flex items-start justify-between gap-3">
      <div>
        <h3 class="text-sm font-semibold">{{ title }}</h3>
        <p v-if="subtitle" class="text-xs text-muted-foreground">{{ subtitle }}</p>
      </div>
      <span :class="statusColor" class="text-xs font-mono uppercase tracking-wide" data-task-card-status>{{ statusLabel }}</span>
    </div>
    <div v-if="showProgress" class="mt-3">
      <UecmProgressBar
        :pct="progressPct"
        :indeterminate="progressPct == null"
        :label="progressLabel"
        :variant="variant"
      />
    </div>
    <p v-if="errorMessage" class="mt-3 rounded bg-rose-500/10 p-2 text-xs text-rose-600" data-task-card-error>{{ errorMessage }}</p>
    <div v-if="showCancel" class="mt-3 flex justify-end">
      <button
        data-task-card-cancel
        class="rounded border border-border px-3 py-1 text-xs hover:bg-muted"
        @click="emit('cancel')"
      >Cancel</button>
    </div>
  </div>
</template>
```

- [ ] **Step 3: `UecmPathInput.vue`**

```vue
<script setup lang="ts">
import { computed } from "vue";

const props = defineProps<{
  modelValue: string;
  placeholder?: string;
  required?: boolean;
}>();
const emit = defineEmits<{ (e: "update:modelValue", v: string): void }>();

const isProbablyValid = computed(() => {
  const v = props.modelValue.trim();
  if (!v) return !props.required;
  // Drive-rooted absolute Windows path (X:\...) or UNC (\\HOST\Share)
  return /^([A-Za-z]:[\\/][^<>:"|?*]*|\\\\[^\\]+\\[^\\]+.*)$/.test(v);
});
</script>

<template>
  <div data-path-input>
    <input
      type="text"
      :value="modelValue"
      :placeholder="placeholder ?? 'D:\\Path or \\\\HOST\\Share'"
      class="w-full rounded border bg-background px-3 py-2 font-mono text-sm"
      :class="isProbablyValid ? '' : 'border-rose-400'"
      @input="emit('update:modelValue', ($event.target as HTMLInputElement).value)"
    />
    <p v-if="!isProbablyValid && modelValue" data-path-input-warn class="mt-1 text-xs text-rose-500">
      Doesn't look like a Windows absolute path or UNC.
    </p>
  </div>
</template>
```

- [ ] **Step 4: Update `primitives/index.ts`**

```typescript
export { default as UecmProgressBar } from "./UecmProgressBar.vue";
export { default as UecmTaskCard } from "./UecmTaskCard.vue";
export { default as UecmPathInput } from "./UecmPathInput.vue";
```

(Append to existing exports — do not remove the Plan 4 ones.)

- [ ] **Step 5: Write component tests**

`src/__tests__/UecmProgressBar.spec.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import UecmProgressBar from "@/components/primitives/UecmProgressBar.vue";

describe("UecmProgressBar", () => {
  it("renders 50% width when pct=0.5", () => {
    const w = mount(UecmProgressBar, { props: { pct: 0.5 } });
    const bar = w.find("div.relative > div");
    expect((bar.element as HTMLElement).style.width).toBe("50.0%");
  });

  it("clamps to 0..1", () => {
    const w = mount(UecmProgressBar, { props: { pct: 2 } });
    const bar = w.find("div.relative > div");
    expect((bar.element as HTMLElement).style.width).toBe("100.0%");
  });

  it("indeterminate fills 100% with pulse", () => {
    const w = mount(UecmProgressBar, { props: { indeterminate: true } });
    const bar = w.find("div.relative > div");
    expect(bar.classes()).toContain("animate-pulse");
  });

  it("shows label and percent when both supplied", () => {
    const w = mount(UecmProgressBar, { props: { pct: 0.42, label: "Saving" } });
    expect(w.text()).toContain("Saving");
    expect(w.text()).toContain("42%");
  });
});
```

`src/__tests__/UecmTaskCard.spec.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import UecmTaskCard from "@/components/primitives/UecmTaskCard.vue";

describe("UecmTaskCard", () => {
  it("emits cancel on cancel click", async () => {
    const w = mount(UecmTaskCard, { props: { title: "Job", status: "running" } });
    await w.find('[data-task-card-cancel]').trigger("click");
    expect(w.emitted("cancel")).toBeTruthy();
  });

  it("hides cancel when completed", () => {
    const w = mount(UecmTaskCard, { props: { title: "Job", status: "completed" } });
    expect(w.find('[data-task-card-cancel]').exists()).toBe(false);
  });

  it("shows error message when error status", () => {
    const w = mount(UecmTaskCard, { props: { title: "Job", status: "error", errorMessage: "boom" } });
    expect(w.find('[data-task-card-error]').text()).toBe("boom");
  });
});
```

`src/__tests__/UecmPathInput.spec.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import UecmPathInput from "@/components/primitives/UecmPathInput.vue";

describe("UecmPathInput", () => {
  it("flags malformed input", () => {
    const w = mount(UecmPathInput, { props: { modelValue: "/unix/path" } });
    expect(w.find('[data-path-input-warn]').exists()).toBe(true);
  });

  it("accepts drive-rooted Windows path", () => {
    const w = mount(UecmPathInput, { props: { modelValue: "D:\\Work\\X" } });
    expect(w.find('[data-path-input-warn]').exists()).toBe(false);
  });

  it("accepts UNC path", () => {
    const w = mount(UecmPathInput, { props: { modelValue: "\\\\HOST\\Share\\Path" } });
    expect(w.find('[data-path-input-warn]').exists()).toBe(false);
  });

  it("emits update:modelValue on input", async () => {
    const w = mount(UecmPathInput, { props: { modelValue: "" } });
    await w.find("input").setValue("D:\\X");
    expect(w.emitted("update:modelValue")).toBeTruthy();
  });
});
```

- [ ] **Step 6: Run + commit**

```bash
pnpm test src/__tests__/UecmProgressBar.spec.ts src/__tests__/UecmTaskCard.spec.ts src/__tests__/UecmPathInput.spec.ts 2>&1 | tail -10
```

Expected: 11 tests pass.

```bash
git add src/components/primitives/UecmProgressBar.vue src/components/primitives/UecmTaskCard.vue src/components/primitives/UecmPathInput.vue src/components/primitives/index.ts src/__tests__/UecmProgressBar.spec.ts src/__tests__/UecmTaskCard.spec.ts src/__tests__/UecmPathInput.spec.ts
git commit -m "feat(frontend): UecmProgressBar + UecmTaskCard + UecmPathInput primitives"
```

---

## Task 19: ProjectDiscoveryWizard + ProjectMatchingModal + DdcPakWizard

**Files:**
- Create: `src/components/modals/ProjectDiscoveryWizard.vue`
- Create: `src/components/modals/ProjectMatchingModal.vue`
- Create: `src/components/modals/DdcPakWizard.vue`
- Create: `src/__tests__/ProjectDiscoveryWizard.spec.ts`
- Create: `src/__tests__/ProjectMatchingModal.spec.ts`
- Create: `src/__tests__/DdcPakWizard.spec.ts`

- [ ] **Step 1: `ProjectDiscoveryWizard.vue`**

```vue
<script setup lang="ts">
import { computed, ref, watch } from "vue";
import BaseModal from "./BaseModal.vue";
import { UecmPathInput } from "@/components/primitives";
import { useMachinesStore } from "@/stores/machines";
import { useProjectsStore } from "@/stores/projects";
import { useCredentialsStore } from "@/stores/credentials";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ (e: "close"): void }>();

const machines = useMachinesStore();
const projects = useProjectsStore();
const credentials = useCredentialsStore();

const machineId = ref<number | null>(null);
const rootsText = ref("D:\\Work\nE:\\Projects");
const credAlias = ref<string | null>(null);
const isRunning = ref(false);
const found = ref<{ uproject_filename: string; abs_path: string }[]>([]);
const errMsg = ref<string | null>(null);

watch(() => props.open, (v) => {
  if (v) {
    machineId.value = null;
    rootsText.value = "D:\\Work\nE:\\Projects";
    credAlias.value = null;
    found.value = [];
    errMsg.value = null;
    machines.loadMachines();
    credentials.load();
  }
});

const winrmCreds = computed(() =>
  credentials.credentials.filter((c) => c.kind === "winrm"),
);
const canRun = computed(
  () => machineId.value != null && rootsText.value.trim().length > 0 && !isRunning.value,
);

async function run() {
  if (machineId.value == null) return;
  isRunning.value = true;
  errMsg.value = null;
  found.value = [];
  try {
    const roots = rootsText.value.split("\n").map((s) => s.trim()).filter(Boolean);
    const r = await projects.discover(machineId.value, roots, credAlias.value);
    found.value = r.map((x) => ({ uproject_filename: x.uproject_filename, abs_path: x.abs_path }));
  } catch (e) {
    errMsg.value = (e as { message?: string }).message ?? "discovery failed";
  } finally {
    isRunning.value = false;
  }
}
</script>

<template>
  <BaseModal :open="open" title="Discover Projects" @close="emit('close')">
    <div class="space-y-4 p-1">
      <div>
        <label class="text-xs uppercase tracking-wide text-muted-foreground">Machine</label>
        <select v-model="machineId" data-discovery-machine-select class="mt-1 w-full rounded border bg-background px-3 py-2 text-sm">
          <option :value="null">Select…</option>
          <option v-for="m in machines.machines" :key="m.id ?? m.ip" :value="m.id">
            {{ m.hostname }} ({{ m.ip }})
          </option>
        </select>
      </div>
      <div>
        <label class="text-xs uppercase tracking-wide text-muted-foreground">Search Roots (one per line)</label>
        <textarea
          v-model="rootsText"
          data-discovery-roots
          rows="4"
          class="mt-1 w-full rounded border bg-background px-3 py-2 font-mono text-sm"
        />
      </div>
      <div>
        <label class="text-xs uppercase tracking-wide text-muted-foreground">Credential</label>
        <select v-model="credAlias" data-discovery-cred-select class="mt-1 w-full rounded border bg-background px-3 py-2 text-sm">
          <option :value="null">(current process token)</option>
          <option v-for="c in winrmCreds" :key="c.alias" :value="c.alias">{{ c.alias }} — {{ c.username }}</option>
        </select>
      </div>
      <p v-if="errMsg" class="rounded bg-rose-500/10 p-2 text-xs text-rose-600">{{ errMsg }}</p>
      <button
        data-discovery-run
        class="w-full rounded bg-primary px-3 py-2 text-sm text-primary-foreground disabled:opacity-50"
        :disabled="!canRun"
        @click="run"
      >{{ isRunning ? "Discovering…" : "Run discovery" }}</button>
      <div v-if="found.length > 0" data-discovery-results>
        <p class="text-xs text-muted-foreground mb-1">Found {{ found.length }} .uproject file(s):</p>
        <ul class="max-h-48 overflow-y-auto rounded border bg-muted/40 p-2 text-xs font-mono">
          <li v-for="(f, i) in found" :key="i">{{ f.uproject_filename }} — {{ f.abs_path }}</li>
        </ul>
      </div>
    </div>
  </BaseModal>
</template>
```

- [ ] **Step 2: `ProjectMatchingModal.vue`** — minimal v1; lets the operator type a path on a chosen machine for a chosen project.

```vue
<script setup lang="ts">
import { computed, ref, watch } from "vue";
import BaseModal from "./BaseModal.vue";
import { UecmPathInput } from "@/components/primitives";
import { useMachinesStore } from "@/stores/machines";
import { useProjectsStore } from "@/stores/projects";

const props = defineProps<{ open: boolean; projectId: number | null }>();
const emit = defineEmits<{ (e: "close"): void }>();

const machines = useMachinesStore();
const projects = useProjectsStore();

const machineId = ref<number | null>(null);
const absPath = ref("");
const uprojectPath = ref("");
const errMsg = ref<string | null>(null);
const isSubmitting = ref(false);

watch(() => props.open, (v) => {
  if (v) {
    machineId.value = null;
    absPath.value = "";
    uprojectPath.value = "";
    errMsg.value = null;
    machines.loadMachines();
  }
});

watch(absPath, (v) => {
  if (!props.projectId) return;
  const p = projects.projects.find((x) => x.id === props.projectId);
  if (p && v && !uprojectPath.value) {
    uprojectPath.value = `${v.replace(/\\$/, "")}\\${p.uproject_name}`;
  }
});

const canSubmit = computed(
  () => machineId.value != null && absPath.value.length > 0 && uprojectPath.value.length > 0 && !isSubmitting.value,
);

async function submit() {
  if (props.projectId == null || machineId.value == null) return;
  isSubmitting.value = true;
  errMsg.value = null;
  try {
    await projects.setLocation(
      props.projectId,
      machineId.value,
      absPath.value,
      uprojectPath.value,
      true,
    );
    emit("close");
  } catch (e) {
    errMsg.value = (e as { message?: string }).message ?? "failed";
  } finally {
    isSubmitting.value = false;
  }
}
</script>

<template>
  <BaseModal :open="open" title="Map project to machine path" @close="emit('close')">
    <div class="space-y-3 p-1">
      <div>
        <label class="text-xs uppercase tracking-wide text-muted-foreground">Machine</label>
        <select v-model="machineId" data-matching-machine-select class="mt-1 w-full rounded border bg-background px-3 py-2 text-sm">
          <option :value="null">Select…</option>
          <option v-for="m in machines.machines" :key="m.id ?? m.ip" :value="m.id">{{ m.hostname }}</option>
        </select>
      </div>
      <div>
        <label class="text-xs uppercase tracking-wide text-muted-foreground">Project root path</label>
        <UecmPathInput v-model="absPath" data-matching-abs />
      </div>
      <div>
        <label class="text-xs uppercase tracking-wide text-muted-foreground">.uproject path</label>
        <UecmPathInput v-model="uprojectPath" data-matching-uproject />
      </div>
      <p v-if="errMsg" class="rounded bg-rose-500/10 p-2 text-xs text-rose-600">{{ errMsg }}</p>
      <button
        data-matching-submit
        class="w-full rounded bg-primary px-3 py-2 text-sm text-primary-foreground disabled:opacity-50"
        :disabled="!canSubmit"
        @click="submit"
      >Save mapping</button>
    </div>
  </BaseModal>
</template>
```

- [ ] **Step 3: `DdcPakWizard.vue`** — three-combo wizard.

```vue
<script setup lang="ts">
import { computed, ref, watch } from "vue";
import BaseModal from "./BaseModal.vue";
import { useMachinesStore } from "@/stores/machines";
import { useProjectsStore } from "@/stores/projects";
import { useCredentialsStore } from "@/stores/credentials";
import { useDdcPakStore } from "@/stores/ddcPak";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ (e: "close"): void }>();

const machines = useMachinesStore();
const projects = useProjectsStore();
const credentials = useCredentialsStore();
const ddcPak = useDdcPakStore();

type SourceMode = "self" | "remote";

const step = ref<1 | 2 | 3 | 4>(1);
const sourceMode = ref<SourceMode>("remote");
const sourceMachineId = ref<number | null>(null);
const localUprojectPath = ref<string>("");      // shown only when sourceMode === "self"
const localEnginePath = ref<string>("");        // optional override
const projectId = ref<number | null>(null);
const distribute = ref(true);
const targetIds = ref<number[]>([]);
const namedShare = ref<string>("");
const credAlias = ref<string | null>(null);
const sourceSmbCredAlias = ref<string | null>(null);
const errMsg = ref<string | null>(null);
const isSubmitting = ref(false);

watch(() => props.open, (v) => {
  if (v) {
    step.value = 1;
    sourceMode.value = "remote";
    sourceMachineId.value = null;
    localUprojectPath.value = "";
    localEnginePath.value = "";
    projectId.value = null;
    distribute.value = true;
    targetIds.value = [];
    namedShare.value = "";
    credAlias.value = null;
    sourceSmbCredAlias.value = null;
    errMsg.value = null;
    machines.loadMachines();
    projects.load();
    credentials.load();
  }
});

const winrmCreds = computed(() =>
  credentials.credentials.filter((c) => c.kind === "winrm"),
);

const projectsWithLocation = computed(() => {
  if (sourceMachineId.value == null) return projects.projects;
  // we only have count, not per-machine list; assume all projects are candidate
  return projects.projects;
});

const targetCandidates = computed(() => {
  return machines.machines.filter((m) => m.id != null && m.id !== sourceMachineId.value);
});

const canAdvance = computed(() => {
  switch (step.value) {
    case 1:
      if (sourceMode.value === "self") {
        // Self mode demands an operator-side .uproject path. Engine path is
        // optional (falls back to operator-config.json on the backend).
        return localUprojectPath.value.trim().length > 0;
      }
      return sourceMachineId.value != null;
    case 2:
      return projectId.value != null;
    case 3:
      // Self-mode distribute needs ≥1 target; remote mode also needs ≥1
      // when distribute is checked.
      return !distribute.value || targetIds.value.length > 0;
    case 4:
      return !isSubmitting.value;
  }
  return false;
});

const previewLines = computed(() => {
  const sourceLabel = sourceMode.value === "self"
    ? "(operator machine — local)"
    : machines.machines.find((m) => m.id === sourceMachineId.value)?.hostname ?? "?";
  const projLabel = projects.projects.find((p) => p.id === projectId.value)?.uproject_name ?? "?";
  const targets = targetIds.value
    .map((id) => machines.machines.find((m) => m.id === id)?.hostname ?? `${id}`)
    .join(", ");
  return [
    `Source: ${sourceLabel}`,
    `Project: ${projLabel}`,
    distribute.value ? `Distribute to: ${targets || "(none selected)"}` : "Distribute: no",
    namedShare.value ? `Named share UNC: ${namedShare.value}` : "Source UNC: admin share (\\\\<host>\\<drive>$\\…)",
    credAlias.value ? `Credential: ${credAlias.value}` : "Credential: (process token)",
  ];
});

async function run() {
  if (projectId.value == null) return;
  isSubmitting.value = true;
  errMsg.value = null;
  try {
    // Build the pending-distribute plan once (not invoked here — the store
    // auto-fires distribute after pak-verified). For self-mode there is no
    // source_machine_id row, so distribute would have to use a named share.
    // Keep the wizard honest: forbid self+distribute unless a named share
    // is supplied (distribute targets cannot \\<operator-localhost>\C$\… by
    // default — admin shares require visibility, which the operator may not
    // have configured). This is a v1 trade-off, not a hard restriction.
    if (sourceMode.value === "self" && distribute.value && !namedShare.value.trim()) {
      throw new Error("Self-mode distribute requires a named share UNC (operator machine has no admin-share contract).");
    }

    const pending = (distribute.value && targetIds.value.length > 0 && sourceMachineId.value != null)
      ? {
          source_machine_id: sourceMachineId.value,
          target_machine_ids: targetIds.value.slice(),
          named_share_unc: namedShare.value || null,
          operator_credential_alias: credAlias.value,
          source_smb_credential_alias: sourceSmbCredAlias.value,
        }
      : null;

    await ddcPak.startGenerate({
      backend: sourceMode.value === "self" ? "local" : "remote",
      sourceMachineId: sourceMode.value === "self" ? null : sourceMachineId.value,
      projectId: projectId.value,
      localUprojectPath: sourceMode.value === "self" ? localUprojectPath.value : null,
      localEnginePath: sourceMode.value === "self" && localEnginePath.value.trim()
        ? localEnginePath.value
        : null,
      ueVersion: null,
      operatorCredentialAlias: credAlias.value,
      pendingDistribute: pending,
    });
    emit("close");
  } catch (e) {
    errMsg.value = (e as { message?: string }).message ?? "submit failed";
  } finally {
    isSubmitting.value = false;
  }
}
</script>

<template>
  <BaseModal :open="open" title="Generate DDC Pak" @close="emit('close')">
    <div class="space-y-4 p-1" data-ddc-pak-wizard>
      <div class="flex gap-2 text-xs">
        <span :class="step >= 1 ? 'text-primary' : 'text-muted-foreground'">1. Source</span>
        <span>›</span>
        <span :class="step >= 2 ? 'text-primary' : 'text-muted-foreground'">2. Project</span>
        <span>›</span>
        <span :class="step >= 3 ? 'text-primary' : 'text-muted-foreground'">3. Targets</span>
        <span>›</span>
        <span :class="step >= 4 ? 'text-primary' : 'text-muted-foreground'">4. Review</span>
      </div>

      <div v-if="step === 1" class="space-y-3">
        <label class="flex items-center gap-2 text-sm">
          <input type="radio" value="remote" v-model="sourceMode" /> Remote dev box (recommended for cluster)
        </label>
        <label class="flex items-center gap-2 text-sm">
          <input type="radio" value="self" v-model="sourceMode" /> Self (operator machine)
        </label>
        <div v-if="sourceMode === 'remote'">
          <label class="text-xs uppercase tracking-wide text-muted-foreground">Source machine</label>
          <select v-model="sourceMachineId" data-wizard-source-select class="mt-1 w-full rounded border bg-background px-3 py-2 text-sm">
            <option :value="null">Select…</option>
            <option v-for="m in machines.machines" :key="m.id ?? m.ip" :value="m.id">{{ m.hostname }}</option>
          </select>
        </div>
        <div v-else class="space-y-2">
          <label class="text-xs uppercase tracking-wide text-muted-foreground">.uproject path on this machine</label>
          <input
            v-model="localUprojectPath"
            data-wizard-local-uproject
            type="text"
            placeholder="C:\\Work\\MyProj\\MyProj.uproject"
            class="w-full rounded border bg-background px-3 py-2 font-mono text-sm"
          />
          <label class="text-xs uppercase tracking-wide text-muted-foreground">Engine path (optional — falls back to operator-config.json)</label>
          <input
            v-model="localEnginePath"
            data-wizard-local-engine
            type="text"
            placeholder="C:\\UnrealEngine\\UE_5.4"
            class="w-full rounded border bg-background px-3 py-2 font-mono text-sm"
          />
        </div>
      </div>

      <div v-else-if="step === 2" class="space-y-3">
        <label class="text-xs uppercase tracking-wide text-muted-foreground">Project</label>
        <select v-model="projectId" data-wizard-project-select class="w-full rounded border bg-background px-3 py-2 text-sm">
          <option :value="null">Select…</option>
          <option v-for="p in projectsWithLocation" :key="p.id" :value="p.id">
            {{ p.uproject_name }} ({{ p.location_count }} machines)
          </option>
        </select>
      </div>

      <div v-else-if="step === 3" class="space-y-3">
        <label class="flex items-center gap-2 text-sm">
          <input type="checkbox" v-model="distribute" /> Distribute after generation
        </label>
        <div v-if="distribute">
          <label class="text-xs uppercase tracking-wide text-muted-foreground">Target machines</label>
          <div class="mt-1 max-h-48 overflow-y-auto rounded border bg-card">
            <label v-for="m in targetCandidates" :key="m.id ?? m.ip" class="flex items-center gap-2 border-b px-3 py-2 text-sm last:border-b-0">
              <input
                type="checkbox"
                :value="m.id"
                v-model="targetIds"
                data-wizard-target-checkbox
              />
              {{ m.hostname }} ({{ m.ip }})
            </label>
          </div>
          <label class="mt-3 block text-xs uppercase tracking-wide text-muted-foreground">Named share UNC (optional)</label>
          <input
            v-model="namedShare"
            data-wizard-named-share
            type="text"
            placeholder="\\\\HOST\\DDC (leave empty to use admin share)"
            class="mt-1 w-full rounded border bg-background px-3 py-2 font-mono text-sm"
          />
          <label class="mt-3 block text-xs uppercase tracking-wide text-muted-foreground">Credential</label>
          <select v-model="credAlias" data-wizard-cred-select class="mt-1 w-full rounded border bg-background px-3 py-2 text-sm">
            <option :value="null">(current process token)</option>
            <option v-for="c in winrmCreds" :key="c.alias" :value="c.alias">{{ c.alias }}</option>
          </select>
        </div>
      </div>

      <div v-else-if="step === 4" class="space-y-2">
        <h3 class="text-sm font-semibold">Preview</h3>
        <ul class="rounded border bg-muted/40 p-3 text-xs font-mono">
          <li v-for="(line, i) in previewLines" :key="i">• {{ line }}</li>
        </ul>
        <p v-if="errMsg" data-wizard-error class="rounded bg-rose-500/10 p-2 text-xs text-rose-600">{{ errMsg }}</p>
      </div>

      <div class="flex justify-between pt-2">
        <button
          v-if="step > 1"
          class="rounded border border-border px-3 py-1 text-xs"
          @click="step = (step - 1) as 1 | 2 | 3 | 4"
        >Back</button>
        <span v-else />
        <button
          v-if="step < 4"
          data-wizard-next
          class="rounded bg-primary px-3 py-1 text-xs text-primary-foreground disabled:opacity-50"
          :disabled="!canAdvance"
          @click="step = (step + 1) as 1 | 2 | 3 | 4"
        >Next</button>
        <button
          v-else
          data-wizard-run
          class="rounded bg-primary px-3 py-1 text-xs text-primary-foreground disabled:opacity-50"
          :disabled="!canAdvance"
          @click="run"
        >{{ isSubmitting ? "Starting…" : "Run" }}</button>
      </div>
    </div>
  </BaseModal>
</template>
```

- [ ] **Step 4: Wizard tests**

`src/__tests__/ProjectDiscoveryWizard.spec.ts`, `ProjectMatchingModal.spec.ts`, `DdcPakWizard.spec.ts` — see Plan 4 T18 fixtures for the structural template (mount with mocks for stores, exercise step navigation + submit). Each test file runs four basic asserts: (a) opens correctly, (b) advances on valid input, (c) blocks on invalid input, (d) emits "close" on successful submit.

Example skeleton (`DdcPakWizard.spec.ts`):

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

const { mockApi, mockListen } = vi.hoisted(() => ({
  mockApi: {
    listMachines: vi.fn().mockResolvedValue([
      { id: 1, hostname: "RENDER-01", ip: "1.1.1.1", role: "render", status: "online", last_seen_at: null },
      { id: 2, hostname: "RENDER-02", ip: "1.1.1.2", role: "render", status: "online", last_seen_at: null },
    ]),
    listProjects: vi.fn().mockResolvedValue([
      { id: 10, uproject_name: "X.uproject", display_name: null, uproject_guid: null, location_count: 2 },
    ]),
    listCredentials: vi.fn().mockResolvedValue([]),
    generateDdcPak: vi.fn().mockResolvedValue({ job_id: "g-1", source_machine_id: 1, project_id: 10, backend: "remote" }),
    distributeDdcPak: vi.fn().mockResolvedValue({ job_id: "d-1", project_id: 10, source_machine_id: 1, plan: [] }),
  },
  mockListen: vi.fn().mockResolvedValue(() => undefined),
}));
vi.mock("@/services/tauri", () => ({ tauriApi: mockApi }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mockListen }));

import DdcPakWizard from "@/components/modals/DdcPakWizard.vue";

describe("DdcPakWizard", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((m) => m.mockReset?.());
  });

  it("renders step 1 when open", () => {
    const w = mount(DdcPakWizard, { props: { open: true } });
    expect(w.find('[data-ddc-pak-wizard]').exists()).toBe(true);
  });

  it("blocks Next until source is chosen", () => {
    const w = mount(DdcPakWizard, { props: { open: true } });
    const next = w.find('[data-wizard-next]');
    expect((next.element as HTMLButtonElement).disabled).toBe(true);
  });
});
```

- [ ] **Step 5: Run + commit**

```bash
pnpm test src/__tests__/ProjectDiscoveryWizard.spec.ts src/__tests__/ProjectMatchingModal.spec.ts src/__tests__/DdcPakWizard.spec.ts 2>&1 | tail -10
```

```bash
git add src/components/modals/ProjectDiscoveryWizard.vue src/components/modals/ProjectMatchingModal.vue src/components/modals/DdcPakWizard.vue src/__tests__/ProjectDiscoveryWizard.spec.ts src/__tests__/ProjectMatchingModal.spec.ts src/__tests__/DdcPakWizard.spec.ts
git commit -m "feat(frontend): project discovery / matching / DDC pak wizards"
```

---

## Task 20: Rewrite `Projects.vue` view

**Files:**
- Modify: `src/views/Projects.vue` (was Plan 1 stub)
- Create: `src/__tests__/Projects-view.spec.ts`

- [ ] **Step 1: Rewrite the view**

```vue
<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useProjectsStore } from "@/stores/projects";
import { useMachinesStore } from "@/stores/machines";
import ProjectDiscoveryWizard from "@/components/modals/ProjectDiscoveryWizard.vue";
import ProjectMatchingModal from "@/components/modals/ProjectMatchingModal.vue";

const projects = useProjectsStore();
const machines = useMachinesStore();

const showDiscovery = ref(false);
const showMatching = ref(false);
const matchingProjectId = ref<number | null>(null);
const expandedId = ref<number | null>(null);

onMounted(async () => {
  await Promise.all([projects.load(), machines.loadMachines()]);
});

async function toggleExpand(id: number) {
  if (expandedId.value === id) {
    expandedId.value = null;
  } else {
    expandedId.value = id;
    if (!projects.locations[id]) await projects.loadLocations(id);
  }
}

function openMatch(id: number) {
  matchingProjectId.value = id;
  showMatching.value = true;
}

async function deleteProject(id: number) {
  if (!confirm("Delete this project and all its location mappings?")) return;
  await projects.removeProject(id);
}

async function deleteLocation(projectId: number, locationId: number) {
  await projects.removeLocation(projectId, locationId);
}

function machineLabel(machineId: number) {
  const m = machines.machines.find((x) => x.id === machineId);
  return m ? `${m.hostname} (${m.ip})` : `machine#${machineId}`;
}
</script>

<template>
  <div class="h-full space-y-6 overflow-auto p-6">
    <header class="flex items-center justify-between gap-4">
      <div>
        <p class="text-xs font-bold uppercase tracking-[0.18em] text-muted-foreground">UE Workspace</p>
        <h1 class="mt-1 font-display text-3xl font-extrabold">Projects</h1>
      </div>
      <button
        data-projects-discover-btn
        class="rounded bg-primary px-3 py-2 text-sm text-primary-foreground"
        @click="showDiscovery = true"
      >Discover on machine…</button>
    </header>

    <p v-if="projects.isLoading" class="text-sm text-muted-foreground">Loading…</p>
    <p
      v-else-if="projects.projects.length === 0"
      data-projects-empty
      class="rounded-lg border bg-card p-6 text-sm text-muted-foreground"
    >No projects yet. Click "Discover on machine" to walk a machine for .uproject files.</p>

    <div v-else class="space-y-3">
      <div
        v-for="p in projects.projects"
        :key="p.id"
        data-project-row
        class="rounded-lg border bg-card"
      >
        <div class="flex items-center justify-between gap-3 p-3">
          <div class="flex-1">
            <p class="font-mono text-sm">{{ p.uproject_name }}</p>
            <p class="text-xs text-muted-foreground">{{ p.location_count }} machine(s)</p>
          </div>
          <button
            data-projects-expand
            class="rounded border border-border px-2 py-1 text-xs"
            @click="toggleExpand(p.id)"
          >{{ expandedId === p.id ? "Hide" : "Show" }} locations</button>
          <button
            data-projects-add-mapping
            class="rounded border border-border px-2 py-1 text-xs"
            @click="openMatch(p.id)"
          >+ mapping</button>
          <button
            data-projects-delete
            class="rounded border border-rose-300 px-2 py-1 text-xs text-rose-600"
            @click="deleteProject(p.id)"
          >Delete</button>
        </div>
        <div v-if="expandedId === p.id && projects.locations[p.id]" class="border-t bg-muted/30 p-3">
          <table class="w-full text-xs">
            <thead class="text-muted-foreground">
              <tr><th class="text-left">Machine</th><th class="text-left">Path</th><th class="text-left">Discovery</th><th></th></tr>
            </thead>
            <tbody>
              <tr v-for="loc in projects.locations[p.id]" :key="loc.id ?? `${loc.project_id}:${loc.machine_id}`" data-project-location-row>
                <td>{{ machineLabel(loc.machine_id) }}</td>
                <td class="font-mono">{{ loc.abs_path }}</td>
                <td class="uppercase tracking-wide">{{ loc.discovery_status }}</td>
                <td class="text-right">
                  <button
                    v-if="loc.id != null"
                    class="text-xs text-rose-600"
                    @click="deleteLocation(p.id, loc.id)"
                  >Remove</button>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>

    <ProjectDiscoveryWizard :open="showDiscovery" @close="showDiscovery = false" />
    <ProjectMatchingModal :open="showMatching" :project-id="matchingProjectId" @close="showMatching = false" />
  </div>
</template>
```

- [ ] **Step 2: View test**

`src/__tests__/Projects-view.spec.ts`:

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    listMachines: vi.fn().mockResolvedValue([]),
    listProjects: vi.fn().mockResolvedValue([
      { id: 1, uproject_name: "X.uproject", display_name: null, uproject_guid: null, location_count: 2 },
    ]),
    listProjectLocations: vi.fn().mockResolvedValue([]),
    listCredentials: vi.fn().mockResolvedValue([]),
  },
}));
vi.mock("@/services/tauri", () => ({ tauriApi: mockApi }));

import Projects from "@/views/Projects.vue";

describe("Projects.vue", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("renders header and discover button", async () => {
    const w = mount(Projects);
    await new Promise((r) => setTimeout(r, 0));
    expect(w.find('[data-projects-discover-btn]').exists()).toBe(true);
  });

  it("renders project rows after load", async () => {
    const w = mount(Projects);
    await new Promise((r) => setTimeout(r, 0));
    await w.vm.$nextTick();
    expect(w.findAll('[data-project-row]').length).toBeGreaterThanOrEqual(1);
  });
});
```

- [ ] **Step 3: Run + commit**

```bash
pnpm test src/__tests__/Projects-view.spec.ts 2>&1 | tail -10
git add src/views/Projects.vue src/__tests__/Projects-view.spec.ts
git commit -m "feat(frontend): Projects.vue (was stub) — discover + locations + delete"
```

---

## Task 21: Rewrite `DDCPak.vue` view

**Files:**
- Modify: `src/views/DDCPak.vue` (was Plan 1 stub)
- Create: `src/components/ddcpak/PakJobCard.vue`
- Create: `src/components/ddcpak/DistributeProgressTable.vue`
- Create: `src/__tests__/DDCPak-view.spec.ts`
- Create: `src/__tests__/PakJobCard.spec.ts`
- Create: `src/__tests__/DistributeProgressTable.spec.ts`

- [ ] **Step 1: `PakJobCard.vue`**

```vue
<script setup lang="ts">
import { computed } from "vue";
import { UecmTaskCard } from "@/components/primitives";
import type { GenerateJobState } from "@/stores/ddcPak";

const props = defineProps<{ job: GenerateJobState; sourceLabel: string; projectLabel: string }>();
const emit = defineEmits<{ (e: "cancel", id: string): void }>();

const subtitle = computed(() => `${props.projectLabel} on ${props.sourceLabel}`);
</script>

<template>
  <UecmTaskCard
    :title="`DDC Pak — ${job.job_id.slice(-6)}`"
    :subtitle="subtitle"
    :status="job.status"
    :progress-pct="job.progress_pct"
    :progress-label="job.progress_label ?? undefined"
    :error-message="job.error_message"
    @cancel="emit('cancel', job.job_id)"
  />
</template>
```

- [ ] **Step 2: `DistributeProgressTable.vue`**

```vue
<script setup lang="ts">
import { computed } from "vue";
import type { DistributeJobState } from "@/stores/ddcPak";

const props = defineProps<{ job: DistributeJobState; machineLabel: (id: number) => string }>();

const summary = computed(() => {
  const ok = props.job.targets.filter((t) => t.status === "ok").length;
  const err = props.job.targets.filter((t) => t.status === "err").length;
  const running = props.job.targets.filter((t) => t.status === "running").length;
  return `${ok} ok / ${err} err / ${running} running / ${props.job.targets.length} total`;
});

function statusClass(s: string) {
  switch (s) {
    case "ok": return "text-emerald-500";
    case "err": return "text-rose-500";
    case "running": return "text-primary";
    default: return "text-muted-foreground";
  }
}
</script>

<template>
  <div data-distribute-progress-table class="rounded-lg border bg-card">
    <div class="flex items-center justify-between border-b p-3">
      <p class="font-mono text-sm">Distribute — {{ job.job_id.slice(-6) }}</p>
      <p class="text-xs text-muted-foreground">{{ summary }}</p>
    </div>
    <table class="w-full text-xs">
      <thead class="bg-muted text-muted-foreground">
        <tr>
          <th class="px-3 py-1 text-left">Target</th>
          <th class="px-3 py-1 text-left">Status</th>
          <th class="px-3 py-1 text-left">Message</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="t in job.targets" :key="t.target_machine_id" data-distribute-row class="border-t">
          <td class="px-3 py-1">{{ machineLabel(t.target_machine_id) }}</td>
          <td class="px-3 py-1">
            <span :class="statusClass(t.status)" class="font-mono uppercase">{{ t.status }}</span>
          </td>
          <td class="px-3 py-1 font-mono text-muted-foreground">{{ t.message ?? "" }}</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
```

- [ ] **Step 3: `DDCPak.vue` view**

```vue
<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useDdcPakStore } from "@/stores/ddcPak";
import { useMachinesStore } from "@/stores/machines";
import { useProjectsStore } from "@/stores/projects";
import DdcPakWizard from "@/components/modals/DdcPakWizard.vue";
import PakJobCard from "@/components/ddcpak/PakJobCard.vue";
import DistributeProgressTable from "@/components/ddcpak/DistributeProgressTable.vue";

const ddcPak = useDdcPakStore();
const machines = useMachinesStore();
const projects = useProjectsStore();

const showWizard = ref(false);

onMounted(async () => {
  await Promise.all([machines.loadMachines(), projects.load(), ddcPak.attach()]);
});
onUnmounted(() => {
  ddcPak.detach();
});

function machineLabel(id: number) {
  const m = machines.machines.find((x) => x.id === id);
  return m ? m.hostname : `m#${id}`;
}
function projectLabel(id: number) {
  const p = projects.projects.find((x) => x.id === id);
  return p ? p.uproject_name : `proj#${id}`;
}

async function cancelJob(id: string) {
  await ddcPak.cancelGenerate(id);
}
</script>

<template>
  <div class="h-full space-y-6 overflow-auto p-6">
    <header class="flex items-center justify-between gap-4">
      <div>
        <p class="text-xs font-bold uppercase tracking-[0.18em] text-muted-foreground">Cache Sync</p>
        <h1 class="mt-1 font-display text-3xl font-extrabold">DDC Pak</h1>
      </div>
      <button
        data-ddc-pak-new-btn
        class="rounded bg-primary px-3 py-2 text-sm text-primary-foreground"
        @click="showWizard = true"
      >Generate DDC Pak…</button>
    </header>

    <section v-if="ddcPak.generateJobs.length > 0" class="space-y-3" data-ddc-pak-generate-jobs>
      <h2 class="text-xs font-bold uppercase tracking-[0.18em] text-muted-foreground">Generation jobs</h2>
      <PakJobCard
        v-for="job in ddcPak.generateJobs"
        :key="job.job_id"
        :job="job"
        :source-label="machineLabel(job.source_machine_id)"
        :project-label="projectLabel(job.project_id)"
        @cancel="cancelJob"
      />
    </section>

    <section v-if="ddcPak.distributeJobs.length > 0" class="space-y-3" data-ddc-pak-distribute-jobs>
      <h2 class="text-xs font-bold uppercase tracking-[0.18em] text-muted-foreground">Distribute jobs</h2>
      <DistributeProgressTable
        v-for="job in ddcPak.distributeJobs"
        :key="job.job_id"
        :job="job"
        :machine-label="machineLabel"
      />
    </section>

    <p
      v-if="ddcPak.generateJobs.length === 0 && ddcPak.distributeJobs.length === 0"
      data-ddc-pak-empty
      class="rounded-lg border bg-card p-6 text-sm text-muted-foreground"
    >
      No jobs yet. Click "Generate DDC Pak" to package a project and optionally fan it out to render nodes.
    </p>

    <DdcPakWizard :open="showWizard" @close="showWizard = false" />
  </div>
</template>
```

- [ ] **Step 4: Tests**

`src/__tests__/PakJobCard.spec.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import PakJobCard from "@/components/ddcpak/PakJobCard.vue";
import type { GenerateJobState } from "@/stores/ddcPak";

function fixture(over: Partial<GenerateJobState> = {}): GenerateJobState {
  return {
    job_id: "ddc-pak-gen-1-0",
    source_machine_id: 1,
    project_id: 10,
    status: "running",
    log_lines: [],
    progress_pct: 0.5,
    progress_label: "Filling DDC",
    exit_code: null,
    error_message: null,
    output: null,
    started_at: new Date().toISOString(),
    finished_at: null,
    ...over,
  };
}

describe("PakJobCard", () => {
  it("emits cancel with job_id", async () => {
    const w = mount(PakJobCard, { props: { job: fixture(), sourceLabel: "RENDER-01", projectLabel: "X.uproject" } });
    await w.find('[data-task-card-cancel]').trigger("click");
    const events = w.emitted("cancel");
    expect(events?.[0]?.[0]).toBe("ddc-pak-gen-1-0");
  });

  it("subtitle includes both source and project label", () => {
    const w = mount(PakJobCard, { props: { job: fixture(), sourceLabel: "RENDER-01", projectLabel: "X.uproject" } });
    expect(w.text()).toContain("X.uproject");
    expect(w.text()).toContain("RENDER-01");
  });
});
```

`src/__tests__/DistributeProgressTable.spec.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import DistributeProgressTable from "@/components/ddcpak/DistributeProgressTable.vue";
import type { DistributeJobState } from "@/stores/ddcPak";

function fixture(over: Partial<DistributeJobState> = {}): DistributeJobState {
  return {
    job_id: "ddc-pak-dist-1-0",
    project_id: 10,
    source_machine_id: 1,
    status: "running",
    targets: [
      { target_machine_id: 2, target_host: "1.1.1.1", status: "running", message: null },
      { target_machine_id: 3, target_host: "1.1.1.2", status: "ok", message: null },
      { target_machine_id: 4, target_host: "1.1.1.3", status: "err", message: "boom" },
    ],
    started_at: new Date().toISOString(),
    finished_at: null,
    ...over,
  };
}

describe("DistributeProgressTable", () => {
  it("renders one row per target", () => {
    const w = mount(DistributeProgressTable, { props: { job: fixture(), machineLabel: (id: number) => `m${id}` } });
    expect(w.findAll('[data-distribute-row]').length).toBe(3);
  });

  it("summarises ok/err/running/total", () => {
    const w = mount(DistributeProgressTable, { props: { job: fixture(), machineLabel: () => "" } });
    expect(w.text()).toContain("1 ok");
    expect(w.text()).toContain("1 err");
    expect(w.text()).toContain("1 running");
    expect(w.text()).toContain("3 total");
  });
});
```

`src/__tests__/DDCPak-view.spec.ts`:

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

const { mockApi, mockListen } = vi.hoisted(() => ({
  mockApi: {
    listMachines: vi.fn().mockResolvedValue([]),
    listProjects: vi.fn().mockResolvedValue([]),
    listCredentials: vi.fn().mockResolvedValue([]),
  },
  mockListen: vi.fn().mockResolvedValue(() => undefined),
}));
vi.mock("@/services/tauri", () => ({ tauriApi: mockApi }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mockListen }));

import DDCPak from "@/views/DDCPak.vue";

describe("DDCPak.vue", () => {
  beforeEach(() => setActivePinia(createPinia()));

  it("renders empty state when no jobs", async () => {
    const w = mount(DDCPak);
    await new Promise((r) => setTimeout(r, 0));
    expect(w.find('[data-ddc-pak-empty]').exists()).toBe(true);
  });

  it("opens wizard on Generate button click", async () => {
    const w = mount(DDCPak);
    await w.find('[data-ddc-pak-new-btn]').trigger("click");
    await w.vm.$nextTick();
    expect(w.find('[data-ddc-pak-wizard]').exists()).toBe(true);
  });
});
```

- [ ] **Step 5: Run + commit**

```bash
pnpm test src/__tests__/PakJobCard.spec.ts src/__tests__/DistributeProgressTable.spec.ts src/__tests__/DDCPak-view.spec.ts 2>&1 | tail -10
git add src/views/DDCPak.vue src/components/ddcpak/PakJobCard.vue src/components/ddcpak/DistributeProgressTable.vue src/__tests__/PakJobCard.spec.ts src/__tests__/DistributeProgressTable.spec.ts src/__tests__/DDCPak-view.spec.ts
git commit -m "feat(frontend): DDCPak.vue + PakJobCard + DistributeProgressTable"
```

---

## Task 22: lanPC E2E — full Plan 5 verification

**Files:** none modified (verification only).

This task is operator-driven on lanPC. Engineer hands off to user when prompted. Each sub-step has a "expected pass" criterion; record results in commit message of T23.

- [ ] **Step 1: Build the production binary on lanPC**

```bash
ssh lanpc "cd E:\code\super_lanPC_uecm\ue-cache-manager; pnpm tauri build" 2>&1 | tail -15
```

Expected: `.exe` produced under `src-tauri/target/release/bundle/`. If NSIS bundle fails, fall back to running the raw `.exe` directly.

- [ ] **Step 2: Set up test fixture project**

If `E:\test-fixtures\PluralityProject\PluralityProject.uproject` does not exist (engineer should ask user via interactive PowerShell):

```powershell
# On lanPC
Test-Path E:\test-fixtures\PluralityProject\PluralityProject.uproject
```

If false: STOP and ask the user to create a Blank UE project at `E:\test-fixtures\PluralityProject\` via the UE 5.4 launcher and save once. (~5 min one-time prep.)

- [ ] **Step 3: Run discovery on lanPC itself**

In the running UECM:
1. Make sure lanPC (`192.168.10.20`) is in the machine list and online.
2. Click "Projects" → "Discover on machine…" → select lanPC → enter `E:\test-fixtures` as search root → Run.

Expected: at least one project (`PluralityProject.uproject`) appears. Record `project_id` in commit message.

- [ ] **Step 4: Generate DDC pak (combo C2 minimal — same source, no distribute targets except… well, none on a single-PC setup)**

Click "DDC Pak" → "Generate DDC Pak…" → step 1 = Remote, source = lanPC → step 2 = Plurality → step 3 = uncheck Distribute → step 4 = Run.

Expected:
- Job card appears on DDC Pak view, status `spawning` → `running`.
- UE log lines stream in.
- After ~5–10 minutes (tiny project): status `completed`, exit_code 0.
- Verify: `dir E:\test-fixtures\PluralityProject\DerivedDataCache\*.ddp` shows a file.

If UE.exe never spawns: check `query-ue-versions.ps1` already returned an `install_path` for lanPC. If install_path is wrong, fix in `machine_ue_installs` row.

- [ ] **Step 5: Verify pak output via UECM**

In the wizard (or via dev-tools console):

```javascript
await window.__TAURI__.core.invoke('verify_pak_output', { machineId: 2, projectId: 1, operatorCredentialAlias: null })
```

Expected: returns `{ path: "E:\\...\\Compressed.ddp", size_bytes: <non-zero> }`.

- [ ] **Step 6: Distribute combo C2 (cross-machine)**

If a second Windows VM is available (lanbipu-razer, or two synthetic IPs both pointing at lanPC for plumbing-test-only): rerun the wizard with Distribute checked. Otherwise SKIP and document deferral in T23 commit.

Expected (when targets available): per-target status flips through `pending → running → ok`, summary shows `N ok / 0 err`.

- [ ] **Step 7: Cancel test**

Restart Generate. While UE log is streaming, click "Cancel" on the job card.

Expected: status flips to `cancelled` within 1–2 seconds. UE.exe disappears from Task Manager.

- [ ] **Step 8: Idempotent re-generate**

Click Generate again on the same project.

Expected: a new job card appears alongside the cancelled one. The new run completes (overwrites prior .ddp).

- [ ] **Step 9: Summarise findings for the T23 commit**

Take note in plain text:
- Final test count (frontend + backend)
- Production build outcome
- E2E pass/fail per sub-step
- Any DONE_WITH_CONCERNS observations

---

## Task 23: Final integration — README + production build smoke

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update README "What's working"**

Append bullets:

```markdown
- DDC Pak generation (UE -run=DerivedDataCache) on remote / local machine, with cancel
- DDC Pak distribution via Robocopy + admin-share UNC, multi-target with per-machine progress
- Project discovery (.uproject walk) + 3-level identity matcher (filename / manual alias / manual path)
- Projects view + DDC Pak view (replaces Plan 1 stubs)
```

Update Status line:

```markdown
**Status:** Plan 5 (DDC Pak) complete. Next: Plan 6 (PSO Cache + visual polish).
```

Move "DDC Pak generation + distribution (Plan 5)" out of "What's NOT yet implemented" and re-list:

```markdown
- PSO Cache operations + visual polish (Plan 6)
```

- [ ] **Step 2: Run full test suite as final smoke**

```bash
export PATH="/Users/bip.lan/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
pnpm test 2>&1 | tail -10
cd src-tauri && cargo test --lib 2>&1 | tail -10 && cd ..
```

Expected: all green. Record exact counts.

- [ ] **Step 3: Production build smoke (macOS dev box)**

```bash
pnpm tauri build --debug 2>&1 | tail -20
```

Expected: clean build (or skip if Tauri build is gated to lanPC; use `pnpm build` for frontend smoke alone).

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "$(cat <<'EOF'
docs: Plan 5 (DDC Pak) complete

Tests: <N> frontend / <M> backend
Production build: <ok | skipped>
lanPC E2E: <summary>

Modules added: ue_runner, project_identity, project_discovery,
ddc_pak (preflight + generate + verify), pak_distribute.
PowerShell sidecars: discover-uprojects, start/tail/stop-ue-process,
generate-ddc-pak, verify-pak-output, distribute-pak-file.
SQLite migrations: 008_projects + 009_project_locations.
Tauri commands: discover_projects, list_projects, list_project_locations,
set_project_location, delete_project, generate_ddc_pak, cancel_ue_job,
verify_pak_output, distribute_ddc_pak, create_project_manual.
Frontend: useProjectsStore + useDdcPakStore, DdcPakWizard +
ProjectDiscoveryWizard + ProjectMatchingModal, Projects.vue + DDCPak.vue
rewrites, UecmProgressBar + UecmTaskCard + UecmPathInput primitives.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

- [ ] **Step 5: Push branch (do NOT merge)**

```bash
git push -u origin feature/plan-5-ddc-pak
```

Open a PR with the commit summary as the body. Do NOT merge — wait for code review.

---

## Plan 5 — Done

Total: 23 tasks. Implementation surface:

- **Backend**: ~1700 lines Rust across 6 new modules + 7 PowerShell sidecars + 2 migrations.
- **Frontend**: ~1500 lines Vue/TS across 2 stores, 3 primitives, 3 modals, 2 view rewrites, 2 component sub-folders.
- **Tests**: ~25 new unit/integration tests (Rust + Vitest).
- **lanPC E2E**: 9-step verification with documented deferrals.

The `core::ue_runner` foundation is the load-bearing piece for Plan 6. The `core::pak_distribute` plus `project_locations` table are reused directly by Plan 6 F3.
