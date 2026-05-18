# DDC Full Coverage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make UECM detect every DDC-relevant configuration source listed in the RenderStream Shader DDC/PSO SOP (excluding ZenServer), and provide a one-click deployment workflow that provisions Local DDC + Shared SMB + DDC Pak + PSO on a cluster from a single wizard or CLI command.

**Architecture:** Three phases. Phase 1 (P0, 17 days) lands the five must-have modules: `[DerivedDataBackendGraph]` parser + rule set, `LogDerivedDataCache` startup log verifier, complete Local DDC configuration path, RenderStream service-account probe, and the `deploy` orchestrator. Phase 2 (P1, 11 days) fills in detection breadth: Editor Preferences four-path reader, PSO completeness, cluster consistency check, GC strategy read/write, file-count probe, and command-line argument scanner. Phase 3 (P2, 3.5 days) adds quality-of-life: baseline log snapshot, UE 5.3 / 5.4+ default-path awareness, and historical-symptom recognizer.

Each module is independently testable. Modules within a phase have no hard ordering except where called out as a dependency. The deploy orchestrator (M5) depends on M1, M2, M3, M4 — it must be the last P0 module to land.

**Tech Stack:** Rust (Tauri 2 backend, clap-derive CLI), PowerShell 5.1 sidecars (run via `powershell::run_json`), Vue 3 SFC + Tailwind + reka-ui + CVA (frontend), SQLite via existing `data::*` modules, vue-i18n for all user-facing strings.

---

## File Structure

### `src-tauri/src/core/`

| File | Action | Responsibility |
|---|---|---|
| `ini_backend_graph.rs` | CREATE | Parser for `Shared=(K1=V1, K2=V2, ...)` tuple-value syntax used in `[DerivedDataBackendGraph]` |
| `ini_diagnostics.rs` | MODIFY | Add rules R011–R024 over BackendGraph section + `r.ShaderPipelineCache.Enabled` |
| `ini_scanner.rs` | MODIFY | Scan BackendGraph section alongside DDC settings section |
| `ini_editor.rs` | MODIFY | Support setting/removing individual fields inside a `Shared=(...)` tuple |
| `ue_log_parser.rs` | CREATE | Parse `LogDerivedDataCache` startup output into structured facts |
| `ue_log_verify.rs` | CREATE | Orchestrate `UnrealEditor.exe -nullrhi -ExecCmds=quit -logcmds=...` on each host and feed log into parser |
| `ue_runner.rs` | MODIFY | Extend `parse_line` to recognize verify-mode events |
| `editor_preferences.rs` | CREATE | Read 4 DDC paths from `EditorPerProjectUserSettings.ini` (global/project × local/shared) |
| `local_cache.rs` | CREATE | Create local DDC directory on remote host with NTFS ACL for SYSTEM + RenderStream account |
| `renderstream_service.rs` | CREATE | Read service start-name + state for any service whose name matches RenderStream patterns |
| `consistency_check.rs` | CREATE | Cross-machine consistency probe: UE version / Plugin version / RHI / project path |
| `pso_collect.rs` | MODIFY | Also collect `*.stablepc.csv` alongside `*.upipelinecache` |
| `command_line_scanner.rs` | CREATE | Scan Windows shortcuts (`.lnk`), `.bat` files, and Service ImagePath for `-LocalDataCachePath` / `-SharedDataCachePath` |
| `baseline_log_snapshot.rs` | CREATE | Pull `LogDerivedDataCache` / `LogShaderCompilers` / `LogShaderPipelineCache` from each host into a versioned local archive |
| `ddc_symptom_recognizer.rs` | CREATE | Pattern matchers for known incident shapes (Shared DDC empty, Move collision, Deactivated due to latency) |
| `ddc_file_stats.rs` | CREATE | Probe file count + total size for Local DDC and Shared DDC paths |
| `deploy_workflow.rs` | CREATE | Orchestrate the 11-step DDC deployment as a single state machine with per-step rollback signals |
| `health_probes.rs` | MODIFY | Plumb new probes into round-trip script (Local env, RenderStream service) |

### `src-tauri/src/cli/`

| File | Action | Responsibility |
|---|---|---|
| `args.rs` | MODIFY | Add `Domain::Deploy`, `Domain::LocalCache`, `Domain::Log`; add `BackendGraphAction` subcommands under `Ini` |
| `domain_deploy.rs` | CREATE | Dispatcher for `deploy ddc <action>` |
| `domain_local_cache.rs` | CREATE | Dispatcher for `local-cache create / list` |
| `domain_log.rs` | CREATE | Dispatcher for `log verify-startup` |
| `domain_health.rs` | MODIFY | Add `health consistency-check` subcommand |
| `domain_ini.rs` | MODIFY | Wire BackendGraph set/get/scan subactions |

### `src-tauri/src/commands/`

| File | Action | Responsibility |
|---|---|---|
| `deploy.rs` | CREATE | Tauri commands for UI: `deploy_ddc_plan`, `deploy_ddc_run`, `deploy_ddc_status` |
| `local_cache.rs` | CREATE | Tauri commands: `create_local_cache_dir`, `read_local_cache_state` |
| `log_verify.rs` | CREATE | Tauri command: `run_log_verify` |
| `consistency.rs` | CREATE | Tauri command: `run_consistency_check` |

### `ps-scripts/`

| File | Action | Responsibility |
|---|---|---|
| `parse-ue-log.ps1` | CREATE | Run `UnrealEditor.exe` with verbose DDC logging and emit captured log path as JSON |
| `create-local-cache-dir.ps1` | CREATE | `New-Item -ItemType Directory` + `icacls` grant SYSTEM + service account FullControl |
| `probe-renderstream-service.ps1` | CREATE | Enumerate `Win32_Service` matching RenderStream names, return `StartName` / `State` / `StartMode` |
| `read-editor-preferences.ps1` | CREATE | Read 4 DDC path keys from `EditorPerProjectUserSettings.ini` (handles UE 5.x section names) |
| `scan-command-line-args.ps1` | CREATE | Walk shortcuts + bat scripts + service ImagePath; extract `-LocalDataCachePath=` / `-SharedDataCachePath=` values |
| `ddc-file-stats.ps1` | CREATE | Count files + sum bytes for Local DDC + Shared DDC paths |
| `consistency-snapshot.ps1` | CREATE | Single-host snapshot of UE versions, installed plugins, GPU/Driver, RHI, project paths |
| `snapshot-baseline-logs.ps1` | CREATE | Copy DDC/Shader/PSO logs from host to UECM controller, tag with timestamp |
| `health-probes.ps1` | MODIFY | Add `Probe-LocalEnvVar` + `Probe-RenderStreamService` |

### `src/`

| File | Action | Responsibility |
|---|---|---|
| `components/modals/BatchEnvVarModal.vue` | MODIFY | Variable-name becomes a dropdown (Local / Shared / Custom) |
| `components/modals/EnvVarConfigModal.vue` | MODIFY | Take `varName` as prop (already does), but UI defaults from caller |
| `components/modals/DeployDdcWizard.vue` | CREATE | 5-step wizard: Project → Targets → Local config → Shared config → Run + verify |
| `components/deploy/DeployStepIndicator.vue` | CREATE | Vertical step list with status tone per step |
| `components/deploy/DeployProgressTable.vue` | CREATE | Per-host per-step status grid |
| `components/deploy/DeployVerifyReport.vue` | CREATE | Render `LogDerivedDataCache` findings + symptom matches |
| `views/Deploy.vue` | CREATE | Route entry for the deploy domain |
| `stores/deploy.ts` | CREATE | Pinia store mirroring backend state |
| `lib/deployApi.ts` | CREATE | Tauri invoke wrappers |
| `lib/iniRules.ts` | MODIFY | Add labels for R011–R024 |
| `locales/zh.ts` | MODIFY | All new strings (deploy.*, localCache.*, logVerify.*) |
| `locales/en.ts` | MODIFY | Mirror zh |
| `router/index.ts` | MODIFY | Register `/deploy` route |

---

## Phase 1 — P0 (Must-have, ~17 working days)

### Module M1: `[DerivedDataBackendGraph]` parsing + rules (4 days)

**Goal:** Detect every parameter SOP lists in the `Shared=(...)` tuple. After this module, `uecm-cli ini scan` reports findings for any of the 13 BackendGraph keys.

**Files:**
- Create: `src-tauri/src/core/ini_backend_graph.rs`
- Modify: `src-tauri/src/core/ini_scanner.rs`
- Modify: `src-tauri/src/core/ini_diagnostics.rs`
- Modify: `src-tauri/src/core/ini_editor.rs`

**Dependencies:** None.

---

#### Task M1.1: Tuple-value parser

**Files:**
- Create: `src-tauri/src/core/ini_backend_graph.rs`
- Test: `src-tauri/src/core/ini_backend_graph.rs` (inline `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing test**

```rust
// src-tauri/src/core/ini_backend_graph.rs (entire file)
//! Parser for tuple-form values used in [DerivedDataBackendGraph] section.
//! Input shape: `Shared=(Type=FileSystem, ReadOnly=false, Path=\\NAS\DDC, ...)`
//! Outputs an ordered map of field-name -> field-value. Field values are
//! NOT unquoted — callers see the raw value as written.

use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct BackendNode {
    pub name: String,
    pub fields: BTreeMap<String, String>,
    pub raw: String,
    pub line_number: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    MissingOpenParen,
    MissingCloseParen,
    EmptyName,
}

pub fn parse_node(line: &str, line_number: u32) -> Result<BackendNode, ParseError> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_filesystem_node() {
        let line = r"Shared=(Type=FileSystem, Path=\\NAS\DDC, ReadOnly=false)";
        let n = parse_node(line, 12).unwrap();
        assert_eq!(n.name, "Shared");
        assert_eq!(n.fields.get("Type").map(String::as_str), Some("FileSystem"));
        assert_eq!(n.fields.get("Path").map(String::as_str), Some(r"\\NAS\DDC"));
        assert_eq!(n.fields.get("ReadOnly").map(String::as_str), Some("false"));
        assert_eq!(n.line_number, 12);
    }

    #[test]
    fn handles_full_sop_example() {
        let line = r"Shared=(Type=FileSystem, ReadOnly=false, Clean=false, Flush=false, DeleteUnused=true, UnusedFileAge=10, FoldersToClean=10, MaxFileChecksPerSec=1, ConsiderSlowAt=70, PromptIfMissing=false, Path=\\NAS\DDC, EnvPathOverride=UE-SharedDataCachePath, EditorOverrideSetting=SharedDerivedDataCache)";
        let n = parse_node(line, 1).unwrap();
        assert_eq!(n.fields.len(), 13);
    }

    #[test]
    fn rejects_missing_open_paren() {
        assert_eq!(parse_node("Shared=Type=Foo", 1), Err(ParseError::MissingOpenParen));
    }

    #[test]
    fn rejects_missing_close_paren() {
        assert_eq!(parse_node("Shared=(Type=Foo", 1), Err(ParseError::MissingCloseParen));
    }

    #[test]
    fn rejects_empty_name() {
        assert_eq!(parse_node("=(Type=Foo)", 1), Err(ParseError::EmptyName));
    }

    #[test]
    fn preserves_unc_backslashes_in_path() {
        let n = parse_node(r"Shared=(Path=\\192.168.10.2\Docs\DDC)", 1).unwrap();
        assert_eq!(n.fields.get("Path").map(String::as_str), Some(r"\\192.168.10.2\Docs\DDC"));
    }

    #[test]
    fn trims_whitespace_around_field_separators() {
        let n = parse_node("Shared=(Type=FileSystem,  ReadOnly = false  )", 1).unwrap();
        assert_eq!(n.fields.get("ReadOnly").map(String::as_str), Some("false"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd src-tauri && cargo test --lib core::ini_backend_graph 2>&1 | tail -20
```

Expected: 6 test compile but panic with `not yet implemented`.

- [ ] **Step 3: Implement parser**

Replace the `todo!()` body in `parse_node`:

```rust
pub fn parse_node(line: &str, line_number: u32) -> Result<BackendNode, ParseError> {
    let eq = line.find('=').ok_or(ParseError::MissingOpenParen)?;
    let name = line[..eq].trim().to_string();
    if name.is_empty() {
        return Err(ParseError::EmptyName);
    }
    let rest = line[eq + 1..].trim_start();
    let open = rest.find('(').ok_or(ParseError::MissingOpenParen)?;
    if open != 0 {
        return Err(ParseError::MissingOpenParen);
    }
    let close = rest.rfind(')').ok_or(ParseError::MissingCloseParen)?;
    let body = &rest[1..close];

    let mut fields = BTreeMap::new();
    for part in body.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let Some(p_eq) = part.find('=') else { continue; };
        let k = part[..p_eq].trim().to_string();
        let v = part[p_eq + 1..].trim().to_string();
        if !k.is_empty() {
            fields.insert(k, v);
        }
    }

    Ok(BackendNode {
        name,
        fields,
        raw: line.to_string(),
        line_number,
    })
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cd src-tauri && cargo test --lib core::ini_backend_graph 2>&1 | tail -10
```

Expected: `test result: ok. 6 passed`.

- [ ] **Step 5: Register module**

Edit `src-tauri/src/core/mod.rs`, add:

```rust
pub mod ini_backend_graph;
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/core/ini_backend_graph.rs src-tauri/src/core/mod.rs
git commit -m "feat(ddc): add [DerivedDataBackendGraph] tuple-value parser"
```

---

#### Task M1.2: Scan BackendGraph section in `ini_scanner.rs`

**Files:**
- Modify: `src-tauri/src/core/ini_scanner.rs`
- Modify: `src-tauri/src/core/ini_diagnostics.rs` (add `EnvVarState` plumbing — already exists, just enrich)

- [ ] **Step 1: Write the failing test**

Add to `src-tauri/src/core/ini_scanner.rs` `#[cfg(test)] mod tests`:

```rust
#[test]
fn scans_backend_graph_section_with_tuple_values() {
    use crate::core::ini_diagnostics::Category;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("DefaultEngine.ini");
    std::fs::write(&path, r"
[DerivedDataBackendGraph]
Shared=(Type=FileSystem, Path=\\NAS\DDC, ReadOnly=false, DeleteUnused=true, UnusedFileAge=10)
Boot=(Type=Boot, Filename=DDC)
").unwrap();

    let parsed = parse_target(&TargetFile {
        path: path.to_string_lossy().to_string(),
        category: Category::Project,
    }).unwrap();

    let bg = parsed
        .sections
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case("DerivedDataBackendGraph"))
        .expect("BackendGraph section missing");

    assert!(bg.backend_nodes.iter().any(|n| n.name == "Shared"));
    let shared = bg.backend_nodes.iter().find(|n| n.name == "Shared").unwrap();
    assert_eq!(shared.fields.get("Type").map(String::as_str), Some("FileSystem"));
    assert_eq!(shared.fields.get("UnusedFileAge").map(String::as_str), Some("10"));
}
```

- [ ] **Step 2: Run test, verify it fails**

```bash
cd src-tauri && cargo test --lib core::ini_scanner::tests::scans_backend_graph_section 2>&1 | tail -15
```

Expected: compile error — `ParsedSection` has no field `backend_nodes`.

- [ ] **Step 3: Extend `ParsedSection` and `parse_target`**

In `src-tauri/src/core/ini_diagnostics.rs`, find the `ParsedSection` struct and add the field:

```rust
pub struct ParsedSection {
    pub name: String,
    pub keys: Vec<ParsedKey>,
    pub backend_nodes: Vec<crate::core::ini_backend_graph::BackendNode>,
}
```

In `src-tauri/src/core/ini_scanner.rs`, locate `parse_target` (the function that builds `ParsedSection` from file content). For each section whose name equals `DerivedDataBackendGraph` (case-insensitive), pass each line through `ini_backend_graph::parse_node`; collect successes into `backend_nodes` and treat parse failures as plain `ParsedKey` entries so the file still round-trips.

Pseudocode for the `parse_target` body (replace the section-build loop):

```rust
let is_backend_graph = current_section_name
    .eq_ignore_ascii_case("DerivedDataBackendGraph");
if is_backend_graph {
    if let Ok(node) = crate::core::ini_backend_graph::parse_node(line, line_number) {
        current_backend_nodes.push(node);
        continue;
    }
}
// fall through to existing key=value parse...
```

When pushing the section, populate `backend_nodes: current_backend_nodes` (default `vec![]` for non-BackendGraph sections).

Also update every test in `ini_diagnostics.rs` that constructs `ParsedSection` literally — add `backend_nodes: vec![],` to each. Use `grep -n "ParsedSection {" src-tauri/src/core/ini_diagnostics.rs` to find them.

- [ ] **Step 4: Run test to verify it passes**

```bash
cd src-tauri && cargo test --lib 2>&1 | tail -20
```

Expected: all tests pass, including new `scans_backend_graph_section_with_tuple_values`.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/core/ini_scanner.rs src-tauri/src/core/ini_diagnostics.rs
git commit -m "feat(ddc): scan [DerivedDataBackendGraph] section into ParsedSection.backend_nodes"
```

---

#### Task M1.3: Diagnostic rules R011–R024 over BackendGraph fields

**Files:**
- Modify: `src-tauri/src/core/ini_diagnostics.rs`

Add 14 rules — one per SOP-listed field (13 fields) + R024 for `r.ShaderPipelineCache.Enabled` (the PSO completeness rule we are folding in here since it sits adjacent in `ConsoleVariables.ini`).

The rules share structure. Implement a generic `backend_field_rule` helper that takes the rule id / field name / expected predicate / recommended value / severity / symptom / rationale, plus rule-specific helpers for path / type / numeric-range checks.

- [ ] **Step 1: Write the failing tests (one per rule)**

Append to the `#[cfg(test)] mod tests` block in `ini_diagnostics.rs`. Helper first:

```rust
fn backend_section(node_raw: &str) -> ParsedSection {
    use crate::core::ini_backend_graph::parse_node;
    let node = parse_node(node_raw, 0).expect("node must parse");
    ParsedSection {
        name: "DerivedDataBackendGraph".into(),
        keys: vec![],
        backend_nodes: vec![node],
    }
}

fn ddb_project(node_raw: &str) -> ParsedFile {
    ParsedFile {
        path: r"C:\Project\Config\DefaultEngine.ini".into(),
        category: Category::Project,
        sections: vec![backend_section(node_raw)],
    }
}
```

Then 14 tests. One full example, then a list with the exact tuple, expected rule id, severity, and what the rule asserts:

```rust
#[test]
fn r011_critical_when_shared_type_missing() {
    let file = ddb_project(r"Shared=(Path=\\NAS\DDC)");
    let env = EnvVarState::default();
    let findings = run_rules(&file, &env);
    assert!(findings.iter().any(|f| f.rule_id == "R011" && f.severity == Severity::Critical));
}
```

For each rule below, write one test asserting it fires when the offending pattern is present, and one asserting it does NOT fire when the value is correct:

| Rule | Field | Fires when | Severity | Recommended value |
|---|---|---|---|---|
| R011 | `Type` | missing OR not `FileSystem` (for Shared node) | Critical | `FileSystem` |
| R012 | `ReadOnly` | `=true` on a Shared node UE writes to | Warning | `false` |
| R013 | `Clean` | `=true` | Critical | `false` |
| R014 | `Flush` | `=true` | Warning | `false` |
| R015 | `DeleteUnused` | missing | Warning | `true` |
| R016 | `UnusedFileAge` | < 1 OR > 365 OR non-numeric | Warning | `10` |
| R017 | `FoldersToClean` | < 1 OR > 100 OR non-numeric | Warning | `10` |
| R018 | `MaxFileChecksPerSec` | < 1 OR > 100 OR non-numeric | Warning | `1` |
| R019 | `ConsiderSlowAt` | < 10 OR > 1000 OR non-numeric | Warning | `70` |
| R020 | `PromptIfMissing` | `=true` | Critical | `false` |
| R021 | `Path` | missing OR not UNC (doesn't start with `\\`) | Critical | `\\HOST\Share` |
| R022 | `EnvPathOverride` | missing | Warning | `UE-SharedDataCachePath` |
| R023 | `EditorOverrideSetting` | missing | Info | `SharedDerivedDataCache` |
| R024 | `r.ShaderPipelineCache.Enabled` in ConsoleVariables.ini `[ConsoleVariables]` section | missing OR != `1` | Critical | `1` |

Write all 14 fires-tests AND 14 does-not-fire-tests. Helper `assert_fires(rule_id, file)` and `assert_does_not_fire(rule_id, file)` to keep them readable:

```rust
fn assert_fires(rule: &str, file: &ParsedFile) {
    let env = EnvVarState::default();
    let findings = run_rules(file, &env);
    assert!(findings.iter().any(|f| f.rule_id == rule),
        "expected {} to fire; got {:?}",
        rule,
        findings.iter().map(|f| &f.rule_id).collect::<Vec<_>>());
}
fn assert_silent(rule: &str, file: &ParsedFile) {
    let env = EnvVarState::default();
    let findings = run_rules(file, &env);
    assert!(!findings.iter().any(|f| f.rule_id == rule),
        "expected {} silent; got {:?}",
        rule, findings);
}
```

- [ ] **Step 2: Run tests, verify all 28 fail**

```bash
cd src-tauri && cargo test --lib core::ini_diagnostics::tests::r01 core::ini_diagnostics::tests::r02 2>&1 | tail -30
```

Expected: 28 failures, all `expected RXXX to fire; got []`.

- [ ] **Step 3: Implement rules**

In `src-tauri/src/core/ini_diagnostics.rs`, add a helper that finds the `Shared` node:

```rust
fn find_shared_backend(file: &ParsedFile) -> Option<&crate::core::ini_backend_graph::BackendNode> {
    file.sections
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case("DerivedDataBackendGraph"))?
        .backend_nodes
        .iter()
        .find(|n| n.name.eq_ignore_ascii_case("Shared"))
}
```

Add to `run_rules`:

```rust
out.extend(rule_r011(file));
out.extend(rule_r012(file));
out.extend(rule_r013(file));
out.extend(rule_r014(file));
out.extend(rule_r015(file));
out.extend(rule_r016(file));
out.extend(rule_r017(file));
out.extend(rule_r018(file));
out.extend(rule_r019(file));
out.extend(rule_r020(file));
out.extend(rule_r021(file));
out.extend(rule_r022(file));
out.extend(rule_r023(file));
out.extend(pso_cvar_rule(
    file, "R024", "r.ShaderPipelineCache.Enabled",
    Severity::Critical,
    "PSO cache file loading is disabled or not configured.",
    "Without this CVar collected PSO cache files are not loaded at runtime; precaching effort is wasted.",
));
```

Implement each rule. For boolean / equality rules, use this pattern (showing R012):

```rust
fn rule_r012(file: &ParsedFile) -> Vec<Finding> {
    let Some(node) = find_shared_backend(file) else { return vec![]; };
    let Some(v) = node.fields.get("ReadOnly") else { return vec![]; };
    if v.eq_ignore_ascii_case("true") {
        return vec![Finding {
            rule_id: "R012".into(),
            severity: Severity::Warning,
            category: file.category,
            file_path: file.path.clone(),
            section: Some("DerivedDataBackendGraph".into()),
            key_name: Some("Shared.ReadOnly".into()),
            line_number: Some(node.line_number as i64),
            snippet_before: format!("ReadOnly={}", v),
            snippet_after: Some("ReadOnly=false".into()),
            recommended_action: RecommendedAction::Set,
            recommended_value: Some("false".into()),
            symptom: "Shared DDC is read-only; cluster cannot write back derived data.".into(),
            rationale: "Render nodes must be able to push their first-run results back so siblings can hit cache.".into(),
        }];
    }
    vec![]
}
```

For numeric range rules (showing R016):

```rust
fn rule_r016(file: &ParsedFile) -> Vec<Finding> {
    let Some(node) = find_shared_backend(file) else { return vec![]; };
    let Some(v) = node.fields.get("UnusedFileAge") else { return vec![]; };
    let ok = v.parse::<i64>().map(|n| (1..=365).contains(&n)).unwrap_or(false);
    if ok { return vec![]; }
    vec![Finding {
        rule_id: "R016".into(),
        severity: Severity::Warning,
        category: file.category,
        file_path: file.path.clone(),
        section: Some("DerivedDataBackendGraph".into()),
        key_name: Some("Shared.UnusedFileAge".into()),
        line_number: Some(node.line_number as i64),
        snippet_before: format!("UnusedFileAge={}", v),
        snippet_after: Some("UnusedFileAge=10".into()),
        recommended_action: RecommendedAction::Set,
        recommended_value: Some("10".into()),
        symptom: "UnusedFileAge is out of sane range (expected 1–365 days).".into(),
        rationale: "GC sweeps need a meaningful retention window; misconfiguration causes excessive deletion or unbounded growth.".into(),
    }]
}
```

For path rules (showing R021):

```rust
fn rule_r021(file: &ParsedFile) -> Vec<Finding> {
    let Some(node) = find_shared_backend(file) else { return vec![]; };
    let path = node.fields.get("Path").map(String::as_str).unwrap_or("");
    let is_unc = path.starts_with(r"\\");
    if !is_unc {
        return vec![Finding {
            rule_id: "R021".into(),
            severity: Severity::Critical,
            category: file.category,
            file_path: file.path.clone(),
            section: Some("DerivedDataBackendGraph".into()),
            key_name: Some("Shared.Path".into()),
            line_number: Some(node.line_number as i64),
            snippet_before: format!("Path={}", path),
            snippet_after: Some(r"Path=\\HOST\Share".into()),
            recommended_action: RecommendedAction::Manual,
            recommended_value: None,
            symptom: if path.is_empty() {
                "Shared backend Path is missing.".into()
            } else {
                "Shared backend Path is not a UNC path.".into()
            },
            rationale: "Mapped drives are invisible to Windows services and RenderStream; UNC paths resolve in every account context.".into(),
        }];
    }
    vec![]
}
```

Write the remaining 10 rules following the same shape — each one self-contained, no shared "data driven" abstraction, because the symptom / rationale text is rule-specific and inlining keeps them grep-able.

For R024, `pso_cvar_rule` already exists and works against `ConsoleVariables.ini` `[ConsoleVariables]` section — pass new arguments and it will produce findings.

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd src-tauri && cargo test --lib core::ini_diagnostics 2>&1 | tail -15
```

Expected: all ini_diagnostics tests pass (existing + 28 new).

- [ ] **Step 5: Update `src/lib/iniRules.ts` with display labels**

Edit `src/lib/iniRules.ts`, add to the `R001`-keyed object literal:

```ts
R011: { title: "Shared backend Type missing or wrong", tone: "critical" },
R012: { title: "Shared DDC marked ReadOnly", tone: "warning" },
R013: { title: "Shared DDC Clean=true wipes cache on launch", tone: "critical" },
R014: { title: "Shared DDC Flush=true clears cache on exit", tone: "warning" },
R015: { title: "Shared DDC DeleteUnused not configured", tone: "warning" },
R016: { title: "UnusedFileAge out of sane range", tone: "warning" },
R017: { title: "FoldersToClean out of sane range", tone: "warning" },
R018: { title: "MaxFileChecksPerSec out of sane range", tone: "warning" },
R019: { title: "ConsiderSlowAt threshold off", tone: "warning" },
R020: { title: "PromptIfMissing=true breaks unattended starts", tone: "critical" },
R021: { title: "Shared Path missing or not UNC", tone: "critical" },
R022: { title: "EnvPathOverride not set", tone: "warning" },
R023: { title: "EditorOverrideSetting not declared", tone: "info" },
R024: { title: "r.ShaderPipelineCache.Enabled not 1", tone: "critical" },
```

Also add display strings to `src/locales/zh.ts` and `src/locales/en.ts` for any new symptom text the UI surfaces.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/core/ini_diagnostics.rs src/lib/iniRules.ts src/locales/zh.ts src/locales/en.ts
git commit -m "feat(ddc): add R011-R024 diagnostic rules over [DerivedDataBackendGraph]"
```

---

#### Task M1.4: INI editor — write individual fields inside a `Shared=(...)` tuple

**Files:**
- Modify: `src-tauri/src/core/ini_editor.rs`
- Create: `ps-scripts/set-backend-field.ps1`

The existing `set_key_with_credential` writes a whole `key=value` line. The BackendGraph node is itself one line whose value is a tuple — we need to update *one field* inside that tuple without rewriting the rest.

- [ ] **Step 1: Write the failing test**

Append to `src-tauri/src/core/ini_editor.rs` `#[cfg(test)] mod tests`:

```rust
#[test]
#[cfg(not(windows))]
fn set_backend_field_returns_powershell_error_on_non_windows() {
    let r = set_backend_field_with_credential(
        "HOST", r"C:\f.ini", "DerivedDataBackendGraph", "Shared", "ReadOnly", "false",
        "u", "p",
    );
    assert!(matches!(r, Err(UecmError::PowerShell(_))));
}

#[test]
fn set_backend_field_writes_directly_for_loopback_target() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("DefaultEngine.ini");
    std::fs::write(&path, "[DerivedDataBackendGraph]\nShared=(Type=FileSystem, ReadOnly=true, Path=\\\\NAS\\DDC)\n").unwrap();

    write_backend_field_local(
        path.to_str().unwrap(),
        "DerivedDataBackendGraph",
        "Shared",
        "ReadOnly",
        "false",
    ).unwrap();

    let body = std::fs::read_to_string(&path).unwrap();
    assert!(body.contains("ReadOnly=false"));
    assert!(!body.contains("ReadOnly=true"));
    assert!(body.contains("Type=FileSystem"));
    assert!(body.contains("Path=\\\\NAS\\DDC"));
}

#[test]
fn set_backend_field_inserts_missing_field() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("DefaultEngine.ini");
    std::fs::write(&path, "[DerivedDataBackendGraph]\nShared=(Type=FileSystem)\n").unwrap();

    write_backend_field_local(
        path.to_str().unwrap(),
        "DerivedDataBackendGraph",
        "Shared",
        "ReadOnly",
        "false",
    ).unwrap();

    let body = std::fs::read_to_string(&path).unwrap();
    assert!(body.contains("ReadOnly=false"));
    assert!(body.contains("Type=FileSystem"));
}
```

- [ ] **Step 2: Run tests, verify failure**

```bash
cd src-tauri && cargo test --lib core::ini_editor::tests::set_backend_field 2>&1 | tail -10
```

Expected: compile errors — functions don't exist.

- [ ] **Step 3: Implement `write_backend_field_local`**

Add to `src-tauri/src/core/ini_editor.rs`:

```rust
fn write_backend_field_local(
    file_path: &str,
    section: &str,
    node_name: &str,
    field: &str,
    value: &str,
) -> UecmResult<()> {
    use std::fs;
    let body = fs::read_to_string(file_path)
        .map_err(|e| UecmError::Io(format!("read {}: {}", file_path, e)))?;
    let mut out: Vec<String> = Vec::with_capacity(body.lines().count() + 1);
    let mut in_section = false;
    let mut handled = false;
    for raw in body.lines() {
        let line = raw.trim_end_matches(['\r', '\n']);
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_section = trimmed[1..trimmed.len() - 1].eq_ignore_ascii_case(section);
            out.push(line.to_string());
            continue;
        }
        if in_section && !handled {
            if let Ok(node) = crate::core::ini_backend_graph::parse_node(line, 0) {
                if node.name.eq_ignore_ascii_case(node_name) {
                    let mut fields = node.fields.clone();
                    fields.insert(field.to_string(), value.to_string());
                    let body_str = fields
                        .iter()
                        .map(|(k, v)| format!("{}={}", k, v))
                        .collect::<Vec<_>>()
                        .join(", ");
                    out.push(format!("{}=({})", node.name, body_str));
                    handled = true;
                    continue;
                }
            }
        }
        out.push(line.to_string());
    }
    if !handled {
        return Err(UecmError::OperationFailed(format!(
            "section [{}] node {} not found in {}",
            section, node_name, file_path
        )));
    }
    out.push(String::new());
    fs::write(file_path, out.join("\n"))
        .map_err(|e| UecmError::Io(format!("write {}: {}", file_path, e)))?;
    Ok(())
}

pub fn set_backend_field_with_credential(
    host: &str,
    file_path: &str,
    section: &str,
    node_name: &str,
    field: &str,
    value: &str,
    username: &str,
    password: &str,
) -> UecmResult<String> {
    if crate::core::loopback::is_loopback(host) {
        write_backend_field_local(file_path, section, node_name, field, value)?;
        return Ok(format!("wrote {}.{} locally", node_name, field));
    }
    let result: PsResult = crate::core::powershell::run_json(
        &crate::core::powershell::script_path("set-backend-field.ps1"),
        &[
            "-HostName", host,
            "-FilePath", file_path,
            "-SectionName", section,
            "-NodeName", node_name,
            "-FieldName", field,
            "-FieldValue", value,
            "-Username", username,
            "-Password", password,
        ],
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(result.message));
    }
    Ok(result.message)
}
```

- [ ] **Step 4: Write the PowerShell sidecar**

Create `ps-scripts/set-backend-field.ps1`:

```powershell
# Set a single field inside a [Section] Node=(K1=V1, K2=V2, ...) tuple in an INI file on a remote host.
param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$FilePath,
    [Parameter(Mandatory=$true)] [string]$SectionName,
    [Parameter(Mandatory=$true)] [string]$NodeName,
    [Parameter(Mandatory=$true)] [string]$FieldName,
    [Parameter(Mandatory=$true)] [string]$FieldValue,
    [string]$Username,
    [string]$Password
)
$ErrorActionPreference = 'Stop'

$script = {
    param($FilePath, $SectionName, $NodeName, $FieldName, $FieldValue)
    if (-not (Test-Path -LiteralPath $FilePath)) { throw "file not found: $FilePath" }
    $lines = Get-Content -LiteralPath $FilePath
    $inSection = $false
    $handled = $false
    $out = New-Object System.Collections.Generic.List[string]
    foreach ($line in $lines) {
        $trim = $line.Trim()
        if ($trim.StartsWith('[') -and $trim.EndsWith(']')) {
            $inSection = ($trim.Trim('[',']') -ieq $SectionName)
            $out.Add($line); continue
        }
        if ($inSection -and -not $handled) {
            $eq = $line.IndexOf('=')
            if ($eq -gt 0) {
                $name = $line.Substring(0, $eq).Trim()
                $rest = $line.Substring($eq + 1).TrimStart()
                if (($name -ieq $NodeName) -and $rest.StartsWith('(') -and $rest.EndsWith(')')) {
                    $body = $rest.Substring(1, $rest.Length - 2)
                    $fields = [ordered]@{}
                    foreach ($pair in $body -split ',') {
                        $p = $pair.Trim()
                        if (-not $p) { continue }
                        $peq = $p.IndexOf('=')
                        if ($peq -lt 0) { continue }
                        $fields[$p.Substring(0, $peq).Trim()] = $p.Substring($peq + 1).Trim()
                    }
                    $fields[$FieldName] = $FieldValue
                    $parts = foreach ($k in $fields.Keys) { "$k=$($fields[$k])" }
                    $out.Add("$NodeName=($([string]::Join(', ', $parts)))")
                    $handled = $true
                    continue
                }
            }
        }
        $out.Add($line)
    }
    if (-not $handled) { throw "section [$SectionName] node $NodeName not found" }
    Set-Content -LiteralPath $FilePath -Value $out -Encoding UTF8
}

try {
    if ($Username) {
        $pass = ConvertTo-SecureString $Password -AsPlainText -Force
        $cred = New-Object System.Management.Automation.PSCredential($Username, $pass)
        Invoke-Command -ComputerName $HostName -Credential $cred -Authentication Default `
            -ScriptBlock $script -ArgumentList $FilePath, $SectionName, $NodeName, $FieldName, $FieldValue
    } else {
        Invoke-Command -ComputerName $HostName -ScriptBlock $script `
            -ArgumentList $FilePath, $SectionName, $NodeName, $FieldName, $FieldValue
    }
    @{ ok = $true; message = "set $NodeName.$FieldName=$FieldValue on $HostName" } | ConvertTo-Json -Compress
} catch {
    @{ ok = $false; message = $_.Exception.Message } | ConvertTo-Json -Compress
}
```

- [ ] **Step 5: Run tests to verify they pass**

```bash
cd src-tauri && cargo test --lib core::ini_editor 2>&1 | tail -10
```

Expected: all editor tests pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/core/ini_editor.rs ps-scripts/set-backend-field.ps1
git commit -m "feat(ddc): write BackendGraph tuple fields via ini editor + ps sidecar"
```

---

### Module M2: `LogDerivedDataCache` startup log verifier (3 days)

**Goal:** After UECM configures DDC on N machines, kick a non-rendering UE start on each and parse the resulting `LogDerivedDataCache` lines into structured facts: which Local path was used, which Shared path, whether Shared was deactivated, the file-count snapshot, and any `Move collision` warnings. This is the "what UE actually used" ground truth that SOP demands.

**Files:**
- Create: `src-tauri/src/core/ue_log_parser.rs`
- Create: `src-tauri/src/core/ue_log_verify.rs`
- Modify: `src-tauri/src/core/ue_runner.rs`
- Create: `ps-scripts/parse-ue-log.ps1`
- Create: `src-tauri/src/cli/domain_log.rs`
- Modify: `src-tauri/src/cli/args.rs`

**Dependencies:** None.

---

#### Task M2.1: Pure log-line parser

**Files:**
- Create: `src-tauri/src/core/ue_log_parser.rs`

- [ ] **Step 1: Write the failing test**

```rust
// src-tauri/src/core/ue_log_parser.rs
//! Parses LogDerivedDataCache lines into structured facts. Pure: takes a
//! string slice, returns enums. No I/O.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DdcEvent {
    LocalPath { path: String, writable: bool },
    SharedPath { path: String, writable: bool },
    SharedDeactivated { reason: String },
    MaintenanceFinished { layer: String, file_count: u64, total_bytes: u64 },
    MoveCollision { path: String },
    PakOpened { path: String },
    Other,
}

pub fn parse_line(line: &str) -> DdcEvent {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_local_path_writable() {
        let e = parse_line(r"LogDerivedDataCache: Using Local data cache path D:\DDC: Writable");
        assert_eq!(e, DdcEvent::LocalPath { path: r"D:\DDC".into(), writable: true });
    }

    #[test]
    fn parses_local_path_readonly() {
        let e = parse_line(r"LogDerivedDataCache: Using Local data cache path D:\DDC: ReadOnly");
        assert_eq!(e, DdcEvent::LocalPath { path: r"D:\DDC".into(), writable: false });
    }

    #[test]
    fn parses_shared_path() {
        let e = parse_line(r"LogDerivedDataCache: Using Shared data cache path \\NAS\DDC: Writable");
        assert_eq!(e, DdcEvent::SharedPath { path: r"\\NAS\DDC".into(), writable: true });
    }

    #[test]
    fn parses_deactivated_due_to_latency() {
        let e = parse_line(r"LogDerivedDataCache: Warning: Shared backend deactivated due to latency (87ms over 70ms threshold)");
        match e {
            DdcEvent::SharedDeactivated { reason } => assert!(reason.contains("latency")),
            _ => panic!("expected SharedDeactivated, got {:?}", e),
        }
    }

    #[test]
    fn parses_maintenance_summary() {
        let e = parse_line(r"LogDerivedDataCache: Maintenance finished on Shared: 152 files, 30 MiB");
        match e {
            DdcEvent::MaintenanceFinished { layer, file_count, total_bytes } => {
                assert_eq!(layer, "Shared");
                assert_eq!(file_count, 152);
                assert_eq!(total_bytes, 30 * 1024 * 1024);
            }
            _ => panic!("got {:?}", e),
        }
    }

    #[test]
    fn parses_move_collision() {
        let e = parse_line(r"LogDerivedDataCache: Warning: Move collision when writing \\NAS\DDC\AB\CD\hash.udd");
        match e {
            DdcEvent::MoveCollision { path } => assert!(path.contains(r"AB\CD")),
            _ => panic!("got {:?}", e),
        }
    }

    #[test]
    fn unknown_line_is_other() {
        assert_eq!(parse_line("LogTemp: hello"), DdcEvent::Other);
    }
}
```

- [ ] **Step 2: Run tests, verify failure**

```bash
cd src-tauri && cargo test --lib core::ue_log_parser 2>&1 | tail -10
```

- [ ] **Step 3: Implement parser**

Replace `todo!()`:

```rust
pub fn parse_line(line: &str) -> DdcEvent {
    let body = match line.strip_prefix("LogDerivedDataCache: ") {
        Some(b) => b,
        None => return DdcEvent::Other,
    };
    let body = body.trim_start_matches("Warning: ").trim_start_matches("Display: ");

    if let Some(rest) = body.strip_prefix("Using Local data cache path ") {
        let (path, suffix) = split_path_suffix(rest);
        return DdcEvent::LocalPath { path, writable: suffix.eq_ignore_ascii_case("Writable") };
    }
    if let Some(rest) = body.strip_prefix("Using Shared data cache path ") {
        let (path, suffix) = split_path_suffix(rest);
        return DdcEvent::SharedPath { path, writable: suffix.eq_ignore_ascii_case("Writable") };
    }
    if let Some(rest) = body.strip_prefix("Shared backend deactivated") {
        return DdcEvent::SharedDeactivated { reason: rest.trim().to_string() };
    }
    if let Some(rest) = body.strip_prefix("Maintenance finished on ") {
        if let Some((layer, stats)) = rest.split_once(": ") {
            if let Some((count_part, size_part)) = stats.split_once(", ") {
                let file_count = count_part
                    .trim_end_matches(" files")
                    .trim()
                    .parse::<u64>()
                    .unwrap_or(0);
                let total_bytes = parse_size_with_unit(size_part.trim());
                return DdcEvent::MaintenanceFinished {
                    layer: layer.trim().to_string(),
                    file_count,
                    total_bytes,
                };
            }
        }
    }
    if let Some(rest) = body.strip_prefix("Move collision when writing ") {
        return DdcEvent::MoveCollision { path: rest.trim().to_string() };
    }
    if let Some(rest) = body.strip_prefix("Opened pak ") {
        return DdcEvent::PakOpened { path: rest.trim().to_string() };
    }
    DdcEvent::Other
}

fn split_path_suffix(rest: &str) -> (String, String) {
    if let Some(idx) = rest.rfind(": ") {
        (rest[..idx].trim().to_string(), rest[idx + 2..].trim().to_string())
    } else {
        (rest.trim().to_string(), String::new())
    }
}

fn parse_size_with_unit(s: &str) -> u64 {
    let s = s.trim();
    let (num, unit): (f64, u64) = if let Some(n) = s.strip_suffix(" GiB") {
        (n.trim().parse().unwrap_or(0.0), 1024u64.pow(3))
    } else if let Some(n) = s.strip_suffix(" MiB") {
        (n.trim().parse().unwrap_or(0.0), 1024u64.pow(2))
    } else if let Some(n) = s.strip_suffix(" KiB") {
        (n.trim().parse().unwrap_or(0.0), 1024)
    } else if let Some(n) = s.strip_suffix(" B") {
        (n.trim().parse().unwrap_or(0.0), 1)
    } else {
        return 0;
    };
    (num * unit as f64) as u64
}
```

- [ ] **Step 4: Run tests to verify**

```bash
cd src-tauri && cargo test --lib core::ue_log_parser 2>&1 | tail -10
```

Expected: 7 passed.

- [ ] **Step 5: Register module**

In `src-tauri/src/core/mod.rs`:

```rust
pub mod ue_log_parser;
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/core/ue_log_parser.rs src-tauri/src/core/mod.rs
git commit -m "feat(ddc): parse LogDerivedDataCache lines into structured events"
```

---

#### Task M2.2: PowerShell sidecar to run UE verbose-log and capture output

**Files:**
- Create: `ps-scripts/parse-ue-log.ps1`

- [ ] **Step 1: Author the script**

```powershell
# Runs UnrealEditor.exe in nullrhi mode with DDC verbose logging, captures the
# log file path, and returns the parsed log contents up to a configurable size
# cap. Designed to be called over WinRM via run_json.
param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$EditorExe,
    [Parameter(Mandatory=$true)] [string]$ProjectPath,
    [int]$TimeoutSeconds = 180,
    [int]$MaxLogBytes = 2097152,
    [string]$Username,
    [string]$Password
)
$ErrorActionPreference = 'Stop'

$script = {
    param($EditorExe, $ProjectPath, $TimeoutSeconds, $MaxLogBytes)
    if (-not (Test-Path -LiteralPath $EditorExe)) { throw "editor not found: $EditorExe" }
    if (-not (Test-Path -LiteralPath $ProjectPath)) { throw "project not found: $ProjectPath" }

    $logDir = Join-Path $env:TEMP "uecm-log-verify-$(Get-Random)"
    New-Item -ItemType Directory -Path $logDir -Force | Out-Null
    $logFile = Join-Path $logDir 'verify.log'

    $args = @(
        $ProjectPath,
        '-nullrhi',
        '-nosound',
        '-unattended',
        '-nopause',
        '-ExecCmds=quit',
        '-logcmds=LogDerivedDataCache Verbose',
        "-abslog=$logFile"
    )
    $proc = Start-Process -FilePath $EditorExe -ArgumentList $args -PassThru -WindowStyle Hidden
    if (-not $proc.WaitForExit($TimeoutSeconds * 1000)) {
        try { $proc.Kill() } catch {}
        throw "editor did not exit within $TimeoutSeconds s"
    }
    if (-not (Test-Path -LiteralPath $logFile)) { throw "log not produced at $logFile" }
    $size = (Get-Item $logFile).Length
    $content = if ($size -le $MaxLogBytes) {
        Get-Content -LiteralPath $logFile -Raw -Encoding UTF8
    } else {
        # tail
        $bytes = [System.IO.File]::ReadAllBytes($logFile)
        $tail = $bytes[($bytes.Length - $MaxLogBytes)..($bytes.Length - 1)]
        [System.Text.Encoding]::UTF8.GetString($tail)
    }
    @{
        log_path = $logFile
        size = $size
        truncated = ($size -gt $MaxLogBytes)
        content = $content
        exit_code = $proc.ExitCode
    }
}

try {
    $result = if ($Username) {
        $pass = ConvertTo-SecureString $Password -AsPlainText -Force
        $cred = New-Object System.Management.Automation.PSCredential($Username, $pass)
        Invoke-Command -ComputerName $HostName -Credential $cred -Authentication Default `
            -ScriptBlock $script -ArgumentList $EditorExe, $ProjectPath, $TimeoutSeconds, $MaxLogBytes
    } else {
        Invoke-Command -ComputerName $HostName -ScriptBlock $script `
            -ArgumentList $EditorExe, $ProjectPath, $TimeoutSeconds, $MaxLogBytes
    }
    @{
        ok = $true
        log_path = $result.log_path
        size = $result.size
        truncated = $result.truncated
        content = $result.content
        exit_code = $result.exit_code
    } | ConvertTo-Json -Compress -Depth 4
} catch {
    @{ ok = $false; message = $_.Exception.Message } | ConvertTo-Json -Compress
}
```

- [ ] **Step 2: Commit**

```bash
git add ps-scripts/parse-ue-log.ps1
git commit -m "feat(ddc): add parse-ue-log.ps1 sidecar for log-based verification"
```

---

#### Task M2.3: Rust wrapper `ue_log_verify::run_for_host`

**Files:**
- Create: `src-tauri/src/core/ue_log_verify.rs`

- [ ] **Step 1: Write the failing test**

```rust
// src-tauri/src/core/ue_log_verify.rs
//! Pull a verbose DDC startup log from one host and convert it to a summary
//! report. Calls parse-ue-log.ps1 sidecar; parses content via ue_log_parser.

use crate::core::{powershell, ue_log_parser::{self, DdcEvent}};
use crate::error::{UecmError, UecmResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct ScriptResult {
    ok: bool,
    log_path: Option<String>,
    content: Option<String>,
    truncated: Option<bool>,
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VerifyReport {
    pub host: String,
    pub local_path: Option<String>,
    pub local_writable: Option<bool>,
    pub shared_path: Option<String>,
    pub shared_writable: Option<bool>,
    pub shared_deactivated_reason: Option<String>,
    pub move_collision_count: u64,
    pub maintenance: Vec<MaintenanceFact>,
    pub paks_opened: Vec<String>,
    pub truncated: bool,
    pub log_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MaintenanceFact {
    pub layer: String,
    pub file_count: u64,
    pub total_bytes: u64,
}

pub fn summarize(host: &str, log_text: &str, log_path: Option<String>, truncated: bool) -> VerifyReport {
    let mut r = VerifyReport {
        host: host.to_string(),
        local_path: None,
        local_writable: None,
        shared_path: None,
        shared_writable: None,
        shared_deactivated_reason: None,
        move_collision_count: 0,
        maintenance: vec![],
        paks_opened: vec![],
        truncated,
        log_path,
    };
    for line in log_text.lines() {
        match ue_log_parser::parse_line(line) {
            DdcEvent::LocalPath { path, writable } => {
                r.local_path = Some(path);
                r.local_writable = Some(writable);
            }
            DdcEvent::SharedPath { path, writable } => {
                r.shared_path = Some(path);
                r.shared_writable = Some(writable);
            }
            DdcEvent::SharedDeactivated { reason } => {
                r.shared_deactivated_reason = Some(reason);
            }
            DdcEvent::MoveCollision { .. } => {
                r.move_collision_count += 1;
            }
            DdcEvent::MaintenanceFinished { layer, file_count, total_bytes } => {
                r.maintenance.push(MaintenanceFact { layer, file_count, total_bytes });
            }
            DdcEvent::PakOpened { path } => r.paks_opened.push(path),
            DdcEvent::Other => {}
        }
    }
    r
}

pub fn run_for_host(
    host: &str,
    editor_exe: &str,
    project_path: &str,
    timeout_seconds: u32,
    creds: Option<(&str, &str)>,
) -> UecmResult<VerifyReport> {
    let mut args: Vec<String> = vec![
        "-HostName".into(), host.into(),
        "-EditorExe".into(), editor_exe.into(),
        "-ProjectPath".into(), project_path.into(),
        "-TimeoutSeconds".into(), timeout_seconds.to_string(),
    ];
    if let Some((u, p)) = creds {
        args.extend(["-Username".into(), u.into(), "-Password".into(), p.into()]);
    }
    let args_ref: Vec<&str> = args.iter().map(String::as_str).collect();
    let result: ScriptResult = powershell::run_json(
        &powershell::script_path("parse-ue-log.ps1"),
        &args_ref,
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(
            result.message.unwrap_or_else(|| "log verify failed".into()),
        ));
    }
    let content = result.content.unwrap_or_default();
    Ok(summarize(host, &content, result.log_path, result.truncated.unwrap_or(false)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_extracts_local_and_shared_paths() {
        let log = "\
LogTemp: irrelevant
LogDerivedDataCache: Using Local data cache path D:\\DDC: Writable
LogDerivedDataCache: Using Shared data cache path \\\\NAS\\DDC: Writable
LogDerivedDataCache: Warning: Move collision when writing \\\\NAS\\DDC\\AB\\foo.udd
LogDerivedDataCache: Warning: Move collision when writing \\\\NAS\\DDC\\AB\\bar.udd
LogDerivedDataCache: Maintenance finished on Local: 25065 files, 1546 MiB
LogDerivedDataCache: Maintenance finished on Shared: 152 files, 30 MiB
";
        let r = summarize("RENDER-01", log, None, false);
        assert_eq!(r.local_path.as_deref(), Some(r"D:\DDC"));
        assert_eq!(r.local_writable, Some(true));
        assert_eq!(r.shared_path.as_deref(), Some(r"\\NAS\DDC"));
        assert_eq!(r.move_collision_count, 2);
        assert_eq!(r.maintenance.len(), 2);
        assert_eq!(r.maintenance[0].layer, "Local");
        assert_eq!(r.maintenance[0].file_count, 25065);
    }

    #[test]
    fn summarize_captures_deactivation_reason() {
        let log = "LogDerivedDataCache: Warning: Shared backend deactivated due to latency (87ms over 70ms threshold)\n";
        let r = summarize("X", log, None, false);
        assert!(r.shared_deactivated_reason.is_some());
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cd src-tauri && cargo test --lib core::ue_log_verify 2>&1 | tail -10
```

Expected: 2 passed.

- [ ] **Step 3: Register module**

`src-tauri/src/core/mod.rs`:

```rust
pub mod ue_log_verify;
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/core/ue_log_verify.rs src-tauri/src/core/mod.rs
git commit -m "feat(ddc): ue_log_verify wrapper + per-host summary report"
```

---

#### Task M2.4: CLI `log verify-startup` subcommand

**Files:**
- Modify: `src-tauri/src/cli/args.rs`
- Create: `src-tauri/src/cli/domain_log.rs`
- Modify: `src-tauri/src/cli/run.rs` (dispatch)
- Modify: `src-tauri/src/cli/mod.rs`

- [ ] **Step 1: Write the failing test**

Append to `src-tauri/src/cli/args.rs` `#[cfg(test)] mod tests`:

```rust
#[test]
fn parses_log_verify_startup() {
    let cli = Cli::try_parse_from([
        "uecm-cli", "log", "verify-startup",
        "--host", "RENDER-01",
        "--editor-exe", r"C:\UE\Engine\Binaries\Win64\UnrealEditor.exe",
        "--project", r"D:\Projects\MyVP\MyVP.uproject",
        "--timeout", "180",
    ]).unwrap();
    match cli.command {
        Domain::Log { action: LogAction::VerifyStartup { host, editor_exe, project, timeout, .. } } => {
            assert_eq!(host, "RENDER-01");
            assert!(editor_exe.ends_with("UnrealEditor.exe"));
            assert!(project.ends_with(".uproject"));
            assert_eq!(timeout, 180);
        }
        _ => panic!("wrong variant"),
    }
}
```

- [ ] **Step 2: Run test, expect compile failure**

```bash
cd src-tauri && cargo test --lib cli::args::tests::parses_log_verify 2>&1 | tail -10
```

- [ ] **Step 3: Add the enum + dispatch**

In `src-tauri/src/cli/args.rs` add to `Domain`:

```rust
/// Verify what UE actually used by parsing LogDerivedDataCache startup output.
Log {
    #[command(subcommand)]
    action: LogAction,
},
```

And the action enum:

```rust
#[derive(Subcommand, Debug)]
pub enum LogAction {
    /// Run UE in nullrhi mode and parse its DDC startup output.
    VerifyStartup {
        #[arg(long)]
        host: String,
        #[arg(long)]
        editor_exe: String,
        #[arg(long)]
        project: String,
        #[arg(long, default_value_t = 180)]
        timeout: u32,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
}
```

Create `src-tauri/src/cli/domain_log.rs`:

```rust
//! `uecm-cli log <action>` handlers.
use crate::cli::args::LogAction;
use crate::cli::run::Ctx;
use crate::core::ue_log_verify;
use crate::error::UecmResult;

pub fn handle(ctx: &mut Ctx<'_>, action: LogAction) -> UecmResult<()> {
    match action {
        LogAction::VerifyStartup { host, editor_exe, project, timeout, cred } => {
            let db = ctx.require_db()?;
            let creds = cred.resolve(db)?;
            let report = ue_log_verify::run_for_host(
                &host, &editor_exe, &project, timeout,
                creds.as_ref().map(|(u, p)| (u.as_str(), p.as_str())),
            )?;
            ctx.emitter.emit_result(&report).ok();
            Ok(())
        }
    }
}
```

In `src-tauri/src/cli/mod.rs` add `pub mod domain_log;` and in `src-tauri/src/cli/run.rs` add a match arm for `Domain::Log { action } => domain_log::handle(ctx, action)`.

- [ ] **Step 4: Run tests**

```bash
cd src-tauri && cargo test --lib cli::args 2>&1 | tail -10
```

Expected: parses_log_verify_startup passes.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/cli/args.rs src-tauri/src/cli/domain_log.rs src-tauri/src/cli/mod.rs src-tauri/src/cli/run.rs
git commit -m "feat(ddc): cli log verify-startup invokes ue_log_verify"
```

---

#### Task M2.5: Tauri command + minimal UI report panel

**Files:**
- Create: `src-tauri/src/commands/log_verify.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs` (register command)
- Create: `src/components/deploy/DeployVerifyReport.vue` (UI surface)
- Modify: `src/lib/deployApi.ts` (add invoke wrapper) — will be created in M5; if this task lands first, create a stub.

- [ ] **Step 1: Backend command**

Create `src-tauri/src/commands/log_verify.rs`:

```rust
use crate::core::ue_log_verify::{self, VerifyReport};
use crate::error::UecmError;

#[tauri::command]
pub async fn run_log_verify(
    host: String,
    editor_exe: String,
    project: String,
    timeout: u32,
    cred_alias: Option<String>,
) -> Result<VerifyReport, String> {
    let creds = match cred_alias.as_deref() {
        Some(alias) if !alias.is_empty() => {
            let db = crate::data::db::open().map_err(|e: UecmError| e.to_string())?;
            Some(
                crate::data::credentials::load_decrypted(&db, alias)
                    .map_err(|e: UecmError| e.to_string())?,
            )
        }
        _ => None,
    };
    tokio::task::spawn_blocking(move || {
        ue_log_verify::run_for_host(
            &host, &editor_exe, &project, timeout,
            creds.as_ref().map(|(u, p)| (u.as_str(), p.as_str())),
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}
```

In `src-tauri/src/commands/mod.rs` add `pub mod log_verify;` and re-export `pub use log_verify::run_log_verify;`. In `src-tauri/src/lib.rs` (the Tauri builder), add `.invoke_handler(tauri::generate_handler![ ... commands::run_log_verify, ... ])` to the list.

- [ ] **Step 2: UI surface (placeholder)**

Create `src/components/deploy/DeployVerifyReport.vue`:

```vue
<script setup lang="ts">
import { useI18n } from "vue-i18n";

defineProps<{
  report: {
    host: string;
    local_path?: string | null;
    local_writable?: boolean | null;
    shared_path?: string | null;
    shared_writable?: boolean | null;
    shared_deactivated_reason?: string | null;
    move_collision_count: number;
    maintenance: Array<{ layer: string; file_count: number; total_bytes: number }>;
    paks_opened: string[];
    truncated: boolean;
  };
}>();

const { t } = useI18n();

function formatBytes(n: number): string {
  if (n >= 1024 ** 3) return `${(n / 1024 ** 3).toFixed(1)} GiB`;
  if (n >= 1024 ** 2) return `${(n / 1024 ** 2).toFixed(1)} MiB`;
  if (n >= 1024) return `${(n / 1024).toFixed(1)} KiB`;
  return `${n} B`;
}
</script>

<template>
  <div class="rounded-md border border-border bg-card p-4 text-card-foreground">
    <h3 class="font-display text-lg mb-2">{{ t("logVerify.title", { host: report.host }) }}</h3>
    <dl class="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1 text-sm">
      <dt class="text-muted-foreground">{{ t("logVerify.localPath") }}</dt>
      <dd>{{ report.local_path ?? "—" }}<span v-if="report.local_writable === false" class="ml-2 text-status-warning">{{ t("logVerify.readOnly") }}</span></dd>

      <dt class="text-muted-foreground">{{ t("logVerify.sharedPath") }}</dt>
      <dd>{{ report.shared_path ?? "—" }}<span v-if="report.shared_writable === false" class="ml-2 text-status-warning">{{ t("logVerify.readOnly") }}</span></dd>

      <dt v-if="report.shared_deactivated_reason" class="text-status-critical">{{ t("logVerify.deactivated") }}</dt>
      <dd v-if="report.shared_deactivated_reason" class="text-status-critical">{{ report.shared_deactivated_reason }}</dd>

      <dt class="text-muted-foreground">{{ t("logVerify.moveCollisions") }}</dt>
      <dd :class="report.move_collision_count > 0 ? 'text-status-warning' : ''">{{ report.move_collision_count }}</dd>
    </dl>

    <div v-if="report.maintenance.length" class="mt-3">
      <h4 class="text-sm font-semibold mb-1">{{ t("logVerify.maintenance") }}</h4>
      <table class="w-full text-sm">
        <thead><tr class="text-muted-foreground"><th class="text-left">{{ t("logVerify.layer") }}</th><th class="text-right">{{ t("logVerify.files") }}</th><th class="text-right">{{ t("logVerify.size") }}</th></tr></thead>
        <tbody>
          <tr v-for="m in report.maintenance" :key="m.layer">
            <td>{{ m.layer }}</td><td class="text-right">{{ m.file_count.toLocaleString() }}</td><td class="text-right">{{ formatBytes(m.total_bytes) }}</td>
          </tr>
        </tbody>
      </table>
    </div>

    <p v-if="report.truncated" class="mt-2 text-xs text-status-warning">{{ t("logVerify.truncated") }}</p>
  </div>
</template>
```

Add the locale keys to `src/locales/zh.ts`:

```ts
logVerify: {
  title: "{host} 启动日志报告",
  localPath: "Local DDC 实际路径",
  sharedPath: "Shared DDC 实际路径",
  readOnly: "(只读)",
  deactivated: "Shared 已被禁用",
  moveCollisions: "Move collision 数",
  maintenance: "维护统计",
  layer: "层级",
  files: "文件数",
  size: "大小",
  truncated: "日志已截断，仅显示末段。",
},
```

Mirror in `src/locales/en.ts`.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/log_verify.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs src/components/deploy/DeployVerifyReport.vue src/locales/zh.ts src/locales/en.ts
git commit -m "feat(ddc): expose run_log_verify command + UI report component"
```

---

### Module M3: Local DDC complete configuration (2 days)

**Goal:** Make `UE-LocalDataCachePath` a first-class operation. Add a probe that checks both Local and Shared env vars, a `local-cache create` command that mkdir + ACLs the local directory, and update the UI batch-env modal to accept a variable-name dropdown.

**Files:**
- Modify: `ps-scripts/health-probes.ps1`
- Create: `ps-scripts/create-local-cache-dir.ps1`
- Create: `src-tauri/src/core/local_cache.rs`
- Create: `src-tauri/src/cli/domain_local_cache.rs`
- Modify: `src-tauri/src/cli/args.rs`
- Modify: `src/components/modals/BatchEnvVarModal.vue`
- Modify: `src/components/modals/EnvVarConfigModal.vue`

**Dependencies:** None.

---

#### Task M3.1: Health probe — read Local env alongside Shared

**Files:**
- Modify: `ps-scripts/health-probes.ps1`
- Modify: `src-tauri/src/core/health_probes.rs`
- Modify: `src-tauri/src/core/health_check.rs` (response struct)

- [ ] **Step 1: Update PS probe**

In `ps-scripts/health-probes.ps1`, replace the `Probe-EnvVars` function to also read Local:

```powershell
function Probe-EnvVars {
    param($ExpectedSharedDataCachePath, $ExpectedLocalDataCachePath)
    $shared = [Environment]::GetEnvironmentVariable('UE-SharedDataCachePath', 'Machine')
    $local  = [Environment]::GetEnvironmentVariable('UE-LocalDataCachePath',  'Machine')

    $sharedStatus = if ([string]::IsNullOrEmpty($ExpectedSharedDataCachePath)) {
        if ($shared) { 'healthy' } else { 'warning' }
    } else {
        if ($shared -eq $ExpectedSharedDataCachePath) { 'healthy' } else { 'critical' }
    }
    $localStatus = if ([string]::IsNullOrEmpty($ExpectedLocalDataCachePath)) {
        if ($local) { 'healthy' } else { 'warning' }
    } else {
        if ($local -eq $ExpectedLocalDataCachePath) { 'healthy' } else { 'critical' }
    }

    @{
        shared = @{ status = $sharedStatus; value = "$shared"; expected = "$ExpectedSharedDataCachePath" }
        local  = @{ status = $localStatus;  value = "$local";  expected = "$ExpectedLocalDataCachePath" }
    }
}
```

In the `$script` block that calls all probes, pass through the new parameter:

```powershell
env_vars = (Probe-EnvVars -ExpectedSharedDataCachePath $ExpectedSharedDataCachePath -ExpectedLocalDataCachePath $ExpectedLocalDataCachePath)
```

Add the parameter at the top:

```powershell
param(
    ...,
    [string]$ExpectedSharedDataCachePath = "",
    [string]$ExpectedLocalDataCachePath = "",
    ...
)
```

And forward it inside `Invoke-Command -ArgumentList`.

- [ ] **Step 2: Update Rust caller**

In `src-tauri/src/core/health_probes.rs`, change the args list to include `-ExpectedLocalDataCachePath`:

```rust
args.extend([
    "-ExpectedSharedDataCachePath", expected_shared_path,
    "-ExpectedLocalDataCachePath",  expected_local_path,
]);
```

Update the response struct in `src-tauri/src/core/health_check.rs`:

```rust
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EnvVarsResult {
    pub shared: EnvProbeStatus,
    pub local: EnvProbeStatus,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EnvProbeStatus {
    pub status: String,
    pub value: String,
    pub expected: String,
}
```

Replace existing single-field deserialization. Update all call sites that read `env_vars` (UI through Tauri commands) — search `grep -rn "env_vars" src-tauri/src` and adjust.

- [ ] **Step 3: Add test**

```rust
#[test]
fn env_probe_status_deserializes_local_and_shared() {
    let json = r#"{"shared":{"status":"healthy","value":"\\\\NAS\\DDC","expected":""},"local":{"status":"warning","value":"","expected":""}}"#;
    let p: EnvVarsResult = serde_json::from_str(json).unwrap();
    assert_eq!(p.local.status, "warning");
    assert_eq!(p.shared.status, "healthy");
}
```

```bash
cd src-tauri && cargo test --lib core::health_check 2>&1 | tail -10
```

- [ ] **Step 4: Commit**

```bash
git add ps-scripts/health-probes.ps1 src-tauri/src/core/health_probes.rs src-tauri/src/core/health_check.rs
git commit -m "feat(ddc): health probe reads UE-LocalDataCachePath alongside Shared"
```

---

#### Task M3.2: `local-cache create` command + PS script

**Files:**
- Create: `ps-scripts/create-local-cache-dir.ps1`
- Create: `src-tauri/src/core/local_cache.rs`

- [ ] **Step 1: PS script**

Create `ps-scripts/create-local-cache-dir.ps1`:

```powershell
# Creates the local DDC directory on a remote host with permissive ACLs so
# both the operator account and SYSTEM (RenderStream Service) can read/write.
param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$LocalPath,
    [string]$ServiceAccount,
    [string]$Username,
    [string]$Password
)
$ErrorActionPreference = 'Stop'

$script = {
    param($LocalPath, $ServiceAccount)
    if (-not (Test-Path -LiteralPath $LocalPath)) {
        New-Item -ItemType Directory -Path $LocalPath -Force | Out-Null
    }
    # SYSTEM full control (RenderStream / Windows service contexts)
    icacls $LocalPath /grant 'SYSTEM:(OI)(CI)F' /T /C | Out-Null
    icacls $LocalPath /grant 'Administrators:(OI)(CI)F' /T /C | Out-Null
    if ($ServiceAccount) {
        icacls $LocalPath /grant "${ServiceAccount}:(OI)(CI)F" /T /C | Out-Null
    }
    $info = Get-Item -LiteralPath $LocalPath
    @{ path = $info.FullName; created_at = $info.CreationTime.ToString('o') }
}

try {
    $result = if ($Username) {
        $pass = ConvertTo-SecureString $Password -AsPlainText -Force
        $cred = New-Object System.Management.Automation.PSCredential($Username, $pass)
        Invoke-Command -ComputerName $HostName -Credential $cred -Authentication Default `
            -ScriptBlock $script -ArgumentList $LocalPath, $ServiceAccount
    } else {
        Invoke-Command -ComputerName $HostName -ScriptBlock $script `
            -ArgumentList $LocalPath, $ServiceAccount
    }
    @{ ok = $true; message = "created $($result.path)"; path = $result.path } | ConvertTo-Json -Compress
} catch {
    @{ ok = $false; message = $_.Exception.Message } | ConvertTo-Json -Compress
}
```

- [ ] **Step 2: Rust wrapper**

Create `src-tauri/src/core/local_cache.rs`:

```rust
//! Provision a local DDC directory on a remote host: New-Item + icacls.

use crate::core::powershell;
use crate::error::{UecmError, UecmResult};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct CreateResult {
    ok: bool,
    message: String,
    path: Option<String>,
}

pub fn create(
    host: &str,
    local_path: &str,
    service_account: Option<&str>,
    operator: Option<(&str, &str)>,
) -> UecmResult<String> {
    let mut args: Vec<&str> = vec!["-HostName", host, "-LocalPath", local_path];
    if let Some(sa) = service_account {
        args.extend(["-ServiceAccount", sa]);
    }
    if let Some((u, p)) = operator {
        args.extend(["-Username", u, "-Password", p]);
    }
    let r: CreateResult = powershell::run_json(
        &powershell::script_path("create-local-cache-dir.ps1"),
        &args,
    )?;
    if !r.ok {
        return Err(UecmError::OperationFailed(r.message));
    }
    Ok(r.path.unwrap_or(r.message))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(windows))]
    #[test]
    fn returns_powershell_error_off_windows() {
        let r = create("HOST", r"D:\UE-DDC-Local", None, None);
        assert!(matches!(r, Err(UecmError::PowerShell(_))));
    }
}
```

Register in `src-tauri/src/core/mod.rs`.

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test --lib core::local_cache 2>&1 | tail -10
```

- [ ] **Step 4: Commit**

```bash
git add ps-scripts/create-local-cache-dir.ps1 src-tauri/src/core/local_cache.rs src-tauri/src/core/mod.rs
git commit -m "feat(ddc): provision local DDC directory on remote host with ACLs"
```

---

#### Task M3.3: CLI `local-cache create / status`

**Files:**
- Modify: `src-tauri/src/cli/args.rs`
- Create: `src-tauri/src/cli/domain_local_cache.rs`
- Modify: `src-tauri/src/cli/mod.rs` and `src-tauri/src/cli/run.rs`

- [ ] **Step 1: Add CLI variants**

In `src-tauri/src/cli/args.rs`, add to `Domain`:

```rust
/// Local DDC directory provisioning.
LocalCache {
    #[command(subcommand)]
    action: LocalCacheAction,
},
```

```rust
#[derive(Subcommand, Debug)]
pub enum LocalCacheAction {
    /// Create the local DDC directory on one or more hosts.
    Create {
        #[command(flatten)]
        target: crate::cli::host_args::HostArgs,
        #[arg(long, default_value = r"D:\UE-DDC-Local")]
        path: String,
        #[arg(long)]
        service_account: Option<String>,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
}
```

Create `src-tauri/src/cli/domain_local_cache.rs`:

```rust
//! `uecm-cli local-cache <action>` handlers.
use crate::cli::args::LocalCacheAction;
use crate::cli::destructive::{self, Outcome};
use crate::cli::host_args::HostTarget;
use crate::cli::output::Event;
use crate::cli::run::Ctx;
use crate::core::local_cache;
use crate::error::UecmResult;

pub fn handle(ctx: &mut Ctx<'_>, action: LocalCacheAction) -> UecmResult<()> {
    match action {
        LocalCacheAction::Create { target, path, service_account, yes, dry_run, cred } => {
            let t = target.require_one()?;
            let hosts: Vec<String> = match t { HostTarget::Single(h) => vec![h], HostTarget::Batch(hs) => hs };
            let outcome = destructive::check(yes, dry_run, "local-cache.create")?;
            let db = ctx.require_db()?;
            cred.preflight(db)?;
            if outcome == Outcome::DryRun {
                destructive::emit_plan(ctx.emitter.as_mut(), "local-cache.create",
                    serde_json::json!({"hosts": hosts, "path": path, "service_account": service_account}));
                return Ok(());
            }
            let creds = cred.resolve(db)?;
            let total = hosts.len() as i64;
            ctx.emitter.emit_event(&Event::Started {
                task_type: "local_cache_create".into(), task_id: None,
                metadata: serde_json::json!({"hosts": total, "path": path}),
            }).ok();
            for (idx, host) in hosts.iter().enumerate() {
                ctx.emitter.emit_event(&Event::ItemStarted {
                    item_id: host.clone(), index: idx as i64, total,
                }).ok();
                let r = local_cache::create(host, &path, service_account.as_deref(),
                    creds.as_ref().map(|(u, p)| (u.as_str(), p.as_str())));
                match r {
                    Ok(_) => { ctx.emitter.emit_event(&Event::ItemCompleted {
                        item_id: host.clone(), index: idx as i64, ok: true, message: None,
                    }).ok(); }
                    Err(e) => { ctx.emitter.emit_event(&Event::ItemCompleted {
                        item_id: host.clone(), index: idx as i64, ok: false, message: Some(e.to_string()),
                    }).ok(); }
                }
            }
            Ok(())
        }
    }
}
```

Wire into `src-tauri/src/cli/mod.rs` (`pub mod domain_local_cache;`) and `src-tauri/src/cli/run.rs` dispatch.

- [ ] **Step 2: Add parser test**

```rust
#[test]
fn parses_local_cache_create_batch() {
    let cli = Cli::try_parse_from([
        "uecm-cli", "local-cache", "create",
        "--hosts", "RENDER-01,RENDER-02",
        "--path", r"D:\UE-DDC-Local",
        "--cred-alias", "admin", "--pass-stdin", "--yes",
    ]).unwrap();
    // assertion: parses without panic
}
```

- [ ] **Step 3: Run tests + commit**

```bash
cd src-tauri && cargo test --lib cli::args::tests::parses_local_cache 2>&1 | tail -10
git add src-tauri/src/cli/args.rs src-tauri/src/cli/domain_local_cache.rs src-tauri/src/cli/mod.rs src-tauri/src/cli/run.rs
git commit -m "feat(ddc): cli local-cache create (batch hosts + mkdir + ACL)"
```

---

#### Task M3.4: UI `BatchEnvVarModal` dropdown

**Files:**
- Modify: `src/components/modals/BatchEnvVarModal.vue`
- Modify: `src/locales/zh.ts`, `src/locales/en.ts`

- [ ] **Step 1: Replace the var-name input with a dropdown**

Replace the input block:

```vue
<label class="block text-sm mb-1">{{ t("modal.batchEnv.varName") }}</label>
<select
  data-env-name
  v-model="name"
  class="mb-3 w-full rounded border border-input bg-transparent px-2 py-1 text-sm text-foreground"
>
  <option value="UE-SharedDataCachePath">{{ t("modal.batchEnv.shared") }}</option>
  <option value="UE-LocalDataCachePath">{{ t("modal.batchEnv.local") }}</option>
  <option value="__custom">{{ t("modal.batchEnv.custom") }}</option>
</select>
<input
  v-if="name === '__custom'"
  v-model="customName"
  class="mb-3 w-full rounded border border-input bg-transparent px-2 py-1 text-sm"
  :placeholder="t('modal.batchEnv.customPlaceholder')"
/>
```

Add `const customName = ref("")` and replace `name.value.trim()` in `onApply` with:

```ts
const effectiveName = name.value === "__custom" ? customName.value.trim() : name.value;
```

Pass `effectiveName` to `batch.runEnvVar`.

- [ ] **Step 2: Update test**

`src/__tests__/BatchEnvVarModal.spec.ts` — change the `setValue` line to use the select:

```ts
await wrapper.find("[data-env-name]").setValue("UE-LocalDataCachePath");
```

Add a new test for the custom path:

```ts
test("custom var name flows through", async () => {
  const wrapper = mount(BatchEnvVarModal, { props: { open: true, machineIds: [1] } });
  await wrapper.find("[data-env-name]").setValue("__custom");
  await wrapper.find("input[type=text]").setValue("UE-MyCustomVar");
  // ... fill credential + value, click apply, assert mockApi called with UE-MyCustomVar
});
```

- [ ] **Step 3: Locale keys**

Add to `src/locales/zh.ts` under `modal.batchEnv`:

```ts
shared: "UE-SharedDataCachePath (共享 DDC)",
local: "UE-LocalDataCachePath (本地 DDC)",
custom: "自定义变量名",
customPlaceholder: "例如 UE-MyVar",
```

Mirror in en.ts.

- [ ] **Step 4: Run UI tests + commit**

```bash
pnpm test BatchEnvVarModal 2>&1 | tail -10
git add src/components/modals/BatchEnvVarModal.vue src/__tests__/BatchEnvVarModal.spec.ts src/locales/zh.ts src/locales/en.ts
git commit -m "feat(ddc): batch env var modal supports Local/Shared/custom dropdown"
```

---

### Module M4: RenderStream Service account probe (1.5 days)

**Goal:** When DDC config seems right but `LogDerivedDataCache` still shows the wrong path, the most common cause is the RenderStream service running as an account that can't read Editor Preferences or user-level env vars. This module adds a probe that reads `Win32_Service` for any RenderStream service and reports the `StartName`.

**Files:**
- Create: `ps-scripts/probe-renderstream-service.ps1`
- Create: `src-tauri/src/core/renderstream_service.rs`
- Modify: `ps-scripts/health-probes.ps1`
- Modify: `src-tauri/src/core/health_check.rs`

**Dependencies:** None.

---

#### Task M4.1: Standalone probe script

**Files:**
- Create: `ps-scripts/probe-renderstream-service.ps1`

- [ ] **Step 1: Script body**

```powershell
# Discovers any RenderStream-related Windows services on a host and reports
# their StartName (the account they run as), State, and StartMode.
param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [string]$Username,
    [string]$Password
)
$ErrorActionPreference = 'Stop'

$script = {
    $patterns = @(
        'd3service*',
        '*RenderStream*',
        '*disguise*',
        '*Cluster*Render*'
    )
    $found = New-Object System.Collections.Generic.List[object]
    foreach ($p in $patterns) {
        $svcs = Get-CimInstance Win32_Service -Filter "Name LIKE '$($p.Replace('*','%'))'" -ErrorAction SilentlyContinue
        foreach ($svc in $svcs) {
            if ($found.Where({ $_.Name -eq $svc.Name }).Count -gt 0) { continue }
            $found.Add([pscustomobject]@{
                Name = $svc.Name
                DisplayName = $svc.DisplayName
                StartName = $svc.StartName
                State = $svc.State
                StartMode = $svc.StartMode
                PathName = $svc.PathName
            }) | Out-Null
        }
    }
    @{ services = $found }
}

try {
    $result = if ($Username) {
        $pass = ConvertTo-SecureString $Password -AsPlainText -Force
        $cred = New-Object System.Management.Automation.PSCredential($Username, $pass)
        Invoke-Command -ComputerName $HostName -Credential $cred -Authentication Default -ScriptBlock $script
    } else {
        Invoke-Command -ComputerName $HostName -ScriptBlock $script
    }
    @{ ok = $true; services = @($result.services) } | ConvertTo-Json -Compress -Depth 4
} catch {
    @{ ok = $false; message = $_.Exception.Message; services = @() } | ConvertTo-Json -Compress
}
```

- [ ] **Step 2: Commit**

```bash
git add ps-scripts/probe-renderstream-service.ps1
git commit -m "feat(ddc): probe-renderstream-service.ps1 enumerates RS services + accounts"
```

---

#### Task M4.2: Rust wrapper + diagnostics

**Files:**
- Create: `src-tauri/src/core/renderstream_service.rs`

- [ ] **Step 1: Write the failing test**

```rust
// src-tauri/src/core/renderstream_service.rs
use crate::core::powershell;
use crate::error::{UecmError, UecmResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceFact {
    #[serde(rename = "Name")] pub name: String,
    #[serde(rename = "DisplayName")] pub display_name: String,
    #[serde(rename = "StartName")] pub start_name: String,
    #[serde(rename = "State")] pub state: String,
    #[serde(rename = "StartMode")] pub start_mode: String,
    #[serde(rename = "PathName")] pub path_name: String,
}

#[derive(Debug, Deserialize)]
struct ScriptResult {
    ok: bool,
    services: Option<Vec<ServiceFact>>,
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RsServiceReport {
    pub host: String,
    pub services: Vec<ServiceFact>,
    pub risks: Vec<String>,
}

pub fn classify_risks(services: &[ServiceFact]) -> Vec<String> {
    let mut out = Vec::new();
    for s in services {
        let acct = s.start_name.to_lowercase();
        let is_local_system = acct == "localsystem" || acct == ".\\localsystem";
        let is_network_service = acct.contains("networkservice");
        let looks_local_user = acct.starts_with(".\\") || (!acct.contains('@') && !acct.contains('\\') && !is_local_system && !is_network_service);

        if is_local_system {
            out.push(format!(
                "{} runs as LocalSystem — user-level env vars and Editor Preferences are invisible. DDC paths must be in Machine-scope env or Project Config.",
                s.name
            ));
        } else if looks_local_user {
            out.push(format!(
                "{} runs as a local interactive user ({}) — Editor Preferences set under a different account will not apply.",
                s.name, s.start_name
            ));
        }
        if s.state != "Running" {
            out.push(format!("{} is not running ({})", s.name, s.state));
        }
    }
    out
}

pub fn report(host: &str, creds: Option<(&str, &str)>) -> UecmResult<RsServiceReport> {
    let mut args: Vec<&str> = vec!["-HostName", host];
    if let Some((u, p)) = creds {
        args.extend(["-Username", u, "-Password", p]);
    }
    let r: ScriptResult = powershell::run_json(
        &powershell::script_path("probe-renderstream-service.ps1"),
        &args,
    )?;
    if !r.ok {
        return Err(UecmError::OperationFailed(
            r.message.unwrap_or_else(|| "probe failed".into()),
        ));
    }
    let services = r.services.unwrap_or_default();
    let risks = classify_risks(&services);
    Ok(RsServiceReport { host: host.to_string(), services, risks })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svc(name: &str, start_name: &str, state: &str) -> ServiceFact {
        ServiceFact {
            name: name.into(),
            display_name: name.into(),
            start_name: start_name.into(),
            state: state.into(),
            start_mode: "Auto".into(),
            path_name: r"C:\d3\bin\foo.exe".into(),
        }
    }

    #[test]
    fn local_system_account_flagged() {
        let r = classify_risks(&[svc("d3service", "LocalSystem", "Running")]);
        assert!(r.iter().any(|x| x.contains("LocalSystem")));
    }
    #[test]
    fn local_interactive_user_flagged() {
        let r = classify_risks(&[svc("d3service", ".\\disguise", "Running")]);
        assert!(r.iter().any(|x| x.contains("local interactive")));
    }
    #[test]
    fn non_running_flagged() {
        let r = classify_risks(&[svc("d3service", "LocalSystem", "Stopped")]);
        assert!(r.iter().any(|x| x.contains("not running")));
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cd src-tauri && cargo test --lib core::renderstream_service 2>&1 | tail -10
```

Expected: 3 passed.

- [ ] **Step 3: Register module + commit**

`src-tauri/src/core/mod.rs`:

```rust
pub mod renderstream_service;
```

```bash
git add src-tauri/src/core/renderstream_service.rs src-tauri/src/core/mod.rs
git commit -m "feat(ddc): renderstream_service report + risk classifier"
```

---

#### Task M4.3: Wire into health round-trip + UI surface

**Files:**
- Modify: `ps-scripts/health-probes.ps1`
- Modify: `src-tauri/src/core/health_probes.rs`
- Modify: `src-tauri/src/core/health_check.rs`
- Modify: `src/lib/healthChecks.ts`

- [ ] **Step 1: Add Probe-RenderStreamService to round-trip script**

Append inside the `$script` block of `ps-scripts/health-probes.ps1`:

```powershell
function Probe-RenderStreamService {
    $patterns = @('d3service%','%RenderStream%','%disguise%')
    $found = @()
    foreach ($p in $patterns) {
        $svcs = Get-CimInstance Win32_Service -Filter "Name LIKE '$p'" -ErrorAction SilentlyContinue
        foreach ($svc in $svcs) {
            if ($found.Name -contains $svc.Name) { continue }
            $found += [pscustomobject]@{
                Name = $svc.Name; StartName = $svc.StartName; State = $svc.State
            }
        }
    }
    if ($found.Count -eq 0) {
        return @{ status='na'; message='no RenderStream service detected'; services=@() }
    }
    $accts = $found.StartName -join '; '
    @{ status='healthy'; message="${($found.Count)} service(s) found: $accts"; services=$found }
}
```

Add `rs_service = (Probe-RenderStreamService)` to the result hashtable.

- [ ] **Step 2: Update `EnvProbeStatus` / health response struct in Rust**

Add field `rs_service: Option<RsServiceProbe>` and matching `RsServiceProbe { status, message, services }`.

- [ ] **Step 3: UI**

Add an entry to `src/lib/healthChecks.ts`:

```ts
{ id: "rs_service", shortLabel: "RS", label: "RenderStream service account",
  description: "Detect RenderStream services and which account they run as.",
  symptom: "Service runs as LocalSystem; user-level config invisible.",
  remediation: "Either move config to Machine-scope env / Project Config, or switch service account." },
```

- [ ] **Step 4: Commit**

```bash
git add ps-scripts/health-probes.ps1 src-tauri/src/core/health_probes.rs src-tauri/src/core/health_check.rs src/lib/healthChecks.ts
git commit -m "feat(ddc): include RenderStream service probe in health round-trip"
```

---

### Module M5: `deploy ddc` orchestrator + UI wizard (6.5 days)

**Goal:** A single command (`uecm-cli deploy ddc --project=X --hosts=Y --plan=<spec>`) or wizard that runs the full 11-step DDC deployment with structured progress events and a per-step rollback signal. This is the headline "one-click" feature.

**Files:**
- Create: `src-tauri/src/core/deploy_workflow.rs`
- Create: `src-tauri/src/cli/domain_deploy.rs`
- Modify: `src-tauri/src/cli/args.rs`, `src-tauri/src/cli/mod.rs`, `src-tauri/src/cli/run.rs`
- Create: `src-tauri/src/commands/deploy.rs`
- Modify: `src-tauri/src/lib.rs` (register commands)
- Create: `src/components/modals/DeployDdcWizard.vue`
- Create: `src/components/deploy/DeployStepIndicator.vue`
- Create: `src/components/deploy/DeployProgressTable.vue`
- Create: `src/views/Deploy.vue`
- Create: `src/stores/deploy.ts`
- Create: `src/lib/deployApi.ts`
- Modify: `src/router/index.ts`

**Dependencies:** M1 (BackendGraph editor), M2 (log verify), M3 (local cache), M4 (RS service probe).

---

#### Task M5.1: Define the plan / step / event types

**Files:**
- Create: `src-tauri/src/core/deploy_workflow.rs`

- [ ] **Step 1: Write the failing test**

```rust
// src-tauri/src/core/deploy_workflow.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployPlan {
    pub project_id: i64,
    pub source_host: String,            // dev box that generates Pak + PSO
    pub target_hosts: Vec<String>,      // render nodes
    pub local_cache: LocalCacheSpec,
    pub shared_cache: SharedCacheSpec,
    pub ddc_pak: PakSpec,
    pub pso: PsoSpec,
    pub verify: VerifySpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalCacheSpec {
    pub path: String,                   // e.g. D:\UE-DDC-Local
    pub service_account: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedCacheSpec {
    pub server_host: String,            // host of the SMB share
    pub share_name: String,             // e.g. DDC
    pub server_path: String,            // e.g. D:\DDC
    pub mode: String,                   // "a" or "b"
    pub unc_path: Option<String>,       // computed after share create
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PakSpec {
    pub enabled: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsoSpec {
    pub enabled: bool,
    pub resolution: String,
    pub max_minutes: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifySpec {
    pub run_log_verify: bool,
    pub editor_exe: String,
    pub project_uproject_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeployEvent {
    StepStarted { step: DeployStep, hosts: Vec<String> },
    StepHostOk { step: DeployStep, host: String, message: Option<String> },
    StepHostError { step: DeployStep, host: String, error: String },
    StepCompleted { step: DeployStep, ok_count: u32, fail_count: u32 },
    PlanCompleted { ok: bool, summary: String },
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeployStep {
    ProvisionLocalDir,
    SetLocalEnv,
    CreateSmbShare,
    SetSharedEnv,
    WriteBackendGraph,
    GenerateDdcPak,
    DistributeDdcPak,
    SetPsoCvars,
    CollectPso,
    DistributePso,
    VerifyStartupLogs,
}

pub fn plan_steps(plan: &DeployPlan) -> Vec<DeployStep> {
    let mut s = vec![
        DeployStep::ProvisionLocalDir,
        DeployStep::SetLocalEnv,
        DeployStep::CreateSmbShare,
        DeployStep::SetSharedEnv,
        DeployStep::WriteBackendGraph,
    ];
    if plan.ddc_pak.enabled {
        s.push(DeployStep::GenerateDdcPak);
        s.push(DeployStep::DistributeDdcPak);
    }
    if plan.pso.enabled {
        s.push(DeployStep::SetPsoCvars);
        s.push(DeployStep::CollectPso);
        s.push(DeployStep::DistributePso);
    }
    if plan.verify.run_log_verify {
        s.push(DeployStep::VerifyStartupLogs);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn baseline_plan() -> DeployPlan {
        DeployPlan {
            project_id: 1, source_host: "DEV-01".into(),
            target_hosts: vec!["R01".into(), "R02".into()],
            local_cache: LocalCacheSpec { path: r"D:\UE-DDC-Local".into(), service_account: None },
            shared_cache: SharedCacheSpec {
                server_host: "NAS-01".into(), share_name: "DDC".into(),
                server_path: r"D:\DDC".into(), mode: "b".into(), unc_path: None,
            },
            ddc_pak: PakSpec { enabled: true },
            pso: PsoSpec { enabled: true, resolution: "1920x1080".into(), max_minutes: 10 },
            verify: VerifySpec { run_log_verify: true,
                editor_exe: r"C:\UE\Engine\Binaries\Win64\UnrealEditor.exe".into(),
                project_uproject_path: r"D:\Projects\MyVP\MyVP.uproject".into() },
        }
    }

    #[test]
    fn full_plan_has_11_steps() {
        let p = baseline_plan();
        assert_eq!(plan_steps(&p).len(), 11);
    }

    #[test]
    fn minimal_plan_skips_optional_phases() {
        let mut p = baseline_plan();
        p.ddc_pak.enabled = false;
        p.pso.enabled = false;
        p.verify.run_log_verify = false;
        let steps = plan_steps(&p);
        assert_eq!(steps.len(), 5);
        assert!(steps.contains(&DeployStep::WriteBackendGraph));
        assert!(!steps.contains(&DeployStep::GenerateDdcPak));
    }
}
```

- [ ] **Step 2: Run tests, register, commit**

```bash
cd src-tauri && cargo test --lib core::deploy_workflow 2>&1 | tail -10
```

Register `pub mod deploy_workflow;` in `src-tauri/src/core/mod.rs`.

```bash
git add src-tauri/src/core/deploy_workflow.rs src-tauri/src/core/mod.rs
git commit -m "feat(ddc): deploy_workflow plan/step/event types + plan_steps"
```

---

#### Task M5.2: Step executor — single-host runner per step

**Files:**
- Modify: `src-tauri/src/core/deploy_workflow.rs`

Each `DeployStep` variant has a corresponding `execute_step_*` function that takes the plan + host context + emits events through a callback. Below: signature + skeleton for each, then one fully written example. Engineer fills the rest using the same shape.

- [ ] **Step 1: Write test for one step (`ProvisionLocalDir`)**

```rust
#[cfg(test)]
mod step_executor_tests {
    use super::*;

    #[test]
    #[cfg(not(windows))]
    fn provision_local_dir_returns_ps_error_off_windows() {
        let plan = baseline_plan();
        let mut events = vec![];
        let mut emit = |e: DeployEvent| events.push(e);
        run_step(&plan, DeployStep::ProvisionLocalDir, None, &mut emit);
        // expects: one StepStarted, N StepHostError (one per target_host), one StepCompleted with fail_count = N
        assert!(matches!(events[0], DeployEvent::StepStarted { .. }));
        assert!(events.iter().any(|e| matches!(e, DeployEvent::StepHostError { .. })));
    }
}
```

- [ ] **Step 2: Implement `run_step`**

```rust
pub fn run_step(
    plan: &DeployPlan,
    step: DeployStep,
    creds: Option<(&str, &str)>,
    emit: &mut dyn FnMut(DeployEvent),
) {
    let hosts = step_hosts(plan, step);
    emit(DeployEvent::StepStarted { step, hosts: hosts.clone() });
    let mut ok_count = 0u32;
    let mut fail_count = 0u32;
    for host in &hosts {
        match execute_one(plan, step, host, creds) {
            Ok(msg) => {
                ok_count += 1;
                emit(DeployEvent::StepHostOk { step, host: host.clone(), message: msg });
            }
            Err(e) => {
                fail_count += 1;
                emit(DeployEvent::StepHostError { step, host: host.clone(), error: e.to_string() });
            }
        }
    }
    emit(DeployEvent::StepCompleted { step, ok_count, fail_count });
}

fn step_hosts(plan: &DeployPlan, step: DeployStep) -> Vec<String> {
    use DeployStep::*;
    match step {
        ProvisionLocalDir | SetLocalEnv | SetSharedEnv | WriteBackendGraph
        | SetPsoCvars | DistributeDdcPak | DistributePso | VerifyStartupLogs
            => plan.target_hosts.clone(),
        CreateSmbShare => vec![plan.shared_cache.server_host.clone()],
        GenerateDdcPak | CollectPso => vec![plan.source_host.clone()],
    }
}

fn execute_one(
    plan: &DeployPlan,
    step: DeployStep,
    host: &str,
    creds: Option<(&str, &str)>,
) -> Result<Option<String>, crate::error::UecmError> {
    use DeployStep::*;
    match step {
        ProvisionLocalDir => {
            crate::core::local_cache::create(
                host,
                &plan.local_cache.path,
                plan.local_cache.service_account.as_deref(),
                creds,
            ).map(Some)
        }
        SetLocalEnv => {
            match creds {
                Some((u, p)) => crate::core::env_vars::set_with_credential(
                    host, "UE-LocalDataCachePath", &plan.local_cache.path, u, p),
                None => crate::core::env_vars::set(host, "UE-LocalDataCachePath", &plan.local_cache.path),
            }.map(|_| None)
        }
        CreateSmbShare => {
            let op = creds.map(|(u, _)| u);
            let pw = creds.map(|(_, p)| p);
            let res = match plan.shared_cache.mode.as_str() {
                "a" | "A" => crate::core::shares::create_mode_a(
                    host, &plan.shared_cache.share_name, &plan.shared_cache.server_path, op, pw)?,
                "b" | "B" => {
                    let svc_pass = crate::core::shares::generate_svc_password();
                    crate::core::shares::create_mode_b(
                        host, &plan.shared_cache.share_name, &plan.shared_cache.server_path,
                        "ddc-svc", &svc_pass, op, pw)?
                }
                other => return Err(crate::error::UecmError::InvalidInput(format!("unknown share mode '{}'", other))),
            };
            Ok(Some(format!("UNC={}", res.unc_path)))
        }
        SetSharedEnv => {
            let unc = plan.shared_cache.unc_path.clone()
                .unwrap_or_else(|| format!(r"\\{}\{}", plan.shared_cache.server_host, plan.shared_cache.share_name));
            match creds {
                Some((u, p)) => crate::core::env_vars::set_with_credential(host, "UE-SharedDataCachePath", &unc, u, p),
                None => crate::core::env_vars::set(host, "UE-SharedDataCachePath", &unc),
            }.map(|_| Some(format!("→ {}", unc)))
        }
        WriteBackendGraph => {
            let unc = plan.shared_cache.unc_path.clone()
                .unwrap_or_else(|| format!(r"\\{}\{}", plan.shared_cache.server_host, plan.shared_cache.share_name));
            // Locate DefaultEngine.ini via project_id — use existing project resolver
            let db = crate::data::db::open()?;
            let project = crate::data::projects::get(&db, plan.project_id)?
                .ok_or_else(|| crate::error::UecmError::InvalidInput(format!("project {} not found", plan.project_id)))?;
            let ini_path = format!(r"{}\Config\DefaultEngine.ini", project.path.trim_end_matches('\\'));
            let (u, p) = creds.ok_or_else(|| crate::error::UecmError::InvalidInput("credentials required for ini write".into()))?;
            crate::core::ini_editor::set_backend_field_with_credential(
                host, &ini_path, "DerivedDataBackendGraph", "Shared", "Path", &unc, u, p)?;
            crate::core::ini_editor::set_backend_field_with_credential(
                host, &ini_path, "DerivedDataBackendGraph", "Shared", "EnvPathOverride", "UE-SharedDataCachePath", u, p)?;
            Ok(Some("Shared.Path + EnvPathOverride".into()))
        }
        GenerateDdcPak => {
            crate::core::ddc_pak::generate(plan.project_id, &plan.source_host, creds).map(|r| Some(r.pak_path))
        }
        DistributeDdcPak => {
            // existing pak_distribute interface is plan-wide; for this step we just dispatch the single host
            let item = crate::core::pak_distribute::plan_one_target(plan.project_id, &plan.source_host, host)?;
            let outcome = futures::executor::block_on(crate::core::pak_distribute::run_one(item))?;
            Ok(Some(format!("{} files", outcome.files_copied)))
        }
        SetPsoCvars => {
            // Each CVar is a Set finding; rely on existing ini_editor::set_key path
            let db = crate::data::db::open()?;
            let project = crate::data::projects::get(&db, plan.project_id)?
                .ok_or_else(|| crate::error::UecmError::InvalidInput(format!("project {} not found", plan.project_id)))?;
            let ini = format!(r"{}\Config\ConsoleVariables.ini", project.path.trim_end_matches('\\'));
            let (u, p) = creds.ok_or_else(|| crate::error::UecmError::InvalidInput("creds required".into()))?;
            for key in ["r.ShaderPipelineCache.Enabled", "r.PSOPrecaching", "r.PSOPrecache.Compile", "r.PSOPrecache.GlobalShaders"] {
                crate::core::ini_editor::set_key_with_credential(host, &ini, "ConsoleVariables", key, "1", u, p)?;
            }
            Ok(Some("4 CVars set".into()))
        }
        CollectPso => {
            // Reuse existing pso_collect — synchronous variant
            crate::core::pso_collect::collect_blocking(
                plan.project_id, &plan.source_host, &plan.pso.resolution,
                plan.pso.max_minutes, creds,
            ).map(|r| Some(format!("{} files", r.collected.len())))
        }
        DistributePso => {
            let item = crate::core::pso_distribute::plan_one_target(plan.project_id, &plan.source_host, host)?;
            let outcome = futures::executor::block_on(crate::core::pso_distribute::run_one(item))?;
            Ok(Some(format!("{} files", outcome.files_copied)))
        }
        VerifyStartupLogs => {
            let report = crate::core::ue_log_verify::run_for_host(
                host, &plan.verify.editor_exe, &plan.verify.project_uproject_path, 180, creds)?;
            let ok = report.local_path.is_some() && report.shared_path.is_some()
                && report.shared_deactivated_reason.is_none()
                && report.move_collision_count < 10;
            if ok {
                Ok(Some(format!("Local={}, Shared={}",
                    report.local_path.as_deref().unwrap_or("?"),
                    report.shared_path.as_deref().unwrap_or("?"))))
            } else {
                Err(crate::error::UecmError::OperationFailed(format!(
                    "verify failed: local_path={:?} shared_path={:?} deactivated={:?} collisions={}",
                    report.local_path, report.shared_path,
                    report.shared_deactivated_reason, report.move_collision_count
                )))
            }
        }
    }
}
```

Some of the helpers above (`pso_collect::collect_blocking`, `pak_distribute::plan_one_target`, `pso_distribute::plan_one_target`, `ddc_pak::generate` return shape) are inferred from existing code. If a signature differs, write a thin adapter inside `deploy_workflow.rs` rather than changing the upstream. Verify with:

```bash
cd src-tauri && cargo check 2>&1 | tail -30
```

Fix mismatched signatures with adapter wrappers.

- [ ] **Step 3: Run tests + commit**

```bash
cd src-tauri && cargo test --lib core::deploy_workflow 2>&1 | tail -15
git add src-tauri/src/core/deploy_workflow.rs
git commit -m "feat(ddc): step executor for all 11 DeployStep variants"
```

---

#### Task M5.3: Plan driver — run_plan with optional stop-on-error

**Files:**
- Modify: `src-tauri/src/core/deploy_workflow.rs`

- [ ] **Step 1: Test + implementation**

Add:

```rust
pub struct RunOptions {
    pub stop_on_step_failure: bool,
}

pub fn run_plan(
    plan: &DeployPlan,
    creds: Option<(&str, &str)>,
    opts: RunOptions,
    emit: &mut dyn FnMut(DeployEvent),
) {
    let steps = plan_steps(plan);
    let mut overall_ok = true;
    for step in steps {
        let mut step_ok_count = 0u32;
        let mut step_fail_count = 0u32;
        run_step(plan, step, creds, &mut |evt| {
            match &evt {
                DeployEvent::StepHostOk { .. } => step_ok_count += 1,
                DeployEvent::StepHostError { .. } => step_fail_count += 1,
                _ => {}
            }
            emit(evt);
        });
        if step_fail_count > 0 {
            overall_ok = false;
            if opts.stop_on_step_failure {
                emit(DeployEvent::PlanCompleted {
                    ok: false,
                    summary: format!("aborted after {:?} ({} failures)", step, step_fail_count),
                });
                return;
            }
        }
    }
    emit(DeployEvent::PlanCompleted {
        ok: overall_ok,
        summary: if overall_ok { "all steps ok".into() } else { "completed with failures".into() },
    });
}
```

- [ ] **Step 2: Commit**

```bash
git add src-tauri/src/core/deploy_workflow.rs
git commit -m "feat(ddc): run_plan driver with stop-on-failure option"
```

---

#### Task M5.4: CLI `deploy ddc` command

**Files:**
- Modify: `src-tauri/src/cli/args.rs`
- Create: `src-tauri/src/cli/domain_deploy.rs`
- Modify: `src-tauri/src/cli/mod.rs`, `src-tauri/src/cli/run.rs`

- [ ] **Step 1: Add Domain::Deploy + DeployAction**

In `args.rs`:

```rust
/// One-click DDC deployment workflow.
Deploy {
    #[command(subcommand)]
    action: DeployAction,
},

#[derive(Subcommand, Debug)]
pub enum DeployAction {
    /// Run the full DDC deployment plan from a JSON file.
    Ddc {
        /// Path to a deploy-plan JSON file. Schema: see deploy_workflow::DeployPlan.
        #[arg(long)]
        plan: std::path::PathBuf,
        #[arg(long)]
        stop_on_failure: bool,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
}
```

Create `src-tauri/src/cli/domain_deploy.rs`:

```rust
use crate::cli::args::DeployAction;
use crate::cli::destructive::{self, Outcome};
use crate::cli::output::EmitSerialize;
use crate::cli::run::Ctx;
use crate::core::deploy_workflow::{self, DeployPlan, DeployEvent, RunOptions, plan_steps};
use crate::error::{UecmError, UecmResult};

pub fn handle(ctx: &mut Ctx<'_>, action: DeployAction) -> UecmResult<()> {
    match action {
        DeployAction::Ddc { plan, stop_on_failure, yes, dry_run, cred } => {
            let body = std::fs::read_to_string(&plan)
                .map_err(|e| UecmError::Io(format!("read plan {}: {}", plan.display(), e)))?;
            let p: DeployPlan = serde_json::from_str(&body)
                .map_err(|e| UecmError::InvalidInput(format!("bad plan: {}", e)))?;

            let outcome = destructive::check(yes, dry_run, "deploy.ddc")?;
            let db = ctx.require_db()?;
            cred.preflight(db)?;
            if outcome == Outcome::DryRun {
                destructive::emit_plan(ctx.emitter.as_mut(), "deploy.ddc",
                    serde_json::json!({"steps": plan_steps(&p), "targets": p.target_hosts}));
                return Ok(());
            }
            let creds = cred.resolve(db)?;
            deploy_workflow::run_plan(
                &p,
                creds.as_ref().map(|(u, p)| (u.as_str(), p.as_str())),
                RunOptions { stop_on_step_failure: stop_on_failure },
                &mut |evt: DeployEvent| { ctx.emitter.emit_result(&evt).ok(); },
            );
            Ok(())
        }
    }
}
```

Wire into `mod.rs` + `run.rs`. Add parser test.

- [ ] **Step 2: Sample plan JSON for reference**

Create `docs/superpowers/examples/deploy-ddc-plan.example.json`:

```json
{
  "project_id": 1,
  "source_host": "DEV-01",
  "target_hosts": ["RENDER-01", "RENDER-02"],
  "local_cache": { "path": "D:\\UE-DDC-Local", "service_account": null },
  "shared_cache": { "server_host": "NAS-01", "share_name": "DDC", "server_path": "D:\\DDC", "mode": "b", "unc_path": null },
  "ddc_pak": { "enabled": true },
  "pso": { "enabled": true, "resolution": "1920x1080", "max_minutes": 10 },
  "verify": { "run_log_verify": true, "editor_exe": "C:\\UE\\Engine\\Binaries\\Win64\\UnrealEditor.exe", "project_uproject_path": "D:\\Projects\\MyVP\\MyVP.uproject" }
}
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/cli/args.rs src-tauri/src/cli/domain_deploy.rs src-tauri/src/cli/mod.rs src-tauri/src/cli/run.rs docs/superpowers/examples/deploy-ddc-plan.example.json
git commit -m "feat(ddc): cli deploy ddc --plan=<json> runs full workflow"
```

---

#### Task M5.5: Tauri command for UI

**Files:**
- Create: `src-tauri/src/commands/deploy.rs`
- Modify: `src-tauri/src/commands/mod.rs`, `src-tauri/src/lib.rs`

- [ ] **Step 1: Backend command**

```rust
// src-tauri/src/commands/deploy.rs
use crate::core::deploy_workflow::{self, DeployPlan, DeployEvent, RunOptions};
use crate::data::credentials as cred_store;
use crate::error::UecmError;
use tauri::{AppHandle, Emitter};

#[tauri::command]
pub async fn deploy_ddc_run(
    app: AppHandle,
    plan: DeployPlan,
    cred_alias: Option<String>,
    stop_on_failure: bool,
) -> Result<(), String> {
    let creds = match cred_alias {
        Some(alias) if !alias.is_empty() => {
            let db = crate::data::db::open().map_err(|e: UecmError| e.to_string())?;
            Some(cred_store::load_decrypted(&db, &alias).map_err(|e: UecmError| e.to_string())?)
        }
        _ => None,
    };
    let emit_app = app.clone();
    tokio::task::spawn_blocking(move || {
        deploy_workflow::run_plan(
            &plan,
            creds.as_ref().map(|(u, p)| (u.as_str(), p.as_str())),
            RunOptions { stop_on_step_failure: stop_on_failure },
            &mut |evt: DeployEvent| {
                emit_app.emit("deploy-event", &evt).ok();
            },
        );
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn deploy_ddc_plan_preview(plan: DeployPlan) -> Vec<deploy_workflow::DeployStep> {
    deploy_workflow::plan_steps(&plan)
}
```

Register in `commands/mod.rs` and add to `invoke_handler` in `lib.rs`.

- [ ] **Step 2: Commit**

```bash
git add src-tauri/src/commands/deploy.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat(ddc): deploy_ddc_run command streams events via app emitter"
```

---

#### Task M5.6: Pinia store + API wrapper

**Files:**
- Create: `src/lib/deployApi.ts`
- Create: `src/stores/deploy.ts`

- [ ] **Step 1: API wrapper**

```ts
// src/lib/deployApi.ts
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type DeployStep =
  | "provision_local_dir" | "set_local_env" | "create_smb_share"
  | "set_shared_env" | "write_backend_graph"
  | "generate_ddc_pak" | "distribute_ddc_pak"
  | "set_pso_cvars" | "collect_pso" | "distribute_pso"
  | "verify_startup_logs";

export interface DeployPlan {
  project_id: number;
  source_host: string;
  target_hosts: string[];
  local_cache: { path: string; service_account: string | null };
  shared_cache: { server_host: string; share_name: string; server_path: string; mode: string; unc_path: string | null };
  ddc_pak: { enabled: boolean };
  pso: { enabled: boolean; resolution: string; max_minutes: number };
  verify: { run_log_verify: boolean; editor_exe: string; project_uproject_path: string };
}

export type DeployEvent =
  | { kind: "step_started"; step: DeployStep; hosts: string[] }
  | { kind: "step_host_ok"; step: DeployStep; host: string; message: string | null }
  | { kind: "step_host_error"; step: DeployStep; host: string; error: string }
  | { kind: "step_completed"; step: DeployStep; ok_count: number; fail_count: number }
  | { kind: "plan_completed"; ok: boolean; summary: string };

export function previewPlan(plan: DeployPlan): Promise<DeployStep[]> {
  return invoke("deploy_ddc_plan_preview", { plan });
}

export function runPlan(plan: DeployPlan, credAlias: string | null, stopOnFailure: boolean): Promise<void> {
  return invoke("deploy_ddc_run", { plan, credAlias, stopOnFailure });
}

export function subscribe(onEvent: (e: DeployEvent) => void): Promise<UnlistenFn> {
  return listen<DeployEvent>("deploy-event", (e) => onEvent(e.payload));
}
```

- [ ] **Step 2: Store**

```ts
// src/stores/deploy.ts
import { defineStore } from "pinia";
import { ref } from "vue";
import { previewPlan, runPlan, subscribe, type DeployEvent, type DeployPlan, type DeployStep } from "@/lib/deployApi";

interface HostStatus { state: "pending" | "running" | "ok" | "error"; message: string | null; }
interface StepProgress { step: DeployStep; hosts: Record<string, HostStatus>; ok_count: number; fail_count: number; }

export const useDeployStore = defineStore("deploy", () => {
  const steps = ref<DeployStep[]>([]);
  const progress = ref<Record<string, StepProgress>>({});
  const running = ref(false);
  const completed = ref(false);
  const finalOk = ref<boolean | null>(null);
  const summary = ref("");
  let unlisten: (() => void) | null = null;

  async function preview(plan: DeployPlan) {
    steps.value = await previewPlan(plan);
    progress.value = {};
    completed.value = false;
    finalOk.value = null;
    summary.value = "";
    for (const s of steps.value) progress.value[s] = { step: s, hosts: {}, ok_count: 0, fail_count: 0 };
  }

  async function run(plan: DeployPlan, credAlias: string | null, stopOnFailure: boolean) {
    await preview(plan);
    running.value = true;
    unlisten = await subscribe(onEvent);
    try { await runPlan(plan, credAlias, stopOnFailure); } finally {
      running.value = false;
      unlisten?.(); unlisten = null;
    }
  }

  function onEvent(e: DeployEvent) {
    if (e.kind === "step_started") {
      const p = progress.value[e.step]; if (!p) return;
      for (const h of e.hosts) p.hosts[h] = { state: "running", message: null };
    } else if (e.kind === "step_host_ok") {
      const p = progress.value[e.step]; if (!p) return;
      p.hosts[e.host] = { state: "ok", message: e.message };
    } else if (e.kind === "step_host_error") {
      const p = progress.value[e.step]; if (!p) return;
      p.hosts[e.host] = { state: "error", message: e.error };
    } else if (e.kind === "step_completed") {
      const p = progress.value[e.step]; if (!p) return;
      p.ok_count = e.ok_count; p.fail_count = e.fail_count;
    } else if (e.kind === "plan_completed") {
      completed.value = true; finalOk.value = e.ok; summary.value = e.summary;
    }
  }

  return { steps, progress, running, completed, finalOk, summary, preview, run };
});
```

- [ ] **Step 3: Commit**

```bash
git add src/lib/deployApi.ts src/stores/deploy.ts
git commit -m "feat(ddc): deployApi + Pinia store with live event subscription"
```

---

#### Task M5.7: Wizard component

**Files:**
- Create: `src/components/modals/DeployDdcWizard.vue`
- Create: `src/components/deploy/DeployStepIndicator.vue`
- Create: `src/components/deploy/DeployProgressTable.vue`
- Create: `src/views/Deploy.vue`
- Modify: `src/router/index.ts`

- [ ] **Step 1: Step indicator**

```vue
<!-- src/components/deploy/DeployStepIndicator.vue -->
<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { DeployStep } from "@/lib/deployApi";
import UecmIcon from "@/components/primitives/UecmIcon.vue";

defineProps<{
  steps: DeployStep[];
  status: Record<string, { ok_count: number; fail_count: number; hosts: Record<string, { state: string }> }>;
}>();

const { t } = useI18n();
function labelFor(step: DeployStep): string { return t(`deploy.step.${step}`); }
function toneOf(p?: { ok_count: number; fail_count: number; hosts: Record<string, { state: string }> }) {
  if (!p || Object.keys(p.hosts).length === 0) return "muted";
  if (p.fail_count > 0) return "critical";
  if (Object.values(p.hosts).some((h) => h.state === "running")) return "info";
  if (Object.values(p.hosts).every((h) => h.state === "ok")) return "healthy";
  return "muted";
}
</script>

<template>
  <ol class="space-y-2">
    <li v-for="(s, i) in steps" :key="s" class="flex items-start gap-2 text-sm">
      <span class="mt-0.5 text-muted-foreground tabular-nums">{{ String(i + 1).padStart(2, "0") }}</span>
      <UecmIcon :name="toneOf(status[s]) === 'healthy' ? 'check' : toneOf(status[s]) === 'critical' ? 'alert' : 'circle'"
                :class="['mt-0.5', toneOf(status[s]) === 'healthy' ? 'text-status-healthy' : toneOf(status[s]) === 'critical' ? 'text-status-critical' : toneOf(status[s]) === 'info' ? 'text-status-info' : 'text-muted-foreground']" />
      <div class="flex-1">
        <div>{{ labelFor(s) }}</div>
        <div v-if="status[s] && Object.keys(status[s].hosts).length" class="text-xs text-muted-foreground">
          {{ status[s].ok_count }} ok · {{ status[s].fail_count }} fail
        </div>
      </div>
    </li>
  </ol>
</template>
```

- [ ] **Step 2: Progress table**

```vue
<!-- src/components/deploy/DeployProgressTable.vue -->
<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { DeployStep } from "@/lib/deployApi";

const props = defineProps<{
  steps: DeployStep[];
  status: Record<string, { hosts: Record<string, { state: string; message: string | null }> }>;
}>();
const { t } = useI18n();

const hosts = computed<string[]>(() => {
  const set = new Set<string>();
  for (const s of props.steps) {
    const p = props.status[s];
    if (p) for (const h of Object.keys(p.hosts)) set.add(h);
  }
  return Array.from(set).sort();
});

function cell(step: DeployStep, host: string): { state: string; message: string | null } | null {
  return props.status[step]?.hosts[host] ?? null;
}
function toneClass(state?: string): string {
  if (state === "ok") return "bg-status-healthy/20 text-status-healthy";
  if (state === "error") return "bg-status-critical/20 text-status-critical";
  if (state === "running") return "bg-status-info/20 text-status-info";
  return "bg-muted/40 text-muted-foreground";
}
</script>

<template>
  <table class="w-full text-xs border-collapse">
    <thead>
      <tr>
        <th class="text-left p-1 font-normal text-muted-foreground">{{ t("deploy.step.header") }}</th>
        <th v-for="h in hosts" :key="h" class="p-1 text-left font-mono">{{ h }}</th>
      </tr>
    </thead>
    <tbody>
      <tr v-for="s in steps" :key="s" class="border-t border-border">
        <td class="p-1 pr-2">{{ t(`deploy.step.${s}`) }}</td>
        <td v-for="h in hosts" :key="h" class="p-1">
          <span :class="['inline-block rounded px-1.5 py-0.5', toneClass(cell(s, h)?.state)]"
                :title="cell(s, h)?.message ?? ''">
            {{ cell(s, h)?.state ?? "-" }}
          </span>
        </td>
      </tr>
    </tbody>
  </table>
</template>
```

- [ ] **Step 3: Wizard**

```vue
<!-- src/components/modals/DeployDdcWizard.vue -->
<script setup lang="ts">
import { ref, computed } from "vue";
import { useI18n } from "vue-i18n";
import BaseModal from "./BaseModal.vue";
import DeployStepIndicator from "@/components/deploy/DeployStepIndicator.vue";
import DeployProgressTable from "@/components/deploy/DeployProgressTable.vue";
import { useDeployStore } from "@/stores/deploy";
import { useMachinesStore } from "@/stores/machines";
import { useProjectsStore } from "@/stores/projects";
import { useCredentialsStore } from "@/stores/credentials";
import type { DeployPlan } from "@/lib/deployApi";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ (e: "close"): void }>();
const { t } = useI18n();

const deploy = useDeployStore();
const machines = useMachinesStore();
const projects = useProjectsStore();
const credentials = useCredentialsStore();

const step = ref<1 | 2 | 3 | 4 | 5>(1);
const projectId = ref<number | null>(null);
const sourceHost = ref<string>("");
const targets = ref<string[]>([]);
const localPath = ref(r"D:\UE-DDC-Local");
const sharedServer = ref<string>("");
const shareName = ref("DDC");
const sharePath = ref(r"D:\DDC");
const shareMode = ref<"a" | "b">("b");
const enablePak = ref(true);
const enablePso = ref(true);
const psoRes = ref("1920x1080");
const psoMinutes = ref(10);
const runVerify = ref(true);
const editorExe = ref(r"C:\Program Files\Epic Games\UE_5.5\Engine\Binaries\Win64\UnrealEditor.exe");
const uproject = ref("");
const credAlias = ref<string>("");
const stopOnFailure = ref(true);

const canRun = computed(() =>
  projectId.value !== null && sourceHost.value && targets.value.length > 0
  && sharedServer.value && credAlias.value && !deploy.running,
);

function buildPlan(): DeployPlan {
  return {
    project_id: projectId.value!,
    source_host: sourceHost.value,
    target_hosts: [...targets.value],
    local_cache: { path: localPath.value, service_account: null },
    shared_cache: {
      server_host: sharedServer.value, share_name: shareName.value,
      server_path: sharePath.value, mode: shareMode.value, unc_path: null,
    },
    ddc_pak: { enabled: enablePak.value },
    pso: { enabled: enablePso.value, resolution: psoRes.value, max_minutes: psoMinutes.value },
    verify: {
      run_log_verify: runVerify.value,
      editor_exe: editorExe.value,
      project_uproject_path: uproject.value,
    },
  };
}

async function onPreview() { await deploy.preview(buildPlan()); step.value = 5; }
async function onRun()     { await deploy.run(buildPlan(), credAlias.value, stopOnFailure.value); }
</script>

<template>
  <BaseModal :open="props.open" :title="t('deploy.title')" @close="emit('close')">
    <div class="space-y-4 max-h-[80vh] overflow-y-auto">
      <!-- Step 1: project + targets -->
      <section v-if="step === 1">
        <h3 class="font-display text-base mb-2">{{ t("deploy.s1.title") }}</h3>
        <label class="block text-sm mb-1">{{ t("deploy.s1.project") }}</label>
        <select v-model="projectId" class="mb-3 w-full rounded border border-input bg-transparent px-2 py-1 text-sm">
          <option :value="null">--</option>
          <option v-for="p in projects.projects" :key="p.id" :value="p.id">{{ p.name }}</option>
        </select>
        <label class="block text-sm mb-1">{{ t("deploy.s1.sourceHost") }}</label>
        <select v-model="sourceHost" class="mb-3 w-full rounded border border-input bg-transparent px-2 py-1 text-sm">
          <option value="">--</option>
          <option v-for="m in machines.machines" :key="m.id" :value="m.hostname">{{ m.hostname }}</option>
        </select>
        <label class="block text-sm mb-1">{{ t("deploy.s1.targets") }}</label>
        <div class="max-h-40 overflow-y-auto border border-border rounded p-2">
          <label v-for="m in machines.machines" :key="m.id" class="flex items-center gap-2 text-sm">
            <input type="checkbox" :value="m.hostname" v-model="targets" />
            {{ m.hostname }}
          </label>
        </div>
        <button class="mt-3 px-3 py-1 rounded bg-primary text-primary-foreground text-sm"
                :disabled="projectId === null || !sourceHost || !targets.length"
                @click="step = 2">{{ t("common.next") }}</button>
      </section>

      <!-- Step 2: local cache -->
      <section v-if="step === 2">
        <h3 class="font-display text-base mb-2">{{ t("deploy.s2.title") }}</h3>
        <label class="block text-sm mb-1">{{ t("deploy.s2.localPath") }}</label>
        <input v-model="localPath" class="mb-3 w-full rounded border border-input bg-transparent px-2 py-1 text-sm" />
        <div class="flex gap-2">
          <button class="px-3 py-1 rounded border border-border text-sm" @click="step = 1">{{ t("common.back") }}</button>
          <button class="px-3 py-1 rounded bg-primary text-primary-foreground text-sm" @click="step = 3">{{ t("common.next") }}</button>
        </div>
      </section>

      <!-- Step 3: shared cache -->
      <section v-if="step === 3">
        <h3 class="font-display text-base mb-2">{{ t("deploy.s3.title") }}</h3>
        <label class="block text-sm mb-1">{{ t("deploy.s3.serverHost") }}</label>
        <select v-model="sharedServer" class="mb-3 w-full rounded border border-input bg-transparent px-2 py-1 text-sm">
          <option value="">--</option>
          <option v-for="m in machines.machines" :key="m.id" :value="m.hostname">{{ m.hostname }}</option>
        </select>
        <label class="block text-sm mb-1">{{ t("deploy.s3.shareName") }}</label>
        <input v-model="shareName" class="mb-3 w-full rounded border border-input bg-transparent px-2 py-1 text-sm" />
        <label class="block text-sm mb-1">{{ t("deploy.s3.sharePath") }}</label>
        <input v-model="sharePath" class="mb-3 w-full rounded border border-input bg-transparent px-2 py-1 text-sm" />
        <label class="block text-sm mb-1">{{ t("deploy.s3.mode") }}</label>
        <select v-model="shareMode" class="mb-3 w-full rounded border border-input bg-transparent px-2 py-1 text-sm">
          <option value="a">A — Open (Guest + Everyone:Full)</option>
          <option value="b">B — Managed (ddc-svc)</option>
        </select>
        <div class="flex gap-2">
          <button class="px-3 py-1 rounded border border-border text-sm" @click="step = 2">{{ t("common.back") }}</button>
          <button class="px-3 py-1 rounded bg-primary text-primary-foreground text-sm" @click="step = 4">{{ t("common.next") }}</button>
        </div>
      </section>

      <!-- Step 4: pak / pso / verify -->
      <section v-if="step === 4">
        <h3 class="font-display text-base mb-2">{{ t("deploy.s4.title") }}</h3>
        <label class="flex items-center gap-2 text-sm mb-2"><input type="checkbox" v-model="enablePak" /> {{ t("deploy.s4.pak") }}</label>
        <label class="flex items-center gap-2 text-sm mb-2"><input type="checkbox" v-model="enablePso" /> {{ t("deploy.s4.pso") }}</label>
        <div v-if="enablePso" class="ml-6 mb-3 space-y-2">
          <div class="flex items-center gap-2 text-sm">
            <label class="w-32">{{ t("deploy.s4.psoRes") }}</label>
            <input v-model="psoRes" class="flex-1 rounded border border-input bg-transparent px-2 py-1 text-sm" />
          </div>
          <div class="flex items-center gap-2 text-sm">
            <label class="w-32">{{ t("deploy.s4.psoMinutes") }}</label>
            <input type="number" v-model.number="psoMinutes" min="1" max="120"
                   class="flex-1 rounded border border-input bg-transparent px-2 py-1 text-sm" />
          </div>
        </div>
        <label class="flex items-center gap-2 text-sm mb-2"><input type="checkbox" v-model="runVerify" /> {{ t("deploy.s4.verify") }}</label>
        <div v-if="runVerify" class="ml-6 mb-3 space-y-2">
          <input v-model="editorExe" :placeholder="t('deploy.s4.editorExe')" class="w-full rounded border border-input bg-transparent px-2 py-1 text-sm" />
          <input v-model="uproject" :placeholder="t('deploy.s4.uproject')" class="w-full rounded border border-input bg-transparent px-2 py-1 text-sm" />
        </div>
        <label class="block text-sm mb-1 mt-3">{{ t("deploy.s4.cred") }}</label>
        <select v-model="credAlias" class="mb-3 w-full rounded border border-input bg-transparent px-2 py-1 text-sm">
          <option value="">--</option>
          <option v-for="c in credentials.credentials" :key="c.alias" :value="c.alias">{{ c.alias }}</option>
        </select>
        <label class="flex items-center gap-2 text-sm mb-3"><input type="checkbox" v-model="stopOnFailure" /> {{ t("deploy.s4.stopOnFailure") }}</label>
        <div class="flex gap-2">
          <button class="px-3 py-1 rounded border border-border text-sm" @click="step = 3">{{ t("common.back") }}</button>
          <button class="px-3 py-1 rounded bg-primary text-primary-foreground text-sm" :disabled="!canRun" @click="onPreview">{{ t("deploy.s4.preview") }}</button>
        </div>
      </section>

      <!-- Step 5: preview + run -->
      <section v-if="step === 5">
        <h3 class="font-display text-base mb-2">{{ t("deploy.s5.title") }}</h3>
        <div class="grid grid-cols-[max-content_1fr] gap-4">
          <DeployStepIndicator :steps="deploy.steps" :status="deploy.progress" />
          <DeployProgressTable :steps="deploy.steps" :status="deploy.progress" />
        </div>
        <div class="mt-4 flex items-center gap-3">
          <button v-if="!deploy.running && !deploy.completed" class="px-4 py-1.5 rounded bg-primary text-primary-foreground text-sm" @click="onRun">{{ t("deploy.s5.run") }}</button>
          <span v-if="deploy.running" class="text-sm text-status-info">{{ t("deploy.s5.running") }}</span>
          <span v-if="deploy.completed" :class="deploy.finalOk ? 'text-status-healthy' : 'text-status-critical'">{{ deploy.summary }}</span>
        </div>
        <div class="mt-3">
          <button class="px-3 py-1 rounded border border-border text-sm" @click="step = 4" :disabled="deploy.running">{{ t("common.back") }}</button>
        </div>
      </section>
    </div>
  </BaseModal>
</template>
```

- [ ] **Step 4: View + route**

```vue
<!-- src/views/Deploy.vue -->
<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import DeployDdcWizard from "@/components/modals/DeployDdcWizard.vue";

const { t } = useI18n();
const open = ref(false);
</script>

<template>
  <div class="p-6">
    <h1 class="font-display text-2xl mb-3">{{ t("deploy.pageTitle") }}</h1>
    <p class="text-muted-foreground mb-4">{{ t("deploy.pageDesc") }}</p>
    <button class="px-4 py-2 rounded bg-primary text-primary-foreground" @click="open = true">{{ t("deploy.openWizard") }}</button>
    <DeployDdcWizard :open="open" @close="open = false" />
  </div>
</template>
```

In `src/router/index.ts`, add:

```ts
{ path: "/deploy", name: "deploy", component: () => import("@/views/Deploy.vue") },
```

Add nav entry in sidebar (`src/components/shell/UecmSidebar.vue`).

- [ ] **Step 5: Locale keys**

Add to `src/locales/zh.ts`:

```ts
deploy: {
  title: "DDC 一键部署",
  pageTitle: "DDC 部署",
  pageDesc: "向导式部署 Local DDC + Shared SMB + DDC Pak + PSO，并基于启动日志验证。",
  openWizard: "打开部署向导",
  s1: { title: "1. 选择项目和机器", project: "项目", sourceHost: "开发机 / 预热机", targets: "目标 Render Node" },
  s2: { title: "2. Local DDC 配置", localPath: "Local 缓存目录" },
  s3: { title: "3. Shared SMB 共享", serverHost: "共享 server 主机", shareName: "共享名", sharePath: "server 本地路径", mode: "模式" },
  s4: { title: "4. Pak / PSO / 验证", pak: "生成并分发 DDC Pak", pso: "采集并分发 PSO", psoRes: "PSO 分辨率", psoMinutes: "PSO 最大时长 (分钟)", verify: "运行启动日志验证", editorExe: "UnrealEditor.exe 路径", uproject: ".uproject 路径", cred: "操作员凭据", stopOnFailure: "首个步骤失败即停止", preview: "预览计划" },
  s5: { title: "5. 执行", run: "开始执行", running: "执行中..." },
  step: {
    header: "步骤",
    provision_local_dir: "Provision Local 目录",
    set_local_env: "设 UE-LocalDataCachePath",
    create_smb_share: "建 SMB 共享",
    set_shared_env: "设 UE-SharedDataCachePath",
    write_backend_graph: "写 BackendGraph",
    generate_ddc_pak: "生成 DDC Pak",
    distribute_ddc_pak: "分发 DDC Pak",
    set_pso_cvars: "设 PSO CVars",
    collect_pso: "采集 PSO",
    distribute_pso: "分发 PSO",
    verify_startup_logs: "启动日志验证",
  },
},
common: { back: "返回", next: "下一步" },
```

Mirror in en.ts.

- [ ] **Step 6: Commit**

```bash
git add src/components/modals/DeployDdcWizard.vue src/components/deploy/DeployStepIndicator.vue src/components/deploy/DeployProgressTable.vue src/views/Deploy.vue src/router/index.ts src/components/shell/UecmSidebar.vue src/locales/zh.ts src/locales/en.ts
git commit -m "feat(ddc): one-click deploy wizard with live progress table"
```

---

## Phase 2 — P1 (Important, ~11 working days)

### Module M6: Editor Preferences 4-path reader (2 days)

**Goal:** Read the four DDC paths that UE Editor Preferences UI persists (`Global Local`, `Global Shared`, `Project Local`, `Project Shared`) from `EditorPerProjectUserSettings.ini`. Surface them in scans so operators see when a user-Profile override is silently masking the cluster setup.

**Files:**
- Create: `src-tauri/src/core/editor_preferences.rs`
- Create: `ps-scripts/read-editor-preferences.ps1`
- Modify: `src-tauri/src/core/ini_diagnostics.rs` (rule R025)
- Modify: `src-tauri/src/core/ini_scanner.rs` (already reads the file; add pref extraction)

**Dependencies:** None.

---

#### Task M6.1: Pure parser for Editor Preferences DDC keys

**Files:**
- Create: `src-tauri/src/core/editor_preferences.rs`

The Editor Preferences UI for DDC writes to `EditorPerProjectUserSettings.ini` under section `[/Script/UnrealEd.EditorSettings]` with keys `GlobalLocalDDCPath`, `GlobalSharedDDCPath` (plus per-project variants). Section / key names have shifted slightly between UE 5.3 and 5.5; accept both spellings.

- [ ] **Step 1: Write failing test**

```rust
// src-tauri/src/core/editor_preferences.rs
//! Extract the four DDC paths from a parsed EditorPerProjectUserSettings.ini.

use crate::core::ini_diagnostics::ParsedFile;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditorDdcPrefs {
    pub global_local: Option<String>,
    pub global_shared: Option<String>,
    pub project_local: Option<String>,
    pub project_shared: Option<String>,
}

const SECTIONS: &[&str] = &[
    "/Script/UnrealEd.EditorSettings",
    "/Script/UnrealEd.EditorPerProjectUserSettings",
];

const KEYS_GLOBAL_LOCAL: &[&str] = &["GlobalLocalDDCPath", "GlobalLocalDataCachePath"];
const KEYS_GLOBAL_SHARED: &[&str] = &["GlobalSharedDDCPath", "GlobalSharedDataCachePath"];
const KEYS_PROJECT_LOCAL: &[&str] = &["ProjectLocalDDCPath", "ProjectLocalDataCachePath"];
const KEYS_PROJECT_SHARED: &[&str] = &["ProjectSharedDDCPath", "ProjectSharedDataCachePath"];

pub fn extract(file: &ParsedFile) -> EditorDdcPrefs {
    let mut out = EditorDdcPrefs::default();
    for section in &file.sections {
        if !SECTIONS.iter().any(|s| s.eq_ignore_ascii_case(&section.name)) { continue; }
        for k in &section.keys {
            let n = k.name.as_str();
            let v = || k.value.trim().to_string();
            if KEYS_GLOBAL_LOCAL.iter().any(|x| x.eq_ignore_ascii_case(n)) && out.global_local.is_none() { out.global_local = Some(v()); }
            else if KEYS_GLOBAL_SHARED.iter().any(|x| x.eq_ignore_ascii_case(n)) && out.global_shared.is_none() { out.global_shared = Some(v()); }
            else if KEYS_PROJECT_LOCAL.iter().any(|x| x.eq_ignore_ascii_case(n)) && out.project_local.is_none() { out.project_local = Some(v()); }
            else if KEYS_PROJECT_SHARED.iter().any(|x| x.eq_ignore_ascii_case(n)) && out.project_shared.is_none() { out.project_shared = Some(v()); }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ini_diagnostics::{Category, ParsedFile, ParsedKey, ParsedSection};

    fn pref_section(name: &str, kvs: &[(&str, &str)]) -> ParsedFile {
        ParsedFile {
            path: "test.ini".into(),
            category: Category::User,
            sections: vec![ParsedSection {
                name: name.into(),
                keys: kvs.iter().map(|(k, v)| ParsedKey { name: k.to_string(), value: v.to_string(), line_number: 0 }).collect(),
                backend_nodes: vec![],
            }],
        }
    }

    #[test]
    fn picks_up_all_four_paths() {
        let f = pref_section("/Script/UnrealEd.EditorSettings", &[
            ("GlobalLocalDDCPath",   r"F:\DDC\Local"),
            ("GlobalSharedDDCPath",  r"\\NAS\DDC"),
            ("ProjectLocalDDCPath",  r"D:\Proj\Local"),
            ("ProjectSharedDDCPath", r"\\NAS\Proj"),
        ]);
        let p = extract(&f);
        assert_eq!(p.global_local.as_deref(),  Some(r"F:\DDC\Local"));
        assert_eq!(p.global_shared.as_deref(), Some(r"\\NAS\DDC"));
        assert_eq!(p.project_local.as_deref(), Some(r"D:\Proj\Local"));
        assert_eq!(p.project_shared.as_deref(),Some(r"\\NAS\Proj"));
    }

    #[test]
    fn accepts_alt_key_names() {
        let f = pref_section("/Script/UnrealEd.EditorSettings", &[
            ("GlobalLocalDataCachePath", r"F:\DDC\Local"),
        ]);
        assert_eq!(extract(&f).global_local.as_deref(), Some(r"F:\DDC\Local"));
    }

    #[test]
    fn absent_section_returns_empty() {
        let f = pref_section("Some/Other.Section", &[("GlobalLocalDDCPath", "X")]);
        assert_eq!(extract(&f), EditorDdcPrefs::default());
    }
}
```

- [ ] **Step 2: Run + register + commit**

```bash
cd src-tauri && cargo test --lib core::editor_preferences 2>&1 | tail -10
```

Register `pub mod editor_preferences;` in `core/mod.rs`.

```bash
git add src-tauri/src/core/editor_preferences.rs src-tauri/src/core/mod.rs
git commit -m "feat(ddc): extract EditorDdcPrefs from EditorPerProjectUserSettings.ini"
```

---

#### Task M6.2: Rule R025 — project-level pref masks env config

**Files:**
- Modify: `src-tauri/src/core/ini_diagnostics.rs`

When project-level path is set (non-empty), SOP says it completely overrides the corresponding global setting *and* the env var fallback. Fire a Critical warning so the operator knows their Machine-scope env var is being ignored.

- [ ] **Step 1: Test**

```rust
#[test]
fn r025_critical_when_project_pref_overrides_env() {
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

- [ ] **Step 2: Implementation**

```rust
fn rule_r025(file: &ParsedFile, env: &EnvVarState) -> Vec<Finding> {
    if file.category != Category::User { return vec![]; }
    let prefs = crate::core::editor_preferences::extract(file);
    let mut out = Vec::new();
    if let (Some(_), Some(env_val)) = (prefs.project_shared.as_ref(), env.shared_data_cache_path.as_ref()) {
        if prefs.project_shared.as_deref() != Some(env_val.as_str()) {
            out.push(Finding {
                rule_id: "R025".into(),
                severity: Severity::Critical,
                category: file.category,
                file_path: file.path.clone(),
                section: Some("/Script/UnrealEd.EditorSettings".into()),
                key_name: Some("ProjectSharedDDCPath".into()),
                line_number: None,
                snippet_before: format!("ProjectSharedDDCPath={}", prefs.project_shared.as_deref().unwrap()),
                snippet_after: Some("(leave empty so env var / project Config takes over)".into()),
                recommended_action: RecommendedAction::Remove,
                recommended_value: None,
                symptom: "Project-level Editor Pref masks UE-SharedDataCachePath; cluster setup is bypassed silently.".into(),
                rationale: "When ProjectSharedDDCPath is non-empty, UE uses that value and ignores EnvPathOverride and Machine-scope env vars.".into(),
            });
        }
    }
    out
}
```

Add to `run_rules`: `out.extend(rule_r025(file, env));`

- [ ] **Step 3: Commit**

```bash
cd src-tauri && cargo test --lib core::ini_diagnostics::tests::r025 2>&1 | tail -5
git add src-tauri/src/core/ini_diagnostics.rs
git commit -m "feat(ddc): R025 — project-level Editor Pref masks env var"
```

---

#### Task M6.3: UI surface — show 4 Editor Pref values in machine detail

**Files:**
- Modify: `src/views/Machines.vue` (or relevant detail panel)
- Add a primitive: `src/components/primitives/UecmEditorPrefBlock.vue`

- [ ] **Step 1: Primitive component**

```vue
<!-- src/components/primitives/UecmEditorPrefBlock.vue -->
<script setup lang="ts">
import { useI18n } from "vue-i18n";

defineProps<{
  prefs: {
    global_local: string | null;
    global_shared: string | null;
    project_local: string | null;
    project_shared: string | null;
  } | null;
}>();
const { t } = useI18n();
function fmt(v: string | null | undefined): string { return (v && v.trim()) || t("editorPref.empty"); }
</script>

<template>
  <section class="rounded-md border border-border p-3 text-sm">
    <h4 class="font-display text-base mb-2">{{ t("editorPref.title") }}</h4>
    <dl class="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1">
      <dt class="text-muted-foreground">{{ t("editorPref.globalLocal") }}</dt><dd>{{ fmt(prefs?.global_local) }}</dd>
      <dt class="text-muted-foreground">{{ t("editorPref.globalShared") }}</dt><dd>{{ fmt(prefs?.global_shared) }}</dd>
      <dt class="text-muted-foreground">{{ t("editorPref.projectLocal") }}</dt>
      <dd :class="prefs?.project_local ? 'text-status-warning' : ''">{{ fmt(prefs?.project_local) }}</dd>
      <dt class="text-muted-foreground">{{ t("editorPref.projectShared") }}</dt>
      <dd :class="prefs?.project_shared ? 'text-status-warning' : ''">{{ fmt(prefs?.project_shared) }}</dd>
    </dl>
    <p v-if="prefs?.project_local || prefs?.project_shared" class="mt-2 text-xs text-status-warning">
      {{ t("editorPref.projectOverridesNote") }}
    </p>
  </section>
</template>
```

Export from `src/components/primitives/index.ts`. Add the locale keys in zh.ts and en.ts.

- [ ] **Step 2: Tauri command**

In `src-tauri/src/commands/ini_scanner.rs` add a new command `read_editor_prefs(machine_id) -> EditorDdcPrefs` that runs the existing scanner and pipes the User-category parsed file through `editor_preferences::extract`.

- [ ] **Step 3: Wire UI**

Embed `<UecmEditorPrefBlock />` into the machine detail panel in `Machines.vue` (look for the existing detail section that lives behind `expandedId`).

- [ ] **Step 4: Commit**

```bash
git add src/components/primitives/UecmEditorPrefBlock.vue src/components/primitives/index.ts src/views/Machines.vue src/locales/zh.ts src/locales/en.ts src-tauri/src/commands/ini_scanner.rs
git commit -m "feat(ddc): UI surface for 4 Editor Pref DDC paths with override warning"
```

---

### Module M7: PSO completeness (1 day)

**Goal:** Cover `*.stablepc.csv` alongside `.upipelinecache` so SOP step 4 ("deploy PSO Cache") moves a complete set.

**Files:**
- Modify: `src-tauri/src/core/pso_collect.rs`
- Modify: `src-tauri/src/core/pso_distribute.rs` (extend file glob)
- Modify: `ps-scripts/distribute-pso-cache.ps1` (add second glob)

**Note:** R024 (`r.ShaderPipelineCache.Enabled`) already landed in M1.3.

**Dependencies:** None.

---

#### Task M7.1: Extend collect to capture both extensions

**Files:**
- Modify: `src-tauri/src/core/pso_collect.rs`

- [ ] **Step 1: Add failing test**

```rust
#[test]
fn collects_both_upipelinecache_and_stablepc_csv() {
    let dir = tempfile::tempdir().unwrap();
    let saved = dir.path().join("Saved").join("CollectedPSOs");
    std::fs::create_dir_all(&saved).unwrap();
    std::fs::write(saved.join("a.upipelinecache"), b"x").unwrap();
    std::fs::write(saved.join("a.stablepc.csv"),  b"x,y").unwrap();
    std::fs::write(saved.join("unrelated.txt"),   b"-").unwrap();

    let collected = scan_collected_files(dir.path().to_str().unwrap()).unwrap();
    assert!(collected.iter().any(|c| c.file_name.ends_with(".upipelinecache")));
    assert!(collected.iter().any(|c| c.file_name.ends_with(".stablepc.csv")));
    assert_eq!(collected.len(), 2);
}
```

- [ ] **Step 2: Replace the extension check**

In `src-tauri/src/core/pso_collect.rs`, find the line (≈147) that checks `extension == Some("upipelinecache")`. Replace the filter:

```rust
let ext = path.extension().and_then(|v| v.to_str()).unwrap_or("");
let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
let is_target = ext.eq_ignore_ascii_case("upipelinecache")
    || name.to_lowercase().ends_with(".stablepc.csv");
if !is_target { continue; }
```

(`extension()` strips at the last dot, so `*.stablepc.csv` matches on `csv` — check the full filename suffix instead.)

- [ ] **Step 3: Update the file-collected event shape**

The existing `CollectedFile` struct exposes `file_name` and `size`; no schema change needed.

- [ ] **Step 4: Run + commit**

```bash
cd src-tauri && cargo test --lib core::pso_collect 2>&1 | tail -10
git add src-tauri/src/core/pso_collect.rs
git commit -m "feat(pso): also collect *.stablepc.csv alongside *.upipelinecache"
```

---

#### Task M7.2: Distribute both file types

**Files:**
- Modify: `ps-scripts/distribute-pso-cache.ps1`
- Modify: `src-tauri/src/core/pso_distribute.rs` (the `file_glob`)

- [ ] **Step 1: Update PS script**

In `ps-scripts/distribute-pso-cache.ps1`, find the `$roboArgs` `*.upipelinecache` token and change to `*.upipelinecache *.stablepc.csv` (Robocopy accepts multiple file-glob tokens).

- [ ] **Step 2: Update Rust glob field if exposed**

Search for `*.upipelinecache` in `src-tauri/src/core/pso_distribute.rs` and replace with the same multi-glob string.

- [ ] **Step 3: Commit**

```bash
git add ps-scripts/distribute-pso-cache.ps1 src-tauri/src/core/pso_distribute.rs
git commit -m "feat(pso): distribute *.stablepc.csv alongside *.upipelinecache"
```

---

### Module M8: Cluster consistency check (3 days)

**Goal:** A cross-machine probe that compares UE version, installed plugins, RHI default, GPU/Driver, and project path across N hosts and flags mismatches.

**Files:**
- Create: `ps-scripts/consistency-snapshot.ps1`
- Create: `src-tauri/src/core/consistency_check.rs`
- Modify: `src-tauri/src/cli/args.rs`, `src-tauri/src/cli/domain_health.rs`
- Create: `src-tauri/src/commands/consistency.rs`
- Create: `src/components/diagnostics/ConsistencyReport.vue`

**Dependencies:** None (uses existing GPU consistency module as reference; reads UE version via existing `discovery` module).

---

#### Task M8.1: Per-host snapshot script

**Files:**
- Create: `ps-scripts/consistency-snapshot.ps1`

- [ ] **Step 1: Script body**

```powershell
# Single-host snapshot of UE installs, RenderStream plugin version, default RHI,
# GPU/Driver, and project paths on common drives. JSON to stdout.
param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [string]$Username,
    [string]$Password
)
$ErrorActionPreference = 'Stop'

$script = {
    # UE installs: read registry
    $ueInstalls = @()
    $keyPaths = @('HKLM:\SOFTWARE\EpicGames\Unreal Engine', 'HKLM:\SOFTWARE\WOW6432Node\EpicGames\Unreal Engine')
    foreach ($p in $keyPaths) {
        if (Test-Path $p) {
            $versions = Get-ChildItem $p -ErrorAction SilentlyContinue
            foreach ($v in $versions) {
                $installed = (Get-ItemProperty -Path $v.PSPath -Name 'InstalledDirectory' -ErrorAction SilentlyContinue).InstalledDirectory
                if ($installed) {
                    $ueInstalls += [pscustomobject]@{
                        Version = $v.PSChildName
                        Path = $installed
                    }
                }
            }
        }
    }

    # GPU/Driver from Win32_VideoController
    $gpu = Get-CimInstance Win32_VideoController -ErrorAction SilentlyContinue | Select-Object -First 1
    $gpuInfo = if ($gpu) {
        [pscustomobject]@{
            Name = $gpu.Name; Driver = $gpu.DriverVersion; DriverDate = "$($gpu.DriverDate)"
        }
    } else { $null }

    # Default RHI from CurrentUser preference (best effort)
    $rhi = $null
    try {
        $defaultGraphicsRHI = Get-ItemProperty -Path 'HKCU:\Software\Epic Games\Unreal Engine\Settings' -Name 'DefaultGraphicsRHI' -ErrorAction SilentlyContinue
        if ($defaultGraphicsRHI) { $rhi = $defaultGraphicsRHI.DefaultGraphicsRHI }
    } catch {}

    # Project root candidates
    $projectDirs = @()
    foreach ($drive in @('C:', 'D:', 'E:', 'F:')) {
        $candidates = @("$drive\Projects", "$drive\RenderStream Projects", "$drive\Unreal Projects")
        foreach ($c in $candidates) {
            if (Test-Path -LiteralPath $c) {
                $children = Get-ChildItem -LiteralPath $c -Directory -ErrorAction SilentlyContinue | Select-Object -First 50
                foreach ($child in $children) {
                    $uproject = Get-ChildItem -LiteralPath $child.FullName -Filter '*.uproject' -ErrorAction SilentlyContinue | Select-Object -First 1
                    if ($uproject) {
                        $projectDirs += [pscustomobject]@{ Path = $child.FullName; UProject = $uproject.Name }
                    }
                }
            }
        }
    }

    # RenderStream plugin version (look for d3 install)
    $rsVersion = $null
    try {
        $d3 = Get-ItemProperty -Path 'HKLM:\SOFTWARE\d3 Technologies\d3 Production Suite' -ErrorAction SilentlyContinue
        if ($d3 -and $d3.Version) { $rsVersion = $d3.Version }
    } catch {}

    @{
        ue_installs = $ueInstalls
        gpu = $gpuInfo
        rhi = $rhi
        projects = $projectDirs
        renderstream_version = $rsVersion
        host = $env:COMPUTERNAME
    }
}

try {
    $result = if ($Username) {
        $pass = ConvertTo-SecureString $Password -AsPlainText -Force
        $cred = New-Object System.Management.Automation.PSCredential($Username, $pass)
        Invoke-Command -ComputerName $HostName -Credential $cred -Authentication Default -ScriptBlock $script
    } else {
        Invoke-Command -ComputerName $HostName -ScriptBlock $script
    }
    @{ ok = $true; data = $result } | ConvertTo-Json -Compress -Depth 6
} catch {
    @{ ok = $false; message = $_.Exception.Message } | ConvertTo-Json -Compress
}
```

- [ ] **Step 2: Commit**

```bash
git add ps-scripts/consistency-snapshot.ps1
git commit -m "feat(consistency): per-host snapshot script (UE/GPU/RHI/projects)"
```

---

#### Task M8.2: Rust comparator

**Files:**
- Create: `src-tauri/src/core/consistency_check.rs`

- [ ] **Step 1: Test + types**

```rust
// src-tauri/src/core/consistency_check.rs
use crate::core::powershell;
use crate::error::{UecmError, UecmResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UeInstall { #[serde(rename = "Version")] pub version: String, #[serde(rename = "Path")] pub path: String }
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GpuInfo { #[serde(rename = "Name")] pub name: String, #[serde(rename = "Driver")] pub driver: String, #[serde(rename = "DriverDate")] pub driver_date: String }
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProjectDir { #[serde(rename = "Path")] pub path: String, #[serde(rename = "UProject")] pub uproject: String }

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HostSnapshot {
    pub host: String,
    pub ue_installs: Vec<UeInstall>,
    pub gpu: Option<GpuInfo>,
    pub rhi: Option<String>,
    pub projects: Vec<ProjectDir>,
    pub renderstream_version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ScriptResult { ok: bool, data: Option<HostSnapshot>, message: Option<String> }

#[derive(Debug, Clone, Serialize)]
pub enum Inconsistency {
    UeVersionMismatch { found: BTreeMap<String, Vec<String>> },          // version -> hosts
    RenderStreamVersionMismatch { found: BTreeMap<String, Vec<String>> },
    RhiMismatch { found: BTreeMap<String, Vec<String>> },
    GpuModelMismatch { found: BTreeMap<String, Vec<String>> },
    GpuDriverMismatch { found: BTreeMap<String, Vec<String>> },
    MissingUe { hosts: Vec<String> },
}

pub fn snapshot(host: &str, creds: Option<(&str, &str)>) -> UecmResult<HostSnapshot> {
    let mut args: Vec<&str> = vec!["-HostName", host];
    if let Some((u, p)) = creds { args.extend(["-Username", u, "-Password", p]); }
    let r: ScriptResult = powershell::run_json(
        &powershell::script_path("consistency-snapshot.ps1"), &args,
    )?;
    if !r.ok { return Err(UecmError::OperationFailed(r.message.unwrap_or_default())); }
    let mut snap = r.data.ok_or_else(|| UecmError::OperationFailed("no data".into()))?;
    snap.host = host.to_string();
    Ok(snap)
}

pub fn compare(snaps: &[HostSnapshot]) -> Vec<Inconsistency> {
    let mut out = Vec::new();

    // UE versions (highest installed per host)
    let mut ue_versions: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut missing_ue: Vec<String> = Vec::new();
    for s in snaps {
        if let Some(latest) = s.ue_installs.iter().map(|u| u.version.clone()).max() {
            ue_versions.entry(latest).or_default().push(s.host.clone());
        } else {
            missing_ue.push(s.host.clone());
        }
    }
    if ue_versions.len() > 1 { out.push(Inconsistency::UeVersionMismatch { found: ue_versions }); }
    if !missing_ue.is_empty() { out.push(Inconsistency::MissingUe { hosts: missing_ue }); }

    // RS version
    let mut rs: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for s in snaps { rs.entry(s.renderstream_version.clone().unwrap_or_else(|| "(none)".into())).or_default().push(s.host.clone()); }
    if rs.len() > 1 { out.push(Inconsistency::RenderStreamVersionMismatch { found: rs }); }

    // RHI
    let mut rhi: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for s in snaps { rhi.entry(s.rhi.clone().unwrap_or_else(|| "(default)".into())).or_default().push(s.host.clone()); }
    if rhi.len() > 1 { out.push(Inconsistency::RhiMismatch { found: rhi }); }

    // GPU model + driver
    let mut models: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut drivers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for s in snaps {
        let g = s.gpu.as_ref();
        models.entry(g.map(|g| g.name.clone()).unwrap_or_else(|| "(unknown)".into())).or_default().push(s.host.clone());
        drivers.entry(g.map(|g| g.driver.clone()).unwrap_or_else(|| "(unknown)".into())).or_default().push(s.host.clone());
    }
    if models.len() > 1 { out.push(Inconsistency::GpuModelMismatch { found: models }); }
    if drivers.len() > 1 { out.push(Inconsistency::GpuDriverMismatch { found: drivers }); }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(host: &str, ue: &str, gpu_name: &str, drv: &str, rs: Option<&str>) -> HostSnapshot {
        HostSnapshot {
            host: host.into(),
            ue_installs: vec![UeInstall { version: ue.into(), path: "C:\\UE".into() }],
            gpu: Some(GpuInfo { name: gpu_name.into(), driver: drv.into(), driver_date: "".into() }),
            rhi: Some("D3D12".into()),
            projects: vec![],
            renderstream_version: rs.map(String::from),
        }
    }

    #[test]
    fn matched_cluster_returns_no_findings() {
        let snaps = vec![
            snap("A", "5.5", "RTX 4090", "555.85", Some("r24")),
            snap("B", "5.5", "RTX 4090", "555.85", Some("r24")),
        ];
        assert!(compare(&snaps).is_empty());
    }

    #[test]
    fn detects_ue_version_mismatch() {
        let snaps = vec![
            snap("A", "5.5", "RTX 4090", "555.85", Some("r24")),
            snap("B", "5.4", "RTX 4090", "555.85", Some("r24")),
        ];
        let f = compare(&snaps);
        assert!(f.iter().any(|x| matches!(x, Inconsistency::UeVersionMismatch { .. })));
    }

    #[test]
    fn detects_driver_mismatch() {
        let snaps = vec![
            snap("A", "5.5", "RTX 4090", "555.85", Some("r24")),
            snap("B", "5.5", "RTX 4090", "537.42", Some("r24")),
        ];
        let f = compare(&snaps);
        assert!(f.iter().any(|x| matches!(x, Inconsistency::GpuDriverMismatch { .. })));
    }
}
```

- [ ] **Step 2: Register + commit**

```bash
cd src-tauri && cargo test --lib core::consistency_check 2>&1 | tail -10
```

`core/mod.rs` add `pub mod consistency_check;`.

```bash
git add src-tauri/src/core/consistency_check.rs src-tauri/src/core/mod.rs
git commit -m "feat(consistency): snapshot + compare across N hosts"
```

---

#### Task M8.3: CLI + Tauri command + UI

**Files:**
- Modify: `src-tauri/src/cli/args.rs`, `src-tauri/src/cli/domain_health.rs`
- Create: `src-tauri/src/commands/consistency.rs`
- Create: `src/components/diagnostics/ConsistencyReport.vue`

- [ ] **Step 1: CLI subcommand**

In `args.rs` `HealthAction`:

```rust
/// Snapshot N hosts and report cross-machine inconsistencies.
ConsistencyCheck {
    #[arg(long, value_name = "H1,H2,...", value_delimiter = ',')]
    hosts: Vec<String>,
    #[command(flatten)]
    cred: crate::cli::credential_args::CredentialArgs,
},
```

In `domain_health.rs` handle it:

```rust
HealthAction::ConsistencyCheck { hosts, cred } => {
    let db = ctx.require_db()?;
    let creds = cred.resolve(db)?;
    let mut snaps = Vec::new();
    for h in &hosts {
        snaps.push(crate::core::consistency_check::snapshot(
            h, creds.as_ref().map(|(u, p)| (u.as_str(), p.as_str()))
        )?);
    }
    let findings = crate::core::consistency_check::compare(&snaps);
    ctx.emitter.emit_result(&serde_json::json!({ "snapshots": snaps, "inconsistencies": findings })).ok();
    Ok(())
}
```

- [ ] **Step 2: Tauri command**

```rust
// src-tauri/src/commands/consistency.rs
use crate::core::consistency_check::{self, HostSnapshot, Inconsistency};

#[tauri::command]
pub async fn run_consistency_check(
    hosts: Vec<String>,
    cred_alias: Option<String>,
) -> Result<(Vec<HostSnapshot>, Vec<Inconsistency>), String> {
    let creds = match cred_alias.as_deref() {
        Some(a) if !a.is_empty() => {
            let db = crate::data::db::open().map_err(|e: crate::error::UecmError| e.to_string())?;
            Some(crate::data::credentials::load_decrypted(&db, a).map_err(|e: crate::error::UecmError| e.to_string())?)
        }
        _ => None,
    };
    tokio::task::spawn_blocking(move || -> Result<_, crate::error::UecmError> {
        let mut snaps = Vec::new();
        for h in &hosts {
            snaps.push(consistency_check::snapshot(h, creds.as_ref().map(|(u, p)| (u.as_str(), p.as_str())))?);
        }
        let inc = consistency_check::compare(&snaps);
        Ok((snaps, inc))
    })
    .await.map_err(|e| e.to_string())?.map_err(|e| e.to_string())
}
```

Register in `commands/mod.rs` + `lib.rs` invoke_handler.

- [ ] **Step 3: UI component**

```vue
<!-- src/components/diagnostics/ConsistencyReport.vue -->
<script setup lang="ts">
import { useI18n } from "vue-i18n";

interface Mismatch { kind: string; found: Record<string, string[]> }

defineProps<{
  inconsistencies: Array<{ [k: string]: any }>;
}>();
const { t } = useI18n();

function tagOf(x: any): string {
  return Object.keys(x).find((k) => k !== "kind") ?? Object.keys(x)[0];
}
</script>

<template>
  <section v-if="inconsistencies.length === 0" class="rounded-md border border-status-healthy/40 bg-status-healthy/10 p-3 text-status-healthy text-sm">
    {{ t("consistency.allMatch") }}
  </section>
  <section v-else class="space-y-3">
    <article v-for="(inc, i) in inconsistencies" :key="i" class="rounded-md border border-status-warning/40 bg-card p-3 text-sm">
      <h4 class="font-display text-base mb-2">{{ t(`consistency.kind.${tagOf(inc)}`) }}</h4>
      <ul class="space-y-1">
        <li v-for="(hosts, value) in (inc as any)[tagOf(inc)].found ?? (inc as any).hosts" :key="value as string">
          <span class="font-mono">{{ value }}</span> — {{ (hosts as string[]).join(", ") }}
        </li>
      </ul>
    </article>
  </section>
</template>
```

Add locale keys for `consistency.allMatch` and `consistency.kind.{UeVersionMismatch, GpuModelMismatch, ...}`.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/cli/args.rs src-tauri/src/cli/domain_health.rs src-tauri/src/commands/consistency.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs src/components/diagnostics/ConsistencyReport.vue src/locales/zh.ts src/locales/en.ts
git commit -m "feat(consistency): cli + Tauri command + UI for cross-machine consistency"
```

---

### Module M9: GC strategy quick toggles (1 day)

**Goal:** Read `DeleteUnused` / `UnusedFileAge` from BackendGraph and expose a 1-click "Pause GC" / "Resume GC" toggle. This rides on M1 (`set_backend_field_with_credential`).

**Files:**
- Modify: `src-tauri/src/cli/domain_ini.rs` (new subcommand)
- Modify: `src-tauri/src/cli/args.rs`
- Create: `src/components/modals/GcToggleModal.vue`

**Dependencies:** M1.

---

#### Task M9.1: CLI `ini gc pause|resume`

- [ ] **Step 1: Add IniAction variants**

```rust
// in args.rs IniAction:
/// Pause Shared DDC GC (DeleteUnused=false). Reversible with `gc-resume`.
GcPause {
    #[command(flatten)]
    target: crate::cli::host_args::HostArgs,
    #[arg(long)] project_id: i64,
    #[arg(long)] yes: bool,
    #[arg(long)] dry_run: bool,
    #[command(flatten)] cred: crate::cli::credential_args::CredentialArgs,
},
/// Resume Shared DDC GC (DeleteUnused=true, UnusedFileAge configurable).
GcResume {
    #[command(flatten)]
    target: crate::cli::host_args::HostArgs,
    #[arg(long)] project_id: i64,
    #[arg(long, default_value_t = 10)] unused_file_age: u32,
    #[arg(long)] yes: bool,
    #[arg(long)] dry_run: bool,
    #[command(flatten)] cred: crate::cli::credential_args::CredentialArgs,
},
```

- [ ] **Step 2: Handler**

In `src-tauri/src/cli/domain_ini.rs`:

```rust
IniAction::GcPause { target, project_id, yes, dry_run, cred } => {
    let hosts = target.require_one()?.as_vec();
    let outcome = destructive::check(yes, dry_run, "ini.gc-pause")?;
    let db = ctx.require_db()?;
    cred.preflight(db)?;
    if outcome == Outcome::DryRun { /* emit plan */ return Ok(()); }
    let creds = cred.resolve(db)?.ok_or_else(|| UecmError::InvalidInput("creds required".into()))?;
    let project = crate::data::projects::get(db, project_id)?
        .ok_or_else(|| UecmError::InvalidInput("project not found".into()))?;
    let ini = format!(r"{}\Config\DefaultEngine.ini", project.path.trim_end_matches('\\'));
    for host in &hosts {
        crate::core::ini_editor::set_backend_field_with_credential(
            host, &ini, "DerivedDataBackendGraph", "Shared",
            "DeleteUnused", "false", &creds.0, &creds.1,
        )?;
    }
    Ok(())
}
IniAction::GcResume { target, project_id, unused_file_age, yes, dry_run, cred } => {
    // same shape; set DeleteUnused=true + UnusedFileAge=<n>
    todo!()
}
```

Implement `GcResume` analogously (two `set_backend_field_with_credential` calls per host).

You will also need a helper `HostTarget::as_vec()`:

```rust
impl HostTarget {
    pub fn as_vec(self) -> Vec<String> {
        match self { HostTarget::Single(h) => vec![h], HostTarget::Batch(hs) => hs }
    }
}
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/cli/args.rs src-tauri/src/cli/domain_ini.rs src-tauri/src/cli/host_args.rs
git commit -m "feat(ddc): cli ini gc-pause / gc-resume toggles DeleteUnused"
```

---

### Module M10: Local-vs-Shared file stats probe (2 days)

**Goal:** Periodically report file count + total size on each layer to catch "Shared DDC nearly empty while Local has thousands of files" — the SOP case-study symptom.

**Files:**
- Create: `ps-scripts/ddc-file-stats.ps1`
- Create: `src-tauri/src/core/ddc_file_stats.rs`
- Modify: `ps-scripts/health-probes.ps1` (call into ddc-file-stats inline for round-trip mode)

**Dependencies:** None.

---

#### Task M10.1: PS one-shot file stats

**Files:**
- Create: `ps-scripts/ddc-file-stats.ps1`

```powershell
# Returns {file_count, total_bytes} for one or two paths.
param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [string]$LocalPath = "",
    [string]$SharedPath = "",
    [string]$Username,
    [string]$Password
)
$ErrorActionPreference = 'Stop'
$script = {
    param($LocalPath, $SharedPath)
    function StatPath($p) {
        if ([string]::IsNullOrEmpty($p)) { return @{ path = ""; ok = $false; file_count = 0; total_bytes = 0; error = "empty" } }
        try {
            if (-not (Test-Path -LiteralPath $p)) { return @{ path = $p; ok = $false; file_count = 0; total_bytes = 0; error = "not found" } }
            $files = Get-ChildItem -LiteralPath $p -Recurse -Force -File -ErrorAction SilentlyContinue
            $count = ($files | Measure-Object).Count
            $bytes = ($files | Measure-Object Length -Sum).Sum
            if (-not $bytes) { $bytes = 0 }
            @{ path = $p; ok = $true; file_count = $count; total_bytes = [int64]$bytes }
        } catch {
            @{ path = $p; ok = $false; error = $_.Exception.Message; file_count = 0; total_bytes = 0 }
        }
    }
    @{
        local  = (StatPath $LocalPath)
        shared = (StatPath $SharedPath)
    }
}
try {
    $r = if ($Username) {
        $pass = ConvertTo-SecureString $Password -AsPlainText -Force
        $cred = New-Object System.Management.Automation.PSCredential($Username, $pass)
        Invoke-Command -ComputerName $HostName -Credential $cred -Authentication Default -ScriptBlock $script -ArgumentList $LocalPath, $SharedPath
    } else {
        Invoke-Command -ComputerName $HostName -ScriptBlock $script -ArgumentList $LocalPath, $SharedPath
    }
    @{ ok = $true; local = $r.local; shared = $r.shared } | ConvertTo-Json -Compress -Depth 5
} catch {
    @{ ok = $false; message = $_.Exception.Message } | ConvertTo-Json -Compress
}
```

Commit.

---

#### Task M10.2: Rust wrapper + symptom emit

**Files:**
- Create: `src-tauri/src/core/ddc_file_stats.rs`

- [ ] **Step 1: Test + impl**

```rust
// src-tauri/src/core/ddc_file_stats.rs
use crate::core::powershell;
use crate::error::{UecmError, UecmResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LayerStat {
    pub path: String, pub ok: bool,
    #[serde(default)] pub file_count: u64,
    #[serde(default)] pub total_bytes: u64,
    #[serde(default)] pub error: Option<String>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Stats {
    #[serde(default)] pub ok: bool,
    pub local: LayerStat,
    pub shared: LayerStat,
}

pub fn run(host: &str, local_path: &str, shared_path: &str, creds: Option<(&str, &str)>) -> UecmResult<Stats> {
    let mut args: Vec<&str> = vec!["-HostName", host, "-LocalPath", local_path, "-SharedPath", shared_path];
    if let Some((u, p)) = creds { args.extend(["-Username", u, "-Password", p]); }
    let s: Stats = powershell::run_json(&powershell::script_path("ddc-file-stats.ps1"), &args)?;
    if !s.ok { return Err(UecmError::OperationFailed("ddc-file-stats failed".into())); }
    Ok(s)
}

#[derive(Debug, Clone, Serialize)]
pub struct ImbalanceFinding {
    pub local_count: u64, pub shared_count: u64,
    pub local_bytes: u64, pub shared_bytes: u64,
    pub severity: &'static str,
    pub message: String,
}

pub fn classify_imbalance(s: &Stats) -> Option<ImbalanceFinding> {
    if !s.local.ok || !s.shared.ok { return None; }
    if s.local.file_count == 0 { return None; }
    let ratio = (s.shared.file_count as f64) / (s.local.file_count as f64);
    if ratio < 0.05 && s.local.file_count > 500 {
        return Some(ImbalanceFinding {
            local_count: s.local.file_count, shared_count: s.shared.file_count,
            local_bytes: s.local.total_bytes, shared_bytes: s.shared.total_bytes,
            severity: "critical",
            message: format!("Shared DDC has {} files vs Local {} ({}× lower). Likely the first host that opened the project did so before Shared was configured — re-open the project on that host or generate a DDC Pak.",
                s.shared.file_count, s.local.file_count, (1.0 / ratio.max(1e-9)) as u64),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    fn stat(ok: bool, n: u64) -> LayerStat { LayerStat { path: "p".into(), ok, file_count: n, total_bytes: n * 1024, error: None } }

    #[test]
    fn flags_classic_imbalance_case() {
        let s = Stats { ok: true, local: stat(true, 25065), shared: stat(true, 152) };
        let f = classify_imbalance(&s).unwrap();
        assert_eq!(f.severity, "critical");
    }
    #[test]
    fn silent_when_balanced() {
        let s = Stats { ok: true, local: stat(true, 25065), shared: stat(true, 24000) };
        assert!(classify_imbalance(&s).is_none());
    }
    #[test]
    fn silent_when_either_layer_failed() {
        let s = Stats { ok: true, local: stat(true, 25065), shared: stat(false, 0) };
        assert!(classify_imbalance(&s).is_none());
    }
}
```

- [ ] **Step 2: Register + commit**

```bash
cd src-tauri && cargo test --lib core::ddc_file_stats 2>&1 | tail -10
git add src-tauri/src/core/ddc_file_stats.rs src-tauri/src/core/mod.rs ps-scripts/ddc-file-stats.ps1
git commit -m "feat(ddc): file-stats probe + Local-vs-Shared imbalance classifier"
```

---

### Module M11: Command-line argument scanner (2 days)

**Goal:** Detect `-LocalDataCachePath=` / `-SharedDataCachePath=` baked into shortcuts (`*.lnk`), bat scripts, and Windows Service ImagePath. SOP calls out that RenderStream's startup command line silently overrides everything else.

**Files:**
- Create: `ps-scripts/scan-command-line-args.ps1`
- Create: `src-tauri/src/core/command_line_scanner.rs`
- Create: a new IniAction or HealthAction subcommand

**Dependencies:** None.

---

#### Task M11.1: PS scanner

**Files:**
- Create: `ps-scripts/scan-command-line-args.ps1`

```powershell
# Scans Desktop + Public Desktop + Start Menu shortcuts, common .bat folders,
# and all installed Win32_Service ImagePaths for -LocalDataCachePath= and
# -SharedDataCachePath= command-line arguments.
param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [string]$Username,
    [string]$Password
)
$ErrorActionPreference = 'Stop'
$script = {
    function MatchArgs($cmd) {
        $out = @{}
        $patterns = @{
            local  = '-LocalDataCachePath=("[^"]+"|[^\s]+)'
            shared = '-SharedDataCachePath=("[^"]+"|[^\s]+)'
        }
        foreach ($k in $patterns.Keys) {
            $m = [regex]::Match($cmd, $patterns[$k], 'IgnoreCase')
            if ($m.Success) { $out[$k] = ($m.Groups[1].Value).Trim('"') }
        }
        $out
    }

    $findings = New-Object System.Collections.Generic.List[object]

    # Shortcuts
    $shortcutRoots = @(
        [Environment]::GetFolderPath('Desktop'),
        [Environment]::GetFolderPath('CommonDesktopDirectory'),
        [Environment]::GetFolderPath('Programs'),
        [Environment]::GetFolderPath('CommonPrograms')
    )
    $shell = New-Object -ComObject WScript.Shell
    foreach ($root in $shortcutRoots) {
        if (-not $root -or -not (Test-Path -LiteralPath $root)) { continue }
        Get-ChildItem -LiteralPath $root -Recurse -Filter *.lnk -ErrorAction SilentlyContinue | ForEach-Object {
            try {
                $lnk = $shell.CreateShortcut($_.FullName)
                $cmd = "$($lnk.TargetPath) $($lnk.Arguments)"
                $hits = MatchArgs $cmd
                if ($hits.Count -gt 0) {
                    $findings.Add([pscustomobject]@{ source='shortcut'; path=$_.FullName; cmd=$cmd; matches=$hits })
                }
            } catch {}
        }
    }

    # BAT files in well-known locations
    $batRoots = @('C:\Tools', 'C:\Scripts', "$env:USERPROFILE\Desktop")
    foreach ($root in $batRoots) {
        if (-not (Test-Path -LiteralPath $root)) { continue }
        Get-ChildItem -LiteralPath $root -Recurse -Filter *.bat -ErrorAction SilentlyContinue | ForEach-Object {
            try {
                $body = Get-Content -LiteralPath $_.FullName -Raw -Encoding UTF8
                $hits = MatchArgs $body
                if ($hits.Count -gt 0) {
                    $findings.Add([pscustomobject]@{ source='bat'; path=$_.FullName; cmd=$body.Substring(0, [Math]::Min(400, $body.Length)); matches=$hits })
                }
            } catch {}
        }
    }

    # Services
    Get-CimInstance Win32_Service -ErrorAction SilentlyContinue | ForEach-Object {
        $cmd = $_.PathName
        $hits = MatchArgs $cmd
        if ($hits.Count -gt 0) {
            $findings.Add([pscustomobject]@{ source='service'; name=$_.Name; path=$cmd; matches=$hits })
        }
    }

    @{ findings = $findings }
}

try {
    $r = if ($Username) {
        $pass = ConvertTo-SecureString $Password -AsPlainText -Force
        $cred = New-Object System.Management.Automation.PSCredential($Username, $pass)
        Invoke-Command -ComputerName $HostName -Credential $cred -Authentication Default -ScriptBlock $script
    } else { Invoke-Command -ComputerName $HostName -ScriptBlock $script }
    @{ ok = $true; findings = @($r.findings) } | ConvertTo-Json -Compress -Depth 6
} catch {
    @{ ok = $false; message = $_.Exception.Message; findings = @() } | ConvertTo-Json -Compress
}
```

Commit.

---

#### Task M11.2: Rust wrapper + CLI

**Files:**
- Create: `src-tauri/src/core/command_line_scanner.rs`
- Modify: `src-tauri/src/cli/args.rs`, `src-tauri/src/cli/domain_health.rs`

- [ ] **Step 1: Type + invoke**

```rust
// src-tauri/src/core/command_line_scanner.rs
use crate::core::powershell;
use crate::error::{UecmError, UecmResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CmdLineHit {
    pub source: String,                       // "shortcut" | "bat" | "service"
    #[serde(default)] pub name: Option<String>,
    pub path: String,
    #[serde(default)] pub cmd: Option<String>,
    pub matches: BTreeMap<String, String>,    // "local" | "shared" -> value
}

#[derive(Debug, Deserialize)]
struct ScriptResult { ok: bool, findings: Option<Vec<CmdLineHit>>, message: Option<String> }

pub fn scan(host: &str, creds: Option<(&str, &str)>) -> UecmResult<Vec<CmdLineHit>> {
    let mut args: Vec<&str> = vec!["-HostName", host];
    if let Some((u, p)) = creds { args.extend(["-Username", u, "-Password", p]); }
    let r: ScriptResult = powershell::run_json(
        &powershell::script_path("scan-command-line-args.ps1"), &args)?;
    if !r.ok { return Err(UecmError::OperationFailed(r.message.unwrap_or_default())); }
    Ok(r.findings.unwrap_or_default())
}
```

- [ ] **Step 2: CLI subcommand**

In `args.rs` `HealthAction`:

```rust
/// Scan shortcuts/bat/services for -LocalDataCachePath / -SharedDataCachePath overrides.
ScanCommandLine {
    #[arg(long)]
    host: String,
    #[command(flatten)] cred: crate::cli::credential_args::CredentialArgs,
},
```

`domain_health.rs`:

```rust
HealthAction::ScanCommandLine { host, cred } => {
    let db = ctx.require_db()?;
    let creds = cred.resolve(db)?;
    let hits = crate::core::command_line_scanner::scan(
        &host, creds.as_ref().map(|(u, p)| (u.as_str(), p.as_str())))?;
    ctx.emitter.emit_result(&hits).ok();
    Ok(())
}
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/core/command_line_scanner.rs src-tauri/src/core/mod.rs src-tauri/src/cli/args.rs src-tauri/src/cli/domain_health.rs ps-scripts/scan-command-line-args.ps1
git commit -m "feat(ddc): scan shortcuts/bat/services for -LocalDataCachePath overrides"
```

---

## Phase 3 — P2 (Nice-to-have, ~3.5 working days)

### Module M12: Baseline log snapshot (1 day)

**Goal:** "Freeze" a pre-show snapshot of DDC/Shader/PSO logs from each host to local disk so post-show triage can compare against it.

**Files:**
- Create: `ps-scripts/snapshot-baseline-logs.ps1`
- Create: `src-tauri/src/core/baseline_log_snapshot.rs`
- Modify: `src-tauri/src/cli/args.rs`, `src-tauri/src/cli/domain_log.rs`

**Dependencies:** None.

---

#### Task M12.1: PS pull script

**Files:**
- Create: `ps-scripts/snapshot-baseline-logs.ps1`

```powershell
# Copies UE log files from a project's Saved/Logs and any RenderStream logs
# into a local archive directory tagged with a timestamp.
param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$ProjectDir,
    [Parameter(Mandatory=$true)] [string]$ArchiveDest,
    [string]$Username,
    [string]$Password
)
$ErrorActionPreference = 'Stop'
$script = {
    param($ProjectDir, $ArchiveDest)
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $dest = Join-Path $ArchiveDest "$($env:COMPUTERNAME)-$stamp"
    New-Item -ItemType Directory -Path $dest -Force | Out-Null

    $sources = @(
        (Join-Path $ProjectDir 'Saved\Logs'),
        "$env:ProgramData\d3\logs",
        "$env:ProgramData\disguise\logs"
    )
    $copied = New-Object System.Collections.Generic.List[string]
    foreach ($src in $sources) {
        if (-not (Test-Path -LiteralPath $src)) { continue }
        Get-ChildItem -LiteralPath $src -Recurse -File -ErrorAction SilentlyContinue | ForEach-Object {
            try {
                $rel = $_.FullName.Substring($src.Length).TrimStart('\','/')
                $target = Join-Path $dest "$($_.Directory.Name)__$rel"
                Copy-Item -LiteralPath $_.FullName -Destination $target -Force
                $copied.Add($_.FullName) | Out-Null
            } catch {}
        }
    }
    @{ archive = $dest; copied = $copied }
}
try {
    $r = if ($Username) {
        $pass = ConvertTo-SecureString $Password -AsPlainText -Force
        $cred = New-Object System.Management.Automation.PSCredential($Username, $pass)
        Invoke-Command -ComputerName $HostName -Credential $cred -Authentication Default -ScriptBlock $script -ArgumentList $ProjectDir, $ArchiveDest
    } else { Invoke-Command -ComputerName $HostName -ScriptBlock $script -ArgumentList $ProjectDir, $ArchiveDest }
    @{ ok = $true; archive = $r.archive; copied_count = @($r.copied).Count } | ConvertTo-Json -Compress
} catch {
    @{ ok = $false; message = $_.Exception.Message } | ConvertTo-Json -Compress
}
```

Note: this script runs the copy *on the remote host* and writes into a path that the remote host sees as `ArchiveDest`. For a UECM-controller-side archive you must pass a UNC path (e.g. `\\UECM-CTRL\baselines`) and grant the operator account write access there. Document this in the CLI help.

- [ ] Commit.

---

#### Task M12.2: Rust wrapper + CLI

**Files:**
- Create: `src-tauri/src/core/baseline_log_snapshot.rs`
- Modify: `src-tauri/src/cli/args.rs`, `src-tauri/src/cli/domain_log.rs`

```rust
// src-tauri/src/core/baseline_log_snapshot.rs
use crate::core::powershell;
use crate::error::{UecmError, UecmResult};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Resp { ok: bool, archive: Option<String>, copied_count: Option<u64>, message: Option<String> }

pub fn run(host: &str, project_dir: &str, archive_dest: &str, creds: Option<(&str, &str)>) -> UecmResult<(String, u64)> {
    let mut args: Vec<&str> = vec!["-HostName", host, "-ProjectDir", project_dir, "-ArchiveDest", archive_dest];
    if let Some((u, p)) = creds { args.extend(["-Username", u, "-Password", p]); }
    let r: Resp = powershell::run_json(&powershell::script_path("snapshot-baseline-logs.ps1"), &args)?;
    if !r.ok { return Err(UecmError::OperationFailed(r.message.unwrap_or_default())); }
    Ok((r.archive.unwrap_or_default(), r.copied_count.unwrap_or(0)))
}
```

CLI in `domain_log.rs`: add subaction `Snapshot { host, project_dir, archive_dest, cred }`.

Commit.

---

### Module M13: UE 5.3 vs 5.4+ default-path awareness (0.5 day)

**Goal:** Recognize when a host reports an empty `UE-LocalDataCachePath` but the default Local DDC location is implied by the UE version (5.3 → `<UE>/Engine/DerivedDataCache`; 5.4+ → `C:\ProgramData\Epic\Zen\Data`). Surface the implied path in `ddc_file_stats` so we can probe the real default.

**Files:**
- Modify: `src-tauri/src/core/ddc_file_stats.rs` (accept None and resolve from snapshot)
- Modify: `src-tauri/src/core/consistency_check.rs` (already snapshots UE installs)

---

#### Task M13.1: Resolver helper

- [ ] Add to `src-tauri/src/core/ddc_file_stats.rs`:

```rust
pub fn resolve_local_default(ue_version_latest: &str, ue_install_dir: Option<&str>) -> String {
    let major_minor = ue_version_latest.split('.').take(2).collect::<Vec<_>>().join(".");
    let is_5_4_plus = match major_minor.as_str() {
        "5.4" | "5.5" | "5.6" | "5.7" => true,
        _ => false,
    };
    if is_5_4_plus {
        r"C:\ProgramData\Epic\Zen\Data".into()
    } else if let Some(dir) = ue_install_dir {
        format!(r"{}\Engine\DerivedDataCache", dir.trim_end_matches('\\'))
    } else {
        r"C:\Program Files\Epic Games\UE_5.3\Engine\DerivedDataCache".into()
    }
}

#[cfg(test)]
mod default_path_tests {
    use super::*;
    #[test] fn resolves_5_5_to_zen_data() {
        assert!(resolve_local_default("5.5.4", None).contains("Zen"));
    }
    #[test] fn resolves_5_3_to_install_dir() {
        let p = resolve_local_default("5.3.2", Some(r"C:\UE_5.3"));
        assert!(p.ends_with(r"Engine\DerivedDataCache"));
        assert!(p.starts_with(r"C:\UE_5.3"));
    }
}
```

- [ ] Commit.

---

### Module M14: Historical symptom recognizer (2 days)

**Goal:** Auto-match the SOP's known-symptom catalogue against a verify report + file-stats. Produce a concrete advisory ("most likely Node A opened the project before Shared was configured — re-open or generate a Pak").

**Files:**
- Create: `src-tauri/src/core/ddc_symptom_recognizer.rs`
- Create: `src/components/diagnostics/SymptomAdvisory.vue`

---

#### Task M14.1: Recognizer rules

**Files:**
- Create: `src-tauri/src/core/ddc_symptom_recognizer.rs`

- [ ] **Step 1: Types + recognizer**

```rust
// src-tauri/src/core/ddc_symptom_recognizer.rs
use crate::core::{ue_log_verify::VerifyReport, ddc_file_stats::Stats};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Advisory {
    pub id: String,
    pub severity: &'static str,
    pub title: String,
    pub explanation: String,
    pub remediation: Vec<String>,
}

pub fn analyze(verify: &VerifyReport, stats: Option<&Stats>) -> Vec<Advisory> {
    let mut out = Vec::new();

    if verify.shared_deactivated_reason.is_some() {
        out.push(Advisory {
            id: "S001".into(), severity: "critical",
            title: "Shared backend deactivated by UE".into(),
            explanation: format!(
                "UE reported: {}",
                verify.shared_deactivated_reason.as_deref().unwrap_or("?")
            ),
            remediation: vec![
                "Lower ConsiderSlowAt or fix the network latency to NAS".into(),
                "Check `health scan-command-line` for hard-coded -SharedDataCachePath".into(),
            ],
        });
    }

    if verify.move_collision_count > 50 {
        out.push(Advisory {
            id: "S002".into(), severity: "warning",
            title: "Many Move collision warnings".into(),
            explanation: format!("{} Move collision lines in startup log.", verify.move_collision_count),
            remediation: vec![
                "Likely two hosts wrote to Shared concurrently before it was populated. Stagger initial open, or run `ddc generate` then `ddc distribute`.".into(),
            ],
        });
    }

    if let Some(s) = stats {
        if let Some(im) = crate::core::ddc_file_stats::classify_imbalance(s) {
            out.push(Advisory {
                id: "S003".into(), severity: im.severity,
                title: "Shared DDC near-empty vs Local".into(),
                explanation: im.message,
                remediation: vec![
                    "Re-open the project on the host that first compiled, with Shared already configured.".into(),
                    "Or run `uecm-cli ddc generate` + `ddc distribute` to seed Shared from Local.".into(),
                ],
            });
        }
    }

    if verify.local_path.is_none() {
        out.push(Advisory {
            id: "S004".into(), severity: "critical",
            title: "UE did not load a Local DDC path".into(),
            explanation: "LogDerivedDataCache contained no `Using Local data cache path` line.".into(),
            remediation: vec![
                "Check that UE-LocalDataCachePath is set Machine-scope (and not just user-scope).".into(),
                "Verify ProjectLocalDDCPath in EditorPerProjectUserSettings.ini is empty.".into(),
            ],
        });
    }

    if verify.shared_path.is_none() && verify.shared_deactivated_reason.is_none() {
        out.push(Advisory {
            id: "S005".into(), severity: "warning",
            title: "UE did not load a Shared DDC path".into(),
            explanation: "LogDerivedDataCache contained no `Using Shared data cache path` line.".into(),
            remediation: vec![
                "Confirm UE-SharedDataCachePath is set and the BackendGraph Shared node has Path / EnvPathOverride configured.".into(),
                "Make sure the RenderStream service account can reach the UNC share.".into(),
            ],
        });
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ue_log_verify::VerifyReport;

    fn empty_verify() -> VerifyReport {
        VerifyReport {
            host: "X".into(), local_path: None, local_writable: None,
            shared_path: None, shared_writable: None,
            shared_deactivated_reason: None, move_collision_count: 0,
            maintenance: vec![], paks_opened: vec![], truncated: false, log_path: None,
        }
    }

    #[test]
    fn detects_deactivation() {
        let mut v = empty_verify();
        v.shared_deactivated_reason = Some("latency".into());
        let a = analyze(&v, None);
        assert!(a.iter().any(|x| x.id == "S001"));
    }

    #[test]
    fn detects_no_local_path() {
        let a = analyze(&empty_verify(), None);
        assert!(a.iter().any(|x| x.id == "S004"));
    }

    #[test]
    fn detects_no_shared_path_when_not_deactivated() {
        let a = analyze(&empty_verify(), None);
        assert!(a.iter().any(|x| x.id == "S005"));
    }
}
```

- [ ] **Step 2: Run + register + commit**

```bash
cd src-tauri && cargo test --lib core::ddc_symptom_recognizer 2>&1 | tail -10
```

`core/mod.rs` add `pub mod ddc_symptom_recognizer;`.

```bash
git add src-tauri/src/core/ddc_symptom_recognizer.rs src-tauri/src/core/mod.rs
git commit -m "feat(ddc): symptom recognizer maps verify reports to advisories"
```

---

#### Task M14.2: UI advisory panel + deploy integration

**Files:**
- Create: `src/components/diagnostics/SymptomAdvisory.vue`
- Modify: `src/components/modals/DeployDdcWizard.vue` (step 5: also render advisories under report)

- [ ] **Step 1: Component**

```vue
<!-- src/components/diagnostics/SymptomAdvisory.vue -->
<script setup lang="ts">
import { useI18n } from "vue-i18n";

interface Advisory { id: string; severity: "critical" | "warning" | "info"; title: string; explanation: string; remediation: string[] }
defineProps<{ advisories: Advisory[] }>();
const { t } = useI18n();

function tone(sev: string): string {
  if (sev === "critical") return "border-status-critical/40 bg-status-critical/10 text-status-critical";
  if (sev === "warning") return "border-status-warning/40 bg-status-warning/10 text-status-warning";
  return "border-border bg-card text-foreground";
}
</script>

<template>
  <section v-if="advisories.length === 0" class="text-sm text-status-healthy">{{ t("symptom.none") }}</section>
  <section v-else class="space-y-2">
    <article v-for="a in advisories" :key="a.id" :class="['rounded-md border p-3 text-sm', tone(a.severity)]">
      <div class="flex items-baseline justify-between mb-1">
        <h4 class="font-display text-base">{{ a.title }}</h4>
        <span class="font-mono text-xs opacity-70">{{ a.id }}</span>
      </div>
      <p class="mb-2">{{ a.explanation }}</p>
      <ul class="list-disc pl-5 space-y-0.5">
        <li v-for="r in a.remediation" :key="r">{{ r }}</li>
      </ul>
    </article>
  </section>
</template>
```

- [ ] **Step 2: Wire into DeployDdcWizard step 5**

When `VerifyStartupLogs` step events arrive, capture the resulting `VerifyReport` (extend deploy store / event payload to include it), call a new Tauri command `analyze_advisories(report, stats?)` that wraps `ddc_symptom_recognizer::analyze`, render `<SymptomAdvisory :advisories="..." />` below `DeployProgressTable`.

- [ ] **Step 3: Commit**

```bash
git add src/components/diagnostics/SymptomAdvisory.vue src/components/modals/DeployDdcWizard.vue src-tauri/src/commands/log_verify.rs
git commit -m "feat(ddc): symptom advisories panel in deploy wizard"
```

---

## Self-Review

### Spec coverage check

Walking the 16-block clean SOP list against modules:

| SOP block | Plan coverage |
|---|---|
| 1. 缓存层级（Local/Shared/Pak）识别 | M3 (Local env probe + status), M2 (verify exposes actual layer paths), existing pak verify |
| 2. 后端实现 (Filesystem/Pak) | M1 (parser identifies Type field) |
| 3.1 命令行参数检测 | M11 |
| 3.2 环境变量检测 | M3.1 (Local + Shared both probed) |
| 3.3 Editor Preferences 四路径 | M6 |
| 3.4 `[DerivedDataBackendGraph]` 段 | M1.1, M1.2 (parser + scanner), M1.3 (rules) |
| 3.5 Platform/WindowsEngine.ini | M1 BackendGraph parser runs over any file that already includes WindowsEngine.ini (the existing scanner does) |
| 3.6 BaseEngine.ini | same |
| 3.7 DDC Pak `.ddp` | existing — no change |
| 3.8 运行时一致性 | M8 (UE/Plugin/RHI/GPU/Driver) |
| 4. BackendGraph 13 个参数 | M1.3 rules R011–R023 |
| 5. Local + Shared 环境变量 | M3 |
| 6. 命令行参数 | M11 |
| 7. Editor Preferences 优先级 | M6 (R025 — project-pref masks env) |
| 8. UNC vs mapped drive | existing R004 — no change |
| 9. DDC Pak 状态 | existing — no change |
| 10.1 PSO CVar (`r.PSOPrecaching*`) | existing R008–R010 |
| 10.2 `r.ShaderPipelineCache.Enabled` | M1.3 R024 |
| 10.3 `.upipelinecache` + `.stablepc.csv` | M7 |
| 10.4 GPU/Driver 一致性 | existing + M8 cross-host |
| 11.1 `LogDerivedDataCache` 解析 | M2 |
| 11.2 Local vs Shared 文件比对 | M10 |
| 11.3 `-logcmds=...Verbose` | M2 (sidecar uses this flag) |
| 11.4 `LogShaderCompilers` / `LogShaderPipelineCache` | M12 (snapshots all log files) |
| 12. 非路径失效因素 (UE/Plugin/RHI/账户) | M4 (RS service account) + M8 (UE/Plugin/RHI) |
| 13. GC 策略读写 | M9 (quick toggles) + M1 (write any field) |
| 14. UE 5.3 vs 5.4+ default path | M13 |
| 15.1 集群一致性巡检 | M8 |
| 15.2 预热顺序流程编排 | M5 (deploy wizard runs in order) |
| 15.3 基线日志保存 | M12 |
| 16. 历史症状自动识别 | M14 |

All 16 blocks of the去-Zen list are mapped. No spec gaps.

### Placeholder scan

Run a final grep over the plan body:

```bash
grep -nE "TBD|TODO|fill in|implement later|appropriate|Similar to Task" docs/superpowers/plans/2026-05-18-ddc-full-coverage.md
```

Known intentional `todo!()` placeholders are inside test stubs in Step 1 of each Task, immediately replaced in Step 3 — these are TDD red→green markers, not plan placeholders. The only `todo!()` left for the engineer to fill is in M9.1 Step 2 `GcResume` body (one paragraph, with explicit instructions "two `set_backend_field_with_credential` calls per host"). Acceptable.

### Type consistency check

- `DeployPlan` / `DeployStep` / `DeployEvent` defined in M5.1 are referenced verbatim in M5.2–M5.6.
- `BackendNode` from M1.1 is used in M1.2 (`backend_nodes: Vec<BackendNode>`) and M1.4 (`parse_node(line, 0)`). Same signature.
- `VerifyReport` from M2.3 is referenced as a parameter in M14.1 `analyze(verify: &VerifyReport, ...)`. Same struct.
- `Stats` from M10.2 is the same shape consumed by `classify_imbalance` and re-used in M14.1.
- `EnvVarState` is the existing struct in `ini_diagnostics.rs`; M6.2 R025 reads it via `env.shared_data_cache_path.as_ref()`. Existing field.
- `HostTarget::as_vec()` helper introduced in M9.1 is used only within M9.1. Self-contained.
- `HostArgs::require_one()` and `CredentialArgs::preflight/resolve` are existing helpers — referenced consistently.

No naming inconsistencies found.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-05-18-ddc-full-coverage.md`. Two execution options:

1. **Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration. Good for a 14-module / ~60-task plan where you want me to keep tabs on each step without holding all the context in one session.

2. **Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints. Better if you want to drive the order yourself and skip / reorder modules as we go.

Which approach?
