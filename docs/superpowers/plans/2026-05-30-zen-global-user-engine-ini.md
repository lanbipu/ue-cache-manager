# zen enable --global (UserEngine.ini) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `machine set-ue-user` + `zen enable/disable --global` commands so operators can write ZenShared into `UserEngine.ini` (applies to all UE 5.4+ projects for a Windows user) instead of configuring each project individually; plus R019 scanner rule that warns when both global and project-level ZenShared configs coexist.

**Architecture:** `ue_runtime_user` stored per-machine in SQLite (same pattern as `ssh_user`). `zen enable --global` resolves the path `C:\Users\<ue_runtime_user>\AppData\Roaming\Unreal Engine\Engine\Config\UserEngine.ini` in Rust, then writes via existing `core::ini_editor` SSH transport with a new `set_key_create` variant that creates the file if absent. R019 is a new machine-level rule in `ini_diagnostics_zen.rs` evaluated post-scan.

**Tech Stack:** Rust (rusqlite, clap, serde_json), PowerShell sidecars (SSH transport), SQLite migrations.

**Spec:** `docs/superpowers/specs/2026-05-30-zen-global-user-engine-ini.md`

---

## File Map

| File | What changes |
|---|---|
| `src-tauri/src/data/schema.rs` | Migration 024: ADD COLUMN ue_runtime_user |
| `src-tauri/src/data/machines.rs` | `get_ue_runtime_user`, `set_ue_runtime_user` |
| `src-tauri/src/cli/args.rs` | `MachineAction::SetUeUser`; `--global` on Enable/Disable |
| `src-tauri/src/cli/domain_machine.rs` | `set_ue_user` handler |
| `ps-scripts/write-ini-key.ps1` | `CreateIfMissing` JSON param |
| `src-tauri/src/core/ini_editor.rs` | `set_key_create` function |
| `src-tauri/src/core/zen/enable.rs` | `enable_global`, `disable_global` |
| `src-tauri/src/cli/domain_zen.rs` | `global_enable`, `global_disable` handlers |
| `src-tauri/src/core/ini_diagnostics_zen.rs` | `evaluate_r019` function + `ZenRuleContext` field |
| `src-tauri/src/core/ini_scanner.rs` | Call `evaluate_r019` post-scan-loop |
| `docs/zen-integration.md` | Document new commands + R019 |

---

## Task 1: DB migration + data/machines.rs

**Files:**
- Modify: `src-tauri/src/data/schema.rs`
- Modify: `src-tauri/src/data/machines.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `src-tauri/src/data/machines.rs`:

```rust
#[test]
fn ue_runtime_user_defaults_none_and_round_trips() {
    let db = setup();
    let id = insert(&db, &Machine::new("R01", "10.0.0.1")).unwrap();
    assert_eq!(get_ue_runtime_user(&db, id).unwrap(), None);
    set_ue_runtime_user(&db, id, Some("lanbp")).unwrap();
    assert_eq!(
        get_ue_runtime_user(&db, id).unwrap(),
        Some("lanbp".to_string())
    );
    set_ue_runtime_user(&db, id, None).unwrap();
    assert_eq!(get_ue_runtime_user(&db, id).unwrap(), None);
}

#[test]
fn set_ue_runtime_user_returns_error_for_unknown_id() {
    let db = setup();
    let result = set_ue_runtime_user(&db, 9999, Some("x"));
    assert!(result.is_err());
    match result.unwrap_err() {
        UecmError::InvalidInput(msg) => assert!(msg.contains("9999")),
        other => panic!("expected InvalidInput, got {:?}", other),
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```
cmd.exe /c "cargo test ue_runtime_user 2>&1"
```
Expected: `error[E0425]: cannot find function 'get_ue_runtime_user'`

- [ ] **Step 3: Add migration 024 to schema.rs**

In `src-tauri/src/data/schema.rs`, after the `"022_machines_ssh_user"` migration block, add:

```rust
    (
        "024_machines_ue_runtime_user",
        r#"
        ALTER TABLE machines ADD COLUMN ue_runtime_user TEXT;
        "#,
    ),
```

(Migration 023 is `ini_config_snapshots` — keep it; 024 follows it.)

- [ ] **Step 4: Add CRUD functions to machines.rs**

After the `set_ssh_user` / `get_ssh_user` functions in `src-tauri/src/data/machines.rs`, add:

```rust
/// Returns the Windows username of the account that runs UE on this machine,
/// or None when unset. Used by `zen enable --global` to construct the
/// UserEngine.ini absolute path without relying on %APPDATA% expansion in the
/// uecm-svc SSH session.
pub fn get_ue_runtime_user(db: &Db, id: i64) -> UecmResult<Option<String>> {
    let conn = db.lock().unwrap();
    let user: Option<String> = conn.query_row(
        "SELECT ue_runtime_user FROM machines WHERE id = ?",
        params![id],
        |row| row.get(0),
    )?;
    Ok(user)
}

/// Sets (or clears, with `None`) the per-machine UE runtime Windows username.
/// Returns `InvalidInput` when no row matched.
pub fn set_ue_runtime_user(db: &Db, id: i64, user: Option<&str>) -> UecmResult<()> {
    let conn = db.lock().unwrap();
    let updated = conn.execute(
        "UPDATE machines SET ue_runtime_user = ? WHERE id = ?",
        params![user, id],
    )?;
    if updated == 0 {
        return Err(UecmError::InvalidInput(format!("machine {} not found", id)));
    }
    Ok(())
}
```

- [ ] **Step 5: Run tests to verify they pass**

```
cmd.exe /c "cargo test ue_runtime_user 2>&1"
```
Expected: `test data::machines::tests::ue_runtime_user_defaults_none_and_round_trips ... ok` and `set_ue_runtime_user_returns_error_for_unknown_id ... ok`

- [ ] **Step 6: Commit**

```
git add src-tauri/src/data/schema.rs src-tauri/src/data/machines.rs
git commit -m "feat(data): add ue_runtime_user per-machine field (migration 024)"
```

---

## Task 2: CLI — `machine set-ue-user`

**Files:**
- Modify: `src-tauri/src/cli/args.rs`
- Modify: `src-tauri/src/cli/domain_machine.rs`

- [ ] **Step 1: Add `SetUeUser` variant to `MachineAction` in args.rs**

In `src-tauri/src/cli/args.rs`, inside the `pub enum MachineAction` block (after `Rename`), add:

```rust
    /// Record the Windows username that runs UE on this machine.
    /// Used by `zen enable --global` to resolve UserEngine.ini path.
    /// Pass empty string to clear.
    SetUeUser {
        #[arg(long, value_name = "ID")]
        machine: i64,
        /// Windows username (e.g. `lanbp`). Empty string clears the value.
        #[arg(long, value_name = "USERNAME")]
        ue_user: String,
    },
```

- [ ] **Step 2: Add handler to domain_machine.rs**

In `src-tauri/src/cli/domain_machine.rs`, add the `SetUeUser` arm to the main match in the public dispatch function (next to `MachineAction::Rename`):

```rust
MachineAction::SetUeUser { machine, ue_user } => set_ue_user(ctx, machine, &ue_user),
```

Then add the function itself:

```rust
fn set_ue_user(ctx: &mut Ctx<'_>, machine_id: i64, ue_user: &str) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let user_opt: Option<&str> = if ue_user.is_empty() { None } else { Some(ue_user) };
    machines::set_ue_runtime_user(db, machine_id, user_opt)?;
    let doc = serde_json::json!({
        "ok": true,
        "machine_id": machine_id,
        "ue_runtime_user": ue_user,
    });
    ctx.emitter.emit_result(&doc).ok();
    Ok(())
}
```

- [ ] **Step 3: Build to verify no compile errors**

```
cmd.exe /c "cargo build --bin uecm-cli 2>&1 | tail -5"
```
Expected: `Finished` with no errors.

- [ ] **Step 4: Smoke-test the command**

```
cmd.exe /c "C:\Tools\UECM\uecm-cli.exe machine set-ue-user --machine 13 --ue-user lanbp 2>&1"
```
Expected: `{"machine_id":13,"ok":true,"ue_runtime_user":"lanbp"}`

- [ ] **Step 5: Commit**

```
git add src-tauri/src/cli/args.rs src-tauri/src/cli/domain_machine.rs
git commit -m "feat(cli): machine set-ue-user command for UserEngine.ini support"
```

---

## Task 3: PS sidecar — `write-ini-key.ps1` CreateIfMissing

**Files:**
- Modify: `ps-scripts/write-ini-key.ps1`

- [ ] **Step 1: Add `CreateIfMissing` parameter**

Replace the current file-existence check block in `ps-scripts/write-ini-key.ps1`. The current line 10-11 is:

```powershell
    $Remove = [bool]$p.Remove
        if (-not (Test-Path $FilePath)) { throw "file not found: $FilePath" }
```

Replace with:

```powershell
    $Remove = [bool]$p.Remove
    $CreateIfMissing = if ($null -ne $p.CreateIfMissing) { [bool]$p.CreateIfMissing } else { $false }
    if (-not (Test-Path $FilePath)) {
        if ($CreateIfMissing -and (-not $Remove)) {
            $dir = Split-Path -Parent $FilePath
            if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
            New-Item -ItemType File -Path $FilePath -Force | Out-Null
        } elseif ($Remove) {
            # Nothing to remove — file absent is success for a remove op.
            @{ ok = $true; backup_path = ""; message = "file absent, nothing to remove" } | ConvertTo-Json -Compress
            exit 0
        } else {
            throw "file not found: $FilePath"
        }
    }
```

- [ ] **Step 2: Deploy the updated sidecar to lanPC**

```bash
# From WSL:
cp ps-scripts/write-ini-key.ps1 /mnt/c/ProgramData/UECM/ps-scripts/write-ini-key.ps1
```

- [ ] **Step 3: Verify the sidecar handles missing file gracefully**

```
cmd.exe /c "echo {\"FilePath\":\"C:\\Temp\\test-uecm-nonexistent.ini\",\"Section\":\"TestSection\",\"Name\":\"MyKey\",\"Value\":\"MyVal\",\"Remove\":false,\"CreateIfMissing\":true} | powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\\ProgramData\\UECM\\ps-scripts\\write-ini-key.ps1 2>&1"
```
Expected: `{"ok":true,"backup_path":"...","message":"wrote MyKey in [TestSection]"}`

Then clean up: `cmd.exe /c "del C:\Temp\test-uecm-nonexistent.ini 2>nul"`

- [ ] **Step 4: Commit**

```
git add ps-scripts/write-ini-key.ps1
git commit -m "feat(ps): write-ini-key.ps1 CreateIfMissing param for UserEngine.ini"
```

---

## Task 4: `core::ini_editor::set_key_create`

**Files:**
- Modify: `src-tauri/src/core/ini_editor.rs`

- [ ] **Step 1: Write failing test**

Add to `#[cfg(test)] mod tests` in `src-tauri/src/core/ini_editor.rs` (look for the loopback test block near end of file):

```rust
#[test]
fn set_key_create_creates_missing_file_and_writes_key() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("subdir").join("UserEngine.ini");
    let path_str = path.to_str().unwrap();
    // File and parent dir do not exist yet.
    set_key_create("127.0.0.1", path_str, "InstalledDerivedDataBackendGraph", "ZenShared", "(Type=Zen)")
        .expect("set_key_create should create file and write key");
    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(contents.contains("ZenShared=(Type=Zen)"));
}
```

(Add `tempfile = "3"` to `[dev-dependencies]` in `src-tauri/Cargo.toml` if not already present — check with `grep tempfile src-tauri/Cargo.toml`.)

- [ ] **Step 2: Run test to verify it fails**

```
cmd.exe /c "cargo test set_key_create 2>&1"
```
Expected: `error[E0425]: cannot find function 'set_key_create'`

- [ ] **Step 3: Add `set_key_create` to ini_editor.rs**

In `src-tauri/src/core/ini_editor.rs`, after the `set_key` function, add:

```rust
/// Same as [`set_key`] but passes `CreateIfMissing: true` to the PS sidecar,
/// so the file (and its parent directory) are created when absent.
/// Used by `zen enable --global` to write `UserEngine.ini` on machines where
/// the user has never opened UE Engine Settings.
pub fn set_key_create(
    host: &str,
    file_path: &str,
    section: &str,
    name: &str,
    value: &str,
) -> UecmResult<String> {
    if loopback::is_loopback_target(host) {
        // Local path: create parent dir + empty file if missing, then write.
        if let Some(parent) = std::path::Path::new(file_path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        if !std::path::Path::new(file_path).exists() {
            std::fs::write(file_path, "")?;
        }
        return write_key_local(file_path, section, name, Some(value));
    }

    let exec = SshExecutor::from_config()?;
    let result: WriteResult = run_json(
        &exec,
        host,
        &NodeScript {
            name: "write-ini-key.ps1",
            args: serde_json::json!({
                "FilePath": file_path, "Section": section, "Name": name,
                "Value": value, "Remove": false, "CreateIfMissing": true
            }),
            ssh_user: None,
        },
    )?;
    if !result.ok {
        return Err(UecmError::OperationFailed(format!(
            "write INI (create) failed: {}",
            result.message
        )));
    }
    Ok(result.backup_path)
}
```

- [ ] **Step 4: Check if tempfile is already in dev-dependencies**

```
cmd.exe /c "grep tempfile src-tauri/Cargo.toml 2>&1"
```

If not present, add under `[dev-dependencies]`:
```toml
tempfile = "3"
```

- [ ] **Step 5: Run test to verify it passes**

```
cmd.exe /c "cargo test set_key_create 2>&1"
```
Expected: `test core::ini_editor::tests::set_key_create_creates_missing_file_and_writes_key ... ok`

- [ ] **Step 6: Commit**

```
git add src-tauri/src/core/ini_editor.rs src-tauri/Cargo.toml
git commit -m "feat(ini_editor): set_key_create variant for UserEngine.ini file creation"
```

---

## Task 5: `core::zen::enable` — `enable_global` / `disable_global`

**Files:**
- Modify: `src-tauri/src/core/zen/enable.rs`

- [ ] **Step 1: Write failing tests**

Add to `#[cfg(test)] mod tests` in `src-tauri/src/core/zen/enable.rs`:

```rust
#[test]
fn enable_global_creates_file_and_writes_zen_shared() {
    use tempfile::tempdir;
    let dir = tempdir().unwrap();
    let ini = dir.path().join("UserEngine.ini");
    let ini_str = ini.to_str().unwrap();
    // File must NOT exist yet — enable_global creates it.
    assert!(!ini.exists());

    let rules = make_test_rules(); // reuse the helper already in this test module
    let master = ClusterMaster {
        host: "192.168.10.20".to_string(),
        port: 8558,
        namespace: "ue.ddc".to_string(),
    };
    let out = enable_global("127.0.0.1", ini_str, &rules, &master).unwrap();
    assert!(out.changed);
    let contents = std::fs::read_to_string(&ini).unwrap();
    assert!(contents.contains("ZenShared"));
}

#[test]
fn disable_global_is_noop_when_file_absent() {
    let rules = make_test_rules();
    let out = disable_global("127.0.0.1", "/nonexistent/UserEngine.ini", &rules).unwrap();
    assert!(!out.changed);
    assert!(out.warnings.iter().any(|w| w.contains("not found")));
}
```

(If `make_test_rules()` doesn't exist, look at the existing test in `enable.rs` for how `ResolvedRules` is constructed and replicate that pattern. Grep for `fn.*rules\|ResolvedRules {` in the test module.)

- [ ] **Step 2: Run tests to verify they fail**

```
cmd.exe /c "cargo test enable_global 2>&1"
```
Expected: `error[E0425]: cannot find function 'enable_global'`

- [ ] **Step 3: Add `enable_global` and `disable_global` to enable.rs**

At the end of `src-tauri/src/core/zen/enable.rs`, before `#[cfg(test)]`, add:

```rust
/// Apply ZenShared config to a machine's `UserEngine.ini` (global, all-project config).
///
/// Identical to [`enable_project`] except the ZenShared key write uses
/// [`crate::core::ini_editor::set_key_create`] so the file is created when
/// absent (first-time setup on a machine where the user hasn't opened UE
/// Engine Settings).
pub fn enable_global(
    host: &str,
    ini_path: &str,
    rules: &ResolvedRules,
    master: &ClusterMaster,
) -> UecmResult<EnableOutcome> {
    let enable_rule = &rules.rules.enable_zen_shared;
    let smb_rule = &rules.rules.disable_legacy_smb_shared;
    let pak_rule = &rules.rules.disable_legacy_pak;

    let section = &enable_rule.section;
    let smb_section = smb_rule.section.as_str();
    let pak_section = pak_rule.section.as_str();

    let desired_value = apply_value_template(&enable_rule.value_template, master)?;

    let mut section_cache: std::collections::HashMap<String, Vec<IniKey>> =
        std::collections::HashMap::new();
    for sec in [section.as_str(), smb_section, pak_section] {
        if section_cache.contains_key(sec) { continue; }
        let rows = read_section(host, ini_path, sec).unwrap_or_default();
        section_cache.insert(sec.to_string(), rows);
    }
    let zen_section_keys = section_cache.get(section.as_str()).cloned().unwrap_or_default();
    let smb_section_keys = section_cache.get(smb_section).cloned().unwrap_or_default();
    let pak_section_keys = section_cache.get(pak_section).cloned().unwrap_or_default();

    let diff = compute_enable_diff(
        &zen_section_keys, section, &enable_rule.key, &desired_value,
        &smb_section_keys, &smb_rule.key, smb_section,
        &pak_section_keys, &pak_rule.keys, pak_section,
    );

    let env_cleanup_planned: Vec<EnvCleanupRequest> = smb_rule.env_cleanup.iter()
        .map(|e| EnvCleanupRequest { var: e.var.clone(), scopes: e.scopes.clone() })
        .collect();

    let mut warnings = rules.warnings.clone();

    if diff.is_noop() {
        return Ok(EnableOutcome {
            changed: false,
            ini_file: ini_path.to_string(),
            backups: Vec::new(),
            env_cleanup_planned,
            keys_set: Vec::new(),
            keys_removed: Vec::new(),
            warnings,
        });
    }

    let mut backups = Vec::new();
    let mut keys_set = Vec::new();
    let mut keys_removed = Vec::new();

    if let Some(rec) = diff.set_zen_shared.clone() {
        // Use set_key_create (not set_key) so UserEngine.ini is created when absent.
        let backup = crate::core::ini_editor::set_key_create(
            host, ini_path, section, &enable_rule.key, &desired_value,
        ).map_err(|e| UecmError::OperationFailed(format!(
            "enable_global: set {}={} in [{}] failed: {}", enable_rule.key, desired_value, section, e
        )))?;
        backups.push(backup);
        keys_set.push(rec);
    }

    for rec in diff.remove_legacy.iter().cloned() {
        let backup = remove_key(host, ini_path, &rec.section, &rec.key)
            .map_err(|e| UecmError::OperationFailed(format!(
                "enable_global: remove {} from [{}] failed: {}", rec.key, rec.section, e
            )))?;
        backups.push(backup);
        keys_removed.push(rec);
    }

    if !env_cleanup_planned.is_empty() {
        warnings.push(format!(
            "{} env var(s) flagged for cleanup; run zen env-cleanup PS sidecar to apply",
            env_cleanup_planned.len()
        ));
    }

    Ok(EnableOutcome {
        changed: true,
        ini_file: ini_path.to_string(),
        backups,
        env_cleanup_planned,
        keys_set,
        keys_removed,
        warnings,
    })
}

/// Remove ZenShared from a machine's `UserEngine.ini`.
///
/// Narrow disable (same contract as [`disable_project`]): only removes
/// `ZenShared`. If the file is absent, returns `changed: false` — this is
/// a valid idempotent state (never enabled globally).
pub fn disable_global(
    host: &str,
    ini_path: &str,
    rules: &ResolvedRules,
) -> UecmResult<DisableOutcome> {
    match disable_project(host, ini_path, rules) {
        Ok(out) => Ok(out),
        Err(ref e) if e.to_string().contains("file not found") => Ok(DisableOutcome {
            changed: false,
            ini_file: ini_path.to_string(),
            backups: vec![],
            keys_removed: vec![],
            warnings: vec![format!(
                "UserEngine.ini not found at {ini_path} — nothing to disable (not an error)"
            )],
        }),
        Err(e) => Err(e),
    }
}
```

Note: `enable_global` calls `read_section(...).unwrap_or_default()` (swallowing not-found as empty) because a missing `UserEngine.ini` is valid; `set_key_create` will create it. The legacy-key removes remain guarded by `diff.remove_legacy` being empty when the file was absent (no keys read → no removes planned).

- [ ] **Step 4: Add `apply_value_template` visibility check**

`apply_value_template` is used by both `enable_project` and `enable_global`. Verify it's not private to a sub-block:

```
cmd.exe /c "grep -n \"fn apply_value_template\" src-tauri/src/core/zen/enable.rs 2>&1"
```

If it's `fn` (private), change to `pub(crate) fn` — or leave private; `enable_global` is in the same file so visibility is fine.

- [ ] **Step 5: Run tests**

```
cmd.exe /c "cargo test enable_global disable_global 2>&1"
```
Expected: both tests pass.

- [ ] **Step 6: Commit**

```
git add src-tauri/src/core/zen/enable.rs
git commit -m "feat(zen/enable): enable_global / disable_global for UserEngine.ini"
```

---

## Task 6: CLI — `zen enable/disable --global`

**Files:**
- Modify: `src-tauri/src/cli/args.rs`
- Modify: `src-tauri/src/cli/domain_zen.rs`

- [ ] **Step 1: Make `project_id` optional and add `--global` flag in args.rs**

In `src-tauri/src/cli/args.rs`, find `ZenAction::Enable` and change:

```rust
    Enable {
        /// Project row id ... (required unless --global)
        #[arg(long, value_name = "ID")]
        project_id: i64,
```

to:

```rust
    Enable {
        /// Project row id whose `DefaultEngine.ini` to mutate.
        /// Required unless `--global` is set.
        #[arg(long, value_name = "ID")]
        project_id: Option<i64>,
        /// Write ZenShared to UserEngine.ini (applies to all UE 5.4+ projects
        /// for the machine's `ue_runtime_user`). Mutually exclusive with --project-id.
        #[arg(long, conflicts_with = "project_id")]
        global: bool,
```

Do the same for `ZenAction::Disable`:

```rust
    Disable {
        #[arg(long, value_name = "ID")]
        project_id: Option<i64>,
        /// Write to UserEngine.ini instead of per-project DefaultEngine.ini.
        #[arg(long, conflicts_with = "project_id")]
        global: bool,
```

- [ ] **Step 2: Update domain_zen.rs dispatch to pass new fields**

In `src-tauri/src/cli/domain_zen.rs`, find `ZenAction::Enable { project_id, machines, ... }` and update the destructure + dispatch:

```rust
ZenAction::Enable {
    project_id,
    global,
    machines,
    upstream_endpoint_id,
    namespace,
    yes,
    dry_run,
    cred,
} => {
    if global {
        global_enable(ctx, &machines, upstream_endpoint_id, &namespace, yes, dry_run, &cred)
    } else {
        let pid = project_id.ok_or_else(|| {
            UecmError::InvalidInput(
                "must supply --project-id or --global".to_string(),
            )
        })?;
        project_enable(ctx, pid, &machines, upstream_endpoint_id, &namespace, yes, dry_run, &cred)
    }
}
```

And for `ZenAction::Disable`:

```rust
ZenAction::Disable { project_id, global, machines, yes, dry_run, cred } => {
    if global {
        global_disable(ctx, &machines, yes, dry_run, &cred)
    } else {
        let pid = project_id.ok_or_else(|| {
            UecmError::InvalidInput("must supply --project-id or --global".to_string())
        })?;
        project_disable(ctx, pid, &machines, yes, dry_run, &cred)
    }
}
```

- [ ] **Step 3: Add `global_enable` function to domain_zen.rs**

Add before the existing `project_enable` function:

```rust
fn global_enable(
    ctx: &mut Ctx<'_>,
    machine_ids: &[i64],
    upstream_endpoint_id: i64,
    namespace: &str,
    yes: bool,
    dry_run: bool,
    cred: &crate::cli::credential_args::CredentialArgs,
) -> UecmResult<()> {
    let db = ctx.require_db()?;

    // Pre-flight: every machine must have ue_runtime_user set.
    let mut targets: Vec<(crate::data::machines::Machine, String)> = Vec::new();
    for &mid in machine_ids {
        let m = machines::find_by_id(db, mid)?
            .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", mid)))?;
        let ue_user = machines::get_ue_runtime_user(db, mid)?.ok_or_else(|| {
            UecmError::InvalidInput(format!(
                "machine id={mid} has no ue_runtime_user set — run `machine set-ue-user --machine {mid} --ue-user <USERNAME>` first"
            ))
        })?;
        let ini_path = format!(
            r"C:\Users\{ue_user}\AppData\Roaming\Unreal Engine\Engine\Config\UserEngine.ini"
        );
        targets.push((m, ini_path));
    }

    let master = resolve_cluster_master(db, upstream_endpoint_id)?;
    let rules = build_global_rules(namespace)?;

    if dry_run || !yes {
        let planned: Vec<_> = targets.iter().map(|(m, p)| serde_json::json!({
            "machine_id": m.id, "hostname": m.hostname, "ini_path": p
        })).collect();
        destructive::emit_plan(ctx.emitter.as_mut(), "zen.enable_global",
            serde_json::json!({
                "master_host": master.host, "master_port": master.port,
                "namespace": namespace, "machines": planned,
            }),
        );
        return Ok(());
    }

    cred.preflight(db)?;

    let mut results = Vec::new();
    for (machine, ini_path) in &targets {
        let host = machine.ip.as_str();
        let machine_id = machine.id.expect("machine in inventory always has id");
        match zen_enable::enable_global(host, &ini_path, &rules, &master) {
            Ok(out) => results.push(serde_json::json!({
                "machine_id": machine_id, "hostname": machine.hostname,
                "ok": true, "changed": out.changed, "ini_path": ini_path,
                "warnings": out.warnings,
            })),
            Err(e) => results.push(serde_json::json!({
                "machine_id": machine_id, "hostname": machine.hostname,
                "ok": false, "error": e.to_string(),
            })),
        }
    }

    let all_ok = results.iter().all(|r| r["ok"].as_bool().unwrap_or(false));
    ctx.emitter.emit_result(&serde_json::json!({ "ok": all_ok, "results": results })).ok();
    Ok(())
}
```

- [ ] **Step 4: Add `global_disable` function to domain_zen.rs**

```rust
fn global_disable(
    ctx: &mut Ctx<'_>,
    machine_ids: &[i64],
    yes: bool,
    dry_run: bool,
    cred: &crate::cli::credential_args::CredentialArgs,
) -> UecmResult<()> {
    let db = ctx.require_db()?;

    let mut targets: Vec<(crate::data::machines::Machine, String)> = Vec::new();
    for &mid in machine_ids {
        let m = machines::find_by_id(db, mid)?
            .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", mid)))?;
        let ue_user = machines::get_ue_runtime_user(db, mid)?.ok_or_else(|| {
            UecmError::InvalidInput(format!(
                "machine id={mid} has no ue_runtime_user set — run `machine set-ue-user` first"
            ))
        })?;
        let ini_path = format!(
            r"C:\Users\{ue_user}\AppData\Roaming\Unreal Engine\Engine\Config\UserEngine.ini"
        );
        targets.push((m, ini_path));
    }

    let rules = build_global_rules("ue.ddc")?;  // namespace irrelevant for disable

    if dry_run || !yes {
        let planned: Vec<_> = targets.iter().map(|(m, p)| serde_json::json!({
            "machine_id": m.id, "hostname": m.hostname, "ini_path": p
        })).collect();
        destructive::emit_plan(ctx.emitter.as_mut(), "zen.disable_global",
            serde_json::json!({ "machines": planned }));
        return Ok(());
    }

    cred.preflight(db)?;

    let mut results = Vec::new();
    for (machine, ini_path) in &targets {
        let host = machine.ip.as_str();
        let machine_id = machine.id.expect("machine in inventory always has id");
        match zen_enable::disable_global(host, &ini_path, &rules) {
            Ok(out) => results.push(serde_json::json!({
                "machine_id": machine_id, "hostname": machine.hostname,
                "ok": true, "changed": out.changed, "warnings": out.warnings,
            })),
            Err(e) => results.push(serde_json::json!({
                "machine_id": machine_id, "hostname": machine.hostname,
                "ok": false, "error": e.to_string(),
            })),
        }
    }

    let all_ok = results.iter().all(|r| r["ok"].as_bool().unwrap_or(false));
    ctx.emitter.emit_result(&serde_json::json!({ "ok": all_ok, "results": results })).ok();
    Ok(())
}
```

- [ ] **Step 5: Add `build_global_rules` helper to domain_zen.rs**

`global_enable`/`global_disable` need `ResolvedRules` but have no project UE version. Use the default rules (same as project_enable when version is unknown — R012-R015 skip, only the rule template matters for the write):

```rust
fn build_global_rules(namespace: &str) -> UecmResult<crate::core::zen::rules_loader::ResolvedRules> {
    use crate::core::zen::rules_loader;
    let rules = rules_loader::load_default()?;
    // Use the default (unversioned) resolve — rules_loader returns defaults
    // when no version override matches. R012-R015 skip is fine for global mode
    // since we're writing, not scanning.
    let mut resolved = rules_loader::resolve(&rules, None)?;
    // Patch the namespace into the value template's substitution context
    // by overriding the namespace field in the ClusterMaster at call site.
    // The template itself uses {namespace} substitution — no change needed here.
    let _ = namespace; // namespace substituted at enable_global call time via ClusterMaster
    Ok(resolved)
}
```

(If `rules_loader::resolve` doesn't accept `Option<&str>` for version, find the correct public API. Grep for `pub fn resolve` in `rules_loader.rs`.)

- [ ] **Step 6: Build to verify**

```
cmd.exe /c "cargo build --bin uecm-cli 2>&1 | tail -10"
```
Expected: `Finished` with no errors.

- [ ] **Step 7: Smoke-test --global flag**

```
cmd.exe /c "C:\Tools\UECM\uecm-cli.exe zen enable --global --machines 13 --upstream-endpoint-id 1 --cred-alias render-svc --dry-run 2>&1"
```
Expected: dry-run plan JSON showing `UserEngine.ini` path for machine 13.

- [ ] **Step 8: Commit**

```
git add src-tauri/src/cli/args.rs src-tauri/src/cli/domain_zen.rs
git commit -m "feat(cli): zen enable/disable --global for UserEngine.ini"
```

---

## Task 7: R019 scanner rule

**Files:**
- Modify: `src-tauri/src/core/ini_diagnostics_zen.rs`
- Modify: `src-tauri/src/core/ini_scanner.rs`

- [ ] **Step 1: Write failing test**

Add to `#[cfg(test)] mod tests` in `src-tauri/src/core/ini_scanner.rs` (after existing R012 test):

```rust
#[test]
fn scan_machine_emits_r019_when_global_and_project_both_have_zen_shared() {
    // Setup: two INI files — project DefaultEngine.ini with ZenShared,
    // and UserEngine.ini with ZenShared. Expect R019.
    use crate::core::ini_diagnostics::Finding;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();

    // Project INI
    let proj_ini = dir.path().join("DefaultEngine.ini");
    std::fs::write(&proj_ini, "[InstalledDerivedDataBackendGraph]\nZenShared=(Type=Zen, Host=\"192.168.10.20\", Port=8558, Namespace=\"ue.ddc\")\n").unwrap();

    // UserEngine.ini (global)
    let user_ini = dir.path().join("UserEngine.ini");
    std::fs::write(&user_ini, "[InstalledDerivedDataBackendGraph]\nZenShared=(Type=Zen, Host=\"192.168.10.20\", Port=8558, Namespace=\"ue.ddc\")\n").unwrap();

    let inputs = ScanInputs {
        host: "127.0.0.1",
        credential: None,
        installs: &[],
        user_profile: "",
        project_roots: &[dir.path().to_str().unwrap().to_string()],
        env_state: Default::default(),
        zen_ctx: None,
        user_engine_ini_path: Some(user_ini.to_str().unwrap()),
    };

    let outcome = scan_machine(&inputs).unwrap();
    assert!(
        outcome.findings.iter().any(|f| f.rule_id == "R019"),
        "expected R019, got: {:?}",
        outcome.findings.iter().map(|f| &f.rule_id).collect::<Vec<_>>()
    );
}

#[test]
fn scan_machine_does_not_emit_r019_when_only_project_has_zen_shared() {
    use tempfile::tempdir;
    let dir = tempdir().unwrap();
    let proj_ini = dir.path().join("DefaultEngine.ini");
    std::fs::write(&proj_ini, "[InstalledDerivedDataBackendGraph]\nZenShared=(Type=Zen, Host=\"192.168.10.20\", Port=8558, Namespace=\"ue.ddc\")\n").unwrap();
    // No UserEngine.ini path supplied.
    let inputs = ScanInputs {
        host: "127.0.0.1",
        credential: None,
        installs: &[],
        user_profile: "",
        project_roots: &[dir.path().to_str().unwrap().to_string()],
        env_state: Default::default(),
        zen_ctx: None,
        user_engine_ini_path: None,
    };
    let outcome = scan_machine(&inputs).unwrap();
    assert!(
        !outcome.findings.iter().any(|f| f.rule_id == "R019"),
        "R019 must not fire when user_engine_ini_path is None"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

```
cmd.exe /c "cargo test r019 2>&1"
```
Expected: compile error — `user_engine_ini_path` field not found on `ScanInputs`.

- [ ] **Step 3: Add `user_engine_ini_path` to `ScanInputs`**

In `src-tauri/src/core/ini_scanner.rs`, modify `ScanInputs`:

```rust
pub struct ScanInputs<'a> {
    pub host: &'a str,
    pub credential: Option<(&'a str, &'a str)>,
    pub installs: &'a [(String, String)],
    pub user_profile: &'a str,
    pub project_roots: &'a [String],
    pub env_state: EnvVarState,
    pub zen_ctx: Option<&'a ZenRuleContext<'a>>,
    /// Absolute path to `UserEngine.ini` on the remote host. When `Some`,
    /// `scan_machine` checks it post-loop for R019 (global+project ZenShared
    /// coexistence). Derived from `machines.ue_runtime_user` by the CLI layer.
    pub user_engine_ini_path: Option<&'a str>,
}
```

- [ ] **Step 4: Fix all existing `ScanInputs` constructors to add the new field**

Search for all places that construct `ScanInputs { ... }`:

```
cmd.exe /c "grep -rn \"ScanInputs {\" src-tauri/src/ 2>&1"
```

For each location found, add `user_engine_ini_path: None,` to keep existing behaviour unchanged.

- [ ] **Step 5: Add `evaluate_r019` to `ini_diagnostics_zen.rs`**

At the end of `src-tauri/src/core/ini_diagnostics_zen.rs` (before `#[cfg(test)]`), add:

```rust
/// R019 — warns when both `UserEngine.ini` (global) and at least one project's
/// `DefaultEngine.ini` contain a `ZenShared` key. UE INI merge order means the
/// project-level config takes precedence, potentially shadowing the global one.
///
/// Reads `user_engine_ini_path` over SSH (or locally for loopback) and checks
/// for `ZenShared` presence. Also needs the set of project-level ZenShared
/// keys already collected in the current scan pass (from `config_snapshots`).
pub fn evaluate_r019(
    host: &str,
    user_engine_ini_path: &str,
    config_snapshots: &[crate::core::ini_config_extract::ConfigEntry],
    machine_id: i64,
) -> Vec<crate::core::ini_diagnostics::Finding> {
    use crate::core::ini_editor::read_section;
    use crate::core::ini_diagnostics::{Finding, RecommendedAction, Severity};

    // Check if any project DefaultEngine.ini snapshot has a ZenShared key.
    let project_has_zen_shared = config_snapshots.iter().any(|e| {
        e.domain == "zen" && e.key_name.eq_ignore_ascii_case("ZenShared")
    });
    if !project_has_zen_shared {
        return vec![];
    }

    // Read UserEngine.ini for ZenShared.
    let section = "InstalledDerivedDataBackendGraph";
    let user_keys = match read_section(host, user_engine_ini_path, section) {
        Ok(keys) => keys,
        Err(_) => return vec![], // file absent or unreadable — not an error for R019
    };
    let global_has_zen_shared = user_keys.iter().any(|k| k.name.eq_ignore_ascii_case("ZenShared"));
    if !global_has_zen_shared {
        return vec![];
    }

    vec![Finding {
        rule_id: "R019".to_string(),
        severity: Severity::Warning,
        machine_id,
        file_path: user_engine_ini_path.to_string(),
        section: Some(section.to_string()),
        key_name: Some("ZenShared".to_string()),
        line_number: None,
        snippet_before: String::new(),
        snippet_after: None,
        recommended_action: RecommendedAction::Manual,
        recommended_value: None,
        symptom: "Global ZenShared (UserEngine.ini) and project-level ZenShared both present \
                  — project-level config takes precedence and may shadow the global setting"
            .to_string(),
        rationale: "UE INI merge order: project DefaultEngine.ini overrides user UserEngine.ini. \
                    Remove one of the two ZenShared entries."
            .to_string(),
    }]
}
```

(Check the exact fields of `Finding` by grepping `pub struct Finding` in `ini_diagnostics.rs` and match them exactly.)

- [ ] **Step 6: Call `evaluate_r019` from `scan_machine`**

In `src-tauri/src/core/ini_scanner.rs`, after the `evaluate_machine_zen` call, add:

```rust
    // R019: global (UserEngine.ini) + project-level ZenShared coexistence check.
    if let Some(user_ini) = inputs.user_engine_ini_path {
        use crate::core::ini_diagnostics_zen::evaluate_r019;
        // machine_id is not directly available in ScanInputs; look it up from
        // zen_ctx if present, or pass 0 as a sentinel. The CLI layer supplies
        // it via ZenRuleContext.machine_id when constructing ScanInputs.
        // For now, add machine_id to ScanInputs (see next step).
        outcome.findings.extend(evaluate_r019(
            inputs.host,
            user_ini,
            &outcome.config_snapshots,
            inputs.machine_id,
        ));
    }
```

- [ ] **Step 7: Add `machine_id` to `ScanInputs`**

R019's `Finding` needs a `machine_id`. Add to `ScanInputs`:

```rust
    /// Machine row id — needed for R019 finding attribution. Use 0 when unknown.
    pub machine_id: i64,
```

Update all existing `ScanInputs { ... }` constructors to add `machine_id: 0,` or supply the real id from the caller context. The CLI layer (`domain_ini.rs`) already has the machine id — supply it there.

- [ ] **Step 8: Run tests**

```
cmd.exe /c "cargo test r019 2>&1"
```
Expected: both R019 tests pass.

- [ ] **Step 9: Run full test suite to check no regressions**

```
cmd.exe /c "cargo test 2>&1 | tail -20"
```
Expected: all tests pass.

- [ ] **Step 10: Commit**

```
git add src-tauri/src/core/ini_diagnostics_zen.rs src-tauri/src/core/ini_scanner.rs
git commit -m "feat(scanner): R019 rule — warn when global and project ZenShared coexist"
```

---

## Task 8: Documentation

**Files:**
- Modify: `docs/zen-integration.md`

- [ ] **Step 1: Add `machine set-ue-user` to CLI reference (§2.1)**

In `docs/zen-integration.md` §2, add a new subsection after the existing machine management section:

```markdown
### 2.0 Per-machine UE runtime user

```
uecm-cli machine set-ue-user --machine ID --ue-user USERNAME
uecm-cli machine set-ue-user --machine ID --ue-user ""   # clear
```

Sets the Windows username that runs UE on the target machine. Required before
`zen enable --global` can resolve the `UserEngine.ini` path. The username is
stored in `machines.ue_runtime_user` (nullable) and is never used for
authentication — it only affects INI path construction.
```

- [ ] **Step 2: Add `zen enable --global` to §2.4**

Add to the existing `zen enable` / `zen disable` section:

```markdown
**Global mode (all-project config via `UserEngine.ini`):**

```
uecm-cli zen enable --global \
  --machines M1,M2,... \
  --upstream-endpoint-id E \
  [--namespace ue.ddc] \
  --cred-alias ALIAS [--yes] [--dry-run]

uecm-cli zen disable --global \
  --machines M1,M2,... \
  --cred-alias ALIAS [--yes] [--dry-run]
```

`--global` writes `ZenShared` to each machine's
`C:\Users\<ue_runtime_user>\AppData\Roaming\Unreal Engine\Engine\Config\UserEngine.ini`
instead of a specific project's `DefaultEngine.ini`. This applies to **all**
UE 5.4+ projects for that Windows user without per-project configuration.

Pre-flight: every `--machines` target must have `ue_runtime_user` set (see
`machine set-ue-user`). `--global` and `--project-id` are mutually exclusive.

`UserEngine.ini` is created if absent (`CreateIfMissing` transport flag).
```

- [ ] **Step 3: Document R019 in §3**

Add R019 row to the INI scanner rule table:

```markdown
| R019 | warning | Global ZenShared (UserEngine.ini) and project-level ZenShared both present on same machine — project config takes precedence |
```

- [ ] **Step 4: Commit**

```
git add docs/zen-integration.md
git commit -m "docs(zen): document machine set-ue-user, zen enable --global, R019"
```

---

## Self-Review Against Spec

| Spec requirement | Covered by |
|---|---|
| Migration 024 `ue_runtime_user` column | Task 1 |
| `get_ue_runtime_user` / `set_ue_runtime_user` | Task 1 |
| `machine set-ue-user` CLI | Task 2 |
| `write-ini-key.ps1` `CreateIfMissing` | Task 3 |
| `set_key_create` in `ini_editor.rs` | Task 4 |
| `enable_global` / `disable_global` | Task 5 |
| `zen enable --global` CLI + pre-flight | Task 6 |
| `zen disable --global` CLI | Task 6 |
| R019 rule (check-only, case-insensitive) | Task 7 |
| `user_engine_ini_path` in `ScanInputs` | Task 7 |
| docs update | Task 8 |
| `--global` ⊕ `--project-id` mutual exclusion | Task 6 |
| Pre-flight aborts if any machine has NULL `ue_runtime_user` | Task 6 |
| `UserEngine.ini` absent on disable → `changed: false` | Task 5 |
