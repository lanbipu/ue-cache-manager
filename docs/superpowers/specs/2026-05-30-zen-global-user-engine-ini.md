# Spec: zen enable --global (UserEngine.ini support)

**Date:** 2026-05-30
**Status:** Approved
**Context:** `zen enable` currently only writes per-project `DefaultEngine.ini`. Cluster operators want a single global ZenShared config that applies to all UE 5.4+ projects on a machine without configuring each project individually.

---

## 1. Problem Statement

`zen enable` writes `ZenShared=(Type=Zen, ...)` into a project's `Config/DefaultEngine.ini`. On render nodes that run many projects (or new projects added after initial setup), operators must run `zen enable` per project. UE provides a user-level override mechanism via `UserEngine.ini` (`%APPDATA%\Unreal Engine\Engine\Config\UserEngine.ini`) that applies to all projects for that Windows user — UECM should support writing there.

**Constraint:** UECM's SSH sessions run as `uecm-svc`, so `%APPDATA%` in that session points to `uecm-svc`'s profile, not the UE operator's. The target user's Windows username must be stored per-machine so the absolute path can be constructed without environment variable expansion.

---

## 2. Design

### 2.1 Data Model

**Migration 024** — add `ue_runtime_user` column to `machines`:

```sql
ALTER TABLE machines ADD COLUMN ue_runtime_user TEXT;
```

- `NULL` = not set. `zen enable --global` aborts pre-flight if any target machine has `NULL`.
- Same pattern as `ssh_user` (migration 022).

`UserEngine.ini` absolute path is constructed in Rust (never via `%APPDATA%` expansion):

```
C:\Users\<ue_runtime_user>\AppData\Roaming\Unreal Engine\Engine\Config\UserEngine.ini
```

New functions in `src-tauri/src/data/machines.rs`:

```rust
pub fn get_ue_runtime_user(db: &Db, id: i64) -> UecmResult<Option<String>>
pub fn set_ue_runtime_user(db: &Db, id: i64, user: Option<&str>) -> UecmResult<()>
```

### 2.2 CLI

**New subcommand: `machine set-ue-user`**

```
uecm-cli machine set-ue-user --machine <ID> --ue-user <USERNAME>
uecm-cli machine set-ue-user --machine <ID> --ue-user ""   # clear
```

- Local DB write only. No `--cred-alias`, no `--yes` (reversible).
- Output: `{ "machine_id": N, "ue_runtime_user": "...", "ok": true }`
- `machine list` / `machine show` output gains `ue_runtime_user` field (read-only display).

**`zen enable` — new `--global` flag**

```
uecm-cli zen enable --global \
  --machines M1,M2,... \
  --upstream-endpoint-id E \
  [--namespace ue.ddc] \
  --cred-alias ALIAS [--yes] [--dry-run]
```

Behavior when `--global` is present:
- `--project-id` is **mutually exclusive** — error if both supplied.
- Pre-flight resolves `ue_runtime_user` for every target machine before any write. If any machine has `ue_runtime_user = NULL`, the entire command aborts with a clear error: `"machine id=N has no ue_runtime_user set — run machine set-ue-user first"`.
- Target INI path: `C:\Users\<ue_runtime_user>\AppData\Roaming\Unreal Engine\Engine\Config\UserEngine.ini`
- All other behavior (ZenShared key write, legacy SMB/Pak cleanup, env-var cleanup, `--dry-run`, `--yes`, continue-on-failure) is identical to project-level `zen enable`.

**`zen disable` — new `--global` flag**

```
uecm-cli zen disable --global \
  --machines M1,M2,... \
  --cred-alias ALIAS [--yes] [--dry-run]
```

Same mutual exclusion and pre-flight as `zen enable --global`. Removes only the `ZenShared` key from `UserEngine.ini` (narrow disable, same contract as project-level `zen disable`).

### 2.3 INI Editor Transport

`zen enable` uses `core::ini_editor::{read_section, set_key, remove_key}` — Rust functions that run pre-deployed PS scripts on the remote host via SSH (`C:\ProgramData\UECM\ps-scripts\`). No separate zen-specific sidecar; the two relevant scripts are `read-ini-section.ps1` and `write-ini-key.ps1`.

**Problem:** `write-ini-key.ps1` currently throws `"file not found: $FilePath"` when the target file does not exist (line 11). `UserEngine.ini` is absent on machines whose user has never opened UE Engine Settings.

**Fix — `write-ini-key.ps1`**: add `CreateIfMissing` JSON parameter (default `$false`). When `$true` and `$Remove` is `$false`, create an empty file (+ parent directory) with `New-Item -Force` before writing. Remove operations with `CreateIfMissing=$true` are no-ops if the file is absent (nothing to remove).

**Fix — `core::ini_editor`**: add `pub fn set_key_create(host, file_path, section, name, value)` that passes `CreateIfMissing: true` to the sidecar. Existing `set_key` is unchanged. `enable_global` in `core::zen::enable` calls `set_key_create`; all project-level callers continue to use `set_key`.

### 2.4 INI Scanner Rule R026

| Attribute | Value |
|---|---|
| Rule ID | R026 |
| Severity | `warning` |
| Trigger | `UserEngine.ini` on machine M contains a `ZenShared` key **and** at least one project on M also has `ZenShared` in its `DefaultEngine.ini` |
| Symptom | "Global ZenShared (UserEngine.ini) and project-level ZenShared both present — project-level config takes precedence and may shadow the global setting" |
| Rationale | UE INI merge order: project `DefaultEngine.ini` overrides user `UserEngine.ini`. Operator likely wants one or the other, not both. |
| Recommended action | `manual` (UECM does not auto-remove either; operator decides which to keep) |

**Scanner implementation details:**
- R026 is only emitted when the machine has `ue_runtime_user` set (path is known).
- `UserEngine.ini` path absence → R019 silently skipped for that machine.
- Scanning `UserEngine.ini` is a new read pass inside the existing `ini scan` flow; no new scan type needed.
- Section and key matching **must** use `eq_ignore_ascii_case` (reuse `find_section` / `find_key` helpers from `core::ini_diagnostics_zen`). UE INI parsing is case-insensitive in practice; a lowercase `[installedderiveddatabackendgraph]` must still trigger R026.
- R026 is a **check-only** rule: it reads `UserEngine.ini` for key presence but does **not** persist its values in `ini_config_snapshots`. No raw INI values from `UserEngine.ini` are stored in the DB or returned in structured output.

---

## 3. File Change Summary

| File | Change |
|---|---|
| `src-tauri/src/data/schema.rs` | Migration 024: `ALTER TABLE machines ADD COLUMN ue_runtime_user TEXT` |
| `src-tauri/src/data/machines.rs` | `get_ue_runtime_user`, `set_ue_runtime_user` + unit tests |
| `src-tauri/src/cli/args.rs` | `machine set-ue-user` subcommand; `--global` flag on `zen enable` / `zen disable` |
| `src-tauri/src/cli/domain_machine.rs` | Handler for `set-ue-user` |
| `src-tauri/src/cli/domain_zen.rs` | `--global` branch in `zen_enable` / `zen_disable` handlers |
| `src-tauri/src/core/zen/enable.rs` | `enable_global` / `disable_global` entry points |
| `src-tauri/src/core/ini_editor.rs` | Add `set_key_create` variant (passes `CreateIfMissing: true`) |
| `ps-scripts/write-ini-key.ps1` | Add `CreateIfMissing` parameter |
| `src-tauri/src/core/ini_scanner/rules/r019.rs` | New rule file |
| `src-tauri/src/core/ini_scanner/rules/mod.rs` | Register R026 |
| `docs/zen-integration.md` | Document machine set-ue-user, zen enable --global, R026 |

---

## 4. Error Cases

| Scenario | Behavior |
|---|---|
| `--global` + `--project-id` supplied together | Error before any I/O: `"--global and --project-id are mutually exclusive"` |
| Target machine has `ue_runtime_user = NULL` | Pre-flight abort (all machines checked before any write) |
| `UserEngine.ini` parent dir missing | PS sidecar with `CreateIfMissing=$true` uses `New-Item -Force` to create directory + file |
| R026: ue_runtime_user not set on machine | R026 skipped silently for that machine |

---

## 5. Codex Adversarial Review Disposition (2026-05-30)

`/codex:adversarial-review` run against working-tree diff. Two findings:

**#1 [high] Raw snapshot values expose sensitive INI fields** — **Not applicable to this spec.** R026 is check-only: it reads `UserEngine.ini` for key *presence* but stores nothing in `ini_config_snapshots`. No raw INI values from `UserEngine.ini` enter the DB or structured output. The broader snapshot-redaction question is a deliberate design decision documented in spec `2026-05-24-cli-scan-display-project-design.md §9/§11` (consciously rejected with home-lab threat-model rationale).

**#2 [medium] `ini_config_extract.rs` uses case-sensitive section/key matching** — **Adopted.** Finding is correct: `DDC_SECTIONS.contains(&sname)` and `sname == INSTALLED_DDBG` are case-sensitive, while `ini_diagnostics_zen.rs`'s `find_section` / `find_key` are already `eq_ignore_ascii_case`. Fix: replaced all comparisons in `ini_config_extract.rs` with case-insensitive equivalents; added 3 regression tests (`extracts_ddc_section_case_insensitive`, `extracts_installed_ddbg_case_insensitive`, `pso_cvar_extraction_case_insensitive`). All 11 `ini_config` tests pass. R019 spec updated to explicitly require `eq_ignore_ascii_case`.

---

## 7. Out of Scope

- Auto-detecting `ue_runtime_user` via SSH (who owns `%LOCALAPPDATA%\UnrealEngine`). Operator sets it manually with `machine set-ue-user`.
- Writing to engine-level `BaseEngine.ini` (affects all users on the machine, not just one Windows user).
- GUI support — CLI-first per project convention; GUI wiring deferred.
