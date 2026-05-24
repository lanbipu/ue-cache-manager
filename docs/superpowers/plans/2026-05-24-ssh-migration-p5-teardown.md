# SSH Migration P5 — WinRM + DPAPI Teardown Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the now-unused WinRM transport (`core/winrm.rs` + `winrm` CLI domain + remote-push onboarding) and DPAPI/cmdkey credential store, while keeping the published Tauri UI working via back-compat shims, and adding the `ssh package-bootstrap` CLI command so onboarding-bundle coverage does not regress.

**Architecture:** P0–P4 already migrated every live transport/credential consumer to SSH (`core/ssh.rs`) + the cross-platform `SecretStore` (`core/secrets.rs`). The only remaining live WinRM consumer is the `winrm` CLI domain itself plus two Tauri bootstrap commands; the only remaining DPAPI consumers are explicit "retire in P5b" fallbacks. This plan deletes the dead infrastructure in dependency order: **P5a** retires WinRM (add SSH replacement → repoint UI/CLI consumers → delete core), **P5b** removes DPAPI (drop fallbacks → strip cmdkey/DPAPI functions → delete scripts). Tauri command signatures + Vue-facing JSON field names are frozen (contract §5 of the design spec).

**Tech Stack:** Rust (`src-tauri`, package `uecm`, tests `cd src-tauri && cargo test --lib`), clap CLI, Tauri 2, PowerShell 5.1 node scripts (`ps-scripts/`), AES-GCM SecretStore.

**Verification boundary (confirmed with user):** This session's "done" = all code changes complete + mac gates green (`cargo test --lib` 997 baseline, `cargo check --all-targets`, `pnpm tauri build --no-bundle`) + grep-zero gates. Windows-only behavioral verification (lanPC real-node E2E, `UECM-Bootstrap.cmd` double-click, Tauri UI smoke) is deferred to the Windows handoff checklist at the end — consistent with how P0–P4 were left (lanPC E2E was already pending).

**Scope decision (confirmed with user):** Add `ssh package-bootstrap` only (the required replacement for the retiring `winrm bootstrap-script` USB-bundle role). `ssh authorize` (key-rotation convenience, not a 1:1 replacement of any retiring capability — remote push is intentionally gone) is deferred and marked TODO.

**Frozen (do NOT touch — design spec §9):** `winrm_open` / `winrm_ok` JSON fields (21 refs, `machine scan` 5985 probe + Vue rendering), `network.rs` `PORT_WINRM`/`winrm_open`, `probe_keys.rs` `tcp_5985` + L2/L3 taxonomy, `output.rs` HostProbe winrm field, the "WinRM 5985" port label in health probes, `CredentialKind::Winrm` + `from_sql` tolerance, all `src/**` Vue. These stay so the final grep does not over-delete.

---

## Pre-flight facts (verified against current worktree `worktree-ssh-migration-p5`, off main `ffa85eb`)

- Baseline: `cargo test --lib` → **997 passed, 0 failed, 2 ignored**.
- P1 done: `cli/domain_ssh.rs` exists (`ssh probe` only); `machine refresh`/`deep_scan`/`commands/discovery` use `exec.probe` (SSH). `deep_scan` hint (domain_machine.rs:533) already SSH-worded.
- P2 done: zen uses `ssh::run_json`; no live `winrm::` call in zen (only stale doc comments).
- P3/P4 done: `save_credential`/`save_resolved` → `SecretStore::put`; `domain_secret.rs` (set/get/list/delete) complete; `data/credentials.rs` `from_sql` already tolerant + keeps `Winrm` variant.
- **Only live `winrm::` callers:** `cli/domain_winrm.rs` (probe/bootstrap/preflight) + its `cli/run.rs` dispatch. **Only `core::bootstrap`/`core::preflight` callers:** `domain_winrm.rs`, `domain_machine.rs::authorize`, `commands/bootstrap.rs`, `core/mod.rs` tests.
- **DPAPI/cmdkey live consumers** (`core/credentials.rs`):
  - `resolve_password` → `commands/bootstrap.rs:32` (P5a removes), `cli/credential_args.rs:108` (P5b), `core/secrets.rs:143` (P5b).
  - `delete_password` → `commands/credentials.rs:56`, `cli/domain_cred.rs:172` (both P5b).
  - `delete` (cmdkey) → `commands/credentials.rs:69`, `cli/domain_cred.rs:156` (both P5b).
  - `store` / `store_password` / `list_uecm_aliases` → **no live consumers** (tests only).
  - `normalize_username_for_storage` → `commands/credentials.rs:25`, `domain_cred.rs:73` → **KEEP this fn**, it is not DPAPI.
- `get_share_secret_migrating` (secrets.rs) has 4 callers (ddc_pak, domain_share, shares, pak_distribute) — keep the fn, drop only its DPAPI branch in P5b.
- `WinrmBootstrapResult` (core/bootstrap.rs:11): `{ ok, method, message, winrm_ok, changed: Vec<String>, manual_script: Option<String> }`. Vue reads `.ok/.message/.manual_script`; `commands/bootstrap.rs:41` reads `.winrm_ok`. P5a moves this struct into `commands/bootstrap.rs` (name + shape frozen).
- `KeyStore::at(&startup::resolve_config_dir()?)` → `.ensure_keypair()` + `.public_key_path()` gives the operator pubkey for `ssh package-bootstrap`.

---

# PHASE P5a — Retire WinRM (gate: P1 + P2 done ✓)

## Task 1: Add `ssh package-bootstrap` CLI command (replacement before deletion)

**Files:**
- Modify: `src-tauri/src/cli/args.rs:239-242` (`SshAction`)
- Modify: `src-tauri/src/cli/domain_ssh.rs`
- Create: `ps-scripts/package-bootstrap.ps1` (SSH-only packager, derived from `package-winrm-bootstrap.ps1`)
- Modify: `src-tauri/src/cli/run.rs` (`needs_db` Ssh arm already `false` — confirm, no change expected)

- [ ] **Step 1: Add the `PackageBootstrap` variant to `SshAction`**

In `args.rs`, replace the `SshAction` enum body (keep `Probe`):

```rust
// ---------- ssh ----------
#[derive(Subcommand, Debug)]
pub enum SshAction {
    /// Probe a host's SSH reachability (uecm-svc key auth).
    Probe { host: String },
    /// Assemble a USB onboarding bundle (UECM-Bootstrap.cmd + enable-ssh.ps1 +
    /// uecm.pub + PsExec64.exe + README) into an output directory. Replaces the
    /// retired `winrm bootstrap-script`. Windows-only (PowerShell packager).
    PackageBootstrap {
        /// Output directory for the bundle (created if missing).
        #[arg(long, value_name = "DIR")]
        out: String,
        /// Optionally bake the uecm-svc local-admin password into the packaged
        /// .cmd so first-contact double-click creates the account unattended.
        #[arg(long, value_name = "PASS")]
        local_admin_password: Option<String>,
    },
    // TODO(P5-followup): `ssh authorize <host>` — re-push the current keystore
    // pubkey to an already-SSH-reachable node (key rotation). Deferred: not a
    // 1:1 replacement of any retiring command; remote push is intentionally gone.
}
```

- [ ] **Step 2: Create `ps-scripts/package-bootstrap.ps1` (SSH-only)**

Copy `package-winrm-bootstrap.ps1` and strip the WinRM half. The result must (LF is fine for `.ps1`; only `.cmd` needs CRLF):
- require `-OutputDirectory` + `-UecmPublicKeyPath`; accept optional `-LocalAdminPassword`;
- copy `UECM-Bootstrap.cmd`, `enable-ssh.ps1`, `uecm-bootstrap-readme.zh-CN.txt` (→ `README.txt`), write `uecm.pub`, copy `vendor/PsExec64.exe`;
- **remove** the `enable-winrm.ps1` → `UECM-Bootstrap-WinRM.ps1` copy and the `UECM-Bootstrap-WinRM.ps1` entry in the emitted file list;
- emit a JSON object `{ ok, output_dir, files: [...] }` (so the Rust caller can `run_json` it).

Concrete header + emit shape (keep the existing password-baking + binary-copy logic verbatim from the source, only delete the WinRM lines):

```powershell
# package-bootstrap.ps1 — assemble the SSH USB onboarding bundle.
# Files bundled:
# - UECM-Bootstrap.cmd  (double-click entry, self-elevates, pure SSH onboarding)
# - enable-ssh.ps1      (node: OpenSSH + authorize uecm.pub + node prep + PsExec64)
# - uecm.pub            (UECM transport public key, plaintext in the bundle)
# - PsExec64.exe        (SYSTEM cmdkey injection used by enable-ssh.ps1)
# - README.txt
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$OutputDirectory,
    [Parameter(Mandatory)][string]$UecmPublicKeyPath,
    [string]$LocalAdminPassword
)
# ... (verbatim source/dest resolution for UECM-Bootstrap.cmd, enable-ssh.ps1,
#      readme, uecm.pub, PsExec64.exe; password-baking into the .cmd; binary
#      Copy-Item for README to dodge PS5.1 ANSI re-encode) ...
$files = [System.Collections.ArrayList]@()
[void]$files.AddRange(@('UECM-Bootstrap.cmd', 'README.txt', 'enable-ssh.ps1', 'uecm.pub', 'PsExec64.exe'))
@{ ok = $true; output_dir = $OutputDirectory; files = $files } | ConvertTo-Json -Compress
```

- [ ] **Step 3: Implement the `PackageBootstrap` handler in `domain_ssh.rs`**

Add to `handle()` match + a `package_bootstrap` fn. It resolves/ensures the keystore pubkey, then shells out to `package-bootstrap.ps1` via `powershell::run_json` (Windows-only; off Windows the sidecar returns `UecmError::PowerShell`, matching every other node command):

```rust
use crate::core::keystore::KeyStore;
use crate::core::powershell;

#[derive(serde::Deserialize)]
struct PackageOut {
    ok: bool,
    output_dir: String,
    files: Vec<String>,
}

pub fn handle(ctx: &mut Ctx<'_>, action: SshAction) -> UecmResult<()> {
    match action {
        SshAction::Probe { host } => probe(ctx, &host),
        SshAction::PackageBootstrap { out, local_admin_password } => {
            package_bootstrap(ctx, &out, local_admin_password.as_deref())
        }
    }
}

fn package_bootstrap(ctx: &mut Ctx<'_>, out: &str, local_admin_password: Option<&str>) -> UecmResult<()> {
    let cfg = crate::startup::resolve_config_dir()?;
    let ks = KeyStore::at(&cfg);
    ks.ensure_keypair()?;
    let pubkey_path = ks.public_key_path();
    let pubkey_str = pubkey_path.to_string_lossy().into_owned();

    let mut args: Vec<&str> = vec![
        "-OutputDirectory", out,
        "-UecmPublicKeyPath", &pubkey_str,
    ];
    if let Some(p) = local_admin_password {
        args.push("-LocalAdminPassword");
        args.push(p);
    }
    let res: PackageOut = powershell::run_json(&powershell::script_path("package-bootstrap.ps1"), &args)?;
    if !res.ok {
        return Err(UecmError::OperationFailed(format!(
            "package-bootstrap failed for {}", res.output_dir
        )));
    }
    ctx.emitter
        .emit_result(&serde_json::json!({ "output_dir": res.output_dir, "files": res.files }))
        .ok();
    Ok(())
}
```

- [ ] **Step 4: Confirm `needs_db` Ssh arm**

`run.rs` `needs_db` already returns `false` for `Domain::Ssh { .. }`. Update its comment to drop "(package-bootstrap/authorize land in P5a)" → "(package-bootstrap is file/keystore only; no DB)". No logic change.

- [ ] **Step 5: Build + test**

Run: `cd src-tauri && cargo test --lib 2>&1 | tail -5`
Expected: 997 passed (no new failing tests yet; new command compiles).
Run: `cargo check --all-targets 2>&1 | tail -5`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/cli/args.rs src-tauri/src/cli/domain_ssh.rs src-tauri/src/cli/run.rs ps-scripts/package-bootstrap.ps1
git -c commit.gpgsign=false commit -m "feat(cli): add ssh package-bootstrap (SSH USB onboarder, replaces winrm bootstrap-script)"
```

---

## Task 2: Convert `UECM-Bootstrap.cmd` to pure SSH

**Files:**
- Modify: `ps-scripts/UECM-Bootstrap.cmd` (CRLF + pure ASCII — memory `bootstrap_cmd_crlf_trap`)
- Modify: `ps-scripts/uecm-bootstrap-readme.zh-CN.txt` (drop the WinRM PS1 reference at line 101)

- [ ] **Step 1: Strip the WinRM half from `UECM-Bootstrap.cmd`**

Remove: the `set "PS1=%SCRIPT_DIR%UECM-Bootstrap-WinRM.ps1"` block, the "WinRM PS1 not found" guard, the WinRM PS1 invocation, and the `%PS_EXIT%`/WinRM portions of the final status echo. Keep: self-elevation, `uecm.pub` discovery, `enable-ssh.ps1` invocation (`SSH_PS1`/`SSH_EXIT`), and SSH-only status reporting. Update the header comment `REM UECM WinRM Bootstrap` → `REM UECM SSH Bootstrap`. `OVERALL` is driven by `SSH_EXIT` alone.

- [ ] **Step 2: Verify CRLF + ASCII (hard requirement — cmd.exe breaks otherwise)**

Run: `file ps-scripts/UECM-Bootstrap.cmd`
Expected: `... CRLF line terminators` and `ASCII text` (NOT UTF-8, NOT LF).
Run: `LC_ALL=C grep -n '[^[:print:][:space:]]' ps-scripts/UECM-Bootstrap.cmd || echo "ASCII-clean"`
Expected: `ASCII-clean`.

- [ ] **Step 3: Drop the WinRM line from the readme**

In `uecm-bootstrap-readme.zh-CN.txt`, remove/replace the `.\UECM-Bootstrap-WinRM.ps1 -CheckOnly` line (line ~101) with the SSH-only re-run guidance (re-run `UECM-Bootstrap.cmd`, or `powershell -File enable-ssh.ps1 ...`). Keep CJK in this `.txt` (only the `.cmd` must be ASCII).

- [ ] **Step 4: Commit**

```bash
git add ps-scripts/UECM-Bootstrap.cmd ps-scripts/uecm-bootstrap-readme.zh-CN.txt
git -c commit.gpgsign=false commit -m "refactor(onboard): UECM-Bootstrap.cmd is now pure SSH (drop WinRM PS1 leg)"
```

> Behavioral verification (cmd.exe double-click on a Windows node) is in the Windows handoff checklist — cannot be validated on mac.

---

## Task 3: Repoint Tauri bootstrap commands (`commands/bootstrap.rs`)

**Files:**
- Modify: `src-tauri/src/commands/bootstrap.rs`

Vue (`stores/machines.ts:130,151`) calls `bootstrapWinrm`/`getWinrmBootstrapScript` and reads `WinrmBootstrapResult.{ok,message,manual_script}`. Commands + signatures + response shape are frozen (contract §5.4); only the implementation changes (no WinRM push, no DPAPI).

- [ ] **Step 1: Rewrite `commands/bootstrap.rs` with a local result type**

```rust
//! Tauri commands for first-contact node onboarding.
//!
//! Remote WinRM push has been retired (SSH migration P5a). These commands are
//! kept registered + signature-frozen for the published UI: `bootstrap_winrm`
//! now returns a graceful "use the USB bootstrap" result and
//! `get_winrm_bootstrap_script` returns the SSH node-onboarder script.

use crate::data::{credentials as data_credentials, machines as data_machines, CredentialKind, Db};
use crate::error::{UecmError, UecmResult};
use serde::{Deserialize, Serialize};
use tauri::State;

/// Frozen response shape (Vue reads `.ok/.message/.manual_script`). Moved here
/// from the deleted `core::bootstrap`. Name kept for the Vue `WinrmBootstrapResult`
/// TS type's serde compatibility.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WinrmBootstrapResult {
    pub ok: bool,
    pub method: String,
    pub message: String,
    pub winrm_ok: bool,
    #[serde(default)]
    pub changed: Vec<String>,
    pub manual_script: Option<String>,
}

fn ssh_onboarder_script() -> String {
    include_str!("../../../ps-scripts/enable-ssh.ps1").to_string()
}

#[tauri::command]
pub fn get_winrm_bootstrap_script() -> UecmResult<String> {
    // Remote WinRM push retired; the manual onboarder is now the SSH node script.
    Ok(ssh_onboarder_script())
}

#[tauri::command]
pub fn bootstrap_winrm(
    db: State<'_, Db>,
    machine_id: i64,
    credential_alias: String,
    enable_local_account_remote_admin: bool,
) -> UecmResult<WinrmBootstrapResult> {
    let _ = enable_local_account_remote_admin; // accepted for back-compat; unused.
    // Validate inputs so the UI still gets precise errors, but do NOT push.
    let _machine = data_machines::find_by_id(&db, machine_id)?
        .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", machine_id)))?;
    let credential = data_credentials::find_by_alias(&db, &credential_alias)?.ok_or_else(|| {
        UecmError::InvalidInput(format!("credential alias '{}' not found", credential_alias))
    })?;
    if credential.kind != CredentialKind::Winrm {
        return Err(UecmError::InvalidInput(format!(
            "credential alias '{}' is not a WinRM credential",
            credential_alias
        )));
    }
    Ok(WinrmBootstrapResult {
        ok: false,
        method: "ssh-onboard-required".into(),
        message: "Remote WinRM push has been retired. Onboard this node with the \
                  UECM-Bootstrap.cmd USB bundle (uecm-cli ssh package-bootstrap), then \
                  use machine refresh over SSH.".into(),
        winrm_ok: false,
        changed: Vec::new(),
        manual_script: Some(ssh_onboarder_script()),
    })
}
```

- [ ] **Step 2: Build + test**

Run: `cd src-tauri && cargo test --lib 2>&1 | tail -5`
Expected: 997 passed. (`commands/bootstrap.rs` no longer imports `core::bootstrap`/`core::credentials`; `mark_seen` no longer called here.)

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/bootstrap.rs
git -c commit.gpgsign=false commit -m "refactor(commands): repoint bootstrap_winrm/get_winrm_bootstrap_script to SSH onboarding (shape frozen)"
```

---

## Task 4: Repoint `machine authorize` + remove WinRM preflight helpers

**Files:**
- Modify: `src-tauri/src/cli/domain_machine.rs` (authorize fn 630-777, helpers 600-628, tests ~913)
- Modify: `src-tauri/src/cli/args.rs` (`MachineAction::Authorize` + `DeepScan`/`Refresh` doc comments)

- [ ] **Step 1: Replace the `authorize` fn body with SSH guidance (no preflight/bootstrap)**

`authorize` must no longer call `preflight::preflight_path_b` or `bootstrap::enable_winrm_with_psexec`. It emits a graceful per-batch result pointing operators at the USB bundle. It no longer requires credentials (so it cannot fail on missing creds). `save_as`/`cred` are accepted-and-ignored shims (args frozen).

```rust
fn authorize(
    ctx: &mut Ctx<'_>,
    machine_ids: Vec<i64>,
    all: bool,
    save_as: Option<String>,
    cred: &crate::cli::credential_args::CredentialArgs,
) -> UecmResult<()> {
    let _ = (save_as, cred); // back-compat shims; remote push retired.
    let ids = {
        let db = ctx.require_db()?;
        resolve_target_ids(db, &machine_ids, all)?
    };
    ctx.emitter
        .emit_event(&Event::Completed {
            summary: json!({
                "machines": ids.len(),
                "remote_push": "retired",
                "hint": "Remote WinRM push is retired. Build a USB onboarding bundle with \
                         `uecm-cli ssh package-bootstrap --out <dir>`, run UECM-Bootstrap.cmd on \
                         each node, then `uecm-cli machine refresh <id>` over SSH.",
            }),
        })
        .ok();
    Ok(())
}
```

- [ ] **Step 2: Delete the now-dead `AuthorizeStep` enum + `authorize_decision_shallow`/`authorize_decision_deep` (domain_machine.rs:600-628) and their unit tests**

Grep for their test names and remove them: `authorize_decision_*` tests + the `authorize` "rejects when no creds" test (~line 913, behavior changed). Keep `deep_scan` + `resolve_target_ids` + all other tests.

- [ ] **Step 3: Update `args.rs` doc comments to SSH wording**

- `MachineAction::Refresh` doc (line 178): "WinRM probe + detect…" → "SSH probe + detect UE installs + GPUs."
- `MachineAction::DeepScan` doc (208-210): "WinRM-unreachable machines are skipped (run `machine authorize` first)" → "SSH-unreachable machines are skipped (re-onboard via UECM-Bootstrap.cmd) and the batch continues."
- `MachineAction::Authorize` doc (220-222): "winrm preflight -> bootstrap…" → "Deprecated: remote push retired. Emits guidance to build a USB bundle with `ssh package-bootstrap`." Update `--save-as` doc (drop "DPAPI alias").

- [ ] **Step 4: Build + test**

Run: `cd src-tauri && cargo test --lib 2>&1 | tail -8`
Expected: passes (fewer tests now that authorize_decision tests are gone; net should be ~997 minus removed authorize tests). Record the new count.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/cli/domain_machine.rs src-tauri/src/cli/args.rs
git -c commit.gpgsign=false commit -m "refactor(cli): retire machine authorize remote-push -> SSH USB guidance"
```

---

## Task 5: Update health remediation text (drop `winrm bootstrap` command references)

**Files:**
- Modify: `src-tauri/src/core/health_check.rs:90,98,106`
- Modify: `src-tauri/src/cli/domain_health.rs:295` (if it references winrm — verify; spec lists it)
- Modify (cosmetic, stale comments): `ps-scripts/query-gpu-driver.ps1:3`, `ps-scripts/query-ue-versions.ps1:2`, `ps-scripts/enable-ssh.ps1:5`

Keep the `"WinRM 5985"` / `winrm_open` port labels (§9 frozen — `machine scan` still probes 5985). Only the *remediation guidance strings* that say "run `uecm-cli winrm bootstrap`" change.

- [ ] **Step 1: Rewrite the three `probe_tcp_ports` remediation strings**

```rust
// tcp_5985:
"Onboard the node over SSH: build a USB bundle with `uecm-cli ssh package-bootstrap` \
 and run UECM-Bootstrap.cmd on the node (opens OpenSSH + SMB + node prep).",
// tcp_445:
"Open inbound TCP 445 (FPS-SMB-In-TCP firewall rule) and start LanmanServer. \
 UECM-Bootstrap.cmd does this when -EnableSmbServer is passed.",
// tcp_135:
"Switch network profile to Private (Public default blocks DCOM-In). RPC 135 is no \
 longer required for UECM transport (SSH); this row is informational.",
```

- [ ] **Step 2: Check + update `cli/domain_health.rs:295`**

Run: `rg -n "winrm bootstrap|run \`uecm-cli winrm" src-tauri/src/cli/domain_health.rs`
If it emits a `winrm bootstrap` remediation, reword to the SSH onboarding guidance above. (The "verify network + WinRM" online-hint at :295 is a connectivity note — reword "WinRM" → "SSH".)

- [ ] **Step 3: Fix stale script comments**

`query-gpu-driver.ps1:3` / `query-ue-versions.ps1:2` "Designed to run via `invoke-remote.ps1`" → "Designed to run as a node-pure script over SSH (stdin JSON)." `enable-ssh.ps1:5` "runs alongside enable-winrm.ps1" → "standalone SSH onboarder."

- [ ] **Step 4: Build + test + commit**

Run: `cd src-tauri && cargo test --lib 2>&1 | tail -5` (any health-text assertions still pass; verify none assert the old strings — `rg -n "winrm bootstrap" src-tauri/src` should show only not-yet-deleted winrm core).
```bash
git add src-tauri/src/core/health_check.rs src-tauri/src/cli/domain_health.rs ps-scripts/query-gpu-driver.ps1 ps-scripts/query-ue-versions.ps1 ps-scripts/enable-ssh.ps1
git -c commit.gpgsign=false commit -m "docs(health): SSH onboarding remediation; drop winrm bootstrap command refs"
```

---

## Task 6: Delete WinRM core (gate: Tasks 1–5 done, consumers repointed)

**Files (delete):**
- `src-tauri/src/core/winrm.rs`
- `src-tauri/src/core/bootstrap.rs`
- `src-tauri/src/core/preflight.rs`
- `src-tauri/src/cli/domain_winrm.rs`
- `ps-scripts/enable-winrm.ps1`
- `ps-scripts/bootstrap-winrm-remote.ps1`
- `ps-scripts/preflight-path-b.ps1`
- `ps-scripts/invoke-remote.ps1`
- `ps-scripts/test-winrm.ps1`
- `ps-scripts/__tests__/enable-winrm-lib.tests.ps1`

**Files (edit — drop registration):**
- `src-tauri/src/core/mod.rs` (drop `pub mod bootstrap; pub mod preflight; pub mod winrm;` + the entire `bootstrap_contract_tests` module 47-88)
- `src-tauri/src/cli/mod.rs` (drop `pub mod domain_winrm;`)
- `src-tauri/src/cli/run.rs` (drop `domain_winrm` from the `use` import; drop `Domain::Winrm { action } => domain_winrm::handle(...)` dispatch; drop the `Domain::Winrm { .. } => false` arm in `needs_db`; update the `needs_db` doc comment that says "winrm bootstrap-script")
- `src-tauri/src/cli/args.rs` (drop the `Domain::Winrm { action: WinrmAction }` variant 55-59 + the entire `WinrmAction` enum 271-317)

- [ ] **Step 1: Pre-delete grep gate — confirm no live consumers remain**

Run: `cd src-tauri/src && rg -n "winrm::|core::bootstrap|core::preflight|domain_winrm|WinrmAction|enable_winrm_with_psexec|manual_winrm_script|preflight_path_b" -g '*.rs' . | rg -v "winrm_open|winrm_ok"`
Expected: matches ONLY in the files about to be deleted (`winrm.rs`, `bootstrap.rs`, `preflight.rs`, `domain_winrm.rs`) + the `mod.rs`/`run.rs`/`args.rs`/`cli/mod.rs` registration lines about to be edited. If anything else appears, STOP and repoint it first.

- [ ] **Step 2: Delete the files**

```bash
cd /Users/bip.lan/AIWorkspace/vp/ue-cache-manager/.claude/worktrees/ssh-migration-p5
git rm src-tauri/src/core/winrm.rs src-tauri/src/core/bootstrap.rs src-tauri/src/core/preflight.rs src-tauri/src/cli/domain_winrm.rs \
       ps-scripts/enable-winrm.ps1 ps-scripts/bootstrap-winrm-remote.ps1 ps-scripts/preflight-path-b.ps1 \
       ps-scripts/invoke-remote.ps1 ps-scripts/test-winrm.ps1 ps-scripts/__tests__/enable-winrm-lib.tests.ps1
```

- [ ] **Step 3: Edit the registration files** (mod.rs / cli/mod.rs / run.rs / args.rs) per the file list above.

- [ ] **Step 4: Build + test**

Run: `cd src-tauri && cargo test --lib 2>&1 | tail -8`
Expected: green (count = post-Task-4 count minus the deleted winrm/bootstrap/preflight unit tests + `bootstrap_contract_tests`).
Run: `cargo check --all-targets 2>&1 | tail -5`
Expected: clean (no `unused import`, no missing module).

- [ ] **Step 5: Post-delete grep gate**

Run: `cd src-tauri/src && rg -n "winrm::|core::bootstrap|core::preflight|WinrmAction|domain_winrm" -g '*.rs' . | rg -v "winrm_open|winrm_ok"`
Expected: **zero** (all live refs gone). Stale doc-comment mentions of "WinRM" in unrelated files are acceptable but prefer clean.

- [ ] **Step 6: Commit (P5a complete)**

```bash
git add -A
git -c commit.gpgsign=false commit -m "feat(transport)!: delete WinRM core + winrm CLI domain (P5a teardown)"
```

---

# PHASE P5b — Remove DPAPI / cmdkey (gate: P3 + P4 done ✓; P5a removed the bootstrap resolve_password consumer)

## Task 7: Drop the DPAPI fallback in the two SecretStore readers

**Files:**
- Modify: `src-tauri/src/cli/credential_args.rs:99-127` (`resolve`)
- Modify: `src-tauri/src/core/secrets.rs:138-157` (`get_share_secret_migrating`)

- [ ] **Step 1: `credential_args.rs::resolve` — SecretStore only**

Replace the `match … None => resolve_password(alias)?` block (106-109) with a hard SecretStore lookup, and update the doc comment (drop the DPAPI-fallback paragraph 92-98 + the line-12 "DPAPI alias" → "saved alias"):

```rust
let pass = crate::core::secrets::SecretStore::from_config()?
    .get(alias)?
    .ok_or_else(|| UecmError::InvalidInput(format!(
        "no secret stored for credential alias '{}'", alias
    )))?;
```

- [ ] **Step 2: `secrets.rs::get_share_secret_migrating` — drop the DPAPI migration branch**

The function keeps its name + signature (4 callers). Body becomes a plain SecretStore read; update the doc comment (drop the "migrating legacy DPAPI" paragraph):

```rust
/// Read a share's svc secret from the SecretStore. (The legacy DPAPI fallback
/// was removed in P5b — the SecretStore is the only home now.)
pub fn get_share_secret_migrating(alias: &str) -> UecmResult<Option<String>> {
    SecretStore::from_config()?.get(alias)
}
```

- [ ] **Step 3: Build + test**

Run: `cd src-tauri && cargo test --lib 2>&1 | tail -5`
Expected: green. `credential_args` resolve tests still pass (inline/unknown-alias paths unchanged; the SecretStore miss now yields `InvalidInput` — verify `resolve_unknown_alias_returns_invalid_input` still holds since the alias isn't in SQLite anyway).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/cli/credential_args.rs src-tauri/src/core/secrets.rs
git -c commit.gpgsign=false commit -m "refactor(secrets): drop DPAPI fallback from cred-alias resolve + share secret read (P5b)"
```

---

## Task 8: Remove legacy cmdkey/DPAPI cleanup from credential delete paths

**Files:**
- Modify: `src-tauri/src/commands/credentials.rs` (delete_credential 46-72 + header comment)
- Modify: `src-tauri/src/cli/domain_cred.rs` (delete fn 128-186 + dry-run side_effects)

- [ ] **Step 1: `commands/credentials.rs::delete_credential` — SQLite + SecretStore only**

Remove the DPAPI `delete_password` block (lines 56-58) and the cmdkey `delete` block (lines 60-70). Drop the `use crate::core::credentials as core_creds;` import IF `normalize_username_for_storage` is its only other use here — it is still used at line 25 in `save_credential`, so KEEP the import. Update the header comment (drop "until they are removed in P5b"). Final `delete_credential`:

```rust
#[tauri::command]
pub fn delete_credential(db: State<'_, Db>, alias: String) -> UecmResult<()> {
    // SQLite metadata is the UI source of truth.
    data_creds::delete_by_alias(&db, &alias)?;
    // SecretStore (AES-GCM) is the only secret home now — best-effort orphan cleanup.
    if let Err(e) = SecretStore::from_config().and_then(|s| s.delete(&alias)) {
        tracing::warn!(alias = %alias, error = %e, "SecretStore delete failed; orphan secret may remain");
    }
    Ok(())
}
```

- [ ] **Step 2: `cli/domain_cred.rs::delete` — drop cmdkey (step 1) + DPAPI (step 4)**

Remove the `cm_result` cmdkey block (155-159), the step-4 `delete_password` block (171-178), and change the final `cm_result.map(...)` to a direct success emit. Update the dry-run `side_effects` array (143) to `["SQLite delete", "SecretStore delete (best-effort)"]`. Keep `use crate::core::credentials as core_creds;` only if `normalize_username_for_storage` (line 73) still needs it — it does (in `save_resolved`), so keep. Final delete tail:

```rust
    // SQLite delete — environment error if this fails.
    data_creds::delete_by_alias(db, alias)?;
    // SecretStore delete — best-effort (mirrors Tauri delete_credential).
    if let Err(e) = crate::core::secrets::SecretStore::from_config().and_then(|s| s.delete(alias)) {
        tracing::warn!(alias = %alias, error = %e, "SecretStore delete failed; orphan secret may remain");
    }
    let _ = ctx.emitter.emit_event(&crate::cli::output::Event::Completed {
        summary: serde_json::json!({ "alias": alias, "deleted": true }),
    });
    Ok(())
```

- [ ] **Step 3: Build + test + commit**

Run: `cd src-tauri && cargo test --lib 2>&1 | tail -5` → green.
```bash
git add src-tauri/src/commands/credentials.rs src-tauri/src/cli/domain_cred.rs
git -c commit.gpgsign=false commit -m "refactor(cred): delete paths use SQLite + SecretStore only (drop cmdkey/DPAPI)"
```

---

## Task 9: Strip cmdkey + DPAPI from `core/credentials.rs` (keep the username normalizer)

**Files:**
- Modify: `src-tauri/src/core/credentials.rs`
- Delete: `ps-scripts/cred-set.ps1`, `ps-scripts/cred-delete.ps1`, `ps-scripts/cred-list.ps1`, `ps-scripts/dpapi.ps1`

- [ ] **Step 1: Reduce `core/credentials.rs` to the normalizer only**

Delete: `CmdKeyResult`, `CmdKeyAlias`, `store`, `delete`, `list_uecm_aliases`, `store_password` (both cfg), `resolve_password` (both cfg), `delete_password` (both cfg), the `dpapi` module, and all `#[cfg(windows)]` JSON-store helpers (`store_path`/`read_store`/`write_store`/`base64_encode`/`base64_decode`). Delete the now-orphaned tests (`store_/delete_/list_returns_powershell_error_on_non_windows`, `store_password_/delete_password_is_noop_on_non_windows`). KEEP `normalize_username_for_storage` + its two tests. Rewrite the module doc comment:

```rust
//! Username normalization for credential metadata. (The cmdkey + DPAPI secret
//! store was removed in the SSH migration P5b; secrets now live in the
//! cross-platform `core::secrets::SecretStore`.)

use crate::error::UecmResult; // only if still referenced; normalize returns String, so this may be removable
```

Verify which imports remain needed (`UecmError`/`UecmResult`/`powershell`/`serde` likely all become unused — remove them; `normalize_username_for_storage` needs none). `cargo check` will flag unused imports.

- [ ] **Step 2: Delete the 4 PS scripts**

```bash
cd /Users/bip.lan/AIWorkspace/vp/ue-cache-manager/.claude/worktrees/ssh-migration-p5
git rm ps-scripts/cred-set.ps1 ps-scripts/cred-delete.ps1 ps-scripts/cred-list.ps1 ps-scripts/dpapi.ps1
```

- [ ] **Step 3: Build + test**

Run: `cd src-tauri && cargo test --lib 2>&1 | tail -8`
Expected: green; `core::credentials` now only exposes `normalize_username_for_storage`.
Run: `cargo check --all-targets 2>&1 | tail -5`
Expected: clean (no unused imports left in credentials.rs).

- [ ] **Step 4: Commit**

```bash
git add -A
git -c commit.gpgsign=false commit -m "feat(creds)!: remove cmdkey + DPAPI store (P5b); keep username normalizer"
```

---

## Task 10: Doc-comment cleanup + final gates

**Files:**
- Modify: `src-tauri/src/cli/args.rs` (`Domain::Cred` doc 65 "DPAPI + cmdkey + SQLite metadata" → "SecretStore (AES-GCM) + SQLite metadata"; `CredAction::Save` doc 324 "cmdkey + DPAPI + SQLite" → "SecretStore + SQLite"; `Authorize --save-as` "DPAPI alias" if not already done in Task 4)

- [ ] **Step 1: Fix the remaining DPAPI/cmdkey doc strings in args.rs**

`rg -n "DPAPI|cmdkey" src-tauri/src/cli/args.rs` and reword each to SecretStore wording.

- [ ] **Step 2: Final P5b grep gate**

Run: `cd src-tauri/src && rg -n "resolve_password|store_password|delete_password|list_uecm_aliases|dpapi|cred-set|cred-list|cred-delete" -g '*.rs' .`
Expected: **zero** live refs (no module, no callers). Comments may mention "DPAPI" historically but no symbol refs.

- [ ] **Step 3: Full mac gate suite**

Run: `cd src-tauri && cargo test --lib 2>&1 | tail -5` → green; record final count.
Run: `cargo check --all-targets 2>&1 | tail -5` → clean.
Run (from repo root): `pnpm tauri build --no-bundle 2>&1 | tail -15` → succeeds (NOT `cargo build --release` — memory `deploy_tauri_build`).

- [ ] **Step 4: Final design-spec grep (spec §11)**

Run: `cd src-tauri/src && rg -n "winrm|resolve_password|store_password|invoke-remote" -g '*.rs' .`
Expected: only `winrm_open`/`winrm_ok` (§9 frozen), `CredentialKind::Winrm` + `from_sql` (frozen), and historical comments. NO live `winrm::` / `core::bootstrap` / `core::preflight` / DPAPI symbol.

- [ ] **Step 5: Commit**

```bash
git add -A
git -c commit.gpgsign=false commit -m "docs(cli): SecretStore wording for cred domain help (P5b cleanup)"
```

---

## Windows handoff checklist (deferred — cannot run on mac)

Run on lanPC (`C:\Tools\UECM\uecm-cli.exe` after `pnpm tauri build --no-bundle` + copy; source repo `E:\AIWorkspace\vp\ue-cache-manager`):

- [ ] `uecm-cli ssh package-bootstrap --out C:\Temp\uecm-bundle` produces `UECM-Bootstrap.cmd` + `enable-ssh.ps1` + `uecm.pub` + `PsExec64.exe` + `README.txt` (no `UECM-Bootstrap-WinRM.ps1`).
- [ ] Double-click `UECM-Bootstrap.cmd` on a test node (razer): self-elevates, runs `enable-ssh.ps1` only, exits 0, node becomes `ssh uecm-svc@<node>`-reachable. (Watch the StagingDir-quote bug — memory `bootstrap_cmd_stagingdir_quote_bug`.)
- [ ] `uecm-cli machine refresh <id>` over SSH still works (probe + UE/GPU).
- [ ] `uecm-cli secret set/get/list/delete` + `uecm-cli cred save/list/delete` round-trip (SecretStore + SQLite).
- [ ] Tauri UI smoke: CredentialDialog save/list/delete; machine-detail bootstrap panel `bootstrapWinrm` returns the graceful guidance (not a crash); `kind==='winrm'` selectors still populate.
- [ ] zen full chain + share/distribute E2E (regression — P5 touched no zen/share logic, but confirm).

---

## Self-review (writing-plans)

- **Spec coverage:** §16 terminal state — winrm.rs/domain/scripts ✓ T6; DPAPI store/resolve/list + cred-*/dpapi.ps1 ✓ T7-9; WinRM bootstrap scripts + bootstrap.rs/preflight.rs ✓ T6; enable-ssh.ps1 onboarder (already P0) + UECM-Bootstrap.cmd pure SSH ✓ T2; ssh package-bootstrap replaces winrm bootstrap-script ✓ T1; bootstrap_winrm/get_winrm_bootstrap_script repoint (shape frozen) ✓ T3; machine authorize repoint ✓ T4; CredentialKind::Winrm + from_sql kept (frozen, no task — already done P4) ✓; health remediation text ✓ T5; §9 frozen items untouched ✓ (T5 keeps port labels; final greps exclude winrm_open/winrm_ok).
- **Deferred (confirmed with user):** `ssh authorize` (T1 TODO comment).
- **Placeholder scan:** none — every step shows concrete code/edits/commands.
- **Type consistency:** `WinrmBootstrapResult` shape identical across T3 (moved struct) and frozen Vue contract; `get_share_secret_migrating` name/signature preserved (T7) for its 4 callers; `normalize_username_for_storage` preserved across T8/T9.
- **Gate order:** Replacement (T1) + repoints (T3-5) precede WinRM deletion (T6); DPAPI fallback drop (T7) + cleanup (T8) precede credentials.rs strip (T9). P5a (T1-6) fully precedes P5b (T7-10).
