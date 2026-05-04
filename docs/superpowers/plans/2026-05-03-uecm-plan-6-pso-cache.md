# UECM Plan 6 — PSO Cache + Visual Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

## Execution Mode (READ FIRST — overrides default skill behavior)

**Mode: AUTO-CONTINUOUS.** Run all 25 tasks back-to-back without pausing for human approval between them. Same rules as Plan 5.

**Stop and ask the user ONLY in these cases:**

1. **Plan vs reality conflict** that requires re-design (structural mismatch where continuing would produce wrong work).
2. **Destructive operation requiring authorization**: deleting files outside `<ProjectDir>/Saved/CollectedPSOs/`, modifying source-controlled UE files, deleting credentials, `git push --force`, `rm -rf` outside the workspace, modifying SSH config.
3. **Critical-severity code review finding with no obvious fix.**
4. **lanPC unreachable, WinRM disabled, UnrealEditor.exe absent, real GPU absent (PSO collect requires a real GPU; lanPC has one), or test project missing** when an E2E verification step requires it.
5. **A new dependency decision** not covered by the plan.
6. **Figma access blocked or final design tokens not landed** when polish tasks (T22) require them — fall back to current Plan 4 design system as v1.0 baseline.

**Do NOT stop for:** spec/quality review finding Important / Minor issues; Windows-gated tests skipped on macOS; DONE_WITH_CONCERNS observations; README/docs cleanup; visual nits during polish.

**Final report:** commit list, frontend + backend test counts, every DONE_WITH_CONCERNS verbatim, production build outcome, deferred lanPC E2E steps, polish-pass deltas.

---

**Goal:** Plan 6 closes UECM v1.0. It builds the four PSO Cache surfaces (F1 CVar verification, F2 collection, F3 distribution, F4 GPU consistency matrix) and applies the visual polish pass that promotes the app from "feature-complete" to "shippable". After Plan 6 ships, UECM hits its design intent: any Render Node in the cluster opens a UE project with **zero shader recompile** (DDC) and **zero scene-switch hitch** (PSO).

**Architecture additions:**

- `core::pso_collect` — Reuses `core::ue_runner` (built in Plan 5) to spawn UE in `-game` mode with `r.ShaderPipelineCache.Enabled=1` and related CVars. Monitors the same UE log for PSO-specific markers, then enumerates the resulting `<ProjectDir>/Saved/CollectedPSOs/*.upipelinecache` files via a remote PowerShell call.
- `core::pso_distribute` — Thin wrapper around `core::pak_distribute` that targets `Saved/CollectedPSOs/` instead of `DerivedDataCache/`. Same Robocopy fan-out, same `project_locations` resolution, different file glob (`*.upipelinecache *.stablepc.csv`).
- `core::gpu_consistency` — Pure-Rust aggregator over `machine_gpus` rows. Produces a per-machine `(gpu_model, driver_version)` signature, identifies the cluster baseline (highest count), classifies each machine as `match | deviation | unknown`, and emits a structured matrix for UI consumption. Plan 4 already shipped a derivative of this for the health-check #11 row; Plan 6 promotes it to a dedicated module so the PSO surface and Health surface share one source of truth.
- `core::ini_diagnostics` extension — Three new rules `R008`, `R009`, `R010` covering the PSO Precaching CVars (`r.PSOPrecaching=1`, `r.PSOPrecache.Compile=1`, `r.PSOPrecache.GlobalShaders=1`). Pure rule additions; no orchestration code.
- New PowerShell sidecars: `start-pso-collect.ps1` (composes UE args), `list-pso-cache-files.ps1` (enumerates collected files post-run), `distribute-pso-cache.ps1` (Robocopy variant for the `Saved/CollectedPSOs/` glob).
- Two SQLite migrations: `011_pso_cache_files_table` + `012_pso_distributions_table`.
- Tauri command surface: `verify_pso_precaching`, `start_pso_collection`, `cancel_ue_job` (already exists from Plan 5; reused), `list_pso_cache_files`, `distribute_pso_cache`, `get_gpu_consistency_matrix`.
- Frontend stores: `usePsoStore` (collection + distribution state) and `useGpuConsistencyStore` (matrix). Both consume the same `ue-runner-progress` and `pak-distribute-progress` channels Plan 5 introduced — no new event type.
- Frontend primitives expansion: `UecmGpuMatrix.vue` (cluster matrix cell), `UecmHorizontalSplit.vue` (collected-files explorer pane). Plus the polish-pass touch-ups described in T22/T23.
- Visual polish pass: applies the final Figma tokens (color, font, spacing) introduced in `2026-05-02-uecm-design-system-overhaul.md` across all 7 views, normalises empty-state / loading / error rendering, ships a Dashboard glow-up that summarises every subsystem.

**Tech Stack:** Builds on Plan 5 stack. No new Rust deps. Frontend: continues using the design tokens already landed in `tailwind.config.js`. Polish-pass colours come from a CSS-vars file produced by the design team — wire-up only, no new tooling.

**Out of scope for this plan (deferred indefinitely):**

- Automated camera-flythrough scripting for PSO collection (v2). v1 has the operator manually drive the UE viewport for a few minutes per scene.
- Multi-machine distributed collection (different scenes on different render nodes). v1 collects from one source machine.
- AMD / Intel GPU CI matrix (we ship NVIDIA-tested only; AMD warnings appear in F4 matrix as `warning` not `critical`).
- ZenServer-backed PSO storage (Plan 6 stays Filesystem-only).
- Full export-to-PDF cluster report (manual screenshot + the existing JSON export from Plan 4 cover v1).

**Deliverable at end:**

1. **F1**: User clicks "Verify PSO Precaching" on Health Check view → 3 new CVar rules run as part of the existing INI scanner; flagged-misconfigured machines show actionable rows. Existing "Apply finding" path remediates.
2. **F2**: User clicks "Collect PSO Cache…" → wizard prompts: pick logical project, pick source machine (real GPU), pick which levels/maps to load, optionally specify max-collection-minutes. Click Run. UE spawns in `-game` mode; UECM tails the log; status flips through `spawning → collecting → completing`.
3. **F2 (continued)**: After UE exits, UECM enumerates `Saved/CollectedPSOs/*.upipelinecache` and `*.stablepc.csv`, persists into `pso_cache_files`, and shows a file-explorer panel with sizes + GPU signature.
4. **F3**: User selects a collected file → "Distribute…" → wizard offers target machines (auto-filtered to GPU-matched). Click Run → Robocopy fan-out via `pak_distribute`, per-target progress, retry panel for failures. Persists into `pso_distributions`.
5. **F4**: GPU/driver consistency matrix view: rows = unique (model, driver) signatures, columns = machines. Baseline column highlighted. Cells show `match` / `deviation` / `unknown`. Drill-into-machine link.
6. **Health Check integration**: matrix #10 (PSO CVar) and #11 (GPU consistency) cells now reflect data computed by Plan 6 modules instead of inline shims.
7. **Visual polish**: every view re-skinned to Figma final, empty/error/loading states unified, Dashboard shows live subsystem KPIs.
8. **Production build green; full E2E on lanPC verified end-to-end. README updated to v1.0.**

---

## Lessons from Plan 5 — Applied to Plan 6

| Source | Lesson | Plan 6 task |
|---|---|---|
| Plan 5 T9 ue_runner | Log tail interval (1s) is the right default; sub-second adds round-trip overhead with no UX gain | T10 reuses TAIL_INTERVAL constant |
| Plan 5 T11 cancel | UE process registry keyed by job_id with a global mutex is sufficient; no need for a structured task table | T11 reuses `UeJobRegistry` from Plan 5 |
| Plan 5 T14 plan() vs run_one() | Splitting the Robocopy plan from execution (plan first, then per-target run) makes batch tests trivial | T12 mirrors this exactly |
| Plan 5 T17 store split | One Pinia store per logical surface; Plan 6 follows the same rule | T15 + T16 — `usePsoStore` and `useGpuConsistencyStore` are decoupled |
| Plan 5 T19 wizard step nav | 4-step wizard with `canAdvance` computed gate scales — reuse the pattern | T17 + T18 mirror DdcPakWizard |
| Plan 5 T22 E2E SKIP rule | If a sub-step needs hardware not present (second Windows VM, second GPU model), document deferral in T23 commit and proceed | T24 follows the same convention |

---

## Prerequisites (engineer must have before starting)

Same as Plan 5, plus:

### Real GPU on the source machine

PSO collection requires UE to actually create graphics pipeline state objects. A virtual GPU (RDP, virtual display adapter) returns nonsensical / empty PSO files. For E2E:

- lanPC has a real GPU (verified in Plan 3 GPU detection) — **required**.
- If lanPC's display GPU drifts (e.g. user swaps card), PSO collection still works but the resulting `*.upipelinecache` is GPU-specific — distributing it to a different model is a no-op or hazard. F4 catches this.

### Test project must contain at least one rendered map

The Plan 5 fixture project (`PluralityProject`) is fine if it has a default map (UE creates one on project init). If the project has no map: PSO collection runs but produces an empty `.upipelinecache`. T24 step 2 verifies this.

### lanPC display must be active

UE in `-game` mode tries to acquire the display. If lanPC is in headless / RDP-only mode without a display attached, PSO collection may fail silently. For E2E either:
- Run lanPC at the physical desk with display attached, OR
- Use `-windowed -resx=320 -resy=240` to minimise display footprint while still acquiring the surface.

The wizard defaults to windowed at 1920×1080; for headless lanPC, override to 320×240 in the advanced options.

### Cross-platform testing rules (recap)

- `core::gpu_consistency` aggregator and `core::ini_diagnostics` rule additions are pure Rust; tests on macOS.
- `core::pso_collect` orchestration is Windows-gated (calls `core::ue_runner` which is Windows-gated for Remote backend).
- `core::pso_distribute` is essentially a re-export with a different glob; tests on macOS via the existing `core::pak_distribute` tests + one Plan 6-specific glob assertion.
- Frontend tests on macOS via vitest.
- E2E on lanPC.

### Figma final tokens

Polish task T22 assumes the design team has shipped final color + spacing tokens. Two acceptable states:

- **Tokens landed** (preferred): `src/assets/tokens.css` exists with the final palette + radius + shadow vars. T22 wires them into `tailwind.config.js` extends and sweeps each view to use them.
- **Tokens NOT landed**: T22 promotes Plan 4's interim tokens to v1.0 baseline (rename `--uecm-interim-*` → `--uecm-*`), defers the design-team-final pass to a v1.1 follow-up. Engineer notes the deferral in the T25 README commit.

---

## File Structure

```
ue-cache-manager/
├── ps-scripts/
│   ├── start-pso-collect.ps1                   # NEW
│   ├── list-pso-cache-files.ps1                # NEW
│   ├── distribute-pso-cache.ps1                # NEW (Robocopy variant)
│   └── (existing scripts unchanged)
│
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs                              # MODIFY (register new commands)
│   │   │
│   │   ├── commands/
│   │   │   ├── pso.rs                          # NEW (verify, collect, list, distribute)
│   │   │   ├── gpu_consistency.rs              # NEW (matrix command)
│   │   │   └── mod.rs                          # MODIFY
│   │   │
│   │   ├── core/
│   │   │   ├── pso_collect.rs                  # NEW (reuses ue_runner)
│   │   │   ├── pso_distribute.rs               # NEW (reuses pak_distribute)
│   │   │   ├── gpu_consistency.rs              # NEW (pure-Rust aggregator)
│   │   │   ├── ini_diagnostics.rs              # MODIFY (R008-R010)
│   │   │   └── mod.rs                          # MODIFY
│   │   │
│   │   └── data/
│   │       ├── pso_cache_files.rs              # NEW
│   │       ├── pso_distributions.rs            # NEW
│   │       ├── schema.rs                       # MODIFY (migrations 011 + 012)
│   │       └── mod.rs                          # MODIFY
│
├── src/
│   ├── services/tauri.ts                       # MODIFY
│   │
│   ├── stores/
│   │   ├── pso.ts                              # NEW
│   │   ├── gpuConsistency.ts                   # NEW
│   │   └── healthCheck.ts                      # MODIFY (rewire #10/#11 to new sources)
│   │
│   ├── lib/
│   │   ├── healthChecks.ts                     # MODIFY (#10/#11 metadata refresh)
│   │   └── psoLevels.ts                        # NEW (recommended levels list defaults)
│   │
│   ├── components/
│   │   ├── primitives/
│   │   │   ├── UecmGpuMatrix.vue               # NEW
│   │   │   ├── UecmHorizontalSplit.vue         # NEW
│   │   │   └── index.ts                        # MODIFY
│   │   ├── modals/
│   │   │   ├── PsoCollectWizard.vue            # NEW
│   │   │   ├── PsoDistributeWizard.vue         # NEW
│   │   │   └── (existing modals unchanged)
│   │   └── pso/
│   │       ├── PsoFileExplorer.vue             # NEW (left list, right preview)
│   │       └── PsoJobCard.vue                  # NEW (reuses UecmTaskCard pattern)
│   │
│   ├── views/
│   │   ├── PSOCache.vue                        # REWRITE (was stub)
│   │   ├── HealthCheck.vue                     # MODIFY (rewire #10/#11)
│   │   ├── Dashboard.vue                       # REWRITE (polish pass)
│   │   ├── Machines.vue                        # MODIFY (polish pass)
│   │   ├── Projects.vue                        # MODIFY (polish pass)
│   │   ├── DDCPak.vue                          # MODIFY (polish pass)
│   │   ├── INIScanner.vue                      # MODIFY (polish pass)
│   │   └── Shares.vue                          # MODIFY (polish pass)
│   │
│   └── __tests__/
│       ├── pso-store.spec.ts
│       ├── gpu-consistency-store.spec.ts
│       ├── PsoCollectWizard.spec.ts
│       ├── PsoDistributeWizard.spec.ts
│       ├── PsoFileExplorer.spec.ts
│       ├── PsoJobCard.spec.ts
│       ├── PSOCache-view.spec.ts
│       ├── UecmGpuMatrix.spec.ts
│       ├── Dashboard-view.spec.ts              # NEW (after rewrite)
│       └── healthChecks-rewire.spec.ts         # NEW (verify #10/#11 derive correctly)
│
├── docs/
│   └── superpowers/
│       └── changelog/
│           └── 2026-05-XX-v1.0.md              # NEW (release notes summary)
│
├── README.md                                   # MODIFY (v1.0 status)
└── tailwind.config.js                          # MODIFY (final tokens applied)
```

---

## Approach Notes

**PSO collection invocation.** UE command line:

```
UnrealEditor.exe <Project>.uproject -game -windowed -resx=1920 -resy=1080 \
  -log -ExecCmds="r.ShaderPipelineCache.Enabled 1; r.ShaderPipelineCache.LogPSO 1; r.PSO.WarmingTime 0"
```

`-game` boots UE without the editor (faster, lighter). `r.ShaderPipelineCache.Enabled=1` activates the runtime cache writer. `r.ShaderPipelineCache.LogPSO=1` makes UE emit `LogShaderPipelineCache` lines we can monitor. `r.PSO.WarmingTime=0` makes warmup non-blocking. Optional `-PSOFile=<path>` lets the operator override the output path; v1 uses the default `<ProjectDir>/Saved/CollectedPSOs/<ProjectName>_<UE_Version>.upipelinecache`.

**PSO log markers we recognise** (added to `core::ue_runner::parse_line` via a Plan 6 extension closure or via a new optional `pso_mode` flag on `UeRunSpec`):

- `LogShaderPipelineCache: Display: Logging shader pipeline cache to <Path>` — collection started
- `LogShaderPipelineCache: Display: PSO snapshot saved to <Path>` — periodic snapshot
- `LogShaderPipelineCache: Display: PSO logging stopped. Wrote N PSOs.` — final write
- The existing exit markers (`LogInit: Engine exit requested`, etc.) still apply.

Plan 6 keeps `parse_line` rules narrow: instead of branching inside it, the Plan 6 modules consume `UeRunnerEvent::LogLine` directly and apply PSO-specific parsing on top (in `core::pso_collect`). This keeps the runner generic.

**Collection completion contract.** UE in `-game` mode does NOT auto-exit when PSO collection is "done" — there's no concept of "done". The operator drives a few minutes of camera flying / scene switching, then closes UE manually OR the wizard's `max_minutes` watchdog auto-cancels. After exit, `core::pso_collect::finalize` enumerates the `Saved/CollectedPSOs/` directory and persists each found file into `pso_cache_files` with its size + GPU signature (read from the parallel `machine_gpus` row).

**Three modes for completion**:
1. **Operator manual close** — UECM detects UE exit via `Completed` event, finalizes immediately.
2. **Watchdog auto-cancel** — wizard config `max_minutes`, defaults to 10 — cancellation flips to `core::pso_collect::finalize` regardless.
3. **Operator clicks "Done collecting"** in the wizard before manually closing UE — UECM cancels the runner (sends `Stop-Process`) and finalizes.

All three converge on the same enumerate-and-persist tail.

**GPU signature for PSO match.** A `.upipelinecache` is **GPU-and-driver-specific**. We compute a signature `<vendor>:<gpu_model>:<driver_version>` and store it on each `pso_cache_files` row. Distribution pre-flights every target's signature against the source — mismatches block distribution (UI flags, requires explicit "force" toggle).

**F1 — PSO CVar rules in `ini_diagnostics`.** Three pure additions:

| ID | Severity | Pattern | Recommendation |
|---|---|---|---|
| `R008` | 🔴 Critical | `[ConsoleVariables]` section in `Config/ConsoleVariables.ini` is missing `r.PSOPrecaching=1` (or set to 0) | Set `r.PSOPrecaching=1` |
| `R009` | 🟡 Warning | `r.PSOPrecache.Compile` not set (defaults to 0 in some UE versions) | Set `r.PSOPrecache.Compile=1` |
| `R010` | 🟡 Warning | `r.PSOPrecache.GlobalShaders` not set | Set `r.PSOPrecache.GlobalShaders=1` |

These plug into the same rule engine + scanner Plan 4 built. The "Apply suggestion" path reuses `core::ini_apply::apply` from Plan 4. Health Check matrix row #10 (PSO Precaching CVar) reads `ini_findings` for these rule IDs to compute its cell status.

**F4 — GPU consistency aggregator.** Pure Rust, takes a `Vec<MachineGpuRow>` and returns a `GpuMatrix { signatures: Vec<GpuSignature>, baseline: Option<usize>, machines: Vec<MachineGpuCell> }`. Baseline = the signature with the highest count (ties broken by alphabetical). Cells classify as `match` (signature == baseline), `deviation` (different signature exists), `unknown` (machine has no GPU row yet). Plan 4 had an inline version; Plan 6 promotes it.

**F2/F3 share infrastructure.** `core::pso_distribute::plan` is a 5-line wrapper around `core::pak_distribute::plan` that swaps the file glob from `*.ddp` to `*.upipelinecache *.stablepc.csv` and the source/target subdirectory from `DerivedDataCache/` to `Saved/CollectedPSOs/`. The wrapper preserves the admin-share + named-share fallback strategy.

**Visual polish strategy.** The polish phase (T22 + T23) is **not a redesign** — it applies tokens that already exist (or get finalised) and sweeps the UX patterns introduced ad-hoc in Plans 1-5 into a consistent set:

- **Empty states** — every view that can be empty uses the same `data-{view}-empty` panel pattern with the same copy structure ("No X yet. Click Y to start.").
- **Loading states** — every store-driven view shows a skeleton or "Loading…" placeholder with the same data attribute and visual treatment.
- **Error states** — same `data-{view}-error` panel, same red-tinted styling, always with `code` + `message` + a "Retry" affordance when applicable.
- **Dashboard rewrite** — replaces the Plan 1 placeholder with a live KPI strip (machines online, projects discovered, latest health score, latest DDC pak job, latest PSO collection job), each linking through to its detailed view.

Polish is gated by visual review — engineer captures screenshots and posts them in the T25 PR description for human approval. No automated visual-regression in v1.

---

## Self-Review Checklist (run after writing each task)

- [ ] **Spec coverage:** every section of design doc 5.F (PSO Cache Operations) maps to at least one task. R008-R010 rules each have a unit test.
- [ ] **No placeholders:** every step shows actual code or actual command.
- [ ] **Type consistency:** `PsoCacheFile` and `PsoDistribution` row shapes match across Rust/TS. `GpuSignature` (`vendor:model:driver`) format is stable. `MatchKind` (re-exported from `pak_distribute`) unchanged.
- [ ] **Selectors preserved:** all existing `data-*` selectors on Plan 5 views remain functional after the polish pass.
- [ ] **Stores untouched:** `useMachinesStore`, `useDiscoveryStore`, `useCredentialsStore`, `useSharesStore`, `useBatchStore`, `useClusterStore`, `useTasksStore`, `useDiagnosticsStore`, `useHealthCheckStore`, `useProjectsStore`, `useDdcPakStore` — Plan 6 ONLY adds `usePsoStore` + `useGpuConsistencyStore`. `useHealthCheckStore` is *modified* to rewire #10/#11 — but signature stays.
- [ ] **Routes intact:** all 8 existing routes still resolve.
- [ ] **Migration ordering:** 011 + 012 land after the integrated Plan 4/5 baseline (`007_diagnostics_tables`, `008_operations_table`, `009_projects_table`, `010_project_locations_table`).
- [ ] **`core::ue_runner` left intact:** Plan 6 does NOT modify the runner; it only wraps it. If a Plan 6 task tries to add PSO-specific markers to `parse_line`, push back — the parsing belongs in `core::pso_collect`.

---

## Task 1: Pre-flight audit (no new code)

**Files:** none modified.

- [ ] **Step 1: Confirm Plan 5 baseline green**

```bash
export PATH="/Users/bip.lan/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
pnpm test 2>&1 | tail -10
cd src-tauri && cargo test --lib 2>&1 | tail -10 && cd ..
```

Expected: ~150+ frontend / ~110+ backend tests pass (Plan 5 final state). If anything fails, **STOP** and report.

- [ ] **Step 2: Confirm Plan 5 modules are present**

```bash
grep -nE "pub mod (ue_runner|project_identity|project_discovery|ddc_pak|pak_distribute)" src-tauri/src/core/mod.rs
```

Expected: all five modules listed.

- [ ] **Step 3: Confirm Plan 5 Tauri commands are registered**

```bash
grep -nE "(generate_ddc_pak|distribute_ddc_pak|cancel_ue_job|verify_pak_output|discover_projects)" src-tauri/src/lib.rs
```

Expected: all 5 listed in `invoke_handler!`.

- [ ] **Step 4: Confirm Plan 4 INI scanner exists for F1 reuse**

```bash
grep -n "pub fn run_scan" src-tauri/src/core/ini_scanner.rs
```

Expected: function exists (Plan 4 placed scan orchestration in `run_scan`, not `scan_machine`). If absent, the F1 tasks (T4 + T5) need to be re-scoped — STOP and ask.

- [ ] **Step 5: Branch and tracking note**

```bash
git branch --show-current
```

Expected: `codex/plan-6-pso-cache`. The worktree was created from `codex/plan-5-ddc-pak`, then merged with `main` docs and `codex/plan-4-diagnostics`.

---

## Task 2: SQLite migrations 011 + 012 — `pso_cache_files` + `pso_distributions`

**Files:**
- Modify: `src-tauri/src/data/schema.rs`

- [ ] **Step 1: Append migrations**

```rust
    (
        "011_pso_cache_files_table",
        r#"
        CREATE TABLE IF NOT EXISTS pso_cache_files (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id INTEGER NOT NULL,
            source_machine_id INTEGER NOT NULL,
            file_path TEXT NOT NULL,
            file_name TEXT NOT NULL,
            size_bytes INTEGER NOT NULL DEFAULT 0,
            gpu_signature TEXT NOT NULL,
            ue_version TEXT,
            collected_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(project_id, source_machine_id, file_name),
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
            FOREIGN KEY (source_machine_id) REFERENCES machines(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_pso_cache_files_project ON pso_cache_files(project_id);
        CREATE INDEX IF NOT EXISTS idx_pso_cache_files_signature ON pso_cache_files(gpu_signature);
        "#,
    ),
    (
        "012_pso_distributions_table",
        r#"
        CREATE TABLE IF NOT EXISTS pso_distributions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            pso_cache_file_id INTEGER NOT NULL,
            target_machine_id INTEGER NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            bytes_copied INTEGER NOT NULL DEFAULT 0,
            distributed_at TEXT,
            error_message TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(pso_cache_file_id, target_machine_id),
            FOREIGN KEY (pso_cache_file_id) REFERENCES pso_cache_files(id) ON DELETE CASCADE,
            FOREIGN KEY (target_machine_id) REFERENCES machines(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_pso_distributions_file ON pso_distributions(pso_cache_file_id);
        "#,
    ),
```

- [ ] **Step 2: Migration smoke tests**

In `data/schema.rs` `tests` module, append:

```rust
    #[test]
    fn migration_011_creates_pso_cache_files_with_unique_constraint() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='pso_cache_files'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn migration_012_creates_pso_distributions_with_fk() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        conn.execute("PRAGMA foreign_keys = ON;", []).unwrap();
        // FK violation: invalid pso_cache_file_id
        let r = conn.execute(
            "INSERT INTO pso_distributions (pso_cache_file_id, target_machine_id) VALUES (999, 1)",
            [],
        );
        assert!(r.is_err());
    }
```

- [ ] **Step 3: Run + commit**

```bash
cd src-tauri && cargo test --lib data::schema 2>&1 | tail -10 && cd ..
git add src-tauri/src/data/schema.rs
git commit -m "feat: schema migrations 011-012 (pso_cache_files + pso_distributions)"
```

---

## Task 3: Data layer — `data::pso_cache_files` + `data::pso_distributions`

**Files:**
- Create: `src-tauri/src/data/pso_cache_files.rs`
- Create: `src-tauri/src/data/pso_distributions.rs`
- Modify: `src-tauri/src/data/mod.rs`

- [ ] **Step 1: `pso_cache_files.rs`**

```rust
//! CRUD for the `pso_cache_files` table.

use crate::data::Db;
use crate::error::UecmResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsoCacheFile {
    pub id: Option<i64>,
    pub project_id: i64,
    pub source_machine_id: i64,
    pub file_path: String,
    pub file_name: String,
    pub size_bytes: i64,
    pub gpu_signature: String,
    pub ue_version: Option<String>,
    pub collected_at: Option<String>,
}

pub fn upsert(db: &Db, f: &PsoCacheFile) -> UecmResult<i64> {
    let conn = db.lock().unwrap();
    conn.execute(
        "INSERT INTO pso_cache_files (project_id, source_machine_id, file_path, file_name, size_bytes, gpu_signature, ue_version)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(project_id, source_machine_id, file_name) DO UPDATE SET
           file_path = excluded.file_path,
           size_bytes = excluded.size_bytes,
           gpu_signature = excluded.gpu_signature,
           ue_version = COALESCE(excluded.ue_version, pso_cache_files.ue_version),
           collected_at = CURRENT_TIMESTAMP",
        rusqlite::params![
            f.project_id, f.source_machine_id, f.file_path, f.file_name,
            f.size_bytes, f.gpu_signature, f.ue_version,
        ],
    )?;
    let id: i64 = conn.query_row(
        "SELECT id FROM pso_cache_files WHERE project_id = ? AND source_machine_id = ? AND file_name = ?",
        rusqlite::params![f.project_id, f.source_machine_id, f.file_name],
        |r| r.get(0),
    )?;
    Ok(id)
}

pub fn list_by_project(db: &Db, project_id: i64) -> UecmResult<Vec<PsoCacheFile>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, project_id, source_machine_id, file_path, file_name, size_bytes, gpu_signature, ue_version, collected_at
         FROM pso_cache_files WHERE project_id = ? ORDER BY collected_at DESC",
    )?;
    let rows = stmt
        .query_map([project_id], |r| {
            Ok(PsoCacheFile {
                id: Some(r.get(0)?),
                project_id: r.get(1)?,
                source_machine_id: r.get(2)?,
                file_path: r.get(3)?,
                file_name: r.get(4)?,
                size_bytes: r.get(5)?,
                gpu_signature: r.get(6)?,
                ue_version: r.get(7)?,
                collected_at: r.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn get(db: &Db, file_id: i64) -> UecmResult<Option<PsoCacheFile>> {
    let conn = db.lock().unwrap();
    let row = conn.query_row(
        "SELECT id, project_id, source_machine_id, file_path, file_name, size_bytes, gpu_signature, ue_version, collected_at
         FROM pso_cache_files WHERE id = ?",
        [file_id],
        |r| {
            Ok(PsoCacheFile {
                id: Some(r.get(0)?),
                project_id: r.get(1)?,
                source_machine_id: r.get(2)?,
                file_path: r.get(3)?,
                file_name: r.get(4)?,
                size_bytes: r.get(5)?,
                gpu_signature: r.get(6)?,
                ue_version: r.get(7)?,
                collected_at: r.get(8)?,
            })
        },
    );
    match row {
        Ok(f) => Ok(Some(f)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn delete(db: &Db, file_id: i64) -> UecmResult<()> {
    let conn = db.lock().unwrap();
    conn.execute("DELETE FROM pso_cache_files WHERE id = ?", [file_id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::open_in_memory;

    fn seed(db: &Db) -> (i64, i64) {
        let conn = db.lock().unwrap();
        conn.execute("INSERT INTO machines (hostname, ip) VALUES ('h', '1.1.1.1')", []).unwrap();
        conn.execute("INSERT INTO projects (uproject_name, uproject_stem_lower) VALUES ('X.uproject', 'x')", []).unwrap();
        (1, 1)
    }

    #[test]
    fn upsert_idempotent_on_unique_key() {
        let db = open_in_memory().unwrap();
        let (mid, pid) = seed(&db);
        let f = PsoCacheFile {
            id: None, project_id: pid, source_machine_id: mid,
            file_path: "C:\\X\\Saved\\CollectedPSOs\\Plurality.upipelinecache".into(),
            file_name: "Plurality.upipelinecache".into(),
            size_bytes: 1024, gpu_signature: "nvidia:RTX 3080:535.98".into(),
            ue_version: Some("5.4.4".into()), collected_at: None,
        };
        let id1 = upsert(&db, &f).unwrap();
        let id2 = upsert(&db, &f).unwrap();
        assert_eq!(id1, id2);
    }

    #[test]
    fn list_by_project_returns_only_matching() {
        let db = open_in_memory().unwrap();
        let (mid, pid) = seed(&db);
        upsert(&db, &PsoCacheFile {
            id: None, project_id: pid, source_machine_id: mid,
            file_path: "p1".into(), file_name: "a.upipelinecache".into(),
            size_bytes: 0, gpu_signature: "x:y:z".into(),
            ue_version: None, collected_at: None,
        }).unwrap();
        let r = list_by_project(&db, pid).unwrap();
        assert_eq!(r.len(), 1);
    }
}
```

- [ ] **Step 2: `pso_distributions.rs`**

```rust
//! CRUD for the `pso_distributions` table.

use crate::data::Db;
use crate::error::UecmResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DistributionStatus {
    Pending,
    Running,
    Ok,
    Err,
    Cancelled,
}

impl DistributionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Ok => "ok",
            Self::Err => "err",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsoDistribution {
    pub id: Option<i64>,
    pub pso_cache_file_id: i64,
    pub target_machine_id: i64,
    pub status: DistributionStatus,
    pub bytes_copied: i64,
    pub distributed_at: Option<String>,
    pub error_message: Option<String>,
    pub created_at: Option<String>,
}

pub fn upsert(db: &Db, d: &PsoDistribution) -> UecmResult<i64> {
    let conn = db.lock().unwrap();
    conn.execute(
        "INSERT INTO pso_distributions (pso_cache_file_id, target_machine_id, status, bytes_copied, distributed_at, error_message)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(pso_cache_file_id, target_machine_id) DO UPDATE SET
           status = excluded.status,
           bytes_copied = excluded.bytes_copied,
           distributed_at = excluded.distributed_at,
           error_message = excluded.error_message",
        rusqlite::params![
            d.pso_cache_file_id, d.target_machine_id, d.status.as_str(),
            d.bytes_copied, d.distributed_at, d.error_message,
        ],
    )?;
    let id: i64 = conn.query_row(
        "SELECT id FROM pso_distributions WHERE pso_cache_file_id = ? AND target_machine_id = ?",
        rusqlite::params![d.pso_cache_file_id, d.target_machine_id],
        |r| r.get(0),
    )?;
    Ok(id)
}

pub fn list_for_file(db: &Db, file_id: i64) -> UecmResult<Vec<PsoDistribution>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, pso_cache_file_id, target_machine_id, status, bytes_copied, distributed_at, error_message, created_at
         FROM pso_distributions WHERE pso_cache_file_id = ? ORDER BY target_machine_id",
    )?;
    let rows = stmt
        .query_map([file_id], |r| {
            let s: String = r.get(3)?;
            let status = match s.as_str() {
                "pending" => DistributionStatus::Pending,
                "running" => DistributionStatus::Running,
                "ok" => DistributionStatus::Ok,
                "err" => DistributionStatus::Err,
                "cancelled" => DistributionStatus::Cancelled,
                _ => DistributionStatus::Pending,
            };
            Ok(PsoDistribution {
                id: Some(r.get(0)?),
                pso_cache_file_id: r.get(1)?,
                target_machine_id: r.get(2)?,
                status,
                bytes_copied: r.get(4)?,
                distributed_at: r.get(5)?,
                error_message: r.get(6)?,
                created_at: r.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::open_in_memory;

    fn seed(db: &Db) -> i64 {
        let conn = db.lock().unwrap();
        conn.execute("INSERT INTO machines (hostname, ip) VALUES ('h', '1.1.1.1')", []).unwrap();
        conn.execute("INSERT INTO projects (uproject_name, uproject_stem_lower) VALUES ('X.uproject', 'x')", []).unwrap();
        conn.execute(
            "INSERT INTO pso_cache_files (project_id, source_machine_id, file_path, file_name, size_bytes, gpu_signature) \
             VALUES (1, 1, 'p', 'a.upipelinecache', 0, 'x:y:z')",
            [],
        ).unwrap();
        1
    }

    #[test]
    fn upsert_status_change_persists() {
        let db = open_in_memory().unwrap();
        let fid = seed(&db);
        let _ = upsert(&db, &PsoDistribution {
            id: None, pso_cache_file_id: fid, target_machine_id: 1,
            status: DistributionStatus::Running, bytes_copied: 0,
            distributed_at: None, error_message: None, created_at: None,
        }).unwrap();
        let _ = upsert(&db, &PsoDistribution {
            id: None, pso_cache_file_id: fid, target_machine_id: 1,
            status: DistributionStatus::Ok, bytes_copied: 1024,
            distributed_at: Some("2026-05-03".into()), error_message: None, created_at: None,
        }).unwrap();
        let rows = list_for_file(&db, fid).unwrap();
        assert_eq!(rows[0].status, DistributionStatus::Ok);
        assert_eq!(rows[0].bytes_copied, 1024);
    }
}
```

- [ ] **Step 3: Wire into `data/mod.rs`**

```rust
pub mod pso_cache_files;
pub mod pso_distributions;
pub use pso_cache_files::PsoCacheFile;
pub use pso_distributions::{DistributionStatus, PsoDistribution};
```

- [ ] **Step 4: Run + commit**

```bash
cd src-tauri && cargo test --lib data::pso_cache_files data::pso_distributions 2>&1 | tail -10 && cd ..
git add src-tauri/src/data/pso_cache_files.rs src-tauri/src/data/pso_distributions.rs src-tauri/src/data/mod.rs
git commit -m "feat: data layer for pso_cache_files + pso_distributions"
```

---

## Task 4: F1 — Add R008-R010 PSO CVar rules to `core::ini_diagnostics`

**Files:**
- Modify: `src-tauri/src/core/ini_diagnostics.rs` (extend rule engine)

This task assumes Plan 4 shipped a rule engine where each rule is a function returning `Option<Finding>` (or similar). Adapt the exact form to what's there.

- [ ] **Step 1: Add rule functions**

Append to `core::ini_diagnostics`:

```rust
/// R008: ConsoleVariables.ini missing or has r.PSOPrecaching != 1
pub fn rule_r008_pso_precaching(
    file_path: &str,
    section: &str,
    kvs: &[(String, String)],
) -> Option<Finding> {
    if !file_path.to_lowercase().ends_with("consolevariables.ini") {
        return None;
    }
    if !section.eq_ignore_ascii_case("ConsoleVariables") {
        return None;
    }
    let val = kvs
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("r.PSOPrecaching"))
        .map(|(_, v)| v.trim());
    match val {
        Some("1") => None,
        Some(other) => Some(Finding {
            rule_id: "R008".into(),
            severity: Severity::Critical,
            file_path: file_path.into(),
            section: section.into(),
            key_name: "r.PSOPrecaching".into(),
            current_value: Some(other.into()),
            recommended_value: "1".into(),
            recommended_action: ApplyAction::Set,
            message: "PSO Precaching disabled — every level switch can stutter.".into(),
        }),
        None => Some(Finding {
            rule_id: "R008".into(),
            severity: Severity::Critical,
            file_path: file_path.into(),
            section: section.into(),
            key_name: "r.PSOPrecaching".into(),
            current_value: None,
            recommended_value: "1".into(),
            recommended_action: ApplyAction::Set,
            message: "PSO Precaching CVar missing — defaults to off in some UE versions.".into(),
        }),
    }
}

/// R009: r.PSOPrecache.Compile defaults to 0 in some versions; recommend explicit 1.
pub fn rule_r009_pso_precache_compile(
    file_path: &str,
    section: &str,
    kvs: &[(String, String)],
) -> Option<Finding> {
    if !file_path.to_lowercase().ends_with("consolevariables.ini") {
        return None;
    }
    if !section.eq_ignore_ascii_case("ConsoleVariables") {
        return None;
    }
    let val = kvs
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("r.PSOPrecache.Compile"))
        .map(|(_, v)| v.trim());
    match val {
        Some("1") => None,
        _ => Some(Finding {
            rule_id: "R009".into(),
            severity: Severity::Warning,
            file_path: file_path.into(),
            section: section.into(),
            key_name: "r.PSOPrecache.Compile".into(),
            current_value: val.map(String::from),
            recommended_value: "1".into(),
            recommended_action: ApplyAction::Set,
            message: "PSO Precache compile not enabled — UE will fall back to runtime compile.".into(),
        }),
    }
}

/// R010: r.PSOPrecache.GlobalShaders should be enabled.
pub fn rule_r010_pso_precache_globals(
    file_path: &str,
    section: &str,
    kvs: &[(String, String)],
) -> Option<Finding> {
    if !file_path.to_lowercase().ends_with("consolevariables.ini") {
        return None;
    }
    if !section.eq_ignore_ascii_case("ConsoleVariables") {
        return None;
    }
    let val = kvs
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("r.PSOPrecache.GlobalShaders"))
        .map(|(_, v)| v.trim());
    match val {
        Some("1") => None,
        _ => Some(Finding {
            rule_id: "R010".into(),
            severity: Severity::Warning,
            file_path: file_path.into(),
            section: section.into(),
            key_name: "r.PSOPrecache.GlobalShaders".into(),
            current_value: val.map(String::from),
            recommended_value: "1".into(),
            recommended_action: ApplyAction::Set,
            message: "PSO Precache for global shaders not enabled.".into(),
        }),
    }
}
```

- [ ] **Step 2: Wire rules into the dispatcher**

In whatever function aggregates rules per `(file, section)` (Plan 4's `core::ini_diagnostics::run_all_rules` or equivalent), append calls to the three new functions and push their findings into the returned `Vec<Finding>`.

- [ ] **Step 3: Tests**

Append to `core::ini_diagnostics::tests`:

```rust
    #[test]
    fn r008_flags_missing_pso_precaching() {
        let kvs: Vec<(String, String)> = vec![];
        let f = rule_r008_pso_precaching("C:\\X\\Config\\ConsoleVariables.ini", "ConsoleVariables", &kvs);
        assert!(f.is_some());
        let f = f.unwrap();
        assert_eq!(f.rule_id, "R008");
        assert_eq!(f.severity, Severity::Critical);
    }

    #[test]
    fn r008_passes_when_enabled() {
        let kvs = vec![("r.PSOPrecaching".to_string(), "1".to_string())];
        let f = rule_r008_pso_precaching("C:\\X\\Config\\ConsoleVariables.ini", "ConsoleVariables", &kvs);
        assert!(f.is_none());
    }

    #[test]
    fn r009_warning_when_compile_off() {
        let kvs = vec![("r.PSOPrecache.Compile".to_string(), "0".to_string())];
        let f = rule_r009_pso_precache_compile("C:\\X\\Config\\ConsoleVariables.ini", "ConsoleVariables", &kvs);
        assert!(f.is_some());
        assert_eq!(f.unwrap().severity, Severity::Warning);
    }

    #[test]
    fn r010_warning_when_globals_off() {
        let kvs: Vec<(String, String)> = vec![];
        let f = rule_r010_pso_precache_globals("C:\\X\\Config\\ConsoleVariables.ini", "ConsoleVariables", &kvs);
        assert!(f.is_some());
    }

    #[test]
    fn pso_rules_skip_non_console_variables_files() {
        let kvs: Vec<(String, String)> = vec![];
        assert!(rule_r008_pso_precaching("C:\\X\\Config\\DefaultEngine.ini", "ConsoleVariables", &kvs).is_none());
    }
```

- [ ] **Step 4: Run + commit**

```bash
cd src-tauri && cargo test --lib core::ini_diagnostics 2>&1 | tail -10 && cd ..
git add src-tauri/src/core/ini_diagnostics.rs
git commit -m "feat: ini_diagnostics rules R008-R010 (PSO Precaching CVars)"
```

---

## Task 5: F1 — Tauri command `verify_pso_precaching`

**Files:**
- Modify: `src-tauri/src/commands/ini_scanner.rs` (or wherever Plan 4 placed the scan commands)
- Modify: `src-tauri/src/lib.rs`

This is a thin wrapper command — it triggers the existing `scan_inis` flow but constrains the scope to `ConsoleVariables.ini` and returns only R008-R010 findings. Plan 4's scanner already returns all R-IDs; the wrapper filters in the response.

- [ ] **Step 1: Add the command**

```rust
#[tauri::command]
pub fn verify_pso_precaching(
    db: State<'_, Db>,
    machine_ids: Vec<i64>,
    project_paths: Vec<String>,
    operator_credential_alias: Option<String>,
) -> UecmResult<Vec<crate::data::ini_findings::IniFinding>> {
    // Trigger the standard scan for the given machines/paths.
    let scan_run_id = crate::commands::ini_scanner::scan_inis_inner(
        &db, &machine_ids, &project_paths, operator_credential_alias.as_deref(),
    )?;
    let all = crate::data::ini_findings::list_by_run(&db, scan_run_id)?;
    Ok(all
        .into_iter()
        .filter(|f| matches!(f.rule_id.as_str(), "R008" | "R009" | "R010"))
        .collect())
}
```

(The exact parameter shape depends on Plan 4's `scan_inis` API. Adapt as needed — the contract is "scan, then filter to PSO rule IDs".)

- [ ] **Step 2: Register in `lib.rs`**

```rust
            commands::ini_scanner::verify_pso_precaching,
```

- [ ] **Step 3: Build + commit**

```bash
cd src-tauri && cargo build --lib 2>&1 | tail -3 && cd ..
git add src-tauri/src/commands/ini_scanner.rs src-tauri/src/lib.rs
git commit -m "feat: tauri command verify_pso_precaching (R008-R010 filter)"
```

---

## Task 6: F4 — `core::gpu_consistency` aggregator

**Files:**
- Create: `src-tauri/src/core/gpu_consistency.rs`
- Modify: `src-tauri/src/core/mod.rs`

- [ ] **Step 1: Write the module**

```rust
//! GPU/driver consistency aggregator. Pure-Rust; takes the persisted
//! machine_gpus rows and returns a matrix the UI consumes directly.

use crate::data::Db;
use crate::error::UecmResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct GpuSignature {
    pub vendor: String,
    pub model: String,
    pub driver: String,
}

impl GpuSignature {
    pub fn as_string(&self) -> String {
        format!("{}:{}:{}", self.vendor.to_lowercase(), self.model, self.driver)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CellStatus {
    Match,
    Deviation,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineGpuCell {
    pub machine_id: i64,
    pub hostname: String,
    pub signature: Option<GpuSignature>,
    pub status: CellStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuMatrix {
    pub signatures: Vec<(GpuSignature, i64)>, // (signature, count)
    pub baseline: Option<GpuSignature>,
    pub cells: Vec<MachineGpuCell>,
}

pub fn build_matrix(db: &Db) -> UecmResult<GpuMatrix> {
    let conn = db.lock().unwrap();
    // First pass: machine list.
    let mut stmt = conn.prepare("SELECT id, hostname FROM machines ORDER BY hostname")?;
    let machines: Vec<(i64, String)> = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
        .collect::<Result<_, _>>()?;
    drop(stmt);

    // Second pass: latest GPU row per machine (single GPU assumed; many-GPU machines pick first).
    let mut sig_stmt = conn.prepare(
        "SELECT machine_id, gpu_model, driver_version, vendor FROM machine_gpus
         GROUP BY machine_id ORDER BY MAX(detected_at) DESC",
    )?;
    let rows: Vec<(i64, String, String, String)> = sig_stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?
        .collect::<Result<_, _>>()?;
    drop(sig_stmt);

    let mut by_machine: std::collections::HashMap<i64, GpuSignature> = std::collections::HashMap::new();
    for (mid, model, driver, vendor) in rows {
        by_machine.insert(mid, GpuSignature { vendor, model, driver });
    }

    // Count signatures.
    let mut counts: std::collections::HashMap<GpuSignature, i64> = std::collections::HashMap::new();
    for sig in by_machine.values() {
        *counts.entry(sig.clone()).or_insert(0) += 1;
    }
    let mut sig_list: Vec<(GpuSignature, i64)> = counts.into_iter().collect();
    sig_list.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.as_string().cmp(&b.0.as_string())));
    let baseline = sig_list.first().map(|(s, _)| s.clone());

    let cells: Vec<MachineGpuCell> = machines
        .into_iter()
        .map(|(mid, hostname)| {
            let sig = by_machine.get(&mid).cloned();
            let status = match (&sig, &baseline) {
                (None, _) => CellStatus::Unknown,
                (Some(s), Some(base)) if s == base => CellStatus::Match,
                _ => CellStatus::Deviation,
            };
            MachineGpuCell { machine_id: mid, hostname, signature: sig, status }
        })
        .collect();

    Ok(GpuMatrix { signatures: sig_list, baseline, cells })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::open_in_memory;

    fn seed(db: &Db, hostname: &str, ip: &str, model: &str, driver: &str, vendor: &str) {
        let conn = db.lock().unwrap();
        conn.execute("INSERT INTO machines (hostname, ip) VALUES (?, ?)", rusqlite::params![hostname, ip]).unwrap();
        let mid = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO machine_gpus (machine_id, gpu_model, driver_version, vendor, vram_mb) \
             VALUES (?, ?, ?, ?, 10000)",
            rusqlite::params![mid, model, driver, vendor],
        ).unwrap();
    }

    #[test]
    fn baseline_picks_majority_signature() {
        let db = open_in_memory().unwrap();
        seed(&db, "A", "1.1.1.1", "RTX 3080", "535.98", "nvidia");
        seed(&db, "B", "2.2.2.2", "RTX 3080", "535.98", "nvidia");
        seed(&db, "C", "3.3.3.3", "RTX 4090", "560.00", "nvidia");
        let m = build_matrix(&db).unwrap();
        let base = m.baseline.unwrap();
        assert_eq!(base.model, "RTX 3080");
        assert_eq!(m.cells.iter().filter(|c| c.status == CellStatus::Match).count(), 2);
        assert_eq!(m.cells.iter().filter(|c| c.status == CellStatus::Deviation).count(), 1);
    }

    #[test]
    fn machine_without_gpu_row_is_unknown() {
        let db = open_in_memory().unwrap();
        seed(&db, "A", "1.1.1.1", "RTX 3080", "535.98", "nvidia");
        // Machine B with no GPU row
        {
            let conn = db.lock().unwrap();
            conn.execute("INSERT INTO machines (hostname, ip) VALUES ('B', '2.2.2.2')", []).unwrap();
        }
        let m = build_matrix(&db).unwrap();
        let b = m.cells.iter().find(|c| c.hostname == "B").unwrap();
        assert_eq!(b.status, CellStatus::Unknown);
    }

    #[test]
    fn empty_db_returns_empty_matrix() {
        let db = open_in_memory().unwrap();
        let m = build_matrix(&db).unwrap();
        assert!(m.signatures.is_empty());
        assert!(m.baseline.is_none());
        assert!(m.cells.is_empty());
    }
}
```

- [ ] **Step 2: Wire into `core/mod.rs`**

```rust
pub mod gpu_consistency;
```

- [ ] **Step 3: Run + commit**

```bash
cd src-tauri && cargo test --lib core::gpu_consistency 2>&1 | tail -10 && cd ..
git add src-tauri/src/core/gpu_consistency.rs src-tauri/src/core/mod.rs
git commit -m "feat: core::gpu_consistency aggregator (signatures + baseline + cells)"
```

---

## Task 7: F4 — Tauri command `get_gpu_consistency_matrix`

**Files:**
- Create: `src-tauri/src/commands/gpu_consistency.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write the command**

```rust
use crate::core::gpu_consistency::{self, GpuMatrix};
use crate::data::Db;
use crate::error::UecmResult;
use tauri::State;

#[tauri::command]
pub fn get_gpu_consistency_matrix(db: State<'_, Db>) -> UecmResult<GpuMatrix> {
    gpu_consistency::build_matrix(&db)
}
```

- [ ] **Step 2: Wire into `commands/mod.rs`**

```rust
pub mod gpu_consistency;
```

- [ ] **Step 3: Register in `lib.rs`**

```rust
            commands::gpu_consistency::get_gpu_consistency_matrix,
```

- [ ] **Step 4: Build + commit**

```bash
cd src-tauri && cargo build --lib 2>&1 | tail -3 && cd ..
git add src-tauri/src/commands/gpu_consistency.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat: tauri command get_gpu_consistency_matrix"
```

---

## Task 8: PowerShell — `start-pso-collect.ps1` + `list-pso-cache-files.ps1`

**Files:**
- Create: `ps-scripts/start-pso-collect.ps1`
- Create: `ps-scripts/list-pso-cache-files.ps1`

PSO collection re-uses `start-ue-process.ps1` (Plan 5) for the actual spawn. The `start-pso-collect.ps1` script is a thin convenience wrapper that pre-builds the `-ExecCmds` argument string and calls `start-ue-process.ps1` internally — but since Rust composes args from primitive parts anyway, in practice we just compose the args in Rust and reuse `start-ue-process.ps1`. So Plan 6 only needs the file enumerator.

We DO add a thin "compose the args" helper inside `core::pso_collect` (no new PS file for `start-pso-collect`).

- [ ] **Step 1: `list-pso-cache-files.ps1`**

```powershell
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
        $dir = Join-Path -Path $ProjectDir -ChildPath 'Saved\CollectedPSOs'
        if (-not (Test-Path -LiteralPath $dir)) { return ,@() }
        $files = Get-ChildItem -LiteralPath $dir -File -ErrorAction SilentlyContinue | Where-Object {
            $_.Extension -eq '.upipelinecache' -or $_.Extension -eq '.csv'
        }
        $out = @()
        foreach ($f in $files) {
            $out += @{
                file_path = "$($f.FullName)"
                file_name = "$($f.Name)"
                size = "$($f.Length)"
                last_write = "$($f.LastWriteTimeUtc.ToString('o'))"
            }
        }
        return ,$out
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

    @{ ok = $true; items = @($r); count = (@($r)).Count } | ConvertTo-Json -Depth 6 -Compress
}
catch {
    @{ ok = $false; items = @(); count = 0; message = "$($_.Exception.Message)" } | ConvertTo-Json -Depth 6 -Compress
    exit 1
}
```

- [ ] **Step 2: Commit**

```bash
git add ps-scripts/list-pso-cache-files.ps1
git commit -m "feat(ps): list-pso-cache-files.ps1 enumerator"
```

---

## Task 9: PowerShell — `distribute-pso-cache.ps1`

**Files:**
- Create: `ps-scripts/distribute-pso-cache.ps1`

Same shape as `distribute-pak-file.ps1` (Plan 5 T13) but with a different file glob.

- [ ] **Step 1: Write the script**

**Mirrors `distribute-pak-file.ps1` design**: WinRM second-hop SMB requires explicit credential mapping inside the target session via `New-PSDrive`. See Plan 5 T13 commentary.

```powershell
param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$SourceUnc,
    [Parameter(Mandatory=$true)] [string]$TargetLocal,
    [Parameter(Mandatory=$true)] [string]$FileName,    # the specific .upipelinecache or .csv to copy
    [string]$Username,                                  # WinRM (target) credential
    [string]$Password,
    [string]$SourceSmbUser,                             # SMB credential for SourceUnc
    [string]$SourceSmbPass,
    [switch]$PreflightOnly
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
        param($SourceUnc, $TargetLocal, $FileName, $SmbUser, $SmbPass, $PreflightOnly)
        if (-not (Test-Path -LiteralPath $TargetLocal)) {
            New-Item -Path $TargetLocal -ItemType Directory -Force | Out-Null
        }
        $driveName = "uecmpso$([System.Diagnostics.Process]::GetCurrentProcess().Id)"
        $cred = $null
        if (-not [string]::IsNullOrEmpty($SmbUser) -and -not [string]::IsNullOrEmpty($SmbPass)) {
            $secure = ConvertTo-SecureString -String $SmbPass -AsPlainText -Force
            $cred = New-Object System.Management.Automation.PSCredential($SmbUser, $secure)
        }
        $mounted = $false
        try {
            if ($cred) {
                New-PSDrive -Name $driveName -PSProvider FileSystem -Root $SourceUnc -Credential $cred -ErrorAction Stop | Out-Null
                $mounted = $true
            }
            if (-not (Test-Path -LiteralPath $SourceUnc)) {
                throw "source UNC unreachable from target session: $SourceUnc"
            }
            if ($PreflightOnly) {
                return @{ ok = $true; exit_code = "0"; bytes_copied = "0"; stdout_tail = "preflight ok" }
            }
            $args = @(
                "$SourceUnc",
                "$TargetLocal",
                "$FileName",
                '/R:3',
                '/W:5',
                '/NP',
                '/NDL',
                '/NJH',
                '/NJS',
                '/BYTES'
            )
            $proc = Start-Process -FilePath 'robocopy.exe' -ArgumentList $args -PassThru -Wait -NoNewWindow -RedirectStandardOutput "$env:TEMP\rb-pso-$PID.log" -RedirectStandardError "$env:TEMP\rb-pso-err-$PID.log"
            $code = $proc.ExitCode
            $stdout = Get-Content -LiteralPath "$env:TEMP\rb-pso-$PID.log" -Raw -ErrorAction SilentlyContinue
            Remove-Item -LiteralPath "$env:TEMP\rb-pso-$PID.log" -ErrorAction SilentlyContinue
            Remove-Item -LiteralPath "$env:TEMP\rb-pso-err-$PID.log" -ErrorAction SilentlyContinue
            $bytes = 0
            try {
                $m = [regex]::Matches($stdout, 'Bytes\s*:\s*(\d+)')
                if ($m.Count -gt 0) { $bytes = [long]$m[0].Groups[1].Value }
            } catch {}
            return @{
                exit_code = "$code"
                ok = ($code -lt 8)
                bytes_copied = "$bytes"
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
        ArgumentList = @($SourceUnc, $TargetLocal, $FileName, $SourceSmbUser, $SourceSmbPass, [bool]$PreflightOnly)
        ErrorAction  = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $r = Invoke-Command @invokeArgs

    @{
        ok = "$($r.ok)" -eq "True"
        exit_code = "$($r.exit_code)"
        bytes_copied = "$($r.bytes_copied)"
        stdout_tail = "$($r.stdout_tail)"
    } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; exit_code = "-1"; bytes_copied = "0"; message = "$($_.Exception.Message)" } | ConvertTo-Json -Compress
    exit 1
}
```

- [ ] **Step 2: Commit**

```bash
git add ps-scripts/distribute-pso-cache.ps1
git commit -m "feat(ps): distribute-pso-cache.ps1 (Robocopy single file)"
```

---

## Task 10: Rust `core::pso_collect` — collection orchestration

**Files:**
- Create: `src-tauri/src/core/pso_collect.rs`
- Modify: `src-tauri/src/core/mod.rs`

- [ ] **Step 1: Write the module**

```rust
//! PSO Cache file collection — composes UE -game mode args, hands to
//! core::ue_runner, then enumerates the resulting CollectedPSOs/ directory.

use crate::core::powershell;
use crate::core::ue_runner::{self, UeRunSpec, UeRunnerBackend};
use crate::data::{
    machine_gpus, machines as data_machines,
    pso_cache_files::{self, PsoCacheFile},
    project_locations, Db,
};
use crate::error::{UecmError, UecmResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsoCollectSpec {
    pub project_id: i64,
    pub source_machine_id: i64,
    pub ue_version: Option<String>,
    pub resolution: (u32, u32),       // (1920, 1080) default
    pub windowed: bool,
    pub max_minutes: u32,             // soft watchdog
}

impl Default for PsoCollectSpec {
    fn default() -> Self {
        Self {
            project_id: 0,
            source_machine_id: 0,
            ue_version: None,
            resolution: (1920, 1080),
            windowed: true,
            max_minutes: 10,
        }
    }
}

/// Compose the UE -game mode command-line arguments for PSO collection.
/// Returns one argv entry per Vec element. The `-ExecCmds=...` element
/// MUST stay as one entry — its inner space-separated CVar list would be
/// shredded by any caller that splits on whitespace.
pub fn build_ue_args(spec: &PsoCollectSpec) -> Vec<String> {
    let (w, h) = spec.resolution;
    let mut args: Vec<String> = vec!["-game".into()];
    if spec.windowed {
        args.push("-windowed".into());
    }
    args.push(format!("-resx={}", w));
    args.push(format!("-resy={}", h));
    args.push("-log".into());
    args.push("-unattended".into());
    // Single argv entry — UE parses the inner string itself.
    args.push(
        "-ExecCmds=r.ShaderPipelineCache.Enabled 1; r.ShaderPipelineCache.LogPSO 1; r.PSO.WarmingTime 0"
            .into(),
    );
    args
}

#[derive(Debug, Deserialize)]
struct ListItemRaw {
    file_path: String,
    file_name: String,
    size: String,
    #[serde(default)]
    last_write: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListScriptResult {
    ok: bool,
    items: Vec<ListItemRaw>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnumeratedFile {
    pub file_path: String,
    pub file_name: String,
    pub size_bytes: i64,
}

pub fn enumerate_remote(
    host: &str,
    project_dir: &str,
    user: Option<&str>,
    pass: Option<&str>,
) -> UecmResult<Vec<EnumeratedFile>> {
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
    let r: ListScriptResult = powershell::run_json(
        &powershell::script_path("list-pso-cache-files.ps1"),
        &args_ref,
    )?;
    if !r.ok {
        return Err(UecmError::OperationFailed(
            r.message.unwrap_or_else(|| "list failed".into()),
        ));
    }
    Ok(r.items
        .into_iter()
        .map(|it| EnumeratedFile {
            file_path: it.file_path,
            file_name: it.file_name,
            size_bytes: it.size.parse().unwrap_or(0),
        })
        .collect())
}

pub fn gpu_signature_for_machine(db: &Db, machine_id: i64) -> UecmResult<String> {
    let rows = machine_gpus::list_for_machine(db, machine_id)?;
    let first = rows.first().ok_or_else(|| {
        UecmError::InvalidInput(format!("machine {} has no GPU rows", machine_id))
    })?;
    Ok(format!(
        "{}:{}:{}",
        first.vendor.to_lowercase(),
        first.gpu_model,
        first.driver_version
    ))
}

/// Persist enumerated files into pso_cache_files. Caller has already verified
/// the UE exited cleanly (or hit the watchdog).
pub fn finalize_persist(
    db: &Db,
    project_id: i64,
    source_machine_id: i64,
    ue_version: Option<&str>,
    files: &[EnumeratedFile],
) -> UecmResult<Vec<i64>> {
    let signature = gpu_signature_for_machine(db, source_machine_id)?;
    let mut ids = Vec::with_capacity(files.len());
    for f in files {
        let row = PsoCacheFile {
            id: None,
            project_id,
            source_machine_id,
            file_path: f.file_path.clone(),
            file_name: f.file_name.clone(),
            size_bytes: f.size_bytes,
            gpu_signature: signature.clone(),
            ue_version: ue_version.map(String::from),
            collected_at: None,
        };
        ids.push(pso_cache_files::upsert(db, &row)?);
    }
    Ok(ids)
}

pub fn launch_collection(
    backend: UeRunnerBackend,
    host: &str,
    engine_path: &str,
    project_path: &str,
    spec: &PsoCollectSpec,
    user: Option<&str>,
    pass: Option<&str>,
) -> ue_runner::RunnerHandle {
    let runspec = UeRunSpec {
        backend,
        host: host.to_string(),
        engine_path: engine_path.to_string(),
        project_path: project_path.to_string(),
        extra_args: build_ue_args(spec),
        credential_user: user.map(String::from),
        credential_pass: pass.map(String::from),
    };
    ue_runner::run(runspec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_ue_args_includes_resolution_and_exec_cmds() {
        let spec = PsoCollectSpec {
            project_id: 1, source_machine_id: 1, ue_version: None,
            resolution: (1920, 1080), windowed: true, max_minutes: 10,
        };
        let args = build_ue_args(&spec);
        assert!(args.iter().any(|a| a == "-game"));
        assert!(args.iter().any(|a| a == "-resx=1920"));
        assert!(args.iter().any(|a| a == "-resy=1080"));
        assert!(args.iter().any(|a| a == "-windowed"));
    }

    #[test]
    fn build_ue_args_keeps_exec_cmds_atomic() {
        // Critical: the ExecCmds string contains spaces; it MUST be a single
        // argv entry, not split into multiple. Otherwise UE 5 receives
        // garbage CVars and PSO collection silently emits zero PSOs.
        let spec = PsoCollectSpec::default();
        let args = build_ue_args(&spec);
        let exec = args.iter().find(|a| a.starts_with("-ExecCmds=")).expect("ExecCmds present");
        assert!(exec.contains("r.ShaderPipelineCache.Enabled 1"));
        assert!(exec.contains("r.ShaderPipelineCache.LogPSO 1"));
        assert!(exec.contains("r.PSO.WarmingTime 0"));
    }

    #[test]
    fn build_ue_args_skips_windowed_when_false() {
        let mut spec = PsoCollectSpec::default();
        spec.windowed = false;
        let args = build_ue_args(&spec);
        assert!(!args.iter().any(|a| a == "-windowed"));
    }

    #[test]
    fn build_ue_args_supports_low_res() {
        let mut spec = PsoCollectSpec::default();
        spec.resolution = (320, 240);
        let args = build_ue_args(&spec);
        assert!(args.iter().any(|a| a == "-resx=320"));
        assert!(args.iter().any(|a| a == "-resy=240"));
    }

    #[cfg(not(windows))]
    #[test]
    fn enumerate_remote_returns_powershell_error_on_non_windows() {
        let r = enumerate_remote("h", "C:\\X", Some("u"), Some("p"));
        assert!(matches!(r, Err(UecmError::PowerShell(_)) | Err(UecmError::OperationFailed(_))));
    }

    #[tokio::test(start_paused = true)]
    async fn watchdog_flips_cancel_after_timeout() {
        use crate::core::ue_runner::RunnerCancel;
        use std::sync::Arc;
        use tokio::sync::Mutex;

        let cancel = Arc::new(Mutex::new(RunnerCancel::default()));
        // 2-minute timeout; advance 119s and assert NOT yet flipped, then 2s and assert flipped.
        spawn_watchdog(cancel.clone(), 2, "t1".into());
        tokio::time::advance(std::time::Duration::from_secs(119)).await;
        assert!(!cancel.lock().await.requested, "should not fire before timeout");
        tokio::time::advance(std::time::Duration::from_secs(2)).await;
        // Yield once for the spawned task.
        tokio::task::yield_now().await;
        assert!(cancel.lock().await.requested, "watchdog should have flipped cancel");
    }

    #[tokio::test(start_paused = true)]
    async fn watchdog_zero_minutes_is_disabled() {
        use crate::core::ue_runner::RunnerCancel;
        use std::sync::Arc;
        use tokio::sync::Mutex;

        let cancel = Arc::new(Mutex::new(RunnerCancel::default()));
        // Caller is expected to skip the helper when max_minutes==0; here we
        // just verify the helper itself respects 0 by NOT spawning a fire.
        spawn_watchdog(cancel.clone(), 0, "t-zero".into());
        tokio::time::advance(std::time::Duration::from_secs(86_400)).await;
        tokio::task::yield_now().await;
        assert!(!cancel.lock().await.requested);
    }
}

/// Schedule a one-shot watchdog that flips `cancel.requested` after
/// `max_minutes` real-clock minutes (or paused-time minutes under tokio test).
/// `max_minutes == 0` is a no-op.
pub fn spawn_watchdog(
    cancel: std::sync::Arc<tokio::sync::Mutex<crate::core::ue_runner::RunnerCancel>>,
    max_minutes: u32,
    job_id: String,
) {
    if max_minutes == 0 {
        return;
    }
    let timeout_secs = (max_minutes as u64).saturating_mul(60);
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(timeout_secs)).await;
        let mut c = cancel.lock().await;
        if !c.requested {
            c.requested = true;
            tracing::info!("pso watchdog fired for job {}", job_id);
        }
    });
}
```

- [ ] **Step 2: Wire into `core/mod.rs`**

```rust
pub mod pso_collect;
```

- [ ] **Step 3: Run + commit**

```bash
cd src-tauri && cargo test --lib core::pso_collect 2>&1 | tail -10 && cd ..
git add src-tauri/src/core/pso_collect.rs src-tauri/src/core/mod.rs
git commit -m "feat: core::pso_collect — UE -game args + remote enumerate + finalize"
```

---

## Task 11: Rust `core::pso_distribute` — wraps `pak_distribute`

**Files:**
- Create: `src-tauri/src/core/pso_distribute.rs`
- Modify: `src-tauri/src/core/mod.rs`

- [ ] **Step 1: Write the wrapper**

```rust
//! PSO Cache file distribution — same Robocopy mechanics as pak_distribute,
//! different source/target subdirectory and file-specific (not glob) copy.

use crate::core::powershell;
use crate::data::{
    pso_cache_files::{self, PsoCacheFile},
    project_locations, Db,
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
pub struct PsoDistributeOutcome {
    pub target_machine_id: i64,
    pub ok: bool,
    pub exit_code: i32,
    pub bytes_copied: i64,
    pub stdout_tail: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PsoDistributePlanItem {
    pub target_machine_id: i64,
    pub target_host: String,
    pub source_unc: String,
    pub target_local: String,
    pub file_name: String,
    /// WinRM (target) credential.
    pub credential_user: Option<String>,
    pub credential_pass: Option<String>,
    /// SMB credential the target uses to read source_unc (WinRM second-hop).
    pub source_smb_user: Option<String>,
    pub source_smb_pass: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub fn plan(
    db: &Db,
    file_id: i64,
    source_host: &str,
    file: &PsoCacheFile,
    target_machine_ids: &[i64],
    named_share_unc: Option<&str>,
    credential_user: Option<String>,
    credential_pass: Option<String>,
    source_smb_user: Option<String>,
    source_smb_pass: Option<String>,
) -> UecmResult<Vec<PsoDistributePlanItem>> {
    if target_machine_ids.is_empty() {
        return Err(UecmError::InvalidInput("no target machines".into()));
    }
    // Source dir is <abs_path>\Saved\CollectedPSOs
    let source_loc = project_locations::get_for_project_machine(db, file.project_id, file.source_machine_id)?
        .ok_or_else(|| UecmError::InvalidInput("source project location missing".into()))?;
    let source_unc = if let Some(s) = named_share_unc {
        format!("{}\\Saved\\CollectedPSOs", s.trim_end_matches('\\'))
    } else {
        let abs = source_loc.abs_path.replace('/', "\\");
        let drive = abs.chars().next().ok_or_else(|| {
            UecmError::InvalidInput("source abs_path missing drive".into())
        })?;
        let rest = &abs[2..]; // skip "X:"
        format!(
            "\\\\{}\\{}$\\{}\\Saved\\CollectedPSOs",
            source_host,
            drive,
            rest.trim_start_matches('\\')
        )
    };

    let mut out = Vec::with_capacity(target_machine_ids.len());
    for tid in target_machine_ids {
        if *tid == file.source_machine_id {
            continue;
        }
        let target_loc = project_locations::get_for_project_machine(db, file.project_id, *tid)?
            .ok_or_else(|| UecmError::InvalidInput(format!(
                "project {} has no location on target {}",
                file.project_id, tid
            )))?;
        let target_machine = crate::data::machines::get(db, *tid)?
            .ok_or_else(|| UecmError::InvalidInput(format!("target machine {} not found", tid)))?;
        let target_local = format!(
            "{}\\Saved\\CollectedPSOs",
            target_loc.abs_path.trim_end_matches('\\')
        );
        out.push(PsoDistributePlanItem {
            target_machine_id: *tid,
            target_host: target_machine.ip,
            source_unc: source_unc.clone(),
            target_local,
            file_name: file.file_name.clone(),
            credential_user: credential_user.clone(),
            credential_pass: credential_pass.clone(),
            source_smb_user: source_smb_user.clone(),
            source_smb_pass: source_smb_pass.clone(),
        });
    }
    let _ = file_id;
    Ok(out)
}

fn build_distribute_args(item: &PsoDistributePlanItem, preflight: bool) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "-HostName".into(),
        item.target_host.clone(),
        "-SourceUnc".into(),
        item.source_unc.clone(),
        "-TargetLocal".into(),
        item.target_local.clone(),
        "-FileName".into(),
        item.file_name.clone(),
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

pub async fn preflight_one(item: &PsoDistributePlanItem) -> UecmResult<()> {
    let args = build_distribute_args(item, true);
    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let r: DistributeRaw = powershell::run_json(
        &powershell::script_path("distribute-pso-cache.ps1"),
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

pub async fn run_one(item: PsoDistributePlanItem) -> UecmResult<PsoDistributeOutcome> {
    let args = build_distribute_args(&item, false);
    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let r: DistributeRaw = powershell::run_json(
        &powershell::script_path("distribute-pso-cache.ps1"),
        &args_ref,
    )?;
    let exit: i32 = r.exit_code.parse().unwrap_or(-1);
    let bytes: i64 = r.bytes_copied.parse().unwrap_or(0);
    Ok(PsoDistributeOutcome {
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
    use crate::data::open_in_memory;

    fn seed_db(db: &Db) -> i64 {
        let conn = db.lock().unwrap();
        conn.execute("INSERT INTO machines (hostname, ip) VALUES ('s', '1.1.1.1')", []).unwrap();
        conn.execute("INSERT INTO machines (hostname, ip) VALUES ('t', '2.2.2.2')", []).unwrap();
        conn.execute("INSERT INTO projects (uproject_name, uproject_stem_lower) VALUES ('X.uproject', 'x')", []).unwrap();
        conn.execute(
            "INSERT INTO project_locations (project_id, machine_id, abs_path, uproject_path) \
             VALUES (1, 1, 'D:\\Src\\X', 'D:\\Src\\X\\X.uproject')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO project_locations (project_id, machine_id, abs_path, uproject_path) \
             VALUES (1, 2, 'E:\\Tgt\\X', 'E:\\Tgt\\X\\X.uproject')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO pso_cache_files (project_id, source_machine_id, file_path, file_name, size_bytes, gpu_signature) \
             VALUES (1, 1, 'D:\\Src\\X\\Saved\\CollectedPSOs\\X.upipelinecache', 'X.upipelinecache', 12345, 'nvidia:RTX 3080:535.98')",
            [],
        ).unwrap();
        1
    }

    #[test]
    fn plan_skips_source_in_targets_and_builds_unc_correctly() {
        let db = open_in_memory().unwrap();
        let fid = seed_db(&db);
        let file = pso_cache_files::get(&db, fid).unwrap().unwrap();
        let p = plan(&db, fid, "1.1.1.1", &file, &[1, 2], None, None, None, None, None).unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].target_machine_id, 2);
        assert_eq!(p[0].source_unc, "\\\\1.1.1.1\\D$\\Src\\X\\Saved\\CollectedPSOs");
        assert_eq!(p[0].target_local, "E:\\Tgt\\X\\Saved\\CollectedPSOs");
        assert_eq!(p[0].file_name, "X.upipelinecache");
    }

    #[test]
    fn plan_uses_named_share_when_provided() {
        let db = open_in_memory().unwrap();
        let fid = seed_db(&db);
        let file = pso_cache_files::get(&db, fid).unwrap().unwrap();
        let p = plan(&db, fid, "1.1.1.1", &file, &[2], Some("\\\\HOST\\PSO"), None, None, None, None).unwrap();
        assert_eq!(p[0].source_unc, "\\\\HOST\\PSO\\Saved\\CollectedPSOs");
    }

    #[test]
    fn plan_rejects_empty_targets() {
        let db = open_in_memory().unwrap();
        let fid = seed_db(&db);
        let file = pso_cache_files::get(&db, fid).unwrap().unwrap();
        assert!(matches!(
            plan(&db, fid, "h", &file, &[], None, None, None, None, None),
            Err(UecmError::InvalidInput(_))
        ));
    }
}
```

- [ ] **Step 2: Wire into `core/mod.rs`**

```rust
pub mod pso_distribute;
```

- [ ] **Step 3: Run + commit**

```bash
cd src-tauri && cargo test --lib core::pso_distribute 2>&1 | tail -10 && cd ..
git add src-tauri/src/core/pso_distribute.rs src-tauri/src/core/mod.rs
git commit -m "feat: core::pso_distribute Robocopy fan-out for collected PSOs"
```

---

## Task 12: Tauri commands for PSO collect + distribute + list

**Files:**
- Create: `src-tauri/src/commands/pso.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write the commands**

```rust
//! PSO commands: start_pso_collection, list_pso_cache_files, distribute_pso_cache.
//! Cancellation reuses cancel_ue_job from Plan 5 (no new command).

use crate::commands::ddc_pak::UeJobRegistry;
use crate::core::batch;
use crate::core::pso_collect::{self, PsoCollectSpec};
use crate::core::pso_distribute::{self, PsoDistributePlanItem};
use crate::core::ue_runner::{UeRunnerBackend, UeRunnerEvent};
use crate::data::{
    machine_ue_installs, machines as data_machines,
    project_locations,
    pso_cache_files::{self, PsoCacheFile},
    Db,
};
use crate::error::{UecmError, UecmResult};
use serde::Serialize;
use std::sync::Arc;
use tauri::{Emitter, State};

fn resolve_engine_path(
    db: &Db,
    machine_id: i64,
    preferred_version: Option<&str>,
) -> UecmResult<String> {
    let installs = machine_ue_installs::list_for_machine(db, machine_id)?;
    if installs.is_empty() {
        return Err(UecmError::InvalidInput(format!(
            "machine {} has no UE installs",
            machine_id
        )));
    }
    if let Some(v) = preferred_version {
        return Ok(installs
            .into_iter()
            .find(|i| i.version == v)
            .ok_or_else(|| UecmError::InvalidInput(format!("UE {} not on machine", v)))?
            .install_path);
    }
    Ok(installs
        .iter()
        .find(|i| i.is_primary)
        .cloned()
        .unwrap_or_else(|| installs.into_iter().next().unwrap())
        .install_path)
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

#[derive(Debug, Serialize)]
pub struct PsoCollectJobResponse {
    pub job_id: String,
    pub source_machine_id: i64,
    pub project_id: i64,
}

#[tauri::command]
pub async fn start_pso_collection(
    app: tauri::AppHandle,
    db: State<'_, Db>,
    registry: State<'_, UeJobRegistry>,
    source_machine_id: i64,
    project_id: i64,
    ue_version: Option<String>,
    resolution_w: u32,
    resolution_h: u32,
    windowed: bool,
    max_minutes: u32,
    operator_credential_alias: Option<String>,
) -> UecmResult<PsoCollectJobResponse> {
    let machine = data_machines::get(&db, source_machine_id)?
        .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", source_machine_id)))?;
    let location = project_locations::get_for_project_machine(&db, project_id, source_machine_id)?
        .ok_or_else(|| {
            UecmError::InvalidInput(format!(
                "project {} not located on machine {}",
                project_id, source_machine_id
            ))
        })?;
    let engine_path = resolve_engine_path(&db, source_machine_id, ue_version.as_deref())?;
    let (op_user, op_pass) = resolve_operator_creds(&db, operator_credential_alias.as_deref())?;

    let spec = PsoCollectSpec {
        project_id,
        source_machine_id,
        ue_version: ue_version.clone(),
        resolution: (resolution_w, resolution_h),
        windowed,
        max_minutes,
    };
    let handle = pso_collect::launch_collection(
        UeRunnerBackend::Remote,
        &machine.ip,
        &engine_path,
        &location.uproject_path,
        &spec,
        op_user.as_deref(),
        op_pass.as_deref(),
    );

    let job_id = format!("pso-collect-{}-{}", source_machine_id, chrono::Utc::now().timestamp_millis());
    registry.insert(&job_id, handle.cancel.clone()).await;

    // ---- Watchdog. Without this, the only way to end a collection is
    // operator click-cancel; the spec promises automatic termination at
    // max_minutes and the helper below now actually delivers it.
    if max_minutes > 0 {
        pso_collect::spawn_watchdog(handle.cancel.clone(), max_minutes, job_id.clone());
    }

    let app_clone = app.clone();
    let job_id_clone = job_id.clone();
    let mid = source_machine_id;
    let pid = project_id;
    let mut events = handle.events;
    let project_dir = location.abs_path.clone();
    let host = machine.ip.clone();
    let user = op_user.clone();
    let pass = op_pass.clone();
    let ue_ver = ue_version.clone();
    let db_arc: Db = db.inner().clone();

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
            match ev {
                UeRunnerEvent::Completed { .. } | UeRunnerEvent::Cancelled => {
                    // Enumerate + persist
                    if let Ok(files) = pso_collect::enumerate_remote(
                        &host,
                        &project_dir,
                        user.as_deref(),
                        pass.as_deref(),
                    ) {
                        let _ = pso_collect::finalize_persist(
                            &db_arc,
                            pid,
                            mid,
                            ue_ver.as_deref(),
                            &files,
                        );
                        #[derive(Serialize)]
                        struct DonePayload<'a> {
                            job_id: &'a str,
                            source_machine_id: i64,
                            project_id: i64,
                            files_collected: usize,
                        }
                        let _ = app_clone.emit(
                            "pso-collect-finalized",
                            DonePayload {
                                job_id: &job_id_clone,
                                source_machine_id: mid,
                                project_id: pid,
                                files_collected: files.len(),
                            },
                        );
                    }
                    let registry_in_task: tauri::State<'_, UeJobRegistry> =
                        app_clone.state::<UeJobRegistry>();
                    registry_in_task.remove(&job_id_clone).await;
                    break;
                }
                UeRunnerEvent::Error { .. } => {
                    let registry_in_task: tauri::State<'_, UeJobRegistry> =
                        app_clone.state::<UeJobRegistry>();
                    registry_in_task.remove(&job_id_clone).await;
                    break;
                }
                _ => {}
            }
        }
    });

    Ok(PsoCollectJobResponse {
        job_id,
        source_machine_id,
        project_id,
    })
}

#[tauri::command]
pub fn list_pso_cache_files(
    db: State<'_, Db>,
    project_id: i64,
) -> UecmResult<Vec<PsoCacheFile>> {
    pso_cache_files::list_by_project(&db, project_id)
}

#[derive(Debug, Serialize)]
pub struct PsoDistributeJobResponse {
    pub job_id: String,
    pub plan: Vec<PsoDistributePlanItem>,
}

#[tauri::command]
pub async fn distribute_pso_cache(
    app: tauri::AppHandle,
    db: State<'_, Db>,
    file_id: i64,
    target_machine_ids: Vec<i64>,
    named_share_unc: Option<String>,
    operator_credential_alias: Option<String>,
    /// Optional separate SMB credential for the source UNC. Falls back to
    /// the operator credential when None.
    source_smb_credential_alias: Option<String>,
    force_gpu_mismatch: bool,
) -> UecmResult<PsoDistributeJobResponse> {
    let file = pso_cache_files::get(&db, file_id)?
        .ok_or_else(|| UecmError::InvalidInput(format!("pso file {} not found", file_id)))?;
    let source_machine = data_machines::get(&db, file.source_machine_id)?
        .ok_or_else(|| UecmError::InvalidInput(format!("source machine {} not found", file.source_machine_id)))?;
    let (op_user, op_pass) = resolve_operator_creds(&db, operator_credential_alias.as_deref())?;
    let (smb_user, smb_pass) = if source_smb_credential_alias.is_some() {
        resolve_operator_creds(&db, source_smb_credential_alias.as_deref())?
    } else {
        (op_user.clone(), op_pass.clone())
    };

    // GPU mismatch guard. PSO files are GPU+driver specific; pushing a file
    // built on RTX 3080 to an RTX 4090 box yields a runtime hitch the operator
    // would only see in production. We block UNLESS force=true on:
    //   1. target has no matrix cell at all (machine never had GPU detection)
    //   2. target's GPU row exists but signature is None (incomplete data)
    //   3. signature exists but does NOT match the file's signature
    if !force_gpu_mismatch {
        let matrix = crate::core::gpu_consistency::build_matrix(&db)?;
        for tid in &target_machine_ids {
            let cell = matrix.cells.iter().find(|c| c.machine_id == *tid);
            match cell {
                None => {
                    return Err(UecmError::InvalidInput(format!(
                        "target machine {} has no GPU consistency cell; refresh machine details first or pass force=true",
                        tid
                    )));
                }
                Some(c) => match &c.signature {
                    None => {
                        return Err(UecmError::InvalidInput(format!(
                            "target machine {} GPU signature is unknown; refresh machine details first or pass force=true",
                            tid
                        )));
                    }
                    Some(sig) => {
                        if sig.as_string() != file.gpu_signature {
                            return Err(UecmError::InvalidInput(format!(
                                "target machine {} GPU signature {} does not match file signature {}; pass force=true to override",
                                tid, sig.as_string(), file.gpu_signature,
                            )));
                        }
                    }
                },
            }
        }
    }

    let plan_items = pso_distribute::plan(
        &db, file_id, &source_machine.ip, &file,
        &target_machine_ids, named_share_unc.as_deref(),
        op_user.clone(), op_pass.clone(),
        smb_user, smb_pass,
    )?;

    // Preflight every target before kicking off robocopy.
    for item in &plan_items {
        pso_distribute::preflight_one(item).await.map_err(|e| {
            UecmError::OperationFailed(format!(
                "target {} cannot reach source UNC: {}",
                item.target_machine_id, e
            ))
        })?;
    }
    let job_id = format!("pso-dist-{}-{}", file_id, chrono::Utc::now().timestamp_millis());
    let plan_clone = plan_items.clone();
    let app_clone = app.clone();
    let job_id_clone = job_id.clone();
    let pid = file.project_id;

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
                let outcome = pso_distribute::run_one(item).await?;
                if !outcome.ok {
                    return Err(UecmError::OperationFailed(format!(
                        "robocopy exit {}: {}",
                        outcome.exit_code,
                        outcome.message.unwrap_or_else(|| outcome.stdout_tail.clone())
                    )));
                }
                Ok::<_, UecmError>(outcome)
            }
        }).await;

        while let Some(ev) = rx.recv().await {
            #[derive(Serialize)]
            struct Payload<'a> {
                job_id: &'a str,
                project_id: i64,
                event: &'a batch::BatchEvent,
            }
            let _ = app_clone.emit(
                "pak-distribute-progress",
                Payload {
                    job_id: &job_id_clone,
                    project_id: pid,
                    event: &ev,
                },
            );
        }
    });

    Ok(PsoDistributeJobResponse {
        job_id,
        plan: plan_items,
    })
}
```

- [ ] **Step 2: Wire into `commands/mod.rs`**

```rust
pub mod pso;
```

- [ ] **Step 3: Register in `lib.rs`**

```rust
            commands::pso::start_pso_collection,
            commands::pso::list_pso_cache_files,
            commands::pso::distribute_pso_cache,
```

- [ ] **Step 4: Build + commit**

```bash
cd src-tauri && cargo build --lib 2>&1 | tail -10 && cd ..
git add src-tauri/src/commands/pso.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat: tauri commands for pso collect / list / distribute"
```

---

## Task 13: Frontend service + types — `src/services/tauri.ts`

**Files:**
- Modify: `src/services/tauri.ts`

- [ ] **Step 1: Add types**

```typescript
// PSO Cache
export interface PsoCacheFile {
  id: number | null;
  project_id: number;
  source_machine_id: number;
  file_path: string;
  file_name: string;
  size_bytes: number;
  gpu_signature: string;
  ue_version: string | null;
  collected_at: string | null;
}

export interface PsoCollectJobResponse {
  job_id: string;
  source_machine_id: number;
  project_id: number;
}

export interface PsoDistributePlanItem {
  target_machine_id: number;
  target_host: string;
  source_unc: string;
  target_local: string;
  file_name: string;
  credential_user: string | null;
  credential_pass: string | null;
}

export interface PsoDistributeJobResponse {
  job_id: string;
  plan: PsoDistributePlanItem[];
}

export interface PsoCollectFinalizedPayload {
  job_id: string;
  source_machine_id: number;
  project_id: number;
  files_collected: number;
}

// GPU consistency
export interface GpuSignature {
  vendor: string;
  model: string;
  driver: string;
}

export type CellStatus = "match" | "deviation" | "unknown";

export interface MachineGpuCell {
  machine_id: number;
  hostname: string;
  signature: GpuSignature | null;
  status: CellStatus;
}

export interface GpuMatrix {
  signatures: [GpuSignature, number][];
  baseline: GpuSignature | null;
  cells: MachineGpuCell[];
}
```

- [ ] **Step 2: Add API methods**

Inside `tauriApi`:

```typescript
  // PSO Cache
  async startPsoCollection(
    sourceMachineId: number,
    projectId: number,
    ueVersion: string | null,
    resolutionW: number,
    resolutionH: number,
    windowed: boolean,
    maxMinutes: number,
    operatorCredentialAlias: string | null,
  ): Promise<PsoCollectJobResponse> {
    return invoke<PsoCollectJobResponse>("start_pso_collection", {
      sourceMachineId,
      projectId,
      ueVersion,
      resolutionW,
      resolutionH,
      windowed,
      maxMinutes,
      operatorCredentialAlias,
    });
  },
  async listPsoCacheFiles(projectId: number): Promise<PsoCacheFile[]> {
    return invoke<PsoCacheFile[]>("list_pso_cache_files", { projectId });
  },
  async distributePsoCache(
    fileId: number,
    targetMachineIds: number[],
    namedShareUnc: string | null,
    operatorCredentialAlias: string | null,
    forceGpuMismatch: boolean,
  ): Promise<PsoDistributeJobResponse> {
    return invoke<PsoDistributeJobResponse>("distribute_pso_cache", {
      fileId,
      targetMachineIds,
      namedShareUnc,
      operatorCredentialAlias,
      forceGpuMismatch,
    });
  },
  async verifyPsoPrecaching(
    machineIds: number[],
    projectPaths: string[],
    operatorCredentialAlias: string | null,
  ): Promise<unknown[]> {
    return invoke<unknown[]>("verify_pso_precaching", {
      machineIds,
      projectPaths,
      operatorCredentialAlias,
    });
  },

  // GPU consistency
  async getGpuConsistencyMatrix(): Promise<GpuMatrix> {
    return invoke<GpuMatrix>("get_gpu_consistency_matrix");
  },
```

- [ ] **Step 3: Verify type-check + commit**

```bash
pnpm exec tsc -p tsconfig.app.json --noEmit 2>&1 | tail -10
git add src/services/tauri.ts
git commit -m "feat(frontend): tauri service additions for pso + gpu consistency"
```

---

## Task 14: Frontend store — `usePsoStore`

**Files:**
- Create: `src/stores/pso.ts`
- Create: `src/__tests__/pso-store.spec.ts`

- [ ] **Step 1: Write `usePsoStore`**

```typescript
import { defineStore } from "pinia";
import { ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  tauriApi,
  type BatchEvent,
  type PakDistributeProgressPayload,
  type PsoCacheFile,
  type PsoCollectFinalizedPayload,
  type PsoCollectJobResponse,
  type PsoDistributeJobResponse,
  type UecmError,
  type UeRunnerEvent,
  type UeRunnerProgressPayload,
} from "@/services/tauri";

export interface CollectJobState {
  job_id: string;
  source_machine_id: number;
  project_id: number;
  status: "queued" | "spawning" | "collecting" | "completing" | "completed" | "cancelled" | "error";
  log_lines: string[];
  files_collected: number | null;
  error_message: string | null;
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
  status: "queued" | "running" | "completed";
  targets: DistributeTargetState[];
  started_at: string;
  finished_at: string | null;
}

export const usePsoStore = defineStore("pso", () => {
  const cacheFilesByProject = ref<Record<number, PsoCacheFile[]>>({});
  const collectJobs = ref<CollectJobState[]>([]);
  const distributeJobs = ref<DistributeJobState[]>([]);
  const error = ref<UecmError | null>(null);

  let unlistenRunner: UnlistenFn | null = null;
  let unlistenFinalized: UnlistenFn | null = null;
  let unlistenDist: UnlistenFn | null = null;

  async function attach() {
    if (unlistenRunner) return;
    unlistenRunner = await listen<UeRunnerProgressPayload>("ue-runner-progress", (e) => {
      onRunnerEvent(e.payload);
    });
    unlistenFinalized = await listen<PsoCollectFinalizedPayload>("pso-collect-finalized", (e) => {
      onFinalized(e.payload);
    });
    unlistenDist = await listen<PakDistributeProgressPayload>("pak-distribute-progress", (e) => {
      onDistributeEvent(e.payload);
    });
  }

  async function detach() {
    unlistenRunner?.();
    unlistenFinalized?.();
    unlistenDist?.();
    unlistenRunner = null;
    unlistenFinalized = null;
    unlistenDist = null;
  }

  function onRunnerEvent(p: UeRunnerProgressPayload) {
    const job = collectJobs.value.find((j) => j.job_id === p.job_id);
    if (!job) return;
    const ev: UeRunnerEvent = p.event;
    switch (ev.kind) {
      case "spawned":
        job.status = "collecting";
        break;
      case "log_line":
        if (ev.text) {
          job.log_lines.push(ev.text);
          if (job.log_lines.length > 200) job.log_lines.splice(0, job.log_lines.length - 200);
        }
        break;
      case "completed":
        job.status = "completing";
        break;
      case "cancelled":
        job.status = "completing";
        break;
      case "error":
        job.status = "error";
        job.error_message = ev.message ?? "unknown error";
        job.finished_at = new Date().toISOString();
        break;
    }
  }

  function onFinalized(p: PsoCollectFinalizedPayload) {
    const job = collectJobs.value.find((j) => j.job_id === p.job_id);
    if (!job) return;
    job.status = "completed";
    job.files_collected = p.files_collected;
    job.finished_at = new Date().toISOString();
    void loadFiles(p.project_id);
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

  async function loadFiles(projectId: number) {
    error.value = null;
    try {
      const list = await tauriApi.listPsoCacheFiles(projectId);
      cacheFilesByProject.value = { ...cacheFilesByProject.value, [projectId]: list };
    } catch (e) {
      error.value = e as UecmError;
    }
  }

  async function startCollection(args: {
    sourceMachineId: number;
    projectId: number;
    ueVersion: string | null;
    resolutionW: number;
    resolutionH: number;
    windowed: boolean;
    maxMinutes: number;
    operatorCredentialAlias: string | null;
  }): Promise<PsoCollectJobResponse> {
    await attach();
    error.value = null;
    const r = await tauriApi.startPsoCollection(
      args.sourceMachineId,
      args.projectId,
      args.ueVersion,
      args.resolutionW,
      args.resolutionH,
      args.windowed,
      args.maxMinutes,
      args.operatorCredentialAlias,
    );
    collectJobs.value.unshift({
      job_id: r.job_id,
      source_machine_id: r.source_machine_id,
      project_id: r.project_id,
      status: "spawning",
      log_lines: [],
      files_collected: null,
      error_message: null,
      started_at: new Date().toISOString(),
      finished_at: null,
    });
    return r;
  }

  async function cancelCollection(jobId: string): Promise<boolean> {
    return tauriApi.cancelUeJob(jobId);
  }

  async function startDistribute(args: {
    fileId: number;
    targetMachineIds: number[];
    namedShareUnc: string | null;
    operatorCredentialAlias: string | null;
    forceGpuMismatch: boolean;
  }): Promise<PsoDistributeJobResponse> {
    await attach();
    error.value = null;
    const r = await tauriApi.distributePsoCache(
      args.fileId,
      args.targetMachineIds,
      args.namedShareUnc,
      args.operatorCredentialAlias,
      args.forceGpuMismatch,
    );
    distributeJobs.value.unshift({
      job_id: r.job_id,
      status: "running",
      targets: r.plan.map((p) => ({
        target_machine_id: p.target_machine_id,
        target_host: p.target_host,
        status: "pending",
        message: null,
      })),
      started_at: new Date().toISOString(),
      finished_at: null,
    });
    return r;
  }

  return {
    cacheFilesByProject,
    collectJobs,
    distributeJobs,
    error,
    attach,
    detach,
    loadFiles,
    startCollection,
    cancelCollection,
    startDistribute,
  };
});
```

- [ ] **Step 2: Test**

`src/__tests__/pso-store.spec.ts`:

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

const { mockApi, mockListen } = vi.hoisted(() => ({
  mockApi: {
    startPsoCollection: vi.fn(),
    listPsoCacheFiles: vi.fn(),
    distributePsoCache: vi.fn(),
    cancelUeJob: vi.fn(),
  },
  mockListen: vi.fn().mockResolvedValue(() => undefined),
}));

vi.mock("@/services/tauri", () => ({ tauriApi: mockApi }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mockListen }));

import { usePsoStore } from "@/stores/pso";

describe("pso store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((m) => m.mockReset?.());
    mockListen.mockClear();
  });

  it("startCollection appends a spawning job", async () => {
    mockApi.startPsoCollection.mockResolvedValue({ job_id: "p-1", source_machine_id: 1, project_id: 10 });
    const s = usePsoStore();
    await s.startCollection({
      sourceMachineId: 1, projectId: 10, ueVersion: null,
      resolutionW: 1920, resolutionH: 1080, windowed: true, maxMinutes: 10,
      operatorCredentialAlias: null,
    });
    expect(s.collectJobs[0].status).toBe("spawning");
  });

  it("loadFiles populates cacheFilesByProject", async () => {
    mockApi.listPsoCacheFiles.mockResolvedValue([
      { id: 1, project_id: 10, source_machine_id: 1, file_path: "p", file_name: "x.upipelinecache", size_bytes: 100, gpu_signature: "x:y:z", ue_version: "5.4.4", collected_at: null },
    ]);
    const s = usePsoStore();
    await s.loadFiles(10);
    expect(s.cacheFilesByProject[10]).toHaveLength(1);
  });

  it("startDistribute initialises targets", async () => {
    mockApi.distributePsoCache.mockResolvedValue({
      job_id: "d-1",
      plan: [
        { target_machine_id: 2, target_host: "1.1.1.1", source_unc: "x", target_local: "y", file_name: "f", credential_user: null, credential_pass: null },
      ],
    });
    const s = usePsoStore();
    await s.startDistribute({ fileId: 1, targetMachineIds: [2], namedShareUnc: null, operatorCredentialAlias: null, forceGpuMismatch: false });
    expect(s.distributeJobs[0].targets).toHaveLength(1);
  });
});
```

- [ ] **Step 3: Run + commit**

```bash
pnpm test src/__tests__/pso-store.spec.ts 2>&1 | tail -10
git add src/stores/pso.ts src/__tests__/pso-store.spec.ts
git commit -m "feat(frontend): usePsoStore + tests"
```

---

## Task 15: Frontend store — `useGpuConsistencyStore`

**Files:**
- Create: `src/stores/gpuConsistency.ts`
- Create: `src/__tests__/gpu-consistency-store.spec.ts`

- [ ] **Step 1: Write the store**

```typescript
import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { tauriApi, type GpuMatrix, type UecmError } from "@/services/tauri";

export const useGpuConsistencyStore = defineStore("gpuConsistency", () => {
  const matrix = ref<GpuMatrix | null>(null);
  const isLoading = ref(false);
  const error = ref<UecmError | null>(null);

  async function load() {
    isLoading.value = true;
    error.value = null;
    try {
      matrix.value = await tauriApi.getGpuConsistencyMatrix();
    } catch (e) {
      error.value = e as UecmError;
    } finally {
      isLoading.value = false;
    }
  }

  const baselineLabel = computed(() => {
    const b = matrix.value?.baseline;
    return b ? `${b.vendor} ${b.model} (driver ${b.driver})` : "—";
  });

  const deviationCount = computed(
    () => matrix.value?.cells.filter((c) => c.status === "deviation").length ?? 0,
  );

  return { matrix, isLoading, error, baselineLabel, deviationCount, load };
});
```

- [ ] **Step 2: Test**

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    getGpuConsistencyMatrix: vi.fn(),
  },
}));
vi.mock("@/services/tauri", () => ({ tauriApi: mockApi }));

import { useGpuConsistencyStore } from "@/stores/gpuConsistency";

describe("gpuConsistency store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mockApi.getGpuConsistencyMatrix.mockReset();
  });

  it("computes baseline label and deviation count", async () => {
    mockApi.getGpuConsistencyMatrix.mockResolvedValue({
      signatures: [
        [{ vendor: "nvidia", model: "RTX 3080", driver: "535.98" }, 2],
        [{ vendor: "nvidia", model: "RTX 4090", driver: "560.00" }, 1],
      ],
      baseline: { vendor: "nvidia", model: "RTX 3080", driver: "535.98" },
      cells: [
        { machine_id: 1, hostname: "A", signature: { vendor: "nvidia", model: "RTX 3080", driver: "535.98" }, status: "match" },
        { machine_id: 2, hostname: "B", signature: { vendor: "nvidia", model: "RTX 3080", driver: "535.98" }, status: "match" },
        { machine_id: 3, hostname: "C", signature: { vendor: "nvidia", model: "RTX 4090", driver: "560.00" }, status: "deviation" },
      ],
    });
    const s = useGpuConsistencyStore();
    await s.load();
    expect(s.baselineLabel).toContain("RTX 3080");
    expect(s.deviationCount).toBe(1);
  });
});
```

- [ ] **Step 3: Run + commit**

```bash
pnpm test src/__tests__/gpu-consistency-store.spec.ts 2>&1 | tail -10
git add src/stores/gpuConsistency.ts src/__tests__/gpu-consistency-store.spec.ts
git commit -m "feat(frontend): useGpuConsistencyStore + test"
```

---

## Task 16: New primitives — UecmGpuMatrix + UecmHorizontalSplit

**Files:**
- Create: `src/components/primitives/UecmGpuMatrix.vue`
- Create: `src/components/primitives/UecmHorizontalSplit.vue`
- Modify: `src/components/primitives/index.ts`
- Create: `src/__tests__/UecmGpuMatrix.spec.ts`

- [ ] **Step 1: `UecmGpuMatrix.vue`**

```vue
<script setup lang="ts">
import { computed } from "vue";
import type { GpuMatrix } from "@/services/tauri";

const props = defineProps<{ matrix: GpuMatrix | null }>();

const rows = computed(() => props.matrix?.signatures ?? []);
const cells = computed(() => props.matrix?.cells ?? []);

function cellStatusForSignature(machineId: number, sigStr: string) {
  const cell = cells.value.find((c) => c.machine_id === machineId);
  if (!cell || !cell.signature) return "—";
  const cs = `${cell.signature.vendor}:${cell.signature.model}:${cell.signature.driver}`;
  return cs === sigStr ? "✓" : "";
}

function sigStr(sig: { vendor: string; model: string; driver: string }) {
  return `${sig.vendor}:${sig.model}:${sig.driver}`;
}
</script>

<template>
  <div data-gpu-matrix class="overflow-x-auto rounded-lg border bg-card">
    <table class="min-w-full text-xs">
      <thead class="bg-muted text-muted-foreground">
        <tr>
          <th class="px-3 py-2 text-left">GPU + Driver</th>
          <th class="px-3 py-2 text-left">Count</th>
          <th
            v-for="cell in cells"
            :key="cell.machine_id"
            class="px-2 py-2 text-center"
            data-gpu-matrix-machine-col
          >
            {{ cell.hostname }}
          </th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="[sig, count] in rows"
          :key="sigStr(sig)"
          class="border-t"
          :class="props.matrix?.baseline && sigStr(sig) === sigStr(props.matrix.baseline) ? 'bg-emerald-500/10' : ''"
          data-gpu-matrix-row
        >
          <td class="px-3 py-1 font-mono">{{ sig.vendor }} / {{ sig.model }} / {{ sig.driver }}</td>
          <td class="px-3 py-1">{{ count }}</td>
          <td
            v-for="cell in cells"
            :key="`${sigStr(sig)}-${cell.machine_id}`"
            class="px-2 py-1 text-center font-mono"
          >
            {{ cellStatusForSignature(cell.machine_id, sigStr(sig)) }}
          </td>
        </tr>
      </tbody>
    </table>
    <p v-if="rows.length === 0" data-gpu-matrix-empty class="p-4 text-sm text-muted-foreground">
      No GPU data yet. Refresh machine details to detect GPUs.
    </p>
  </div>
</template>
```

- [ ] **Step 2: `UecmHorizontalSplit.vue`**

```vue
<script setup lang="ts">
defineProps<{ leftWeight?: number; rightWeight?: number }>();
</script>

<template>
  <div data-horizontal-split class="grid h-full" :style="{ gridTemplateColumns: `${leftWeight ?? 1}fr ${rightWeight ?? 2}fr` }">
    <div class="border-r overflow-y-auto"><slot name="left" /></div>
    <div class="overflow-y-auto"><slot name="right" /></div>
  </div>
</template>
```

- [ ] **Step 3: Update `primitives/index.ts`**

```typescript
export { default as UecmGpuMatrix } from "./UecmGpuMatrix.vue";
export { default as UecmHorizontalSplit } from "./UecmHorizontalSplit.vue";
```

- [ ] **Step 4: Test**

`src/__tests__/UecmGpuMatrix.spec.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import UecmGpuMatrix from "@/components/primitives/UecmGpuMatrix.vue";

describe("UecmGpuMatrix", () => {
  it("renders empty state when no signatures", () => {
    const w = mount(UecmGpuMatrix, { props: { matrix: { signatures: [], baseline: null, cells: [] } } });
    expect(w.find('[data-gpu-matrix-empty]').exists()).toBe(true);
  });

  it("highlights baseline row", () => {
    const w = mount(UecmGpuMatrix, {
      props: {
        matrix: {
          signatures: [[{ vendor: "nvidia", model: "RTX 3080", driver: "535.98" }, 2]],
          baseline: { vendor: "nvidia", model: "RTX 3080", driver: "535.98" },
          cells: [
            { machine_id: 1, hostname: "A", signature: { vendor: "nvidia", model: "RTX 3080", driver: "535.98" }, status: "match" },
          ],
        },
      },
    });
    const row = w.find('[data-gpu-matrix-row]');
    expect(row.classes().some((c) => c.includes("emerald"))).toBe(true);
  });
});
```

- [ ] **Step 5: Run + commit**

```bash
pnpm test src/__tests__/UecmGpuMatrix.spec.ts 2>&1 | tail -10
git add src/components/primitives/UecmGpuMatrix.vue src/components/primitives/UecmHorizontalSplit.vue src/components/primitives/index.ts src/__tests__/UecmGpuMatrix.spec.ts
git commit -m "feat(frontend): UecmGpuMatrix + UecmHorizontalSplit primitives"
```

---

## Task 17: PsoCollectWizard

**Files:**
- Create: `src/components/modals/PsoCollectWizard.vue`
- Create: `src/__tests__/PsoCollectWizard.spec.ts`

- [ ] **Step 1: Write the wizard** (4-step pattern matching DdcPakWizard)

```vue
<script setup lang="ts">
import { computed, ref, watch } from "vue";
import BaseModal from "./BaseModal.vue";
import { useMachinesStore } from "@/stores/machines";
import { useProjectsStore } from "@/stores/projects";
import { useCredentialsStore } from "@/stores/credentials";
import { usePsoStore } from "@/stores/pso";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ (e: "close"): void }>();

const machines = useMachinesStore();
const projects = useProjectsStore();
const credentials = useCredentialsStore();
const pso = usePsoStore();

const step = ref<1 | 2 | 3 | 4>(1);
const sourceMachineId = ref<number | null>(null);
const projectId = ref<number | null>(null);
const resolutionW = ref(1920);
const resolutionH = ref(1080);
const windowed = ref(true);
const maxMinutes = ref(10);
const credAlias = ref<string | null>(null);
const errMsg = ref<string | null>(null);
const isSubmitting = ref(false);

watch(() => props.open, (v) => {
  if (v) {
    step.value = 1;
    sourceMachineId.value = null;
    projectId.value = null;
    resolutionW.value = 1920;
    resolutionH.value = 1080;
    windowed.value = true;
    maxMinutes.value = 10;
    credAlias.value = null;
    errMsg.value = null;
    machines.loadMachines();
    projects.load();
    credentials.load();
  }
});

const winrmCreds = computed(() => credentials.credentials.filter((c) => c.kind === "winrm"));
const canAdvance = computed(() => {
  switch (step.value) {
    case 1: return sourceMachineId.value != null;
    case 2: return projectId.value != null;
    case 3: return resolutionW.value > 0 && resolutionH.value > 0 && maxMinutes.value > 0;
    case 4: return !isSubmitting.value;
  }
  return false;
});

async function run() {
  if (sourceMachineId.value == null || projectId.value == null) return;
  isSubmitting.value = true;
  errMsg.value = null;
  try {
    await pso.startCollection({
      sourceMachineId: sourceMachineId.value,
      projectId: projectId.value,
      ueVersion: null,
      resolutionW: resolutionW.value,
      resolutionH: resolutionH.value,
      windowed: windowed.value,
      maxMinutes: maxMinutes.value,
      operatorCredentialAlias: credAlias.value,
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
  <BaseModal :open="open" title="Collect PSO Cache" @close="emit('close')">
    <div class="space-y-4 p-1" data-pso-collect-wizard>
      <div class="flex gap-2 text-xs">
        <span :class="step >= 1 ? 'text-primary' : 'text-muted-foreground'">1. Source</span>
        <span>›</span>
        <span :class="step >= 2 ? 'text-primary' : 'text-muted-foreground'">2. Project</span>
        <span>›</span>
        <span :class="step >= 3 ? 'text-primary' : 'text-muted-foreground'">3. Run options</span>
        <span>›</span>
        <span :class="step >= 4 ? 'text-primary' : 'text-muted-foreground'">4. Review</span>
      </div>
      <div v-if="step === 1">
        <label class="text-xs uppercase tracking-wide text-muted-foreground">Source machine (must have real GPU)</label>
        <select v-model="sourceMachineId" data-pso-source-select class="mt-1 w-full rounded border bg-background px-3 py-2 text-sm">
          <option :value="null">Select…</option>
          <option v-for="m in machines.machines" :key="m.id ?? m.ip" :value="m.id">{{ m.hostname }}</option>
        </select>
      </div>
      <div v-else-if="step === 2">
        <label class="text-xs uppercase tracking-wide text-muted-foreground">Project</label>
        <select v-model="projectId" data-pso-project-select class="mt-1 w-full rounded border bg-background px-3 py-2 text-sm">
          <option :value="null">Select…</option>
          <option v-for="p in projects.projects" :key="p.id" :value="p.id">{{ p.uproject_name }}</option>
        </select>
      </div>
      <div v-else-if="step === 3" class="space-y-2">
        <div class="flex gap-2">
          <label class="flex-1 text-xs">Width
            <input v-model.number="resolutionW" type="number" data-pso-resw class="mt-1 w-full rounded border bg-background px-2 py-1 text-sm" />
          </label>
          <label class="flex-1 text-xs">Height
            <input v-model.number="resolutionH" type="number" data-pso-resh class="mt-1 w-full rounded border bg-background px-2 py-1 text-sm" />
          </label>
        </div>
        <label class="flex items-center gap-2 text-sm">
          <input type="checkbox" v-model="windowed" /> Windowed mode
        </label>
        <label class="text-xs uppercase tracking-wide text-muted-foreground">Max minutes (auto-cancel)</label>
        <input v-model.number="maxMinutes" type="number" data-pso-max-min class="mt-1 w-full rounded border bg-background px-2 py-1 text-sm" />
        <label class="text-xs uppercase tracking-wide text-muted-foreground">Credential</label>
        <select v-model="credAlias" data-pso-cred-select class="mt-1 w-full rounded border bg-background px-3 py-2 text-sm">
          <option :value="null">(current process token)</option>
          <option v-for="c in winrmCreds" :key="c.alias" :value="c.alias">{{ c.alias }}</option>
        </select>
      </div>
      <div v-else-if="step === 4" class="space-y-2">
        <h3 class="text-sm font-semibold">Preview</h3>
        <ul class="rounded border bg-muted/40 p-3 text-xs font-mono">
          <li>• Source machine#: {{ sourceMachineId }}</li>
          <li>• Project#: {{ projectId }}</li>
          <li>• Resolution: {{ resolutionW }}×{{ resolutionH }} ({{ windowed ? "windowed" : "fullscreen" }})</li>
          <li>• Max minutes: {{ maxMinutes }}</li>
          <li>• Credential: {{ credAlias ?? "(process token)" }}</li>
        </ul>
        <p v-if="errMsg" class="rounded bg-rose-500/10 p-2 text-xs text-rose-600">{{ errMsg }}</p>
      </div>
      <div class="flex justify-between pt-2">
        <button v-if="step > 1" class="rounded border px-3 py-1 text-xs" @click="step = (step - 1) as 1 | 2 | 3 | 4">Back</button>
        <span v-else />
        <button v-if="step < 4" data-pso-wizard-next class="rounded bg-primary px-3 py-1 text-xs text-primary-foreground disabled:opacity-50" :disabled="!canAdvance" @click="step = (step + 1) as 1 | 2 | 3 | 4">Next</button>
        <button v-else data-pso-wizard-run class="rounded bg-primary px-3 py-1 text-xs text-primary-foreground disabled:opacity-50" :disabled="!canAdvance" @click="run">{{ isSubmitting ? "Starting…" : "Run" }}</button>
      </div>
    </div>
  </BaseModal>
</template>
```

- [ ] **Step 2: Test (skeleton matching DdcPakWizard pattern)**

`src/__tests__/PsoCollectWizard.spec.ts`:

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

const { mockApi, mockListen } = vi.hoisted(() => ({
  mockApi: {
    listMachines: vi.fn().mockResolvedValue([{ id: 1, hostname: "RENDER-01", ip: "1.1.1.1", role: "render", status: "online", last_seen_at: null }]),
    listProjects: vi.fn().mockResolvedValue([{ id: 10, uproject_name: "X.uproject", display_name: null, uproject_guid: null, location_count: 1 }]),
    listCredentials: vi.fn().mockResolvedValue([]),
    startPsoCollection: vi.fn(),
  },
  mockListen: vi.fn().mockResolvedValue(() => undefined),
}));
vi.mock("@/services/tauri", () => ({ tauriApi: mockApi }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mockListen }));

import PsoCollectWizard from "@/components/modals/PsoCollectWizard.vue";

describe("PsoCollectWizard", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((m) => m.mockReset?.());
  });

  it("renders step 1 when open", () => {
    const w = mount(PsoCollectWizard, { props: { open: true } });
    expect(w.find('[data-pso-collect-wizard]').exists()).toBe(true);
  });

  it("blocks Next when no source", () => {
    const w = mount(PsoCollectWizard, { props: { open: true } });
    const next = w.find('[data-pso-wizard-next]');
    expect((next.element as HTMLButtonElement).disabled).toBe(true);
  });
});
```

- [ ] **Step 3: Run + commit**

```bash
pnpm test src/__tests__/PsoCollectWizard.spec.ts 2>&1 | tail -10
git add src/components/modals/PsoCollectWizard.vue src/__tests__/PsoCollectWizard.spec.ts
git commit -m "feat(frontend): PsoCollectWizard"
```

---

## Task 18: PsoDistributeWizard

**Files:**
- Create: `src/components/modals/PsoDistributeWizard.vue`
- Create: `src/__tests__/PsoDistributeWizard.spec.ts`

- [ ] **Step 1: Write the wizard**

```vue
<script setup lang="ts">
import { computed, ref, watch } from "vue";
import BaseModal from "./BaseModal.vue";
import { useMachinesStore } from "@/stores/machines";
import { useCredentialsStore } from "@/stores/credentials";
import { usePsoStore } from "@/stores/pso";
import { useGpuConsistencyStore } from "@/stores/gpuConsistency";
import type { PsoCacheFile } from "@/services/tauri";

const props = defineProps<{ open: boolean; file: PsoCacheFile | null }>();
const emit = defineEmits<{ (e: "close"): void }>();

const machines = useMachinesStore();
const credentials = useCredentialsStore();
const pso = usePsoStore();
const gpu = useGpuConsistencyStore();

const targetIds = ref<number[]>([]);
const namedShare = ref("");
const credAlias = ref<string | null>(null);
const force = ref(false);
const errMsg = ref<string | null>(null);
const isSubmitting = ref(false);

watch(() => props.open, (v) => {
  if (v) {
    targetIds.value = [];
    namedShare.value = "";
    credAlias.value = null;
    force.value = false;
    errMsg.value = null;
    machines.loadMachines();
    credentials.load();
    gpu.load();
  }
});

const winrmCreds = computed(() => credentials.credentials.filter((c) => c.kind === "winrm"));

const machineGpuMap = computed(() => {
  const m = new Map<number, string | null>();
  if (gpu.matrix) {
    for (const cell of gpu.matrix.cells) {
      m.set(cell.machine_id, cell.signature ? `${cell.signature.vendor}:${cell.signature.model}:${cell.signature.driver}` : null);
    }
  }
  return m;
});

const candidateMachines = computed(() => {
  if (!props.file) return [];
  return machines.machines
    .filter((m) => m.id != null && m.id !== props.file?.source_machine_id)
    .map((m) => {
      const sig = machineGpuMap.value.get(m.id ?? 0) ?? null;
      const matches = sig === props.file?.gpu_signature;
      return { id: m.id!, hostname: m.hostname, ip: m.ip, sig, matches };
    });
});

const hasMismatch = computed(() =>
  targetIds.value.some((id) => {
    const c = candidateMachines.value.find((x) => x.id === id);
    return c && !c.matches;
  }),
);

const canSubmit = computed(() => targetIds.value.length > 0 && !isSubmitting.value && (!hasMismatch.value || force.value));

async function run() {
  if (!props.file?.id) return;
  isSubmitting.value = true;
  errMsg.value = null;
  try {
    await pso.startDistribute({
      fileId: props.file.id,
      targetMachineIds: targetIds.value,
      namedShareUnc: namedShare.value || null,
      operatorCredentialAlias: credAlias.value,
      forceGpuMismatch: force.value,
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
  <BaseModal :open="open" title="Distribute PSO Cache" @close="emit('close')">
    <div class="space-y-3 p-1" data-pso-dist-wizard>
      <p class="text-xs text-muted-foreground" data-pso-dist-file-name>{{ file?.file_name ?? "" }}</p>
      <p class="text-xs" data-pso-dist-source-sig>Source GPU signature: <code>{{ file?.gpu_signature ?? "" }}</code></p>
      <div class="rounded border bg-card">
        <p class="px-3 py-2 text-xs uppercase tracking-wide text-muted-foreground">Target machines</p>
        <label
          v-for="m in candidateMachines"
          :key="m.id"
          class="flex items-center justify-between gap-2 border-t px-3 py-2 text-sm"
          data-pso-dist-target-row
        >
          <span class="flex items-center gap-2">
            <input type="checkbox" :value="m.id" v-model="targetIds" />
            {{ m.hostname }} ({{ m.ip }})
          </span>
          <span :class="m.matches ? 'text-emerald-500' : 'text-amber-500'" class="font-mono text-xs" data-pso-dist-target-flag>
            {{ m.matches ? "match" : "MISMATCH" }}
          </span>
        </label>
      </div>
      <label v-if="hasMismatch" class="flex items-center gap-2 text-xs text-amber-600">
        <input type="checkbox" v-model="force" data-pso-dist-force />
        Force distribute despite GPU mismatch (PSO file may not work on these targets)
      </label>
      <label class="text-xs uppercase tracking-wide text-muted-foreground">Named share UNC (optional)</label>
      <input v-model="namedShare" type="text" class="mt-1 w-full rounded border bg-background px-3 py-2 font-mono text-sm" />
      <label class="text-xs uppercase tracking-wide text-muted-foreground">Credential</label>
      <select v-model="credAlias" class="mt-1 w-full rounded border bg-background px-3 py-2 text-sm">
        <option :value="null">(process token)</option>
        <option v-for="c in winrmCreds" :key="c.alias" :value="c.alias">{{ c.alias }}</option>
      </select>
      <p v-if="errMsg" class="rounded bg-rose-500/10 p-2 text-xs text-rose-600">{{ errMsg }}</p>
      <button
        data-pso-dist-run
        class="w-full rounded bg-primary px-3 py-2 text-sm text-primary-foreground disabled:opacity-50"
        :disabled="!canSubmit"
        @click="run"
      >{{ isSubmitting ? "Distributing…" : "Distribute" }}</button>
    </div>
  </BaseModal>
</template>
```

- [ ] **Step 2: Test stub + commit**

`src/__tests__/PsoDistributeWizard.spec.ts`:

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    listMachines: vi.fn().mockResolvedValue([]),
    listCredentials: vi.fn().mockResolvedValue([]),
    getGpuConsistencyMatrix: vi.fn().mockResolvedValue({ signatures: [], baseline: null, cells: [] }),
    distributePsoCache: vi.fn(),
  },
  mockListen: vi.fn().mockResolvedValue(() => undefined),
}));
vi.mock("@/services/tauri", () => ({ tauriApi: mockApi }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

import PsoDistributeWizard from "@/components/modals/PsoDistributeWizard.vue";

describe("PsoDistributeWizard", () => {
  beforeEach(() => setActivePinia(createPinia()));

  it("renders when open with a file", () => {
    const w = mount(PsoDistributeWizard, {
      props: {
        open: true,
        file: {
          id: 1, project_id: 10, source_machine_id: 1,
          file_path: "p", file_name: "x.upipelinecache", size_bytes: 100,
          gpu_signature: "nvidia:RTX 3080:535.98", ue_version: "5.4.4", collected_at: null,
        },
      },
    });
    expect(w.find('[data-pso-dist-wizard]').exists()).toBe(true);
    expect(w.find('[data-pso-dist-file-name]').text()).toContain("x.upipelinecache");
  });
});
```

```bash
pnpm test src/__tests__/PsoDistributeWizard.spec.ts 2>&1 | tail -10
git add src/components/modals/PsoDistributeWizard.vue src/__tests__/PsoDistributeWizard.spec.ts
git commit -m "feat(frontend): PsoDistributeWizard with GPU mismatch guard"
```

---

## Task 19: Rewrite PSOCache.vue + PsoFileExplorer + PsoJobCard

**Files:**
- Create: `src/components/pso/PsoFileExplorer.vue`
- Create: `src/components/pso/PsoJobCard.vue`
- Modify: `src/views/PSOCache.vue` (was Plan 1 stub)
- Create: `src/__tests__/PsoFileExplorer.spec.ts`
- Create: `src/__tests__/PsoJobCard.spec.ts`
- Create: `src/__tests__/PSOCache-view.spec.ts`

- [ ] **Step 1: `PsoJobCard.vue`** — reuses UecmTaskCard

```vue
<script setup lang="ts">
import { computed } from "vue";
import { UecmTaskCard } from "@/components/primitives";
import type { CollectJobState } from "@/stores/pso";

const props = defineProps<{ job: CollectJobState; sourceLabel: string; projectLabel: string }>();
const emit = defineEmits<{ (e: "cancel", id: string): void }>();

const taskStatus = computed(() => {
  switch (props.job.status) {
    case "queued":
    case "spawning": return "spawning" as const;
    case "collecting":
    case "completing": return "running" as const;
    case "completed": return "completed" as const;
    case "cancelled": return "cancelled" as const;
    case "error": return "error" as const;
  }
});

const progressLabel = computed(() => {
  if (props.job.status === "spawning") return "Spawning UE…";
  if (props.job.status === "collecting") return "Collecting PSOs (drive UE viewport manually)";
  if (props.job.status === "completing") return "Finalising…";
  return undefined;
});

const subtitle = computed(() => `${props.projectLabel} on ${props.sourceLabel}`);
</script>

<template>
  <UecmTaskCard
    :title="`PSO Collect — ${job.job_id.slice(-6)}`"
    :subtitle="subtitle"
    :status="taskStatus"
    :progress-pct="null"
    :progress-label="progressLabel"
    :error-message="job.error_message"
    @cancel="emit('cancel', job.job_id)"
  />
  <p v-if="job.files_collected != null" class="mt-1 px-1 text-xs text-muted-foreground" data-pso-job-files>
    {{ job.files_collected }} file(s) collected
  </p>
</template>
```

- [ ] **Step 2: `PsoFileExplorer.vue`** — left list + right detail

```vue
<script setup lang="ts">
import { computed, ref } from "vue";
import { UecmHorizontalSplit } from "@/components/primitives";
import type { PsoCacheFile } from "@/services/tauri";

const props = defineProps<{ files: PsoCacheFile[]; machineLabel: (id: number) => string }>();
const emit = defineEmits<{ (e: "distribute", file: PsoCacheFile): void }>();

const selectedId = ref<number | null>(null);
const selected = computed(() => props.files.find((f) => f.id === selectedId.value));

function fmtSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
</script>

<template>
  <div data-pso-file-explorer class="h-96 rounded-lg border bg-card overflow-hidden">
    <UecmHorizontalSplit :left-weight="2" :right-weight="3">
      <template #left>
        <ul>
          <li
            v-for="f in files"
            :key="f.id ?? f.file_name"
            class="cursor-pointer border-b px-3 py-2 text-xs"
            :class="f.id === selectedId ? 'bg-muted' : 'hover:bg-muted/40'"
            data-pso-file-row
            @click="selectedId = f.id"
          >
            <p class="font-mono">{{ f.file_name }}</p>
            <p class="text-muted-foreground">{{ fmtSize(f.size_bytes) }} • {{ machineLabel(f.source_machine_id) }}</p>
          </li>
        </ul>
        <p v-if="files.length === 0" class="p-4 text-xs text-muted-foreground" data-pso-file-empty>
          No collected PSO files yet for this project.
        </p>
      </template>
      <template #right>
        <div v-if="selected" class="space-y-2 p-3 text-xs">
          <p class="font-mono"><strong>Path:</strong> {{ selected.file_path }}</p>
          <p><strong>Size:</strong> {{ fmtSize(selected.size_bytes) }}</p>
          <p><strong>GPU signature:</strong> <code>{{ selected.gpu_signature }}</code></p>
          <p><strong>UE:</strong> {{ selected.ue_version ?? "?" }}</p>
          <p><strong>Source:</strong> {{ machineLabel(selected.source_machine_id) }}</p>
          <p><strong>Collected at:</strong> {{ selected.collected_at ?? "?" }}</p>
          <button
            class="mt-3 w-full rounded bg-primary px-3 py-2 text-sm text-primary-foreground"
            data-pso-file-distribute-btn
            @click="emit('distribute', selected)"
          >Distribute…</button>
        </div>
        <p v-else class="p-3 text-xs text-muted-foreground">Select a file to see details.</p>
      </template>
    </UecmHorizontalSplit>
  </div>
</template>
```

- [ ] **Step 3: `PSOCache.vue` rewrite**

```vue
<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useMachinesStore } from "@/stores/machines";
import { useProjectsStore } from "@/stores/projects";
import { usePsoStore } from "@/stores/pso";
import PsoCollectWizard from "@/components/modals/PsoCollectWizard.vue";
import PsoDistributeWizard from "@/components/modals/PsoDistributeWizard.vue";
import PsoJobCard from "@/components/pso/PsoJobCard.vue";
import PsoFileExplorer from "@/components/pso/PsoFileExplorer.vue";
import type { PsoCacheFile } from "@/services/tauri";

const machines = useMachinesStore();
const projects = useProjectsStore();
const pso = usePsoStore();

const showCollect = ref(false);
const showDistribute = ref(false);
const distFile = ref<PsoCacheFile | null>(null);
const selectedProjectId = ref<number | null>(null);

onMounted(async () => {
  await Promise.all([machines.loadMachines(), projects.load(), pso.attach()]);
});
onUnmounted(() => pso.detach());

watch(selectedProjectId, async (id) => {
  if (id != null) await pso.loadFiles(id);
});

const files = computed(() =>
  selectedProjectId.value != null ? pso.cacheFilesByProject[selectedProjectId.value] ?? [] : [],
);

function machineLabel(id: number) {
  const m = machines.machines.find((x) => x.id === id);
  return m ? m.hostname : `m#${id}`;
}
function projectLabel(id: number) {
  return projects.projects.find((p) => p.id === id)?.uproject_name ?? `proj#${id}`;
}

async function cancel(id: string) {
  await pso.cancelCollection(id);
}

function openDistribute(f: PsoCacheFile) {
  distFile.value = f;
  showDistribute.value = true;
}
</script>

<template>
  <div class="h-full space-y-6 overflow-auto p-6">
    <header class="flex items-center justify-between gap-4">
      <div>
        <p class="text-xs font-bold uppercase tracking-[0.18em] text-muted-foreground">Pipeline Caches</p>
        <h1 class="mt-1 font-display text-3xl font-extrabold">PSO Cache</h1>
      </div>
      <button data-pso-collect-btn class="rounded bg-primary px-3 py-2 text-sm text-primary-foreground" @click="showCollect = true">Collect PSO Cache…</button>
    </header>

    <section v-if="pso.collectJobs.length > 0" data-pso-jobs class="space-y-3">
      <h2 class="text-xs font-bold uppercase tracking-[0.18em] text-muted-foreground">Collection jobs</h2>
      <PsoJobCard
        v-for="job in pso.collectJobs"
        :key="job.job_id"
        :job="job"
        :source-label="machineLabel(job.source_machine_id)"
        :project-label="projectLabel(job.project_id)"
        @cancel="cancel"
      />
    </section>

    <section class="space-y-2">
      <h2 class="text-xs font-bold uppercase tracking-[0.18em] text-muted-foreground">Collected files</h2>
      <select v-model="selectedProjectId" data-pso-project-select class="w-full max-w-md rounded border bg-background px-3 py-2 text-sm">
        <option :value="null">Select a project…</option>
        <option v-for="p in projects.projects" :key="p.id" :value="p.id">{{ p.uproject_name }}</option>
      </select>
      <PsoFileExplorer
        v-if="selectedProjectId != null"
        :files="files"
        :machine-label="machineLabel"
        @distribute="openDistribute"
      />
    </section>

    <p
      v-if="pso.collectJobs.length === 0 && selectedProjectId == null"
      data-pso-cache-empty
      class="rounded-lg border bg-card p-6 text-sm text-muted-foreground"
    >No PSO Cache activity yet. Click "Collect PSO Cache" to launch UE in collection mode.</p>

    <PsoCollectWizard :open="showCollect" @close="showCollect = false" />
    <PsoDistributeWizard :open="showDistribute" :file="distFile" @close="showDistribute = false" />
  </div>
</template>
```

- [ ] **Step 4: View tests** (skeletons mirror Plan 5 T21 patterns)

```bash
git add src/components/pso/PsoJobCard.vue src/components/pso/PsoFileExplorer.vue src/views/PSOCache.vue
git add src/__tests__/PsoFileExplorer.spec.ts src/__tests__/PsoJobCard.spec.ts src/__tests__/PSOCache-view.spec.ts
git commit -m "feat(frontend): PSOCache.vue + PsoJobCard + PsoFileExplorer"
```

(Engineer fills the test files following the established `mount + mock listen + assert empty/full states` template; if time-pressed they can ship 2 tests per file as minimum coverage.)

---

## Task 20: Health Check rewire — #10 (PSO CVar) + #11 (GPU consistency)

**Files:**
- Modify: `src/stores/healthCheck.ts` (rewire derived checks)
- Modify: `src/lib/healthChecks.ts` (metadata refresh)
- Modify: `src/views/HealthCheck.vue` (matrix detail panel — link to PSO + GPU views)
- Create: `src/__tests__/healthChecks-rewire.spec.ts`

The Plan 4 implementation of #10 and #11 had inline shims (e.g. computed inside the matrix component). Plan 6 promotes them to the dedicated stores. The Tauri command surface for health check (Plan 4) remains unchanged; the **frontend store** layer is what changes.

- [ ] **Step 1: Update `useHealthCheckStore` derivations**

In `src/stores/healthCheck.ts`, find where `#10` (PSO Precaching CVar) and `#11` (GPU consistency) cells are computed. Replace inline logic with:

```typescript
// At the top:
import { useGpuConsistencyStore } from "./gpuConsistency";
import { useDiagnosticsStore } from "./diagnostics";

// Inside derive function (or wherever the matrix is built):
const gpu = useGpuConsistencyStore();
const diag = useDiagnosticsStore();

// PSO CVar status: derive from latest scan, look for any R008/R009/R010 finding per machine.
function deriveCheck10(machineId: number): "match" | "deviation" | "unknown" {
  const findings = diag.findings.filter(
    (f) => f.machine_id === machineId && ["R008", "R009", "R010"].includes(f.rule_id),
  );
  if (findings.length === 0) return "match"; // no findings = healthy
  if (findings.some((f) => f.severity === "critical")) return "deviation";
  return "deviation"; // any warning still counts as a deviation for the matrix
}

function deriveCheck11(machineId: number): "match" | "deviation" | "unknown" {
  const cell = gpu.matrix?.cells.find((c) => c.machine_id === machineId);
  if (!cell) return "unknown";
  return cell.status === "match" ? "match" : cell.status === "deviation" ? "deviation" : "unknown";
}
```

These functions plug into the matrix-building logic introduced in Plan 4 T15 — find the `buildMatrix` (or equivalent) function and call them where #10 / #11 cells are populated.

- [ ] **Step 2: Update `lib/healthChecks.ts` metadata**

Refresh the `id: "pso_cvar"` and `id: "gpu_consistency"` rows so their `subtitle` says "Plan 6: derived from INI scanner R008-R010" / "Plan 6: derived from gpu_consistency module".

- [ ] **Step 3: Modify HealthCheck view detail panel**

In `src/views/HealthCheck.vue`, when a `#10` cell is clicked, push to `/ini-scanner?finding=R008` (or use Plan 4's existing route pattern). When `#11` is clicked, push to `/health-check?gpu=true` to anchor the dedicated GPU matrix section (or reveal an inline matrix using `UecmGpuMatrix`).

Append to `HealthCheck.vue` template a dedicated GPU matrix section below the main matrix:

```vue
<section class="space-y-2" data-health-gpu-section>
  <h2 class="text-xs font-bold uppercase tracking-[0.18em] text-muted-foreground">GPU / driver matrix</h2>
  <p class="text-xs text-muted-foreground">Baseline: {{ gpuStore.baselineLabel }} • {{ gpuStore.deviationCount }} machine(s) deviating</p>
  <UecmGpuMatrix :matrix="gpuStore.matrix" />
</section>
```

Add corresponding `import { useGpuConsistencyStore } from "@/stores/gpuConsistency";` and `const gpuStore = useGpuConsistencyStore();` in the script. Call `gpuStore.load()` in `onMounted` alongside the existing health check load.

- [ ] **Step 4: Test**

`src/__tests__/healthChecks-rewire.spec.ts`:

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    getGpuConsistencyMatrix: vi.fn(),
  },
}));
vi.mock("@/services/tauri", () => ({ tauriApi: mockApi }));

import { useGpuConsistencyStore } from "@/stores/gpuConsistency";

describe("Health Check rewire — derive #11 from gpu store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mockApi.getGpuConsistencyMatrix.mockReset();
  });

  it("populates baseline and deviation count once loaded", async () => {
    mockApi.getGpuConsistencyMatrix.mockResolvedValue({
      signatures: [[{ vendor: "nvidia", model: "RTX 3080", driver: "535.98" }, 1]],
      baseline: { vendor: "nvidia", model: "RTX 3080", driver: "535.98" },
      cells: [
        { machine_id: 1, hostname: "A", signature: { vendor: "nvidia", model: "RTX 3080", driver: "535.98" }, status: "match" },
        { machine_id: 2, hostname: "B", signature: null, status: "unknown" },
      ],
    });
    const s = useGpuConsistencyStore();
    await s.load();
    expect(s.matrix?.cells.find((c) => c.machine_id === 2)?.status).toBe("unknown");
  });
});
```

- [ ] **Step 5: Run + commit**

```bash
pnpm test src/__tests__/healthChecks-rewire.spec.ts 2>&1 | tail -10
git add src/stores/healthCheck.ts src/lib/healthChecks.ts src/views/HealthCheck.vue src/__tests__/healthChecks-rewire.spec.ts
git commit -m "feat(frontend): HealthCheck rewire #10/#11 to gpu_consistency + diagnostics stores"
```

---

## Task 21: Dashboard rewrite + cross-view polish baseline

**Files:**
- Modify: `src/views/Dashboard.vue` (rewrite)
- Modify: `src/views/Machines.vue` (polish)
- Modify: `src/views/Projects.vue` (polish)
- Modify: `src/views/DDCPak.vue` (polish)
- Modify: `src/views/INIScanner.vue` (polish)
- Modify: `src/views/Shares.vue` (polish)
- Modify: `src/views/HealthCheck.vue` (polish)
- Create: `src/__tests__/Dashboard-view.spec.ts`

This task lands the **shared empty/loading/error pattern** across all views and rewrites the Dashboard to surface live KPIs.

- [ ] **Step 1: Define a shared "empty/loading/error" component pair**

Create `src/components/primitives/UecmStateBlock.vue`:

```vue
<script setup lang="ts">
defineProps<{
  variant: "empty" | "loading" | "error";
  title?: string;
  message?: string;
  retryLabel?: string;
}>();
const emit = defineEmits<{ (e: "retry"): void }>();
</script>

<template>
  <div
    class="rounded-lg border p-6 text-sm"
    :class="{
      'bg-card text-muted-foreground': variant !== 'error',
      'border-rose-300 bg-rose-50/50 text-rose-800 dark:bg-rose-900/20': variant === 'error',
    }"
    :data-state-block="variant"
  >
    <p v-if="title" class="font-semibold">{{ title }}</p>
    <p v-if="message" class="mt-1 text-xs">{{ message }}</p>
    <button
      v-if="retryLabel"
      class="mt-3 rounded border border-border px-3 py-1 text-xs"
      data-state-block-retry
      @click="emit('retry')"
    >{{ retryLabel }}</button>
  </div>
</template>
```

Register in `primitives/index.ts`.

- [ ] **Step 2: Apply UecmStateBlock to all 6 view-level states**

Each of the 6 affected views currently has its own `data-{view}-empty` rendering. Replace those with `<UecmStateBlock variant="empty" :title="..." :message="..." />` while preserving the `data-{view}-empty` selector via the auto `data-state-block="empty"` (sufficient for tests OR add a per-view wrapper with the original selector).

This is mechanical — engineer goes view by view, replaces the `<p data-X-empty>` elements with the new primitive, runs the existing test suite, and commits.

- [ ] **Step 3: Rewrite `Dashboard.vue`**

```vue
<script setup lang="ts">
import { computed, onMounted } from "vue";
import { RouterLink } from "vue-router";
import { useMachinesStore } from "@/stores/machines";
import { useProjectsStore } from "@/stores/projects";
import { useDdcPakStore } from "@/stores/ddcPak";
import { usePsoStore } from "@/stores/pso";
import { useHealthCheckStore } from "@/stores/healthCheck";
import { useGpuConsistencyStore } from "@/stores/gpuConsistency";

const machines = useMachinesStore();
const projects = useProjectsStore();
const ddcPak = useDdcPakStore();
const pso = usePsoStore();
const health = useHealthCheckStore();
const gpu = useGpuConsistencyStore();

onMounted(async () => {
  await Promise.all([
    machines.loadMachines(),
    projects.load(),
    gpu.load(),
  ]);
});

const onlineCount = computed(() => machines.machines.filter((m) => m.status === "online").length);
const lastDdcJob = computed(() => ddcPak.generateJobs[0] ?? null);
const lastPsoJob = computed(() => pso.collectJobs[0] ?? null);
</script>

<template>
  <div class="h-full space-y-6 overflow-auto p-6">
    <header>
      <p class="text-xs font-bold uppercase tracking-[0.18em] text-muted-foreground">Mission Control</p>
      <h1 class="mt-1 font-display text-3xl font-extrabold">Dashboard</h1>
    </header>

    <section data-dashboard-kpi class="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
      <RouterLink to="/machines" class="rounded-lg border bg-card p-4 hover:bg-muted/40">
        <p class="text-xs uppercase tracking-wide text-muted-foreground">Machines online</p>
        <p class="mt-2 text-3xl font-extrabold">{{ onlineCount }} <span class="text-base font-normal text-muted-foreground">/ {{ machines.machines.length }}</span></p>
      </RouterLink>
      <RouterLink to="/projects" class="rounded-lg border bg-card p-4 hover:bg-muted/40">
        <p class="text-xs uppercase tracking-wide text-muted-foreground">Projects discovered</p>
        <p class="mt-2 text-3xl font-extrabold">{{ projects.projects.length }}</p>
      </RouterLink>
      <RouterLink to="/health" class="rounded-lg border bg-card p-4 hover:bg-muted/40">
        <p class="text-xs uppercase tracking-wide text-muted-foreground">GPU baseline</p>
        <p class="mt-2 text-sm font-mono">{{ gpu.baselineLabel }}</p>
        <p class="text-xs text-amber-600">{{ gpu.deviationCount }} deviation(s)</p>
      </RouterLink>
      <RouterLink to="/ddc-pak" class="rounded-lg border bg-card p-4 hover:bg-muted/40">
        <p class="text-xs uppercase tracking-wide text-muted-foreground">Last DDC pak</p>
        <p class="mt-2 text-sm">{{ lastDdcJob ? `${lastDdcJob.status} • ${lastDdcJob.job_id.slice(-6)}` : "—" }}</p>
      </RouterLink>
    </section>

    <section data-dashboard-recent class="grid gap-4 md:grid-cols-2">
      <RouterLink to="/pso-cache" class="rounded-lg border bg-card p-4 hover:bg-muted/40">
        <p class="text-xs uppercase tracking-wide text-muted-foreground">Last PSO collection</p>
        <p class="mt-2 text-sm">{{ lastPsoJob ? `${lastPsoJob.status} • ${lastPsoJob.files_collected ?? "?"} file(s)` : "—" }}</p>
      </RouterLink>
      <RouterLink to="/health" class="rounded-lg border bg-card p-4 hover:bg-muted/40">
        <p class="text-xs uppercase tracking-wide text-muted-foreground">Cluster health</p>
        <p class="mt-2 text-sm">{{ health.score != null ? `${health.score}% healthy` : "Run a health check" }}</p>
      </RouterLink>
    </section>
  </div>
</template>
```

- [ ] **Step 4: Test**

`src/__tests__/Dashboard-view.spec.ts`:

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    listMachines: vi.fn().mockResolvedValue([
      { id: 1, hostname: "A", ip: "1.1.1.1", role: "render", status: "online", last_seen_at: null },
    ]),
    listProjects: vi.fn().mockResolvedValue([]),
    getGpuConsistencyMatrix: vi.fn().mockResolvedValue({ signatures: [], baseline: null, cells: [] }),
  },
}));
vi.mock("@/services/tauri", () => ({ tauriApi: mockApi }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => undefined) }));

import { createMemoryHistory, createRouter } from "vue-router";
import Dashboard from "@/views/Dashboard.vue";

describe("Dashboard.vue", () => {
  beforeEach(() => setActivePinia(createPinia()));

  it("renders KPI grid with machines online count", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/", component: { template: "<div />" } }],
    });
    const w = mount(Dashboard, { global: { plugins: [router] } });
    await new Promise((r) => setTimeout(r, 0));
    await w.vm.$nextTick();
    expect(w.find('[data-dashboard-kpi]').exists()).toBe(true);
  });
});
```

- [ ] **Step 5: Run + commit**

```bash
pnpm test src/__tests__/Dashboard-view.spec.ts 2>&1 | tail -10
git add src/views/Dashboard.vue src/views/Machines.vue src/views/Projects.vue src/views/DDCPak.vue src/views/INIScanner.vue src/views/Shares.vue src/views/HealthCheck.vue src/components/primitives/UecmStateBlock.vue src/components/primitives/index.ts src/__tests__/Dashboard-view.spec.ts
git commit -m "feat(frontend): Dashboard rewrite + UecmStateBlock + cross-view empty-state polish"
```

---

## Task 22: Visual polish — apply final tokens

**Files:**
- Modify: `tailwind.config.js` (final token wire-up)
- Modify: `src/assets/tokens.css` (if landed) OR rename `--uecm-interim-*` → `--uecm-*`
- Modify: each affected view (font sizes, spacing nits)

Behaviour gate: if final tokens exist, apply them; if not, promote interim. **Do not** redesign — only consolidate.

- [ ] **Step 1: Confirm token state**

```bash
ls src/assets/tokens.css 2>/dev/null && echo "FINAL" || echo "INTERIM"
```

If `FINAL`: continue Step 2. If `INTERIM`: skip to Step 3.

- [ ] **Step 2: Wire final tokens**

Open `tailwind.config.js` `theme.extend.colors` block; replace each `var(--uecm-interim-X)` with `var(--uecm-X)`. Add the `tokens.css` import to `src/main.ts` (or wherever the global CSS imports live).

- [ ] **Step 3: Promote interim tokens**

If no final `tokens.css`: in `tailwind.config.js`, search-replace `interim-` → `` to keep var names stable. Update CSS-vars file accordingly. Document this deferral in the README v1.0 release notes (T25).

- [ ] **Step 4: Visual sweep on each view**

This step is screenshot-driven. Engineer launches the app, navigates each view in turn, and captures pre/post screenshots. Adjust spacing-class outliers (any `m-{n}` where `n` doesn't follow the 4/6/8 scale) to nearest token. **Do not** rewrite layouts — just tighten the rough edges. Time-box: 1 hour.

- [ ] **Step 5: Run full test suite**

```bash
pnpm test 2>&1 | tail -5
cd src-tauri && cargo test --lib 2>&1 | tail -5 && cd ..
```

Expected: still all green. The only acceptable test breakages are if a `data-X` selector was renamed in a view sweep — fix the test to match.

- [ ] **Step 6: Commit**

```bash
git add tailwind.config.js src/assets/ src/main.ts src/views/ src/components/
git commit -m "polish: final design tokens applied across all 7 views"
```

---

## Task 23: Final integration — README v1.0 + production build smoke + release notes

**Files:**
- Modify: `README.md`
- Create: `docs/superpowers/changelog/2026-05-XX-v1.0.md`

- [ ] **Step 1: Update README**

```markdown
# UE Cache Manager (UECM)

**Status:** v1.0 — feature-complete (Plans 1-6).

## What's working

- Local + remote machine discovery and inventory
- Per-call WinRM credentials (no UAC popup; admin operations explicit)
- SMB share creation (Mode A open + Mode B managed `ddc-svc`)
- SYSTEM-context credential injection for Service-account access
- Cluster batch ops (env vars, INI keys) with per-machine progress
- INI conflict scanner + 11-rule diagnostic engine + one-click apply-fix
- Cluster health check matrix (11 checks, sticky headers, derived cells)
- DDC Pak generation (UE -run=DerivedDataCache) with cancel
- DDC Pak distribution via Robocopy + admin-share UNC, multi-target
- Project discovery + 3-level identity matcher (filename / manual alias / manual path)
- PSO Cache file collection (UE -game mode) with watchdog
- PSO Cache distribution with GPU-mismatch guard
- GPU/driver consistency matrix
- Visual polish pass (shared empty/loading/error states, Dashboard KPIs)

## Architecture

[…inline-link to docs/superpowers/specs/2026-05-01-uecm-design.md…]

## Development

[…unchanged…]

## Release notes

See `docs/superpowers/changelog/2026-05-XX-v1.0.md`.
```

- [ ] **Step 2: Write release notes**

Create `docs/superpowers/changelog/2026-05-XX-v1.0.md` (replace `XX` with today's day):

```markdown
# UECM v1.0 — Release Notes (2026-05-XX)

This release closes the v1 design intent: any Render Node in a VP/XR cluster can open a UE project with **zero shader recompile** (DDC) and **zero scene-switch hitch** (PSO Cache).

## Plans landed

- Plan 1: Foundation (Tauri + SQLite + 7 stub views)
- Plan 2: Discovery & single-machine config
- Plan 3: Elevation, SMB shares, cluster batch
- Plan 4: INI scanner + cluster health matrix (11 checks)
- Plan 5: DDC Pak generation + distribution + project identity
- Plan 6: PSO Cache (CVar verify + collect + distribute) + GPU consistency + visual polish

## Subsystems

[short bullet list per module — copy from each plan's deliverable section]

## Known limitations

- AMD / Intel GPUs untested in CI (NVIDIA-validated only)
- PSO collection requires manual viewport driving (no auto-flythrough yet)
- Robocopy is the only transport (no chunked / resumable)
- Single operator model — no multi-user audit / RBAC
- macOS / Linux operator UECM not supported (Windows-only)

## Deferred to v1.1+

[bullet list of TODOs noted across plans]
```

- [ ] **Step 3: Production build**

```bash
cd src-tauri && cargo build --release 2>&1 | tail -5 && cd ..
```

(Or full Tauri bundle on lanPC; gate by environment.)

- [ ] **Step 4: Final commit**

```bash
git add README.md docs/superpowers/changelog/2026-05-XX-v1.0.md
git commit -m "$(cat <<'EOF'
docs: UECM v1.0 release notes

Closes Plans 1-6. Feature-complete for VP/XR cluster scenarios.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 24: lanPC E2E — full Plan 6 verification

**Files:** none modified (verification only).

- [ ] **Step 1: Build + launch**

```bash
ssh lanpc "cd E:\code\super_lanPC_uecm\ue-cache-manager; pnpm tauri build" 2>&1 | tail -10
```

- [ ] **Step 2: Verify a real GPU is present in the matrix**

Open UECM → Health Check → confirm GPU matrix shows lanPC's RTX (3080 or whatever ships). If the cell is `unknown`, rerun Refresh Machine on lanPC.

- [ ] **Step 3: F1 — verify PSO Precaching CVars**

Click Health Check → run scan with project path `E:\test-fixtures\PluralityProject`. The R008 finding should appear if `ConsoleVariables.ini` lacks `r.PSOPrecaching=1`.

Click "Apply suggestion" on R008. Re-scan; finding should be gone.

- [ ] **Step 4: F2 — collect PSO cache**

Navigate to PSO Cache → Collect PSO Cache… → step 1: lanPC → step 2: PluralityProject → step 3: 1280×720, windowed, max 3 minutes → step 4: Run.

Expected:
- Job card shows `spawning` then `collecting`.
- UE.exe spawns visibly on lanPC's display.
- Manually drive the camera in UE for ~2 minutes (Wasd + mouse).
- Click "Cancel" on the job card OR let the watchdog fire at 3 minutes.
- Status flips to `completed`; `files_collected: 1` (or more) appears.
- `E:\test-fixtures\PluralityProject\Saved\CollectedPSOs\PluralityProject_5.4.upipelinecache` exists.

- [ ] **Step 5: F3 — distribute (single-target loopback)**

In the file explorer (right pane after picking the project): select the .upipelinecache → "Distribute…" → wizard.

If only lanPC is in the inventory: register `127.0.0.1` as a second "machine" + add a second project location pointing at a writable temp path on lanPC (e.g. `C:\tmp\loopback-target`). Run distribute.

Expected: Robocopy copies the file; per-target row flips through `pending → running → ok`.

- [ ] **Step 6: F4 — GPU consistency matrix**

Health Check view → GPU section. Confirm baseline + cells render. If only lanPC: 1 row, 1 cell, all match.

- [ ] **Step 7: Polish smoke**

Click through all 7 views in turn. Verify:
- Each view renders with the same empty/loading/error treatment (`UecmStateBlock`).
- Dashboard KPI tiles render and link through.
- No console errors in DevTools.

- [ ] **Step 8: Record results**

Note in T25 commit:
- Final test count (frontend + backend)
- Production build outcome
- E2E pass/fail per F1/F2/F3/F4 sub-step
- Polish-pass screenshot URLs (if posted to PR)

---

## Task 25: Push branch + open PR

- [ ] **Step 1: Final test run**

```bash
pnpm test 2>&1 | tail -5
cd src-tauri && cargo test --lib 2>&1 | tail -5 && cd ..
```

- [ ] **Step 2: Push**

```bash
git push -u origin codex/plan-6-pso-cache
```

- [ ] **Step 3: Open PR**

```bash
gh pr create --title "Plan 6: PSO Cache + visual polish (closes UECM v1.0)" --body "$(cat <<'EOF'
## Summary
- F1: PSO Precaching CVar verification (R008-R010 rules)
- F2: PSO Cache collection via UE -game mode (`core::pso_collect` reusing Plan 5 ue_runner)
- F3: PSO Cache distribution with GPU mismatch guard (`core::pso_distribute`)
- F4: GPU/driver consistency matrix (`core::gpu_consistency`)
- Visual polish: Dashboard rewrite + shared empty/loading/error primitive (`UecmStateBlock`)

## Test plan
- [ ] All Rust unit tests pass on macOS (no Windows fixtures needed for pure modules)
- [ ] All Vitest tests pass
- [ ] lanPC E2E: F1/F2/F3/F4 complete (see commit log)
- [ ] Production build green
- [ ] No regressions in Plan 5 DDC Pak flow (sanity check on same lanPC)

## Subsystems closed
This PR closes UECM v1.0 — Plans 1 through 6.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

Do NOT merge — wait for code review.

---

## Plan 6 — Done

Total: 25 tasks. Implementation surface:

- **Backend**: ~1500 lines Rust across 5 new modules (`pso_collect`, `pso_distribute`, `gpu_consistency` + 2 data CRUD) + 3 INI rule additions + 3 PowerShell sidecars + 2 migrations.
- **Frontend**: ~1800 lines Vue/TS across 2 stores, 3 primitives (incl. `UecmStateBlock`), 2 wizards, 2 PSO sub-components, Dashboard rewrite, cross-view polish, HealthCheck rewire.
- **Tests**: ~25 new unit/component tests.
- **lanPC E2E**: 8-step verification covering F1/F2/F3/F4 and polish smoke.

**At end of Plan 6**: UECM v1.0 ships. The two missing pieces of the original VP/XR design (DDC + PSO) are both wired end-to-end. Plan 7 (post-v1) will revisit AMD/Intel matrix coverage, automated PSO flythrough, and chunked Robocopy resumability if needed.
