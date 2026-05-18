# DDC Full Coverage Implementation Plan (v2 — API-corrected)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Revision Note:** v2 supersedes v1 (commit `c23079f`) after Codex review surfaced API mismatches that would fail `cargo check`. All snippets in this file have been verified against current source. Baseline `cargo check` passes from `c23079f`. Each task that adds a CLI `Domain` variant **must** also extend `needs_db()` — see Module M0.

**Goal:** Make UECM detect every DDC-relevant configuration source listed in the RenderStream Shader DDC/PSO SOP (excluding ZenServer), and provide a one-click deployment workflow that provisions Local DDC + Shared SMB + DDC Pak + PSO on a cluster from a single wizard or CLI command.

**Architecture:** Four phases. Phase 0 (M0, 0.5 day) locks the API contract surface and validates that today's `read-ini-file.ps1` already preserves tuple values intact. Phase 1 (P0, 17 days) lands the five must-have modules: `[DerivedDataBackendGraph]` parser + rule set, `LogDerivedDataCache` startup log verifier, complete Local DDC configuration path, RenderStream service-account probe, and the `deploy` orchestrator. Phase 2 (P1, 11 days) adds Editor Preferences four-path reader, PSO completeness, cluster consistency check, GC strategy toggles, file-count probe, and command-line argument scanner. Phase 3 (P2, 3.5 days) adds baseline log snapshot, UE 5.3 / 5.4+ default-path awareness, and historical-symptom recognizer.

Module ordering inside a phase: M0 → M1 → M2,M3,M4 (parallel) → M5 (depends on M1+M2+M3+M4). M6–M11 parallel after M0. M12–M14 parallel after M2 (M14 depends on M2).

**Tech Stack:** Rust (Tauri 2 backend, clap-derive CLI), PowerShell 5.1 sidecars via `core::powershell::run_json`, SQLite via `data::*` (re-exported through `data::Db`), Vue 3 SFC + Tailwind + reka-ui + CVA, vue-i18n.

---

## API Contract Reference (verified against `c23079f`)

These are the real signatures that v2 snippets target. Engineer must re-grep before each task to confirm none has shifted.

### Error type — `src-tauri/src/error.rs`

```rust
pub enum UecmError {
    Io(#[from] std::io::Error),                 // NOT Io(String)
    PowerShell(String),
    OperationFailed(String),
    InvalidInput(String),
    // ... etc
}
pub type UecmResult<T> = Result<T, UecmError>;
```

When constructing a fs/io error from string context use `UecmError::OperationFailed(format!("read {}: {}", path, e))`, not `UecmError::Io(...)`.

### Database handle — `src-tauri/src/data/mod.rs`

```rust
pub use connection::{open, open_in_memory, Db};  // re-export
pub use credentials::{CredentialKind, CredentialRecord};
pub use projects::Project;
pub use project_locations::{DiscoveryStatus, ProjectLocation};
```

`Project` does **not** have a `path` field. Per-machine project root lives in `ProjectLocation`:

```rust
pub struct ProjectLocation {
    pub project_id: i64,
    pub machine_id: i64,
    pub abs_path: String,       // <-- project root directory on that machine
    pub uproject_path: String,
    // ...
}
pub fn get_for_project_machine(db: &Db, project_id: i64, machine_id: i64) -> UecmResult<Option<ProjectLocation>>
```

### Credential lookup — Tauri command pattern

Real pattern (see `src-tauri/src/commands/ini_editor.rs`):

```rust
use crate::data::{credentials as data_credentials, Db};
use crate::core::credentials as core_credentials;
use tauri::State;

#[tauri::command]
pub fn whatever(db: State<'_, Db>, credential_alias: String) -> UecmResult<...> {
    let cred = data_credentials::find_by_alias(&db, &credential_alias)?
        .ok_or_else(|| UecmError::InvalidInput(format!("credential '{}' not found", credential_alias)))?;
    let password = core_credentials::resolve_password(&credential_alias)?;
    let username = cred.username;
    // use (username, password)
}
```

There is **no** `data::db::open()` or `credentials::load_decrypted()` — those are inventions.

### CLI credential resolution — `src-tauri/src/cli/credential_args.rs`

```rust
pub fn preflight(&self, db: &Db) -> UecmResult<()>
pub fn resolve(&self, db: &Db) -> UecmResult<Option<(String, String)>>  // (user, pass)
```

### HostTarget — `src-tauri/src/cli/host_args.rs`

```rust
pub enum HostTarget { Single(String), Batch(Vec<String>) }
impl HostArgs { pub fn require_one(&self) -> UecmResult<HostTarget> }
```

`HostTarget::as_vec()` does NOT exist — when you want a `Vec<String>` from a `HostTarget`, write `match` inline:

```rust
let hosts: Vec<String> = match target.require_one()? {
    HostTarget::Single(h) => vec![h],
    HostTarget::Batch(hs) => hs,
};
```

### INI scan path — `src-tauri/src/core/ini_scanner.rs`

```rust
pub fn enumerate_engine_paths(installs: &[(String, String)]) -> Vec<TargetFile>
pub fn enumerate_user_paths(installs: &[(String, String)], user_profile: &str) -> Vec<TargetFile>
pub fn enumerate_project_paths(project_roots: &[String]) -> Vec<TargetFile>
pub fn read_file(host: &str, target: &TargetFile, creds: Option<(&str,&str)>) -> UecmResult<RawFileRead>
fn parse_ini_contents(target: &TargetFile, contents: &str) -> ParsedFile   // PRIVATE
pub fn scan_machine(inputs: &ScanInputs) -> UecmResult<ScanOutcome>
```

There is **no** `parse_target`. Local parser is `parse_ini_contents`. Remote path goes through `read_file` which calls `read-ini-file.ps1` and returns sections+keys but **preserves the original value text** for tuple keys. M1.2 leverages this by post-processing keys whose value starts with `(`.

### read-ini-file.ps1 — `ps-scripts/read-ini-file.ps1`

Today's output shape:

```json
{ "ok": true, "found": true, "sections": [{ "name": "...", "keys": [{ "name", "value", "line_number" }] }] }
```

`Shared=(K1=V1, K2=V2)` is parsed as `name="Shared", value="(K1=V1, K2=V2)"` — the parens stay in `value`. We recover BackendGraph nodes by re-parsing this value in Rust. M0.2 confirms this behaviour via a regression test.

### DDC pak — `src-tauri/src/core/ddc_pak.rs`

```rust
pub fn preflight(project_dir: &str, host: &str, creds: Option<(&str,&str)>) -> UecmResult<PreflightReport>
pub fn launch_generation(/* see ddc_pak.rs:129 for full signature */) -> UecmResult<GenerationHandle>
pub fn verify_output(host: &str, project_dir: &str, creds: Option<(&str,&str)>) -> UecmResult<PakOutput>
pub struct PakOutput { /* exists; see ddc_pak.rs:33 */ }
```

There is **no** `ddc_pak::generate(...)` convenience. The single-shot flow used by `commands/ddc_pak.rs` is `preflight → launch_generation → poll → verify_output`. For deploy_workflow we wrap this in `deploy_workflow::generate_pak_sync(...)`.

### Pak distribute — `src-tauri/src/core/pak_distribute.rs`

```rust
pub struct DistributeProfile { pub file_glob: String, /* ... */ }
impl DistributeProfile {
    pub fn ddc_pak() -> Self { Self { file_glob: "*.ddp".into(), /* ... */ } }
    pub fn pso_cache() -> Self { Self { file_glob: "*.upipelinecache".into(), /* ... */ } }
}
pub fn plan(/* ... */) -> UecmResult<Vec<DistributePlanItem>>
pub async fn preflight_one_with_profile(profile: &DistributeProfile, item: &DistributePlanItem) -> UecmResult<()>
pub async fn run_one_with_profile(profile: &DistributeProfile, item: DistributePlanItem) -> UecmResult<DistributeOutcome>
```

There is **no** `plan_one_target`. Single-target run = `plan(...)` then `.into_iter().find(...)`. PSO `file_glob` lives in `DistributeProfile::pso_cache()` — not in `pso_distribute.rs`.

### PSO collect — `src-tauri/src/core/pso_collect.rs`

```rust
pub fn build_ue_args(spec: &PsoCollectSpec) -> Vec<String>
pub fn launch_collection(/* see pso_collect.rs:77 */) -> UecmResult<CollectHandle>
pub fn enumerate_remote(host: &str, project_dir: &str, creds: Option<(&str,&str)>) -> UecmResult<Vec<CollectedFile>>
pub fn finalize_persist(/* ... */) -> UecmResult<...>
pub fn gpu_signature_for_machine(db: &Db, machine_id: i64) -> UecmResult<String>
```

No `collect_blocking`. Real flow: `launch_collection → wait → enumerate_remote → finalize_persist`.

### PSO distribute — `src-tauri/src/core/pso_distribute.rs`

```rust
pub fn plan(/* ... */) -> UecmResult<Vec<PsoDistributePlanItem>>
pub async fn preflight_one(item: &PsoDistributePlanItem) -> UecmResult<()>
pub async fn run_one(item: PsoDistributePlanItem) -> UecmResult<PsoDistributeOutcome>
```

### Health probe shape — `src-tauri/src/core/health_check.rs` + `health_probes.rs`

```rust
pub struct CheckOutcome { pub status: String, pub message: String, pub sample: String }
pub struct ProbeResults { pub results: HashMap<String, CheckOutcome>, /* ... */ }

pub fn run_for_host(/* ... */) -> UecmResult<HashMap<String, CheckOutcome>>
```

**Do not change `CheckOutcome` shape.** When adding probes (`env_local`, `rs_service`), surface them as **additional `HashMap` keys**, each with their own `CheckOutcome`. Detailed payloads (e.g. RS service list) go through separate dedicated Tauri commands.

### `cli/run.rs::needs_db`

```rust
fn needs_db(cmd: &Domain) -> bool {
    matches!(cmd,
        Domain::Machine { .. } | Domain::Winrm { .. } | Domain::Cred { .. } |
        Domain::Env { .. } | Domain::Ini { .. } | Domain::Share { .. } |
        Domain::Project { .. } | Domain::Health { .. } | Domain::Gpu { .. } |
        Domain::Ddc { .. } | Domain::Pso { .. }
        // <-- any new Domain variant MUST appear here
    )
}
```

Every task that adds a `Domain` variant has a dedicated step "extend `needs_db()`".

### Cargo dependencies — `src-tauri/Cargo.toml`

`futures` crate is **not** declared. Async coordination uses `tokio` (already there). To block on async from sync context:

```rust
match tokio::runtime::Handle::try_current().ok() {
    Some(h) => h.block_on(async_fn()),
    None => {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| UecmError::OperationFailed(format!("tokio rt: {}", e)))?;
        rt.block_on(async_fn())
    }
}
```

---

## File Structure

### `src-tauri/src/core/`

| File | Action | Responsibility |
|---|---|---|
| `ini_backend_graph.rs` | CREATE | Tuple parser preserving original field order (`Vec<(String,String)>`) |
| `ini_diagnostics.rs` | MODIFY | Add R011–R025, extend `ParsedSection` with `backend_nodes` |
| `ini_scanner.rs` | MODIFY | Post-process tuple-shaped keys into `backend_nodes`; expose `parse_for_diagnostics` wrapper |
| `ini_editor.rs` | MODIFY | `set_backend_field_with_credential` (local + remote, order-preserving) |
| `ue_log_parser.rs` | CREATE | Pure parser for `LogDerivedDataCache` lines |
| `ue_log_verify.rs` | CREATE | Per-host verify orchestrator |
| `editor_preferences.rs` | CREATE (stub in M1.3, full in M6.1) | Read 4 DDC paths from `EditorPerProjectUserSettings.ini` |
| `local_cache.rs` | CREATE | Provision local DDC dir on remote host |
| `renderstream_service.rs` | CREATE | RS service introspection + risk classifier + `into_check_outcome` helper |
| `consistency_check.rs` | CREATE | Cross-machine snapshot + compare |
| `command_line_scanner.rs` | CREATE | Scan shortcuts/bat/services for DDC override args |
| `baseline_log_snapshot.rs` | CREATE | Copy logs to archive dir |
| `ddc_symptom_recognizer.rs` | CREATE | Map verify report + stats to advisories |
| `ddc_file_stats.rs` | CREATE | Local/Shared file-count + size probe + imbalance classifier + UE-version default-path resolver |
| `deploy_workflow.rs` | CREATE | Plan + step executor (takes `&Db` injected, never opens DB itself) |
| `pso_collect.rs` | MODIFY | Capture both `.upipelinecache` and `.stablepc.csv` (filename suffix check) |
| `health_probes.rs` | MODIFY | Pass through `ExpectedLocalDataCachePath`; emit `env_local` + `rs_service` keys |

### `src-tauri/src/cli/`

| File | Action | Responsibility |
|---|---|---|
| `args.rs` | MODIFY | Add `Domain::Deploy`, `Domain::LocalCache`, `Domain::Log`; add `IniAction::BackendGraph` |
| `run.rs` | MODIFY | Extend `needs_db()` for 3 new domains; dispatch them |
| `domain_deploy.rs` | CREATE | `deploy ddc` handler |
| `domain_local_cache.rs` | CREATE | `local-cache create` handler |
| `domain_log.rs` | CREATE | `log verify-startup` + `log snapshot-baseline` |
| `domain_ini.rs` | MODIFY | `ini backend-graph get/set/scan` + `ini gc pause/resume` |
| `domain_health.rs` | MODIFY | `health consistency-check`, `health scan-command-line` |

### `src-tauri/src/commands/`

| File | Action | Responsibility |
|---|---|---|
| `deploy.rs` | CREATE | `deploy_ddc_run`, `deploy_ddc_plan_preview` |
| `local_cache.rs` | CREATE | `create_local_cache_dir`, `read_local_cache_state` |
| `log_verify.rs` | CREATE | `run_log_verify`, `analyze_advisories` |
| `consistency.rs` | CREATE | `run_consistency_check` |
| `editor_prefs.rs` | CREATE | `read_editor_prefs` |
| `renderstream_service.rs` | CREATE | `get_renderstream_services` (detailed array for UI) |
| `mod.rs` + `lib.rs` | MODIFY | Register new commands in `invoke_handler!` |

### `ps-scripts/`

| File | Action | Responsibility |
|---|---|---|
| `parse-ue-log.ps1` | CREATE | Run UE in `-nullrhi -ExecCmds=quit -logcmds=...` |
| `create-local-cache-dir.ps1` | CREATE | mkdir + icacls |
| `probe-renderstream-service.ps1` | CREATE | `Get-CimInstance Win32_Service` filter on RS patterns |
| `read-editor-preferences.ps1` | CREATE | Pull 4 DDC path keys from per-user INI |
| `scan-command-line-args.ps1` | CREATE | shortcuts + bat + services |
| `ddc-file-stats.ps1` | CREATE | Recursive count + size |
| `consistency-snapshot.ps1` | CREATE | UE + GPU + RHI + projects |
| `snapshot-baseline-logs.ps1` | CREATE | Pull logs to UNC archive |
| `set-backend-field.ps1` | CREATE | Order-preserving tuple field write |
| `health-probes.ps1` | MODIFY | Add `env_local` + `rs_service` HashMap entries |
| `list-pso-cache-files.ps1` | MODIFY | Narrow `.csv` to `*.stablepc.csv` |

### `src/`

| File | Action | Responsibility |
|---|---|---|
| `components/modals/BatchEnvVarModal.vue` | MODIFY | Variable-name dropdown |
| `components/modals/DeployDdcWizard.vue` | CREATE | 5-step wizard |
| `components/deploy/*.vue` | CREATE | Step indicator, progress table, verify report |
| `components/diagnostics/SymptomAdvisory.vue` | CREATE | Advisory cards |
| `components/diagnostics/ConsistencyReport.vue` | CREATE | Mismatch list |
| `components/primitives/UecmEditorPrefBlock.vue` | CREATE | 4-path display |
| `views/Deploy.vue` | CREATE | Route entry |
| `stores/deploy.ts` | CREATE | Pinia store |
| `lib/deployApi.ts` | CREATE | Tauri invoke wrappers |
| `lib/iniRules.ts` | MODIFY | R011–R025 labels |
| `locales/*.ts` | MODIFY | New strings |
| `router/index.ts` | MODIFY | Register `/deploy` |

---

## Phase 0 — API Reality / Preflight (0.5 day, blocking)

### Module M0: Baseline lock + tuple-survival regression

**Goal:** Lock baseline `cargo check` / `pnpm typecheck` state. Confirm `read-ini-file.ps1` preserves tuple values intact (required precondition for M1.2).

---

#### Task M0.1: Baseline marker

- [ ] **Step 1: Verify baselines pass**

```bash
cd src-tauri && cargo check 2>&1 | tail -5
cd .. && pnpm typecheck 2>&1 | tail -5
```

Both must pass. Stop if not.

- [ ] **Step 2: Marker file**

Create `docs/superpowers/plans/2026-05-18-baseline.txt`:

```
baseline_commit: c23079f
cargo_check: pass
pnpm_typecheck: pass
date: 2026-05-18
```

- [ ] **Step 3: Commit**

```bash
git add docs/superpowers/plans/2026-05-18-baseline.txt
git commit -m "docs(plan): record DDC full-coverage baseline state"
```

---

#### Task M0.2: Tuple-survival check + `is_tuple_value` helper

- [ ] **Step 1: PS regression doc-test** (run-once on Windows)

Create `ps-scripts/__tests__/read-ini-file.tests.ps1`:

```powershell
# Manual: run on a Windows box to verify tuple values survive parse.
$tempFile = [System.IO.Path]::GetTempFileName()
Set-Content -Path $tempFile -Value @"
[DerivedDataBackendGraph]
Shared=(Type=FileSystem, Path=\\NAS\DDC, ReadOnly=false)
"@ -Encoding UTF8
$out = & "$PSScriptRoot\..\read-ini-file.ps1" -HostName 'localhost' -FilePath $tempFile -Local | ConvertFrom-Json
$bg = $out.sections | Where-Object { $_.name -eq 'DerivedDataBackendGraph' }
$shared = $bg.keys | Where-Object { $_.name -eq 'Shared' }
if ($shared.value -notmatch '^\(') { throw "tuple value lost the leading paren" }
if ($shared.value -notmatch 'Type=FileSystem') { throw "tuple body lost Type" }
Remove-Item $tempFile
"OK"
```

- [ ] **Step 2: Rust helper + test**

Append to `src-tauri/src/core/ini_scanner.rs`:

```rust
/// True if a raw value string looks like a `(K1=V1, K2=V2)` tuple.
fn is_tuple_value(v: &str) -> bool {
    let v = v.trim();
    v.starts_with('(') && v.ends_with(')')
}

#[cfg(test)]
mod tuple_detector_tests {
    use super::is_tuple_value;
    #[test] fn detects_paren_wrapped() { assert!(is_tuple_value("(Type=FileSystem)")); }
    #[test] fn rejects_plain() { assert!(!is_tuple_value("FileSystem")); }
    #[test] fn rejects_half_open() {
        assert!(!is_tuple_value("(Type=FileSystem"));
        assert!(!is_tuple_value("Type=FileSystem)"));
    }
}
```

- [ ] **Step 3: Run + commit**

```bash
cd src-tauri && cargo test --lib core::ini_scanner::tuple_detector_tests 2>&1 | tail -5
git add src-tauri/src/core/ini_scanner.rs ps-scripts/__tests__/read-ini-file.tests.ps1
git commit -m "feat(ini): is_tuple_value detector + read-ini-file regression note"
```

---

## Phase 1 — P0 (Must-have, ~17 working days)

### Module M1: BackendGraph parser + rules + editor + CLI (4.5 days)

**Files:**
- Create: `src-tauri/src/core/ini_backend_graph.rs`
- Modify: `src-tauri/src/core/{ini_scanner,ini_diagnostics,ini_editor}.rs`, `src-tauri/src/cli/{args,domain_ini}.rs`
- Create: `src-tauri/src/core/editor_preferences.rs` (stub; expanded in M6.1)
- Create: `ps-scripts/set-backend-field.ps1`

**Dependencies:** M0.

---

#### Task M1.1: Tuple parser preserving field order

**Files:**
- Create: `src-tauri/src/core/ini_backend_graph.rs`

- [ ] **Step 1: Failing tests + skeleton**

```rust
//! Parser for tuple-form values like Shared=(K1=V1, K2=V2). PRESERVES original
//! field order via Vec<(String, String)>.

use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct BackendNode {
    pub name: String,
    pub fields: Vec<(String, String)>,
    pub line_number: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError { MissingOpenParen, MissingCloseParen, EmptyName }

pub fn parse_node(line: &str, line_number: u32) -> Result<BackendNode, ParseError> {
    let eq = line.find('=').ok_or(ParseError::MissingOpenParen)?;
    let name = line[..eq].trim().to_string();
    if name.is_empty() { return Err(ParseError::EmptyName); }
    let rest = line[eq + 1..].trim_start();
    if !rest.starts_with('(') { return Err(ParseError::MissingOpenParen); }
    let close = rest.rfind(')').ok_or(ParseError::MissingCloseParen)?;
    let body = &rest[1..close];
    let mut fields = Vec::new();
    for part in body.split(',') {
        let part = part.trim();
        if part.is_empty() { continue; }
        let Some(p_eq) = part.find('=') else { continue; };
        let k = part[..p_eq].trim().to_string();
        let v = part[p_eq + 1..].trim().to_string();
        if !k.is_empty() { fields.push((k, v)); }
    }
    Ok(BackendNode { name, fields, line_number })
}

pub fn get_field<'a>(node: &'a BackendNode, name: &str) -> Option<&'a str> {
    node.fields.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}

pub fn upsert_field(node: &mut BackendNode, name: &str, value: &str) {
    if let Some((_, v)) = node.fields.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(name)) {
        *v = value.to_string();
    } else {
        node.fields.push((name.to_string(), value.to_string()));
    }
}

pub fn write_node(node: &BackendNode) -> String {
    let body = node.fields.iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>().join(", ");
    format!("{}=({})", node.name, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_node() {
        let n = parse_node(r"Shared=(Type=FileSystem, Path=\\NAS\DDC, ReadOnly=false)", 12).unwrap();
        assert_eq!(n.name, "Shared");
        assert_eq!(n.fields.len(), 3);
        assert_eq!(n.fields[0], ("Type".into(), "FileSystem".into()));
        assert_eq!(n.fields[1], ("Path".into(), r"\\NAS\DDC".into()));
        assert_eq!(n.fields[2], ("ReadOnly".into(), "false".into()));
        assert_eq!(n.line_number, 12);
    }

    #[test]
    fn preserves_sop_13_field_order() {
        let n = parse_node(r"Shared=(Type=FileSystem, ReadOnly=false, Clean=false, Flush=false, DeleteUnused=true, UnusedFileAge=10, FoldersToClean=10, MaxFileChecksPerSec=1, ConsiderSlowAt=70, PromptIfMissing=false, Path=\\NAS\DDC, EnvPathOverride=UE-SharedDataCachePath, EditorOverrideSetting=SharedDerivedDataCache)", 1).unwrap();
        assert_eq!(n.fields.len(), 13);
        let keys: Vec<&str> = n.fields.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, vec!["Type","ReadOnly","Clean","Flush","DeleteUnused","UnusedFileAge","FoldersToClean","MaxFileChecksPerSec","ConsiderSlowAt","PromptIfMissing","Path","EnvPathOverride","EditorOverrideSetting"]);
    }

    #[test] fn rejects_missing_open_paren() { assert_eq!(parse_node("Shared=Foo", 1), Err(ParseError::MissingOpenParen)); }
    #[test] fn rejects_missing_close_paren() { assert_eq!(parse_node("Shared=(Foo", 1), Err(ParseError::MissingCloseParen)); }
    #[test] fn rejects_empty_name() { assert_eq!(parse_node("=(Foo)", 1), Err(ParseError::EmptyName)); }

    #[test]
    fn upsert_preserves_existing_order() {
        let mut n = parse_node(r"Shared=(Type=FileSystem, ReadOnly=true, Path=\\NAS)", 1).unwrap();
        upsert_field(&mut n, "ReadOnly", "false");
        let keys: Vec<&str> = n.fields.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, vec!["Type", "ReadOnly", "Path"]);
        assert_eq!(get_field(&n, "ReadOnly"), Some("false"));
    }

    #[test]
    fn upsert_appends_new_field() {
        let mut n = parse_node(r"Shared=(Type=FileSystem)", 1).unwrap();
        upsert_field(&mut n, "ReadOnly", "false");
        assert_eq!(n.fields.len(), 2);
        assert_eq!(n.fields[1].0, "ReadOnly");
    }

    #[test]
    fn write_node_round_trips() {
        let raw = r"Shared=(Type=FileSystem, Path=\\NAS\DDC, ReadOnly=false)";
        let n = parse_node(raw, 1).unwrap();
        assert_eq!(write_node(&n), raw);
    }
}
```

- [ ] **Step 2: Run + register + commit**

```bash
cd src-tauri && cargo test --lib core::ini_backend_graph 2>&1 | tail -10
```

Add `pub mod ini_backend_graph;` to `src-tauri/src/core/mod.rs`.

```bash
git add src-tauri/src/core/ini_backend_graph.rs src-tauri/src/core/mod.rs
git commit -m "feat(ddc): tuple-value parser preserving field order"
```

---

#### Task M1.2: Post-pass `parse_ini_contents` → tuple → `backend_nodes`

**Files:**
- Modify: `src-tauri/src/core/ini_diagnostics.rs` (add `backend_nodes`)
- Modify: `src-tauri/src/core/ini_scanner.rs` (post-pass + expose `parse_for_diagnostics`)

- [ ] **Step 1: Extend `ParsedSection`**

In `ini_diagnostics.rs`:

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedSection {
    pub name: String,
    pub keys: Vec<ParsedKey>,
    pub backend_nodes: Vec<crate::core::ini_backend_graph::BackendNode>,
}
```

Run `grep -n "ParsedSection {" src-tauri/src/` and add `backend_nodes: vec![],` to every struct-literal, or use `..Default::default()`.

- [ ] **Step 2: Post-pass + wrapper**

In `ini_scanner.rs`, refactor the tail of `parse_ini_contents` to bind the result then post-process:

```rust
fn parse_ini_contents(target: &TargetFile, contents: &str) -> ParsedFile {
    // ... existing K=V parser body, but bind to `mut parsed: ParsedFile` instead of returning inline ...
    for section in &mut parsed.sections {
        for k in &section.keys {
            if is_tuple_value(&k.value) {
                let synthetic = format!("{}={}", k.name, k.value);
                if let Ok(node) = crate::core::ini_backend_graph::parse_node(&synthetic, k.line_number) {
                    section.backend_nodes.push(node);
                }
            }
        }
    }
    parsed
}

/// Public wrapper for callers outside this module that need a parsed file
/// (used by CLI `ini backend-graph scan`).
pub fn parse_for_diagnostics(target: &TargetFile, contents: &str) -> ParsedFile {
    parse_ini_contents(target, contents)
}
```

- [ ] **Step 3: Tests**

Append to `ini_scanner.rs`:

```rust
#[test]
fn parse_extracts_backend_nodes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("DefaultEngine.ini");
    std::fs::write(&path, "[DerivedDataBackendGraph]\nShared=(Type=FileSystem, Path=\\\\NAS\\DDC, ReadOnly=false)\nBoot=(Type=Boot, Filename=DDC)\n").unwrap();
    let target = TargetFile { path: path.to_string_lossy().to_string(),
        category: crate::core::ini_diagnostics::Category::Project };
    let body = std::fs::read_to_string(&path).unwrap();
    let parsed = parse_ini_contents(&target, &body);
    let bg = parsed.sections.iter().find(|s| s.name.eq_ignore_ascii_case("DerivedDataBackendGraph")).unwrap();
    assert_eq!(bg.backend_nodes.len(), 2);
    let shared = bg.backend_nodes.iter().find(|n| n.name == "Shared").unwrap();
    assert_eq!(crate::core::ini_backend_graph::get_field(shared, "ReadOnly"), Some("false"));
}

#[test]
fn non_tuple_keys_stay_out_of_backend_nodes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("DefaultEngine.ini");
    std::fs::write(&path, "[Foo]\nBar=baz\n").unwrap();
    let target = TargetFile { path: path.to_string_lossy().to_string(),
        category: crate::core::ini_diagnostics::Category::Project };
    let body = std::fs::read_to_string(&path).unwrap();
    let parsed = parse_ini_contents(&target, &body);
    assert!(parsed.sections.iter().all(|s| s.backend_nodes.is_empty()));
}
```

- [ ] **Step 4: Run + commit**

```bash
cd src-tauri && cargo test --lib core::ini_scanner 2>&1 | tail -15
git add src-tauri/src/core/ini_scanner.rs src-tauri/src/core/ini_diagnostics.rs
git commit -m "feat(ddc): post-pass extracts BackendGraph tuple nodes into ParsedSection"
```

---

#### Task M1.3: Rules R011–R025

**Files:**
- Modify: `src-tauri/src/core/ini_diagnostics.rs`
- Create: `src-tauri/src/core/editor_preferences.rs` (stub for R025 — full version in M6.1)

15 rules. Implement helper-driven for low repetition. Tests pair fire / silent assertions for each rule.

- [ ] **Step 1: Editor Pref stub**

Create `src-tauri/src/core/editor_preferences.rs`:

```rust
//! Stub for R025. M6.1 extends with alt key names and section variants.

use crate::core::ini_diagnostics::ParsedFile;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditorDdcPrefs {
    pub global_local: Option<String>,
    pub global_shared: Option<String>,
    pub project_local: Option<String>,
    pub project_shared: Option<String>,
}

pub fn extract(file: &ParsedFile) -> EditorDdcPrefs {
    let mut out = EditorDdcPrefs::default();
    for s in &file.sections {
        if !s.name.eq_ignore_ascii_case("/Script/UnrealEd.EditorSettings") { continue; }
        for k in &s.keys {
            let n = k.name.as_str();
            let v = || k.value.trim().to_string();
            if n.eq_ignore_ascii_case("GlobalLocalDDCPath") && out.global_local.is_none() { out.global_local = Some(v()); }
            else if n.eq_ignore_ascii_case("GlobalSharedDDCPath") && out.global_shared.is_none() { out.global_shared = Some(v()); }
            else if n.eq_ignore_ascii_case("ProjectLocalDDCPath") && out.project_local.is_none() { out.project_local = Some(v()); }
            else if n.eq_ignore_ascii_case("ProjectSharedDDCPath") && out.project_shared.is_none() { out.project_shared = Some(v()); }
        }
    }
    out
}
```

Register `pub mod editor_preferences;`.

- [ ] **Step 2: Helpers**

In `ini_diagnostics.rs`:

```rust
fn find_shared_backend(file: &ParsedFile) -> Option<&crate::core::ini_backend_graph::BackendNode> {
    file.sections.iter()
        .find(|s| s.name.eq_ignore_ascii_case("DerivedDataBackendGraph"))?
        .backend_nodes.iter().find(|n| n.name.eq_ignore_ascii_case("Shared"))
}

fn bg_finding(
    file: &ParsedFile, node: &crate::core::ini_backend_graph::BackendNode,
    rule_id: &str, severity: Severity, field: &str, current: &str,
    recommended: &str, symptom: &str, rationale: &str, action: RecommendedAction,
) -> Finding {
    Finding {
        rule_id: rule_id.into(), severity, category: file.category,
        file_path: file.path.clone(),
        section: Some("DerivedDataBackendGraph".into()),
        key_name: Some(format!("Shared.{}", field)),
        line_number: Some(node.line_number as i64),
        snippet_before: format!("{}={}", field, current),
        snippet_after: Some(format!("{}={}", field, recommended)),
        recommended_action: action,
        recommended_value: Some(recommended.into()),
        symptom: symptom.into(),
        rationale: rationale.into(),
    }
}

fn rule_numeric_range(
    file: &ParsedFile, n: &crate::core::ini_backend_graph::BackendNode,
    rule_id: &str, severity: Severity, field: &str,
    lo: i64, hi: i64, default_value: &str, symptom: &str, rationale: &str,
) -> Vec<Finding> {
    let Some(v) = crate::core::ini_backend_graph::get_field(n, field) else { return vec![]; };
    let ok = v.parse::<i64>().map(|x| x >= lo && x <= hi).unwrap_or(false);
    if ok { return vec![]; }
    vec![bg_finding(file, n, rule_id, severity, field, v, default_value,
        symptom, rationale, RecommendedAction::Set)]
}
```

- [ ] **Step 3: Rules R011–R023**

```rust
use crate::core::ini_backend_graph::get_field;

fn rule_r011(file: &ParsedFile) -> Vec<Finding> {
    let Some(n) = find_shared_backend(file) else { return vec![]; };
    let v = get_field(n, "Type").unwrap_or("");
    if !v.eq_ignore_ascii_case("FileSystem") {
        return vec![bg_finding(file, n, "R011", Severity::Critical, "Type",
            v, "FileSystem",
            "Shared backend Type missing or wrong (expected FileSystem).",
            "Without Type=FileSystem UE may build a no-op backend and silently fall back to Local only.",
            RecommendedAction::Set)];
    }
    vec![]
}

fn rule_r012(file: &ParsedFile) -> Vec<Finding> {
    let Some(n) = find_shared_backend(file) else { return vec![]; };
    match get_field(n, "ReadOnly") {
        Some(v) if v.eq_ignore_ascii_case("true") => vec![bg_finding(file, n, "R012",
            Severity::Warning, "ReadOnly", v, "false",
            "Shared DDC marked ReadOnly; cluster cannot write back.",
            "Render nodes must push first-run results so siblings hit cache.",
            RecommendedAction::Set)],
        _ => vec![],
    }
}

fn rule_r013(file: &ParsedFile) -> Vec<Finding> {
    let Some(n) = find_shared_backend(file) else { return vec![]; };
    match get_field(n, "Clean") {
        Some(v) if v.eq_ignore_ascii_case("true") => vec![bg_finding(file, n, "R013",
            Severity::Critical, "Clean", v, "false",
            "Clean=true wipes Shared DDC every launch.",
            "Production Shared DDC must persist between sessions.",
            RecommendedAction::Set)],
        _ => vec![],
    }
}

fn rule_r014(file: &ParsedFile) -> Vec<Finding> {
    let Some(n) = find_shared_backend(file) else { return vec![]; };
    match get_field(n, "Flush") {
        Some(v) if v.eq_ignore_ascii_case("true") => vec![bg_finding(file, n, "R014",
            Severity::Warning, "Flush", v, "false",
            "Flush=true drops cache on exit.",
            "Shared DDC must survive editor close.",
            RecommendedAction::Set)],
        _ => vec![],
    }
}

fn rule_r015(file: &ParsedFile) -> Vec<Finding> {
    let Some(n) = find_shared_backend(file) else { return vec![]; };
    if get_field(n, "DeleteUnused").is_none() {
        return vec![bg_finding(file, n, "R015", Severity::Warning, "DeleteUnused",
            "(missing)", "true",
            "DeleteUnused not configured; GC behaviour ambiguous.",
            "Default may differ across UE versions; pin it.",
            RecommendedAction::Set)];
    }
    vec![]
}

fn rule_r016(file: &ParsedFile) -> Vec<Finding> {
    let Some(n) = find_shared_backend(file) else { return vec![]; };
    rule_numeric_range(file, n, "R016", Severity::Warning, "UnusedFileAge",
        1, 365, "10",
        "UnusedFileAge out of 1–365 day range.",
        "GC sweeps need a meaningful retention window.")
}

fn rule_r017(file: &ParsedFile) -> Vec<Finding> {
    let Some(n) = find_shared_backend(file) else { return vec![]; };
    rule_numeric_range(file, n, "R017", Severity::Warning, "FoldersToClean",
        1, 100, "10",
        "FoldersToClean out of 1–100 range.",
        "GC sweep granularity off.")
}

fn rule_r018(file: &ParsedFile) -> Vec<Finding> {
    let Some(n) = find_shared_backend(file) else { return vec![]; };
    rule_numeric_range(file, n, "R018", Severity::Warning, "MaxFileChecksPerSec",
        1, 100, "1",
        "MaxFileChecksPerSec out of 1–100 range.",
        "Too high stresses NAS; too low slows DDC reads.")
}

fn rule_r019(file: &ParsedFile) -> Vec<Finding> {
    let Some(n) = find_shared_backend(file) else { return vec![]; };
    rule_numeric_range(file, n, "R019", Severity::Warning, "ConsiderSlowAt",
        10, 1000, "70",
        "ConsiderSlowAt out of 10–1000 ms.",
        "If wrong UE may deactivate Shared backend.")
}

fn rule_r020(file: &ParsedFile) -> Vec<Finding> {
    let Some(n) = find_shared_backend(file) else { return vec![]; };
    match get_field(n, "PromptIfMissing") {
        Some(v) if v.eq_ignore_ascii_case("true") => vec![bg_finding(file, n, "R020",
            Severity::Critical, "PromptIfMissing", v, "false",
            "PromptIfMissing=true breaks unattended starts.",
            "RenderStream service has no UI; a missing-path dialog hangs boot.",
            RecommendedAction::Set)],
        _ => vec![],
    }
}

fn rule_r021(file: &ParsedFile) -> Vec<Finding> {
    let Some(n) = find_shared_backend(file) else { return vec![]; };
    let path = get_field(n, "Path").unwrap_or("");
    if !path.starts_with(r"\\") {
        return vec![bg_finding(file, n, "R021", Severity::Critical, "Path",
            if path.is_empty() { "(missing)" } else { path },
            r"\\HOST\Share",
            "Shared backend Path missing or not UNC.",
            "Mapped drives are invisible to Windows services and RenderStream.",
            RecommendedAction::Manual)];
    }
    vec![]
}

fn rule_r022(file: &ParsedFile) -> Vec<Finding> {
    let Some(n) = find_shared_backend(file) else { return vec![]; };
    if get_field(n, "EnvPathOverride").is_none() {
        return vec![bg_finding(file, n, "R022", Severity::Warning, "EnvPathOverride",
            "(missing)", "UE-SharedDataCachePath",
            "EnvPathOverride not set; env var fallback disabled.",
            "Without it UE ignores UE-SharedDataCachePath; per-machine override impossible.",
            RecommendedAction::Set)];
    }
    vec![]
}

fn rule_r023(file: &ParsedFile) -> Vec<Finding> {
    let Some(n) = find_shared_backend(file) else { return vec![]; };
    if get_field(n, "EditorOverrideSetting").is_none() {
        return vec![bg_finding(file, n, "R023", Severity::Info, "EditorOverrideSetting",
            "(missing)", "SharedDerivedDataCache",
            "EditorOverrideSetting not declared.",
            "Without this, Editor Preferences UI cannot override the INI Path.",
            RecommendedAction::Set)];
    }
    vec![]
}

fn rule_r025(file: &ParsedFile, env: &EnvVarState) -> Vec<Finding> {
    if file.category != Category::User { return vec![]; }
    let prefs = crate::core::editor_preferences::extract(file);
    let mut out = Vec::new();
    if let (Some(proj), Some(env_val)) = (prefs.project_shared.as_ref(), env.shared_data_cache_path.as_ref()) {
        if proj != env_val {
            out.push(Finding {
                rule_id: "R025".into(), severity: Severity::Critical, category: file.category,
                file_path: file.path.clone(),
                section: Some("/Script/UnrealEd.EditorSettings".into()),
                key_name: Some("ProjectSharedDDCPath".into()),
                line_number: None,
                snippet_before: format!("ProjectSharedDDCPath={}", proj),
                snippet_after: Some("(leave empty so env var / project Config takes over)".into()),
                recommended_action: RecommendedAction::Remove,
                recommended_value: None,
                symptom: "Project-level Editor Pref masks UE-SharedDataCachePath silently.".into(),
                rationale: "When ProjectSharedDDCPath is non-empty, UE uses it and ignores EnvPathOverride.".into(),
            });
        }
    }
    out
}
```

Wire into `run_rules` (append):

```rust
out.extend(rule_r011(file)); out.extend(rule_r012(file)); out.extend(rule_r013(file));
out.extend(rule_r014(file)); out.extend(rule_r015(file)); out.extend(rule_r016(file));
out.extend(rule_r017(file)); out.extend(rule_r018(file)); out.extend(rule_r019(file));
out.extend(rule_r020(file)); out.extend(rule_r021(file)); out.extend(rule_r022(file));
out.extend(rule_r023(file));
out.extend(pso_cvar_rule(file, "R024", "r.ShaderPipelineCache.Enabled", Severity::Critical,
    "PSO cache file loading is disabled or not configured.",
    "Without this CVar collected PSO cache files are not loaded at runtime."));
out.extend(rule_r025(file, env));
```

- [ ] **Step 4: Tests**

Helper + paired fire/silent tests (one example shown; engineer mirrors the pattern for R011–R025):

```rust
fn ddb_project(node_raw: &str) -> ParsedFile {
    use crate::core::ini_backend_graph::parse_node;
    ParsedFile {
        path: r"C:\Project\Config\DefaultEngine.ini".into(),
        category: Category::Project,
        sections: vec![ParsedSection {
            name: "DerivedDataBackendGraph".into(),
            keys: vec![],
            backend_nodes: vec![parse_node(node_raw, 0).unwrap()],
        }],
    }
}

fn assert_fires(rule: &str, file: &ParsedFile) {
    let env = EnvVarState::default();
    let findings = run_rules(file, &env);
    assert!(findings.iter().any(|f| f.rule_id == rule),
        "expected {} to fire; got: {:?}", rule,
        findings.iter().map(|f| f.rule_id.clone()).collect::<Vec<_>>());
}
fn assert_silent(rule: &str, file: &ParsedFile) {
    let env = EnvVarState::default();
    let findings = run_rules(file, &env);
    assert!(!findings.iter().any(|f| f.rule_id == rule), "expected {} silent", rule);
}

#[test] fn r011_fires_on_wrong_type() { assert_fires("R011", &ddb_project(r"Shared=(Path=\\NAS)")); }
#[test] fn r011_silent_on_correct_type() { assert_silent("R011", &ddb_project(r"Shared=(Type=FileSystem, Path=\\NAS)")); }
#[test] fn r012_fires_on_readonly_true() { assert_fires("R012", &ddb_project(r"Shared=(Type=FileSystem, ReadOnly=true)")); }
#[test] fn r012_silent_on_readonly_false() { assert_silent("R012", &ddb_project(r"Shared=(Type=FileSystem, ReadOnly=false)")); }
#[test] fn r013_fires_on_clean_true() { assert_fires("R013", &ddb_project(r"Shared=(Type=FileSystem, Clean=true)")); }
#[test] fn r013_silent_on_clean_false() { assert_silent("R013", &ddb_project(r"Shared=(Type=FileSystem, Clean=false)")); }
#[test] fn r014_fires_on_flush_true() { assert_fires("R014", &ddb_project(r"Shared=(Type=FileSystem, Flush=true)")); }
#[test] fn r014_silent_on_flush_false() { assert_silent("R014", &ddb_project(r"Shared=(Type=FileSystem, Flush=false)")); }
#[test] fn r015_fires_when_missing() { assert_fires("R015", &ddb_project(r"Shared=(Type=FileSystem)")); }
#[test] fn r015_silent_when_present() { assert_silent("R015", &ddb_project(r"Shared=(Type=FileSystem, DeleteUnused=true)")); }
#[test] fn r016_fires_for_out_of_range_zero() { assert_fires("R016", &ddb_project(r"Shared=(Type=FileSystem, UnusedFileAge=0)")); }
#[test] fn r016_fires_for_out_of_range_huge() { assert_fires("R016", &ddb_project(r"Shared=(Type=FileSystem, UnusedFileAge=9999)")); }
#[test] fn r016_silent_for_normal() { assert_silent("R016", &ddb_project(r"Shared=(Type=FileSystem, UnusedFileAge=10)")); }
#[test] fn r017_fires_oor() { assert_fires("R017", &ddb_project(r"Shared=(Type=FileSystem, FoldersToClean=0)")); }
#[test] fn r017_silent_ok() { assert_silent("R017", &ddb_project(r"Shared=(Type=FileSystem, FoldersToClean=10)")); }
#[test] fn r018_fires_oor() { assert_fires("R018", &ddb_project(r"Shared=(Type=FileSystem, MaxFileChecksPerSec=9999)")); }
#[test] fn r018_silent_ok() { assert_silent("R018", &ddb_project(r"Shared=(Type=FileSystem, MaxFileChecksPerSec=1)")); }
#[test] fn r019_fires_oor() { assert_fires("R019", &ddb_project(r"Shared=(Type=FileSystem, ConsiderSlowAt=0)")); }
#[test] fn r019_silent_ok() { assert_silent("R019", &ddb_project(r"Shared=(Type=FileSystem, ConsiderSlowAt=70)")); }
#[test] fn r020_fires_on_prompt_true() { assert_fires("R020", &ddb_project(r"Shared=(Type=FileSystem, PromptIfMissing=true)")); }
#[test] fn r020_silent_on_prompt_false() { assert_silent("R020", &ddb_project(r"Shared=(Type=FileSystem, PromptIfMissing=false)")); }
#[test] fn r021_fires_on_drive_letter() { assert_fires("R021", &ddb_project(r"Shared=(Type=FileSystem, Path=Z:\DDC)")); }
#[test] fn r021_fires_on_missing_path() { assert_fires("R021", &ddb_project(r"Shared=(Type=FileSystem)")); }
#[test] fn r021_silent_on_unc() { assert_silent("R021", &ddb_project(r"Shared=(Type=FileSystem, Path=\\NAS\DDC)")); }
#[test] fn r022_fires_when_missing() { assert_fires("R022", &ddb_project(r"Shared=(Type=FileSystem, Path=\\NAS)")); }
#[test] fn r022_silent_when_present() { assert_silent("R022", &ddb_project(r"Shared=(Type=FileSystem, Path=\\NAS, EnvPathOverride=UE-SharedDataCachePath)")); }
#[test] fn r023_fires_when_missing() { assert_fires("R023", &ddb_project(r"Shared=(Type=FileSystem, Path=\\NAS)")); }

#[test]
fn r025_fires_when_project_shared_pref_masks_env() {
    let f = ParsedFile {
        path: r"C:\Users\op\AppData\Local\UnrealEngine\5.5\Saved\Config\WindowsEditor\EditorPerProjectUserSettings.ini".into(),
        category: Category::User,
        sections: vec![ParsedSection {
            name: "/Script/UnrealEd.EditorSettings".into(),
            keys: vec![ParsedKey { name: "ProjectSharedDDCPath".into(), value: r"\\WRONG\DDC".into(), line_number: 4 }],
            backend_nodes: vec![],
        }],
    };
    let env = EnvVarState { shared_data_cache_path: Some(r"\\RIGHT\DDC".into()), local_data_cache_path: None };
    let findings = run_rules(&f, &env);
    assert!(findings.iter().any(|x| x.rule_id == "R025" && x.severity == Severity::Critical));
}
```

- [ ] **Step 5: UI labels**

Update `src/lib/iniRules.ts`:

```ts
R011: { title: "Shared backend Type missing or wrong", tone: "critical" },
R012: { title: "Shared DDC marked ReadOnly", tone: "warning" },
R013: { title: "Clean=true wipes Shared every launch", tone: "critical" },
R014: { title: "Flush=true clears cache on exit", tone: "warning" },
R015: { title: "DeleteUnused not configured", tone: "warning" },
R016: { title: "UnusedFileAge out of range", tone: "warning" },
R017: { title: "FoldersToClean out of range", tone: "warning" },
R018: { title: "MaxFileChecksPerSec out of range", tone: "warning" },
R019: { title: "ConsiderSlowAt out of range", tone: "warning" },
R020: { title: "PromptIfMissing=true blocks unattended starts", tone: "critical" },
R021: { title: "Shared Path missing or not UNC", tone: "critical" },
R022: { title: "EnvPathOverride not set", tone: "warning" },
R023: { title: "EditorOverrideSetting not declared", tone: "info" },
R024: { title: "r.ShaderPipelineCache.Enabled not 1", tone: "critical" },
R025: { title: "Project-level Pref masks env var", tone: "critical" },
```

- [ ] **Step 6: Run + commit**

```bash
cd src-tauri && cargo test --lib core::ini_diagnostics 2>&1 | tail -20
git add src-tauri/src/core/ini_diagnostics.rs src-tauri/src/core/editor_preferences.rs src-tauri/src/core/mod.rs src/lib/iniRules.ts
git commit -m "feat(ddc): rules R011-R025 over BackendGraph + Editor Pref masking + R024 ShaderPipelineCache"
```

---

#### Task M1.4: `set_backend_field_with_credential`

Same body as v1 M1.4 with `UecmError::Io` → `UecmError::OperationFailed` and the field-order-preserving PS sidecar from M1.4 in v1 (which already preserves order). Engineer follows v1 M1.4 verbatim — see `c23079f` commit. Tests assert field order preservation.

---

#### Task M1.5: CLI `ini backend-graph get/set/scan`

**Files:**
- Modify: `src-tauri/src/cli/args.rs` — add `IniAction::BackendGraph(BackendGraphAction)`
- Modify: `src-tauri/src/cli/domain_ini.rs`

`Domain::Ini` already exists; `needs_db()` already returns true. No `needs_db()` update needed.

- [ ] **Step 1: CLI variants**

In `IniAction` add:

```rust
/// Read or write [DerivedDataBackendGraph] tuple nodes.
BackendGraph {
    #[command(subcommand)]
    action: BackendGraphAction,
},
```

```rust
#[derive(Subcommand, Debug)]
pub enum BackendGraphAction {
    Get {
        #[arg(long)] host: String,
        #[arg(long)] file_path: String,
        #[arg(long, default_value = "Shared")] node: String,
        #[arg(long)] field: String,
        #[command(flatten)] cred: crate::cli::credential_args::CredentialArgs,
    },
    Set {
        #[command(flatten)] target: crate::cli::host_args::HostArgs,
        #[arg(long)] file_path: String,
        #[arg(long, default_value = "Shared")] node: String,
        #[arg(long)] field: String,
        #[arg(long)] value: String,
        #[arg(long)] yes: bool,
        #[arg(long)] dry_run: bool,
        #[command(flatten)] cred: crate::cli::credential_args::CredentialArgs,
    },
    Scan {
        #[arg(long)] host: String,
        #[arg(long)] file_path: String,
        #[command(flatten)] cred: crate::cli::credential_args::CredentialArgs,
    },
}
```

- [ ] **Step 2: Handler in `domain_ini.rs`**

Match the new variant:

```rust
IniAction::BackendGraph { action } => match action {
    BackendGraphAction::Get { host, file_path, node, field, cred } => {
        let db = ctx.require_db()?;
        let creds = cred.resolve(db)?;
        let target = crate::core::ini_scanner::TargetFile {
            path: file_path.clone(),
            category: crate::core::ini_diagnostics::Category::Project,
        };
        let read = crate::core::ini_scanner::read_file(
            &host, &target, creds.as_ref().map(|(u, p)| (u.as_str(), p.as_str())))?;
        let parsed = crate::core::ini_scanner::parse_for_diagnostics(&target, &read.contents);
        let value = parsed.sections.iter()
            .flat_map(|s| s.backend_nodes.iter())
            .filter(|n| n.name.eq_ignore_ascii_case(&node))
            .find_map(|n| crate::core::ini_backend_graph::get_field(n, &field).map(String::from));
        ctx.emitter.emit_result(&serde_json::json!({
            "host": host, "file": file_path, "node": node, "field": field, "value": value
        })).ok();
        Ok(())
    }
    BackendGraphAction::Set { target, file_path, node, field, value, yes, dry_run, cred } => {
        let hosts: Vec<String> = match target.require_one()? {
            crate::cli::host_args::HostTarget::Single(h) => vec![h],
            crate::cli::host_args::HostTarget::Batch(hs) => hs,
        };
        let outcome = crate::cli::destructive::check(yes, dry_run, "ini.backend-graph.set")?;
        let db = ctx.require_db()?;
        cred.preflight(db)?;
        if outcome == crate::cli::destructive::Outcome::DryRun {
            crate::cli::destructive::emit_plan(ctx.emitter.as_mut(), "ini.backend-graph.set",
                serde_json::json!({ "hosts": hosts, "file": file_path, "node": node, "field": field, "value": value }));
            return Ok(());
        }
        let (u, p) = cred.resolve(db)?
            .ok_or_else(|| crate::error::UecmError::InvalidInput("credentials required".into()))?;
        for host in &hosts {
            crate::core::ini_editor::set_backend_field_with_credential(
                host, &file_path, "DerivedDataBackendGraph", &node, &field, &value, &u, &p)?;
        }
        Ok(())
    }
    BackendGraphAction::Scan { host, file_path, cred } => {
        let db = ctx.require_db()?;
        let creds = cred.resolve(db)?;
        let target = crate::core::ini_scanner::TargetFile {
            path: file_path.clone(),
            category: crate::core::ini_diagnostics::Category::Project,
        };
        let read = crate::core::ini_scanner::read_file(
            &host, &target, creds.as_ref().map(|(u, p)| (u.as_str(), p.as_str())))?;
        let parsed = crate::core::ini_scanner::parse_for_diagnostics(&target, &read.contents);
        let nodes: Vec<_> = parsed.sections.iter()
            .flat_map(|s| s.backend_nodes.iter().cloned()).collect();
        ctx.emitter.emit_result(&nodes).ok();
        Ok(())
    }
},
```

- [ ] **Step 3: Parser test + commit**

```rust
#[test]
fn parses_ini_backend_graph_set() {
    let cli = Cli::try_parse_from([
        "uecm-cli", "ini", "backend-graph", "set",
        "--hosts", "R01,R02", "--file-path", r"D:\Proj\Config\DefaultEngine.ini",
        "--node", "Shared", "--field", "ReadOnly", "--value", "false",
        "--cred-alias", "admin", "--pass-stdin", "--yes",
    ]).unwrap();
    match cli.command {
        Domain::Ini { action: IniAction::BackendGraph { action: BackendGraphAction::Set { node, field, value, .. } } } => {
            assert_eq!(node, "Shared"); assert_eq!(field, "ReadOnly"); assert_eq!(value, "false");
        }
        _ => panic!("wrong variant"),
    }
}
```

```bash
cd src-tauri && cargo test --lib cli::args::tests::parses_ini_backend_graph 2>&1 | tail -5
git add src-tauri/src/cli/args.rs src-tauri/src/cli/domain_ini.rs
git commit -m "feat(ddc): cli ini backend-graph get/set/scan"
```

---

### Module M2: Log verifier (3 days)

Per v1 spec, with these corrections applied:

- `UecmError::Io(format!(...))` → `UecmError::OperationFailed(format!(...))` throughout.
- M2.4 step 3 extends `needs_db()` in `cli/run.rs` to include `Domain::Log { .. }`.
- M2.5 Tauri command uses `db: State<'_, Db>` + `data_credentials::find_by_alias` + `core_credentials::resolve_password`.

Engineer follows v1 M2.1–M2.5 (see commit `c23079f` historical body) and applies these corrections.

---

### Module M3: Local DDC complete config (2 days)

#### Task M3.1: Surface `env_local` as a parallel HashMap key

**Files:**
- Modify: `ps-scripts/health-probes.ps1`
- Modify: `src-tauri/src/core/health_probes.rs`
- Modify: `src/lib/healthChecks.ts`

- [ ] **Step 1: PS — add a second function + entry**

In `ps-scripts/health-probes.ps1`, locate `Probe-EnvVars`. Replace with two functions:

```powershell
function Probe-EnvShared {
    param($Expected)
    $value = [Environment]::GetEnvironmentVariable('UE-SharedDataCachePath', 'Machine')
    if ([string]::IsNullOrEmpty($Expected)) {
        $status = if ($value) { 'healthy' } else { 'warning' }
        return @{ status = $status; message = "UE-SharedDataCachePath = $value"; sample = "$value" }
    }
    $status = if ($value -eq $Expected) { 'healthy' } else { 'critical' }
    @{ status = $status; message = "expected $Expected, got $value"; sample = "$value" }
}

function Probe-EnvLocal {
    param($Expected)
    $value = [Environment]::GetEnvironmentVariable('UE-LocalDataCachePath', 'Machine')
    if ([string]::IsNullOrEmpty($Expected)) {
        $status = if ($value) { 'healthy' } else { 'warning' }
        return @{ status = $status; message = "UE-LocalDataCachePath = $value"; sample = "$value" }
    }
    $status = if ($value -eq $Expected) { 'healthy' } else { 'critical' }
    @{ status = $status; message = "expected $Expected, got $value"; sample = "$value" }
}
```

In the `$script` body, replace the result hashtable's `env_vars` line with:

```powershell
env_vars   = (Probe-EnvShared -Expected $ExpectedSharedDataCachePath)
env_shared = (Probe-EnvShared -Expected $ExpectedSharedDataCachePath)
env_local  = (Probe-EnvLocal  -Expected $ExpectedLocalDataCachePath)
```

(Keeping `env_vars` aliased to shared preserves existing UI.)

Add to top-level `param(...)`:

```powershell
[string]$ExpectedLocalDataCachePath = "",
```

And forward via `Invoke-Command -ArgumentList`.

- [ ] **Step 2: Rust caller**

In `src-tauri/src/core/health_probes.rs`, extend `run_for_host` signature and args. Update every caller (`grep -rn "health_probes::run_for_host" src-tauri/src/`) to pass an empty `""` initially.

- [ ] **Step 3: UI registration**

Add to `src/lib/healthChecks.ts`:

```ts
{ id: "env_local", shortLabel: "ENV-L", label: "Local DDC env var",
  description: "UE-LocalDataCachePath is set at machine scope.",
  symptom: "RenderStream uses default %LOCALAPPDATA%; intended local path ignored.",
  remediation: "Set UE-LocalDataCachePath via Batch Env Var modal." },
```

- [ ] **Step 4: Build + commit**

```bash
cd src-tauri && cargo build 2>&1 | tail -5
git add ps-scripts/health-probes.ps1 src-tauri/src/core/health_probes.rs src/lib/healthChecks.ts src/locales/zh.ts src/locales/en.ts
git commit -m "feat(ddc): surface env_local as parallel CheckOutcome key"
```

---

#### Task M3.2: `core::local_cache::create` + PS script

Per v1 M3.2 with `UecmError::Io` → `UecmError::OperationFailed`.

---

#### Task M3.3: CLI `local-cache create`

Per v1 M3.3, with:
- **Add `Domain::LocalCache { .. }` to `needs_db()` in `cli/run.rs`.**
- `HostTarget` destructured inline (no `.as_vec()`).

---

#### Task M3.4: `BatchEnvVarModal` dropdown

Per v1 M3.4. No API-surface concerns.

---

### Module M4: RenderStream Service probe (1.5 days)

Per v1 M4.1–M4.3 with these corrections:

- **`UecmError::Io` → `UecmError::OperationFailed`**.
- M4.2 adds an `into_check_outcome(&RsServiceReport) -> CheckOutcome` helper (signature in this plan's API Reference section).
- M4.3 surfaces `rs_service` as a **single `CheckOutcome`** entry in the PS round-trip — same shape as other probes:

```powershell
function Probe-RsService {
    $patterns = @('d3service%','%RenderStream%','%disguise%')
    $found = New-Object System.Collections.Generic.List[object]
    foreach ($p in $patterns) {
        $svcs = Get-CimInstance Win32_Service -Filter "Name LIKE '$p'" -ErrorAction SilentlyContinue
        foreach ($svc in $svcs) {
            $exists = $found.Where({ $_.Name -eq $svc.Name }).Count -gt 0
            if (-not $exists) {
                $found.Add([pscustomobject]@{ Name = $svc.Name; StartName = $svc.StartName; State = $svc.State }) | Out-Null
            }
        }
    }
    if ($found.Count -eq 0) {
        return @{ status = 'na'; message = 'no RenderStream service detected'; sample = '' }
    }
    $risks = @()
    foreach ($s in $found) {
        $acct = $s.StartName.ToLower()
        if ($acct -eq 'localsystem' -or $acct -eq '.\localsystem') {
            $risks += "$($s.Name) runs as LocalSystem"
        }
    }
    $names = ($found | ForEach-Object { $_.Name }) -join ', '
    $status = if ($risks.Count -gt 0) { 'warning' } else { 'healthy' }
    $msg = if ($risks.Count -gt 0) { $risks -join '; ' } else { "services: $names" }
    @{ status = $status; message = $msg; sample = $names }
}
```

Add `rs_service = (Probe-RsService)` to the result hashtable.

The detailed services array (with PathName, StartMode, etc.) goes through a dedicated Tauri command `get_renderstream_services(db: State<'_, Db>, machine_id: i64, credential_alias: String) -> UecmResult<RsServiceReport>` in `src-tauri/src/commands/renderstream_service.rs`.

---

### Module M5: `deploy ddc` orchestrator (6 days)

#### Task M5.1: Plan + Step + Event types

**Files:**
- Create: `src-tauri/src/core/deploy_workflow.rs`

```rust
//! Orchestrates the 11-step DDC deployment workflow. Takes &Db from caller;
//! never opens DB itself.

use crate::data::Db;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployPlan {
    pub project_id: i64,
    pub source_machine_id: i64,
    pub target_machine_ids: Vec<i64>,
    pub local_cache: LocalCacheSpec,
    pub shared_cache: SharedCacheSpec,
    pub ddc_pak: PakSpec,
    pub pso: PsoSpec,
    pub verify: VerifySpec,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalCacheSpec { pub path: String, pub service_account: Option<String> }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedCacheSpec {
    pub server_machine_id: i64,
    pub share_name: String,
    pub server_path: String,
    pub mode: String,
    pub unc_path: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PakSpec { pub enabled: bool }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsoSpec { pub enabled: bool, pub resolution: String, pub max_minutes: u32 }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifySpec { pub run_log_verify: bool, pub editor_exe: String, pub timeout_seconds: u32 }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DeployStep {
    ProvisionLocalDir, SetLocalEnv, CreateSmbShare, SetSharedEnv, WriteBackendGraph,
    GenerateDdcPak, DistributeDdcPak, SetPsoCvars, CollectPso, DistributePso, VerifyStartupLogs,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeployEvent {
    StepStarted { step: DeployStep, hosts: Vec<String> },
    StepHostOk { step: DeployStep, host: String, message: Option<String> },
    StepHostError { step: DeployStep, host: String, error: String },
    StepCompleted { step: DeployStep, ok_count: u32, fail_count: u32 },
    PlanCompleted { ok: bool, summary: String },
}

pub fn plan_steps(plan: &DeployPlan) -> Vec<DeployStep> {
    use DeployStep::*;
    let mut s = vec![ProvisionLocalDir, SetLocalEnv, CreateSmbShare, SetSharedEnv, WriteBackendGraph];
    if plan.ddc_pak.enabled { s.push(GenerateDdcPak); s.push(DistributeDdcPak); }
    if plan.pso.enabled { s.push(SetPsoCvars); s.push(CollectPso); s.push(DistributePso); }
    if plan.verify.run_log_verify { s.push(VerifyStartupLogs); }
    s
}

#[derive(Debug, Clone)]
pub struct RunOptions { pub stop_on_step_failure: bool }

#[cfg(test)]
mod tests {
    use super::*;
    fn baseline_plan() -> DeployPlan {
        DeployPlan {
            project_id: 1, source_machine_id: 100, target_machine_ids: vec![200, 201],
            local_cache: LocalCacheSpec { path: "D:\\UE-DDC-Local".into(), service_account: None },
            shared_cache: SharedCacheSpec { server_machine_id: 300, share_name: "DDC".into(),
                server_path: "D:\\DDC".into(), mode: "b".into(), unc_path: None },
            ddc_pak: PakSpec { enabled: true },
            pso: PsoSpec { enabled: true, resolution: "1920x1080".into(), max_minutes: 10 },
            verify: VerifySpec { run_log_verify: true, editor_exe: "C:\\UE\\UnrealEditor.exe".into(), timeout_seconds: 180 },
        }
    }
    #[test] fn full_plan_has_11_steps() { assert_eq!(plan_steps(&baseline_plan()).len(), 11); }
    #[test] fn minimal_plan_skips_optional_phases() {
        let mut p = baseline_plan();
        p.ddc_pak.enabled = false; p.pso.enabled = false; p.verify.run_log_verify = false;
        assert_eq!(plan_steps(&p).len(), 5);
    }
}
```

Register, test, commit.

---

#### Task M5.2: Step executor using REAL APIs

**Files:**
- Modify: `src-tauri/src/core/deploy_workflow.rs`

```rust
use crate::core::{ddc_pak, env_vars, ini_editor, local_cache, pak_distribute, pso_collect, pso_distribute, shares, ue_log_verify};
use crate::data::{machines as data_machines, project_locations, Db};
use crate::error::{UecmError, UecmResult};

fn hostname_for(db: &Db, machine_id: i64) -> UecmResult<String> {
    data_machines::find_by_id(db, machine_id)?
        .map(|m| m.hostname)
        .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", machine_id)))
}

fn project_root_for(db: &Db, project_id: i64, machine_id: i64) -> UecmResult<String> {
    project_locations::get_for_project_machine(db, project_id, machine_id)?
        .map(|loc| loc.abs_path)
        .ok_or_else(|| UecmError::InvalidInput(format!(
            "project {} not located on machine {}", project_id, machine_id)))
}

fn step_machine_ids(plan: &DeployPlan, step: DeployStep) -> Vec<i64> {
    use DeployStep::*;
    match step {
        ProvisionLocalDir | SetLocalEnv | SetSharedEnv | WriteBackendGraph
        | SetPsoCvars | DistributeDdcPak | DistributePso | VerifyStartupLogs
            => plan.target_machine_ids.clone(),
        CreateSmbShare => vec![plan.shared_cache.server_machine_id],
        GenerateDdcPak | CollectPso => vec![plan.source_machine_id],
    }
}

pub fn run_step(
    db: &Db, plan: &mut DeployPlan, step: DeployStep,
    creds: Option<(&str, &str)>, emit: &mut dyn FnMut(DeployEvent),
) {
    let machine_ids = step_machine_ids(plan, step);
    let mut hosts: Vec<String> = Vec::with_capacity(machine_ids.len());
    for mid in &machine_ids {
        match hostname_for(db, *mid) {
            Ok(h) => hosts.push(h),
            Err(e) => emit(DeployEvent::StepHostError {
                step, host: format!("machine_id={}", mid), error: e.to_string() }),
        }
    }
    emit(DeployEvent::StepStarted { step, hosts: hosts.clone() });
    let mut ok = 0u32; let mut fail = 0u32;
    for (mid, host) in machine_ids.iter().zip(hosts.iter()) {
        match execute_one(db, plan, step, *mid, host, creds) {
            Ok(msg) => { ok += 1; emit(DeployEvent::StepHostOk { step, host: host.clone(), message: msg }); }
            Err(e)  => { fail += 1; emit(DeployEvent::StepHostError { step, host: host.clone(), error: e.to_string() }); }
        }
    }
    emit(DeployEvent::StepCompleted { step, ok_count: ok, fail_count: fail });
}

fn execute_one(
    db: &Db, plan: &mut DeployPlan, step: DeployStep,
    machine_id: i64, host: &str, creds: Option<(&str, &str)>,
) -> UecmResult<Option<String>> {
    use DeployStep::*;
    match step {
        ProvisionLocalDir => local_cache::create(
            host, &plan.local_cache.path, plan.local_cache.service_account.as_deref(), creds).map(Some),
        SetLocalEnv => match creds {
            Some((u, p)) => env_vars::set_with_credential(host, "UE-LocalDataCachePath", &plan.local_cache.path, u, p),
            None => env_vars::set(host, "UE-LocalDataCachePath", &plan.local_cache.path),
        }.map(|_| None),
        CreateSmbShare => {
            let op_user = creds.map(|(u, _)| u);
            let op_pass = creds.map(|(_, p)| p);
            let r = match plan.shared_cache.mode.as_str() {
                "a" | "A" => shares::create_mode_a(
                    host, &plan.shared_cache.share_name, &plan.shared_cache.server_path, op_user, op_pass)?,
                "b" | "B" => {
                    let svc_pass = shares::generate_svc_password();
                    shares::create_mode_b(host, &plan.shared_cache.share_name,
                        &plan.shared_cache.server_path, "ddc-svc", &svc_pass, op_user, op_pass)?
                }
                other => return Err(UecmError::InvalidInput(format!("unknown share mode '{}'", other))),
            };
            plan.shared_cache.unc_path = Some(r.unc_path.clone());
            Ok(Some(format!("UNC={}", r.unc_path)))
        }
        SetSharedEnv => {
            let unc = plan.shared_cache.unc_path.clone().unwrap_or_else(|| {
                let server_host = hostname_for(db, plan.shared_cache.server_machine_id)
                    .unwrap_or_else(|_| "?".into());
                format!("\\\\{}\\{}", server_host, plan.shared_cache.share_name)
            });
            match creds {
                Some((u, p)) => env_vars::set_with_credential(host, "UE-SharedDataCachePath", &unc, u, p),
                None => env_vars::set(host, "UE-SharedDataCachePath", &unc),
            }.map(|_| Some(format!("→ {}", unc)))
        }
        WriteBackendGraph => {
            let unc = plan.shared_cache.unc_path.clone().unwrap_or_else(|| {
                let server_host = hostname_for(db, plan.shared_cache.server_machine_id)
                    .unwrap_or_else(|_| "?".into());
                format!("\\\\{}\\{}", server_host, plan.shared_cache.share_name)
            });
            let project_root = project_root_for(db, plan.project_id, machine_id)?;
            let ini_path = format!("{}\\Config\\DefaultEngine.ini", project_root.trim_end_matches('\\'));
            let (u, p) = creds.ok_or_else(|| UecmError::InvalidInput("credentials required".into()))?;
            ini_editor::set_backend_field_with_credential(
                host, &ini_path, "DerivedDataBackendGraph", "Shared", "Path", &unc, u, p)?;
            ini_editor::set_backend_field_with_credential(
                host, &ini_path, "DerivedDataBackendGraph", "Shared", "EnvPathOverride", "UE-SharedDataCachePath", u, p)?;
            Ok(Some("Shared.Path + EnvPathOverride".into()))
        }
        GenerateDdcPak => {
            let project_root = project_root_for(db, plan.project_id, machine_id)?;
            generate_pak_sync(host, &project_root, creds)
                .map(|out| Some(format!("pak={}", out.pak_path)))
        }
        DistributeDdcPak => {
            let profile = pak_distribute::DistributeProfile::ddc_pak();
            let source_host = hostname_for(db, plan.source_machine_id)?;
            let source_root = project_root_for(db, plan.project_id, plan.source_machine_id)?;
            let target_root = project_root_for(db, plan.project_id, machine_id)?;
            let items = pak_distribute::plan(
                &profile, &source_host, &source_root,
                &[(host.to_string(), target_root)], creds)?;
            let item = items.into_iter().find(|i| i.target_host == host)
                .ok_or_else(|| UecmError::InvalidInput("no plan item for target".into()))?;
            let outcome = block_on_async(pak_distribute::run_one_with_profile(&profile, item))?;
            Ok(Some(format!("{} files", outcome.files_copied)))
        }
        SetPsoCvars => {
            let project_root = project_root_for(db, plan.project_id, machine_id)?;
            let ini = format!("{}\\Config\\ConsoleVariables.ini", project_root.trim_end_matches('\\'));
            let (u, p) = creds.ok_or_else(|| UecmError::InvalidInput("creds required".into()))?;
            for key in ["r.ShaderPipelineCache.Enabled", "r.PSOPrecaching",
                        "r.PSOPrecache.Compile", "r.PSOPrecache.GlobalShaders"] {
                ini_editor::set_key_with_credential(host, &ini, "ConsoleVariables", key, "1", u, p)?;
            }
            Ok(Some("4 CVars set".into()))
        }
        CollectPso => {
            let project_root = project_root_for(db, plan.project_id, machine_id)?;
            collect_pso_sync(host, &project_root, &plan.pso, creds)
                .map(|count| Some(format!("{} PSO files", count)))
        }
        DistributePso => {
            let source_host = hostname_for(db, plan.source_machine_id)?;
            let source_root = project_root_for(db, plan.project_id, plan.source_machine_id)?;
            let target_root = project_root_for(db, plan.project_id, machine_id)?;
            let items = pso_distribute::plan(
                &source_host, &source_root,
                &[(host.to_string(), target_root)], creds)?;
            let item = items.into_iter().find(|i| i.target_host == host)
                .ok_or_else(|| UecmError::InvalidInput("no plan item for target".into()))?;
            let outcome = block_on_async(pso_distribute::run_one(item))?;
            Ok(Some(format!("{} files", outcome.files_copied)))
        }
        VerifyStartupLogs => {
            let uproject = project_locations::get_for_project_machine(db, plan.project_id, machine_id)?
                .map(|l| l.uproject_path)
                .ok_or_else(|| UecmError::InvalidInput("uproject path missing".into()))?;
            let report = ue_log_verify::run_for_host(
                host, &plan.verify.editor_exe, &uproject, plan.verify.timeout_seconds, creds)?;
            let ok = report.local_path.is_some() && report.shared_path.is_some()
                && report.shared_deactivated_reason.is_none()
                && report.move_collision_count < 10;
            if ok {
                Ok(Some(format!("Local={}, Shared={}",
                    report.local_path.as_deref().unwrap_or("?"),
                    report.shared_path.as_deref().unwrap_or("?"))))
            } else {
                Err(UecmError::OperationFailed(format!(
                    "verify failed: local={:?} shared={:?} deactivated={:?} collisions={}",
                    report.local_path, report.shared_path,
                    report.shared_deactivated_reason, report.move_collision_count)))
            }
        }
    }
}

fn block_on_async<F, T>(fut: F) -> UecmResult<T>
where F: std::future::Future<Output = UecmResult<T>> {
    match tokio::runtime::Handle::try_current().ok() {
        Some(h) => h.block_on(fut),
        None => {
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| UecmError::OperationFailed(format!("tokio rt: {}", e)))?;
            rt.block_on(fut)
        }
    }
}

/// Drive ddc_pak's preflight + launch_generation + verify_output pipeline to
/// completion. ENGINEER NOTE: copy the wait/poll loop from
/// `src-tauri/src/commands/ddc_pak.rs` (the existing generate flow) into this
/// helper. The exact handle type and poll mechanism is inside `launch_generation`
/// — do not invent new infrastructure, just adapt the existing UI driver.
fn generate_pak_sync(host: &str, project_dir: &str, creds: Option<(&str, &str)>) -> UecmResult<ddc_pak::PakOutput> {
    ddc_pak::preflight(project_dir, host, creds)?;
    // TODO(engineer): launch_generation + poll loop adapted from commands/ddc_pak.rs.
    // Once the run completes successfully, fall through to verify_output.
    ddc_pak::verify_output(host, project_dir, creds)
}

/// Drive pso_collect's launch + enumerate pipeline. ENGINEER NOTE: same as
/// above — copy wait loop from the existing PSO collect UI driver in
/// `src-tauri/src/commands/pso.rs`.
fn collect_pso_sync(host: &str, project_dir: &str, spec: &PsoSpec, creds: Option<(&str, &str)>) -> UecmResult<usize> {
    // TODO(engineer): build PsoCollectSpec from PsoSpec fields, then
    // launch_collection + wait. Then call enumerate_remote.
    let files = pso_collect::enumerate_remote(host, project_dir, creds)?;
    Ok(files.len())
}
```

The two `TODO(engineer)` blocks are the only intentional unknowns. Engineer reads `commands/ddc_pak.rs` and `commands/pso.rs` (or equivalents — `grep -n "launch_generation\|launch_collection" src-tauri/src/commands/`) for the exact wait shape, then copies it into these helpers. Budget: 2 hours each. If over budget, surface as a blocker.

Implement, test, commit.

---

#### Task M5.3: `run_plan` driver

Per v1 M5.3 with signature `pub fn run_plan(db: &Db, plan: &mut DeployPlan, creds: Option<(&str, &str)>, opts: RunOptions, emit: &mut dyn FnMut(DeployEvent))`.

---

#### Task M5.4: CLI `deploy ddc --plan=<json>`

Per v1 M5.4. Add `Domain::Deploy { .. }` to `needs_db()` in `cli/run.rs`. Mutate plan in handler.

---

#### Task M5.5: Tauri command `deploy_ddc_run`

```rust
// src-tauri/src/commands/deploy.rs
use crate::core::credentials as core_credentials;
use crate::core::deploy_workflow::{self, DeployEvent, DeployPlan, DeployStep, RunOptions};
use crate::data::{credentials as data_credentials, Db};
use crate::error::{UecmError, UecmResult};
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub fn deploy_ddc_plan_preview(plan: DeployPlan) -> Vec<DeployStep> {
    deploy_workflow::plan_steps(&plan)
}

#[tauri::command]
pub async fn deploy_ddc_run(
    db: State<'_, Db>,
    app: AppHandle,
    plan: DeployPlan,
    credential_alias: Option<String>,
    stop_on_failure: bool,
) -> UecmResult<()> {
    // Resolve credentials BEFORE entering spawn_blocking (State<Db> not Send).
    let creds: Option<(String, String)> = match credential_alias.as_deref() {
        Some(a) if !a.is_empty() => {
            let cred = data_credentials::find_by_alias(&db, a)?
                .ok_or_else(|| UecmError::InvalidInput(format!("credential '{}' not found", a)))?;
            let password = core_credentials::resolve_password(a)?;
            Some((cred.username, password))
        }
        _ => None,
    };

    let plan_owned = plan;
    let app_inner = app.clone();
    tokio::task::spawn_blocking(move || -> UecmResult<()> {
        // Open a thread-local DB handle. `crate::data::open` is the public re-export.
        let db_local = crate::data::open()?;
        let mut plan_mut = plan_owned;
        deploy_workflow::run_plan(
            &db_local, &mut plan_mut,
            creds.as_ref().map(|(u, p)| (u.as_str(), p.as_str())),
            RunOptions { stop_on_step_failure: stop_on_failure },
            &mut |e: DeployEvent| { app_inner.emit("deploy-event", &e).ok(); },
        );
        Ok(())
    })
    .await
    .map_err(|e| UecmError::OperationFailed(format!("task join: {}", e)))?
}
```

Register in `commands/mod.rs` and add to `tauri::generate_handler![...]` in `src-tauri/src/lib.rs`. Commit.

---

#### Task M5.6: `deployApi.ts` + Pinia store

Per v1 with type rename:

```ts
export interface DeployPlan {
  project_id: number;
  source_machine_id: number;        // was source_host
  target_machine_ids: number[];     // was target_hosts
  local_cache: { path: string; service_account: string | null };
  shared_cache: { server_machine_id: number; share_name: string; server_path: string; mode: string; unc_path: string | null };
  ddc_pak: { enabled: boolean };
  pso: { enabled: boolean; resolution: string; max_minutes: number };
  verify: { run_log_verify: boolean; editor_exe: string; timeout_seconds: number };
}
```

`runPlan(plan, credAlias, stopOnFailure)` invocation unchanged. Store rename `targets` → `targetMachineIds`. Commit.

---

#### Task M5.7: Wizard component

Per v1 with all paths as escaped JS strings:

```ts
const localPath = ref("D:\\UE-DDC-Local");
const sharePath = ref("D:\\DDC");
const editorExe = ref("C:\\Program Files\\Epic Games\\UE_5.5\\Engine\\Binaries\\Win64\\UnrealEditor.exe");
const uproject = ref("");
```

Source-host dropdown binds `:value="m.id"` (not hostname). Commit.

---

## Phase 2 — P1 (~11 working days)

Module M6–M11 per v1 with these blanket corrections applied:

- All `UecmError::Io(format!(...))` → `UecmError::OperationFailed(format!(...))`.
- All Tauri commands use `db: State<'_, Db>` + `data_credentials::find_by_alias` + `core_credentials::resolve_password`.
- All Vue strings use `\\` not `r"..."`.
- M11 adds a `HealthAction::ScanCommandLine` — `Domain::Health` is already in `needs_db()`.

### Module M6: Editor Pref 4-path reader (2 days)

M6.1 extends the M1.3 stub `editor_preferences::extract` with alt key names. M6.2 keeps R025 (already wired in M1.3). M6.3 creates Tauri command:

```rust
#[tauri::command]
pub fn read_editor_prefs(
    db: State<'_, Db>,
    machine_id: i64,
    credential_alias: String,
) -> UecmResult<EditorDdcPrefs>
```

Internally: resolve cred via `data_credentials::find_by_alias`, look up user profile via existing discovery code (`grep` for `user_profile` to find the helper — `discovery::get_user_profile_for_machine` if present; otherwise pass user profile path as an explicit parameter).

### Module M7: PSO completeness (1 day)

- `pso_collect.rs:147` filter: change extension check to file-name suffix check:
  ```rust
  let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
  let is_target = name.to_lowercase().ends_with(".upipelinecache")
      || name.to_lowercase().ends_with(".stablepc.csv");
  if !is_target { continue; }
  ```
- `DistributeProfile::pso_cache()` glob → `"*.upipelinecache *.stablepc.csv"` (Robocopy multi-glob).
- `ps-scripts/list-pso-cache-files.ps1:24` narrow `.csv` to `*.stablepc.csv`:
  ```powershell
  $_.Extension -eq '.upipelinecache' -or $_.Name -like '*.stablepc.csv'
  ```

### Module M8: Cluster consistency check (3 days)

Per v1. UecmError fixes applied.

### Module M9: GC strategy quick toggles (1 day)

Per v1, using `ini_editor::set_backend_field_with_credential` from M1.4. `Domain::Ini` already in `needs_db()`.

### Module M10: Local-vs-Shared file stats (2 days)

Per v1.

### Module M11: Command-line argument scanner (2 days)

Per v1.

---

## Phase 3 — P2 (~3.5 working days)

### Module M12: Baseline log snapshot (1 day)
### Module M13: UE 5.3 vs 5.4+ default-path awareness (0.5 day)
### Module M14: Historical symptom recognizer (2 days)

All per v1 with the same blanket corrections (`UecmError::Io` → `OperationFailed`, Tauri State<Db> pattern, Vue strings).

---

## Self-Review (v2)

### Codex review items — addressed

| Codex finding | v2 resolution |
|---|---|
| `needs_db()` not updated | M2.4, M3.3, M5.4 each explicitly extend it. API Reference section calls it out. |
| `parse_target` doesn't exist | M1.2 uses `parse_ini_contents` post-pass + adds `parse_for_diagnostics` wrapper. M1.5 uses it. |
| `UecmError::Io(String)` wrong | All v2 snippets use `UecmError::OperationFailed(format!(...))`. |
| `data::db::open()` invented | All Tauri commands use `db: State<'_, Db>` + `data_credentials::find_by_alias` + `core_credentials::resolve_password`. M5.5 uses `crate::data::open()` (the REAL re-export) only inside `spawn_blocking` where State is unavailable. |
| Invented pak/pso APIs | M5.2 calls `preflight + launch_generation + verify_output`, `plan + run_one_with_profile`, `launch_collection + enumerate_remote`, `plan + run_one`. Two `TODO(engineer)` wait loops with explicit pointers to existing copy sources. |
| `Project.path` wrong | M5.2 uses `project_locations::get_for_project_machine(db, pid, mid)?.abs_path` and `.uproject_path`. |
| Health probe shape | M3.1 emits `env_local` / `env_shared` as parallel `HashMap<String, CheckOutcome>` keys (with `env_vars` alias for back-compat). M4.3 emits `rs_service` as single `CheckOutcome`. Detailed payloads through dedicated Tauri commands. |
| Vue raw strings | M5.7 explicitly fixes all four defaults. |
| PSO glob wrong location | M7 fixes `DistributeProfile::pso_cache()` glob + narrows `list-pso-cache-files.ps1` to `*.stablepc.csv`. |
| `BTreeMap` loses order | M1.1 uses `Vec<(String, String)>` + test asserts 13-field order. |
| `BackendGraphAction` CLI missing | M1.5 dedicated task. |
| `read_editor_prefs` needs cred/profile | M6.3 takes `(db, machine_id, credential_alias)`. |

### Spec coverage

All 16 blocks of the去-Zen SOP map to a v2 module. Same coverage table as v1 — no regressions.

### Type consistency

- `DeployPlan` uses `source_machine_id: i64` / `target_machine_ids: Vec<i64>` everywhere.
- `BackendNode { name, fields: Vec<(String,String)>, line_number }` consistent across M1.1, M1.2, M1.4, M1.5.
- `VerifyReport` from M2.3 referenced verbatim in M5.2 + M14.1.
- `CheckOutcome` shape never extended — only new HashMap keys with the same shape.

### Engineer TODO budget

Two intentional `TODO(engineer)` blocks in M5.2 (`generate_pak_sync`, `collect_pso_sync`) — each bounded to 2 hours of "adapt existing wait loop from commands/". All other code is fully written.

---

## Execution Handoff

Plan v2 saved. Per user direction "写完直接执行", proceeding to subagent-driven execution now via `superpowers:subagent-driven-development`. First wave: M0 (one quick module), then M1 sequentially (5 tasks), then M2/M3/M4 in parallel, then M5 once its dependencies have landed.
