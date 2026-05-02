# UECM Plan 3 — Elevation, SMB Shares & Cluster Batch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

## Execution Mode (READ FIRST — overrides default skill behavior)

**Mode: AUTO-CONTINUOUS.** Run all 19 tasks back-to-back without pausing for human approval between them. Same rules as Plan 2.

**Stop and ask the user ONLY in these cases:**

1. **Plan vs reality conflict** that requires re-design (structural mismatch where continuing would produce wrong work).
2. **Destructive operation requiring authorization**: creating local Windows accounts on lanPC (Mode B `ddc-svc`), modifying Group Policy, opening firewall ports, dropping production credentials, `git push --force`, `rm -rf` outside the workspace, modifying SSH config.
3. **Critical-severity code review finding with no obvious fix.**
4. **lanPC unreachable, WinRM disabled, or PsExec download blocked** when an E2E verification step requires it.
5. **A new dependency decision** not covered by the plan.

**Do NOT stop for:** spec/quality review finding Important / Minor issues (fix in `fix:` follow-up commit, proceed); Windows-gated tests skipped on macOS; DONE_WITH_CONCERNS observations; README/docs cleanup.

**Final report:** commit list, frontend + backend test counts, every DONE_WITH_CONCERNS verbatim, production build outcome, deferred lanPC E2E steps awaiting user.

---

**Goal:** Plan 3 builds three pieces on top of Plan 2's single-machine foundation:

1. **Elevation & explicit credentials** — make UECM able to do admin-level operations (HKLM env vars, share creation, registry edits) regardless of how it was launched, and let WinRM use the cmdkey-stored credentials explicitly instead of relying on implicit current-user token.
2. **SMB share creation wizard** (Module B1 from spec) — Mode A (open Guest) + Mode B (`ddc-svc` account + SYSTEM cmdkey injection via PsExec). Ends with a working DDC share that even a non-interactive Windows Service (e.g. RenderStream Service) can read.
3. **Cluster batch operations** — apply env var or INI changes to N machines in one operation, with per-machine progress reporting and partial-failure tolerance. Also: foundation polish from Plan 2 E2E (last_seen_at, hostname rename, GPU VRAM DXGI fallback).

**Why this order:** Elevation is a hard blocker for everything else in this plan. Without proper admin context, Mode B can't `New-LocalUser`, can't `New-SmbShare`, can't write to `HKLM\System\CurrentControlSet`. So Phase 0 lands first. Once elevation is correct, the SMB wizard (Phase 3) and the cluster batch ops (Phase 2) become straightforward extensions of Plan 2 patterns.

**Architecture additions:**
- All admin-required PS scripts gain a `-Credential` parameter. UECM calls into a new `core::winrm::invoke_with_credential` that pipes username + plaintext password as named arguments, the script wraps them via `New-PSCredential` and passes to `Invoke-Command -Credential`. Password lifetime in UECM memory is the duration of one IPC call.
- The Tauri Windows manifest **opt-out** of `requireAdministrator` (we keep launch UAC-free for normal browse/scan/view). Elevation comes from per-call credentials, not from elevating the whole process.
- New `core::psexec` for SYSTEM-context invocations (only used by Mode B + share-credential injection).
- New `core::shares` for `New-SmbShare` / `Set-SmbShareAccess` wrappers.
- New `core::batch` orchestrating per-machine async fan-out with progress events emitted to the frontend via `tauri::Window::emit`.
- Frontend gains a Share wizard, a Batch operations modal, and a Hostname rename inline edit.

**Tech Stack:** Builds on Plan 2 stack. New Rust deps: `tokio = { features = ["sync", "macros"] }` (already present, but we use `mpsc` for progress channels). New PowerShell scripts under `ps-scripts/`. PsExec64.exe must be obtainable on first wizard launch (download from Sysinternals, vendored alternative discussed below). No new frontend deps.

**Out of scope for this plan (deferred):**
- INI conflict scanner + auto-fix (Plan 4)
- Cluster health check matrix (Plan 4)
- DDC Pak generation + distribution (Plan 5)
- PSO Cache operations (Plan 6)
- Visual polish, Figma application (Plan 6)
- Group Policy deployment of credentials (out of scope indefinitely — too invasive)
- Active Directory integration (out of scope; assume workgroup or local accounts)

**Deliverable at end:**
1. UECM launches without UAC prompt; admin operations either prompt for elevation per-action OR succeed transparently when valid `-Credential` is supplied.
2. User can run "Create SMB Share" wizard, pick Mode A (open) → host shares `\\HOST\DDC` with Everyone:Full; or Mode B (managed) → host creates local `ddc-svc` account, shares with that account only, and clients receive both user + SYSTEM cmdkey entries.
3. After Mode B, RenderStream Service (running as `LocalSystem`) successfully accesses the share.
4. User can multi-select machines + apply one env var / INI key change to all of them at once, with a real-time per-machine progress UI (✓ / ✗ / running).
5. `refresh_machine` updates `machines.last_seen_at` + `status`; UI online badge actually reflects truth.
6. User can rename a machine's hostname inline from the detail panel.
7. GPU VRAM correctly reports >4 GB GPUs (RTX 3080 shows 10 GB, not 4095 MB).
8. Production build still succeeds; all tests green; full E2E on lanPC verified end-to-end.

---

## Lessons from Plan 2 E2E (2026-05-02) — Folded into Plan 3 Tasks

These are real bugs discovered during lanPC validation that Plan 3 explicitly closes:

| # | Issue | Plan 3 task |
|---|---|---|
| L1 | `script_path` was cwd-relative; built `.exe` couldn't find ps-scripts. **Fixed in commit `a0c0777`** (smart `current_exe()` ancestor search + `bundle.resources`). Verify still works after Plan 3 changes. | T1 verification step |
| L2 | `write-ini-key.ps1` returned `Invoke-Command` value as wrapped PSObject; JSON deserialize failed. **Fixed in commit `f6b40ff`** (`"$remoteResult"` cast). Audit other PS scripts for the same pattern. | T1 audit step |
| L3 | UECM standard-token process can't write HKLM env var / create shares — Plan 2 needed RunAs Administrator workaround. | **T2-T3 (Phase 0 entire)** |
| L4 | `refresh_machine` doesn't update `machines.last_seen_at` / `status`. UI online badge stuck on `unknown`. | **T5** |
| L5 | `add_discovered_machine` ignores supplied hostname when machine row already exists; need separate rename command. | **T6** |
| L6 | `refresh_machine` drops `detected_ue` when `detect_gpus` fails; partial-failure UI shows empty UE list contradicting reality. | **T5 fold-in (persist UE before GPU detect)** |
| L7 | GPU VRAM reports 4095 MB cap (WMI `AdapterRAM` is unsigned 32-bit). RTX 3080 should show 10 GB. | **T7** |
| L8 | NSIS bundle skipped because GitHub releases `nsis-3.11.zip` download timed out from China network. | **T19 (vendored mirror in build docs)** |
| L9 | P4 client `noallwrite` blocks `Cargo.toml` rewrite during `tauri build`; lanPC dev-tool workspace must use `allwrite`. | Prereq doc |
| L10 | P4 graph depot free-tier license cap = 3 repos. | Prereq doc |
| L11 | Rust install via rust-lang.org / GitHub releases hangs on China network; use rsproxy.cn / npmmirror.com. | Prereq doc |

---

## Prerequisites (engineer must have before starting)

Same as Plan 2, plus:

### lanPC (designated Windows test machine, `192.168.10.20`)

Already established in Plan 2:
- WinRM service enabled and `TrustedHosts = '*'`
- Rust 1.95+ via rsproxy.cn mirror
- Node.js 20+, pnpm 10+, MSVC Build Tools 2022 with VCTools workload
- p4 CLI present at `D:\Program Files\Perforce\p4.exe`
- P4 client `super_lanPC_uecm` with `Options: allwrite`, view `//ue/ue-cache-manager/... //super_lanPC_uecm/ue-cache-manager/...`, root `E:\code`

**New for Plan 3:**

- **PsExec64.exe** — must be obtainable on lanPC. Three options:
  1. **Vendor in repo** under `vendor/PsExec64.exe` (Sysinternals license allows redistribution). Easiest, but bumps repo size by ~700 KB.
  2. **Download on first wizard run** from `https://download.sysinternals.com/files/PSTools.zip`, extract, cache to `%LOCALAPPDATA%\UECM\PsExec64.exe`. Requires internet on first use; can be slow from China.
  3. **Pre-stage manually** to `%LOCALAPPDATA%\UECM\PsExec64.exe` before first run.
  - Plan 3 default: **option 1** (vendor it). Smallest UX risk. Document in README.

- **Local Mode B verification target on lanPC:**
  - Mode B will create a local user `ddc-svc` and a share `DDC` on `D:\` (or `E:\`, configurable). Verify these don't pre-exist on lanPC (`Get-LocalUser ddc-svc` should error; `Get-SmbShare DDC` should error). If they do, Plan 3 wizard will detect and offer "use existing" / "recreate" path.
  - **Cleanup hook**: at end of E2E, the verifier should `Remove-LocalUser ddc-svc; Remove-SmbShare -Name DDC -Force; Remove-Item D:\DDC -Recurse -Force` to leave lanPC clean. Wizard does NOT auto-cleanup (would be destructive).

- **Test harness machine for batch:** Plan 3 batch ops should target ≥2 machines. macOS dev box (`192.168.10.10`) doesn't have WinRM. Options:
  1. Spin up a second Windows VM on lanPC (`lanbipu-razer` laptop is the natural second target if you can wake it up)
  2. Loop the test by adding `127.0.0.1` AND `192.168.10.20` as two "machines" in UECM (both resolve to lanPC; tests batch dispatch logic without needing a real second host)
  - Default: **option 2** for unit-equivalent E2E; option 1 if user has lanbipu-razer awake

### Sysinternals license

PsExec is part of Sysinternals Suite. Microsoft EULA permits redistribution provided the license file ships alongside. Vendor `vendor/PsExec64.exe` plus `vendor/Sysinternals-EULA.txt` together. Add a one-line attribution to README.

### One-time PowerShell ExecutionPolicy

Plan 2 sidesteps ExecutionPolicy via `-ExecutionPolicy Bypass` flag on each invocation. Mode B's `Invoke-Command -Credential` over WinRM may hit a stricter policy (`Restricted` / `AllSigned`) on the remote endpoint. Verify on lanPC:

```powershell
Get-ExecutionPolicy -List
```

If `LocalMachine` is `Restricted`, run once as admin: `Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope LocalMachine`. Plan 3 PS scripts assume `RemoteSigned` or looser on the target.

### Cross-platform testing rules (recap from Plan 2)

- All PowerShell-dependent Rust unit tests are gated `#[cfg(windows)]`. Skipped on macOS.
- Frontend tests run on macOS via vitest.
- E2E (wizards, share creation, batch ops, SYSTEM cred injection) run on lanPC via the built `.exe`.

---

## File Structure

```
ue-cache-manager/
├── vendor/
│   ├── PsExec64.exe                            # NEW (Sysinternals, vendored)
│   └── Sysinternals-EULA.txt                   # NEW
│
├── ps-scripts/
│   ├── (existing Plan 2 scripts)
│   ├── invoke-remote.ps1                       # MODIFY (accept -Credential)
│   ├── setx-machine.ps1                        # MODIFY (accept -Credential)
│   ├── getx-machine.ps1                        # MODIFY (accept -Credential)
│   ├── write-ini-key.ps1                       # MODIFY (accept -Credential)
│   ├── read-ini-section.ps1                    # MODIFY (accept -Credential)
│   ├── query-ue-versions.ps1                   # (no change; runs as scriptblock body)
│   ├── query-gpu-driver.ps1                    # MODIFY (DXGI VRAM fallback)
│   ├── setup-share-mode-a.ps1                  # NEW
│   ├── setup-share-mode-b.ps1                  # NEW
│   ├── inject-system-credential.ps1            # NEW (uses bundled PsExec64)
│   ├── remove-share.ps1                        # NEW (cleanup helper)
│   └── mark-machine-seen.ps1                   # NEW (lightweight ping for status)
│
├── src-tauri/
│   ├── tauri.conf.json                         # MODIFY (resources include vendor/)
│   └── src/
│       ├── lib.rs                              # MODIFY (register new commands + emit events)
│       │
│       ├── commands/
│       │   ├── batch.rs                        # NEW (batch_set_env_var, batch_set_ini_key)
│       │   ├── credentials.rs                  # MODIFY (no schema change; pass-through API)
│       │   ├── discovery.rs                    # MODIFY (refresh_machine updates last_seen_at)
│       │   ├── env_vars.rs                     # MODIFY (accept credential alias)
│       │   ├── ini_editor.rs                   # MODIFY (accept credential alias)
│       │   ├── machines.rs                     # MODIFY (rename_machine command)
│       │   ├── shares.rs                       # NEW
│       │   └── mod.rs                          # MODIFY
│       │
│       ├── core/
│       │   ├── batch.rs                        # NEW (fan-out orchestration with mpsc progress)
│       │   ├── credentials.rs                  # MODIFY (resolve_password_via_powershell)
│       │   ├── discovery.rs                    # (no change)
│       │   ├── env_vars.rs                     # MODIFY (set_with_credential / get_with_credential)
│       │   ├── ini_editor.rs                   # MODIFY (set_key_with_credential / read_section_with_credential)
│       │   ├── network.rs                      # MODIFY (probe_host returns latency_ms)
│       │   ├── psexec.rs                       # NEW
│       │   ├── shares.rs                       # NEW
│       │   ├── winrm.rs                        # MODIFY (invoke_with_credential)
│       │   ├── powershell.rs                   # (no change beyond a0c0777)
│       │   └── mod.rs                          # MODIFY
│       │
│       └── data/
│           ├── machines.rs                     # MODIFY (rename, mark_seen)
│           ├── share_configs.rs                # NEW (CRUD for share_configs table)
│           ├── schema.rs                       # MODIFY (migration 006 share_configs)
│           └── mod.rs                          # MODIFY
│
├── src/
│   ├── services/tauri.ts                       # MODIFY (new types + functions)
│   │
│   ├── stores/
│   │   ├── batch.ts                            # NEW
│   │   ├── shares.ts                           # NEW
│   │   └── machines.ts                         # MODIFY (rename action, last_seen reactive)
│   │
│   ├── components/
│   │   ├── machines/
│   │   │   ├── MachineDetail.vue               # MODIFY (rename inline edit, online badge)
│   │   │   └── HostnameEditor.vue              # NEW (small inline editable field)
│   │   ├── modals/
│   │   │   ├── ShareCreateWizard.vue           # NEW
│   │   │   ├── BatchEnvVarModal.vue            # NEW
│   │   │   ├── BatchIniEditModal.vue           # NEW
│   │   │   └── (existing modals)
│   │   └── batch/
│   │       └── BatchProgressTable.vue          # NEW (reusable per-machine progress rows)
│   │
│   ├── views/
│   │   ├── Machines.vue                        # MODIFY (multi-select for batch)
│   │   └── DDCPak.vue                          # (no change in Plan 3)
│   │
│   └── __tests__/
│       ├── ShareCreateWizard.spec.ts           # NEW
│       ├── BatchEnvVarModal.spec.ts            # NEW
│       ├── BatchIniEditModal.spec.ts           # NEW
│       ├── BatchProgressTable.spec.ts          # NEW
│       ├── HostnameEditor.spec.ts              # NEW
│       ├── batch-store.spec.ts                 # NEW
│       ├── shares-store.spec.ts                # NEW
│       └── (existing Plan 2 specs unchanged)
│
└── README.md                                   # MODIFY (Plan 3 status + PsExec attribution + NSIS pre-stage note)
```

---

## Approach Notes

**Elevation strategy:** Two layers.

1. **Per-call credential injection** (preferred). UECM resolves the per-machine credential alias from SQLite + cmdkey (or asks the user once and caches in OS keyring), then passes username + password to the PS sidecar via named arguments. The sidecar wraps as `PSCredential` and passes to `Invoke-Command -Credential`. WinRM authenticates as the credential's user, runs the scriptblock with that user's full unfiltered token. Works without UAC prompts in the UECM process.
2. **Local-process elevation fallback** (rare). If a credential isn't stored and the operation requires admin on the local host (only applies to `inject-system-credential.ps1` against `127.0.0.1`), UECM displays a "Run as Administrator required" dialog with a one-click "Re-launch elevated" button (uses Windows `runas` verb via `ShellExecute`). After elevation, the user re-clicks the originating action.

We deliberately do NOT mark the whole UECM process as `requireAdministrator` — most actions (browse, scan, view detail) are read-only and shouldn't pop UAC on launch. Per-call credentials win for UX.

**Password handling:** Plaintext password lives in UECM Rust memory only for the duration of one IPC call (≤1 second). It's passed to `powershell.exe` via stdin (avoids process command-line exposure to `Get-Process` / Process Explorer) and zeroed after `wait_with_output`. The script reads it from stdin into a `[SecureString]` via `Read-Host -AsSecureString` equivalent, then constructs `PSCredential`. We never write the password to disk; cmdkey storage is opaque to UECM.

**SMB share creation flow (Mode B detail):**

```
1. Host (operator picks one machine, must be admin-credentialed):
   - New-LocalUser -Name "ddc-svc" -Password <generated 24-byte>
   - Add-LocalGroupMember -Group "Users" "ddc-svc"
   - mkdir D:\DDC
   - icacls D:\DDC /grant "ddc-svc:(OI)(CI)F"
   - New-SmbShare -Name "DDC" -Path "D:\DDC" -FullAccess "ddc-svc"
2. Save host credential alias UECM:share-host:<host> + ddc-svc password to cmdkey
3. For each client machine (operator multi-selects):
   - cmdkey /add:HOST /user:ddc-svc /pass:<password>      # user-level
   - psexec -s -i 0 cmdkey /add:HOST /user:ddc-svc /pass:<password>   # SYSTEM-level
   - Verify: Test-Path \\HOST\DDC (should succeed)
4. Persist to SQLite share_configs table.
```

The 24-byte password is generated locally (macOS or operator machine), pushed to host via Mode B script, then pushed to each client via the same mechanism. After persisting credential aliases, UECM never reads the password back; future SMB access is transparent (Windows uses the cmdkey-stored credential for `\\HOST\DDC`).

**Cluster batch ops:** UECM holds a `tokio::mpsc::channel` per batch operation. Each per-machine task pushes `BatchEvent { machine_id, status: "running"|"ok"|"err", message }`. The Tauri command yields the receiver as a stream the frontend subscribes to via `tauri::Window::emit("batch-progress", payload)`. Frontend `BatchProgressTable.vue` renders rows with reactive status. Default concurrency = 8 simultaneous machines (semaphore-capped, same idiom as `scan_cidr`).

**Backwards compatibility:** All Plan 2 single-machine commands (`set_machine_env_var`, `get_machine_env_var`, `read_ini_section`, `set_ini_key`) gain an optional `credential_alias: Option<String>` parameter. When `None`, behavior is identical to Plan 2 (uses current process token; works only when UECM is elevated). When `Some`, dispatches to `*_with_credential` variant. Frontend defaults to `None` for backward compat; new wizards explicitly pass the alias.

**No DB migration for credential alias on existing commands** — the alias is supplied per-call by the frontend, derived from `data::credentials::find_by_alias`. We don't add a foreign key from machines to credentials; the linkage is operational, not structural.

---

## Task 1: Audit + verify Plan 2 fixes still hold (no new code)

**Files:** none modified.

This is a **pre-flight audit**, not implementation. Goal: confirm the three E2E fixes from Plan 2 (commits `a0c0777`, `f6b40ff`, `6529c1e`) still pass tests on a fresh build, before we touch anything.

- [ ] **Step 1: Confirm script_path resolves correctly in test runtime**

```bash
export PATH="/Users/bip.lan/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
cd src-tauri && cargo test --lib core::powershell core::discovery 2>&1 | tail -10 && cd ..
```

Expected: tests pass; the `discovery_scripts_are_loadable` test in `core/discovery.rs` confirms `read_script("query-ue-versions.ps1")` returns the actual PS body (i.e., the smart ancestor search works from cargo test cwd).

- [ ] **Step 2: Audit other PS scripts for Invoke-Command string-wrap pattern**

Search for any PS file where `$x = Invoke-Command ... { return <single string> }` is followed by JSON serialization that uses `$x` directly:

```bash
grep -nE 'Invoke-Command.*ScriptBlock' ps-scripts/*.ps1
```

Inspect each match. The fix from `f6b40ff` was on `write-ini-key.ps1` only. Other scripts that return arrays (e.g. `read-ini-section.ps1` returning `,$result`) are safe because PowerShell array serialization handles wrapped elements correctly. Scripts that return primitives via `return` MUST be cast via `"$result"` before JSON. Document findings in commit message — fix any new occurrences.

- [ ] **Step 3: Run full Plan 2 test suite + sanity build**

```bash
pnpm test
cd src-tauri && cargo test --lib && cd ..
```

Expected: 54 frontend + 48 backend tests pass (Plan 2 final state). If anything fails, **STOP** and report — Plan 3 must start from a green baseline.

- [ ] **Step 4: Commit (audit-only, may be empty)**

If Step 2 found no new issues, no commit needed. If it did, commit:

```bash
git add ps-scripts
git commit -m "fix(ps): audit Invoke-Command string-wrap pattern across all sidecars"
```

---

## Task 2: Add `-Credential` parameter to WinRM-using PS scripts

**Files:**
- Modify: `ps-scripts/invoke-remote.ps1`
- Modify: `ps-scripts/setx-machine.ps1`
- Modify: `ps-scripts/getx-machine.ps1`
- Modify: `ps-scripts/write-ini-key.ps1`
- Modify: `ps-scripts/read-ini-section.ps1`

Each script gains optional `-Username` + `-Password` parameters (NOT `-Credential` directly because PowerShell `[PSCredential]` parameter binding is brittle from CLI). Inside the script, if both are supplied, build a `PSCredential` and pass to `Invoke-Command -Credential`. If neither supplied, fall back to current Plan 2 behavior (current-user token).

- [ ] **Step 1: Modify `ps-scripts/setx-machine.ps1`**

Replace existing param block + Invoke-Command call. Final shape:

```powershell
# Sets a system-level environment variable on a remote host via WinRM.
# Parameters: -HostName <string> -Name <string> -Value <string>
#             [-Username <string>] [-Password <string>]
# Output: JSON { ok: bool, message: string }

param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$Name,
    [Parameter(Mandatory=$true)] [string]$Value,
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
        param($Name, $Value)
        [System.Environment]::SetEnvironmentVariable($Name, $Value, 'Machine')
        $readback = [System.Environment]::GetEnvironmentVariable($Name, 'Machine')
        if ($readback -ne $Value) { throw "verify failed: read '$readback', expected '$Value'" }
        return $true
    }
    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock  = $script
        ArgumentList = @($Name, $Value)
        ErrorAction  = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    Invoke-Command @invokeArgs | Out-Null
    @{ ok = $true; message = "set $Name on $HostName" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
```

- [ ] **Step 2: Apply identical pattern to `getx-machine.ps1`** (params + `Build-CredentialOrNull` + splat `$invokeArgs`).

- [ ] **Step 3: Apply identical pattern to `write-ini-key.ps1` + `read-ini-section.ps1`.** Preserve the `"$remoteResult"` cast from `f6b40ff` in `write-ini-key.ps1`.

- [ ] **Step 4: `invoke-remote.ps1`** — add `-Username` + `-Password`. The script body (read from stdin) doesn't change; only the wrapping `Invoke-Command` does.

- [ ] **Step 5: SSH manual smoke on lanPC** — run `setx-machine.ps1` once with credentials supplied, verify it works:

```bash
# From macOS, replacing <PWD> with lanPC's lanpc account password
ssh lanpc "powershell -NoProfile -ExecutionPolicy Bypass -File E:\code\ue-cache-manager\ps-scripts\setx-machine.ps1 -HostName 192.168.10.20 -Name UECM_TEST -Value hello -Username lanpc -Password '<PWD>'"
```

Expected: `{"ok":true,"message":"set UECM_TEST on 192.168.10.20"}`. Verify on lanPC: `[Environment]::GetEnvironmentVariable('UECM_TEST','Machine')` returns `hello`. Cleanup: `[Environment]::SetEnvironmentVariable('UECM_TEST', $null, 'Machine')`.

If user prefers not to type their password into a terminal, mark this step DONE_WITH_CONCERNS and rely on Task 4's E2E.

- [ ] **Step 6: Commit**

```bash
git add ps-scripts/{invoke-remote,setx-machine,getx-machine,write-ini-key,read-ini-section}.ps1
git commit -m "feat(ps): add optional -Username/-Password to WinRM sidecars for explicit cred passing"
```

---

## Task 3: Rust `core::winrm::invoke_with_credential` + propagate to env_vars / ini_editor

**Files:**
- Modify: `src-tauri/src/core/winrm.rs`
- Modify: `src-tauri/src/core/env_vars.rs`
- Modify: `src-tauri/src/core/ini_editor.rs`
- Modify: `src-tauri/src/commands/env_vars.rs`
- Modify: `src-tauri/src/commands/ini_editor.rs`
- Modify: `src-tauri/src/services/tauri.ts`

Add `_with_credential` variants without breaking the existing zero-credential signatures.

- [ ] **Step 1: Extend `core/winrm.rs`**

Add to the existing module:

```rust
/// Invoke a remote scriptblock body with explicit credentials. Behavior on
/// non-Windows is identical to `invoke()`: returns `UecmError::PowerShell`.
#[cfg(windows)]
pub fn invoke_with_credential(
    host: &str,
    script_body: &str,
    username: &str,
    password: &str,
) -> UecmResult<String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let wrapper = powershell::script_path("invoke-remote.ps1");
    let mut child = Command::new("powershell.exe")
        .arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(&wrapper)
        .arg("-HostName")
        .arg(host)
        .arg("-Username")
        .arg(username)
        .arg("-Password")
        .arg(password)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| UecmError::PowerShell(format!("failed to spawn powershell.exe: {}", e)))?;
    {
        let stdin = child.stdin.as_mut().ok_or_else(|| {
            UecmError::PowerShell("failed to open stdin".to_string())
        })?;
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
pub fn invoke_with_credential(
    host: &str,
    script_body: &str,
    username: &str,
    password: &str,
) -> UecmResult<String> {
    let _ = (host, script_body, username, password);
    Err(UecmError::PowerShell("WinRM is Windows-only".to_string()))
}
```

Note: passing `username` + `password` as `-Username <user> -Password <pass>` argv flags is acceptable here because (a) script body is on stdin and (b) Windows process command-line is not visible to other users by default. If we later need stricter protection, switch to passing both via stdin too.

- [ ] **Step 2: Extend `core/env_vars.rs`** — add `set_with_credential` + `get_with_credential` mirroring `set` / `get` but invoking `setx-machine.ps1` with `-Username` + `-Password` args. Use `powershell::run_json` directly (script accepts the new params optionally per Task 2).

- [ ] **Step 3: Extend `core/ini_editor.rs`** — add `read_section_with_credential` + `set_key_with_credential`.

- [ ] **Step 4: Extend `commands/env_vars.rs` + `commands/ini_editor.rs`** — each existing command gets a sibling that takes `credential_alias: String`, looks up the alias in `data::credentials`, retrieves the password from cmdkey via a new helper `core::credentials::resolve_password(alias)` (PowerShell sidecar that calls `cmdkey /list:<alias>` … wait, cmdkey doesn't expose passwords by design).

- [ ] **Step 4a: Decision point — how UECM gets the password**

Cmdkey does NOT allow reading back stored passwords (security feature). Three viable approaches:

1. **Re-prompt the user each time** they pick a credential alias → secure, but UX heavy
2. **Use Windows Data Protection API (DPAPI) directly** in UECM Rust to encrypt and store the password under the user profile, separate from cmdkey → robust + transparent, but deviates from "cmdkey only" spec
3. **Use Windows Credential Manager via `vault::PasswordVault` API** instead of cmdkey → similar to DPAPI but uses the Credential Manager UI

Plan 3 default: **option 2** (DPAPI). Add a Rust crate `windows = { features = ["Win32_Security_Cryptography_DataProtection"] }` and store ciphertext under `%LOCALAPPDATA%\UECM\creds.bin`. Cmdkey storage remains for backwards compat with Plan 2 alias listing, but passwords also live in DPAPI-encrypted blob keyed by alias.

This means Task 11's Mode B share creation will write the generated `ddc-svc` password to BOTH cmdkey (so SMB transparent auth works) AND DPAPI (so UECM can re-supply it for `Invoke-Command -Credential`).

If user prefers option 1 (re-prompt) for security purity, mark this point and ship without DPAPI; UI will pop a "Confirm credential" mini-modal each time.

- [ ] **Step 4b: Implement `core::credentials::resolve_password(alias)`** per option 2 above. New file or appendix to existing.

- [ ] **Step 5: Tauri commands** — add `set_machine_env_var_with_credential(machine_id, name, value, credential_alias)` etc. Frontend types + tauriApi entries.

- [ ] **Step 6: Tests** — add cfg(not(windows)) tests asserting `_with_credential` variants return `UecmError::PowerShell` on macOS.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src ps-scripts src/services/tauri.ts
git commit -m "feat: per-call credential injection for WinRM operations (env_vars, ini_editor)"
```

---

## Task 4: lanPC E2E — verify per-call credential path replaces RunAs requirement

**Files:** none modified (verification task).

- [ ] **Step 1: Push commits + p4 sync to lanPC** (you do this manually or via an SSH session)

- [ ] **Step 2: Sync ps-scripts + rebuild on lanPC**

```powershell
ssh lanpc "p4 sync"
ssh lanpc "cd /d E:\code\ue-cache-manager && pnpm tauri build"
```

- [ ] **Step 3: Launch UECM on lanPC NORMALLY (no RunAs Administrator)**

Double-click `E:\code\ue-cache-manager\src-tauri\target\release\uecm.exe`. Verify no UAC prompt appears (Plan 3 explicitly chose to NOT manifest as `requireAdministrator`).

- [ ] **Step 4: Add credential first, then env var with credential**

In UECM:
1. Credentials → Save `UECM:winrm:LANPC` / kind=winrm / user=`lanpc` / password=Microsoft account password
2. Machines → Refresh on 192.168.10.20 → verify still works (Refresh uses `_with_credential` variant when alias exists)
3. Env vars → set `UE-SharedDataCachePath` = `\\TEST\DDC2` → Apply

Expected: green "Applied + verified." WITHOUT having launched UECM as administrator. The credential pipe replaces the elevation requirement.

- [ ] **Step 5: Verify HKLM via SSH**

```bash
ssh lanpc 'powershell -c "[Environment]::GetEnvironmentVariable(\"UE-SharedDataCachePath\", \"Machine\")"'
```

Expected: `\\TEST\DDC2`

- [ ] **Step 6: Cleanup**

```bash
ssh lanpc 'powershell -c "[Environment]::SetEnvironmentVariable(\"UE-SharedDataCachePath\", $null, \"Machine\")"'
```

If Step 4 fails with "Access denied" despite supplying credentials, the credential pipe didn't actually engage. Diagnose by running the PS script directly via SSH (passes credentials explicitly) and comparing UECM's PS spawn (look at Process Monitor or attach Process Hacker to see actual command-line UECM spawned).

- [ ] **Step 7: Commit (no code, just task closure)**

If E2E passed, no commit; record ✅ in summary report. If a fix was needed, commit it.

---

## Task 5: `mark_seen` + `refresh_machine` updates last_seen_at + status; persist UE before GPU detect

**Files:**
- Modify: `src-tauri/src/data/machines.rs`
- Modify: `src-tauri/src/commands/discovery.rs`
- Modify: `src-tauri/src/services/tauri.ts`
- Modify: `src/components/machines/MachineDetail.vue`
- Modify: `src/__tests__/MachineDetail.spec.ts`

- [ ] **Step 1: Add `data::machines::mark_seen`**

```rust
pub fn mark_seen(db: &Db, id: i64, status: &str) -> UecmResult<()> {
    let conn = db.lock().unwrap();
    conn.execute(
        "UPDATE machines SET last_seen_at = CURRENT_TIMESTAMP, status = ? WHERE id = ?",
        params![status, id],
    )?;
    Ok(())
}
```

Add a test verifying it updates both fields and bumps last_seen on each call.

- [ ] **Step 2: Modify `commands/discovery.rs::refresh_machine`** —
  - On any successful WinRM probe (regardless of UE/GPU detect outcome): call `mark_seen(&db, machine_id, "online")`
  - On WinRM probe failure: call `mark_seen(&db, machine_id, "offline")`
  - Persist `detected_ue` BEFORE attempting `detect_gpus`. If GPU detection then fails, return `RefreshResult { winrm_ok: true, ue_installs: list_for_machine(...), gpus: [], error: Some("GPU detection failed: ...") }`. UI sees real UE list + empty GPU + error banner. Closes Plan 2 concern L6.

- [ ] **Step 3: Frontend** — `MachineDetail.vue` adds an online/offline badge next to hostname. Use `store.selectedDetail.machine.status` as source of truth. Add badge component or inline `<span>` with class `bg-green-500` / `bg-red-500` / `bg-gray-400`.

- [ ] **Step 4: Test** — `MachineDetail.spec.ts` adds 1-2 tests for badge rendering per status. Backend test for `refresh_machine` dispatch order (UE persist before GPU error path) requires Windows mocking; defer that to Plan 3 lanPC E2E.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src src/components src/services src/__tests__
git commit -m "feat: mark_seen + persist UE before GPU detect; UI online badge"
```

---

## Task 6: Hostname rename command + inline editor

**Files:**
- Modify: `src-tauri/src/data/machines.rs`
- Modify: `src-tauri/src/commands/machines.rs`
- Modify: `src-tauri/src/services/tauri.ts`
- Modify: `src/stores/machines.ts`
- Create: `src/components/machines/HostnameEditor.vue`
- Modify: `src/components/machines/MachineDetail.vue`
- Create: `src/__tests__/HostnameEditor.spec.ts`

- [ ] **Step 1: `data::machines::rename`**

```rust
pub fn rename(db: &Db, id: i64, new_hostname: &str) -> UecmResult<()> {
    let conn = db.lock().unwrap();
    let updated = conn.execute(
        "UPDATE machines SET hostname = ? WHERE id = ?",
        params![new_hostname, id],
    )?;
    if updated == 0 {
        return Err(UecmError::InvalidInput(format!("machine {} not found", id)));
    }
    Ok(())
}
```

Plus 2 tests (success + machine not found).

- [ ] **Step 2: Tauri command `rename_machine(db, id, hostname)`** in `commands/machines.rs`. Register in `lib.rs`.

- [ ] **Step 3: Frontend store action** `renameMachine(id, hostname)` that calls api, then `await loadMachines() + selectMachine(id)` to refresh both list + detail.

- [ ] **Step 4: `HostnameEditor.vue`** — small inline-editable component. Click hostname → input field appears, blur or Enter saves, Escape cancels. ~40 lines.

- [ ] **Step 5: Wire into `MachineDetail.vue`** — replace the static `<h2>{{ store.selectedDetail.machine.hostname }}</h2>` with `<HostnameEditor :value="..." @save="store.renameMachine(id, $event)" />`.

- [ ] **Step 6: Tests** — 4-5 spec.ts tests for HostnameEditor (renders value, click enters edit, Enter saves, Escape cancels, blur saves).

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src src/components src/stores src/services src/__tests__
git commit -m "feat: rename_machine command + inline hostname editor in detail panel"
```

---

## Task 7: GPU VRAM DXGI fallback

**Files:**
- Modify: `ps-scripts/query-gpu-driver.ps1`

The current script reads `Win32_VideoController.AdapterRAM` which is unsigned 32-bit and caps at ~4 GB. Real fix is to query DXGI via the .NET `SharpDX` or PowerShell-accessible `Microsoft.Direct3D11` APIs. Since neither is reliably present, use a different approach: read GPU memory from the registry (`HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\<NNNN>\HardwareInformation.qwMemorySize`).

- [ ] **Step 1: Modify `query-gpu-driver.ps1`**

Before falling back to `AdapterRAM`, try the registry path:

```powershell
function Get-GpuVramMb {
    param([string]$DeviceID)
    # WMI Win32_VideoController.PNPDeviceID looks like:
    # "PCI\VEN_10DE&DEV_2204&SUBSYS_..." which corresponds to a registry
    # display class subkey. Find the subkey whose MatchingDeviceId starts
    # with the DeviceID prefix, then read HardwareInformation.qwMemorySize.
    $classRoot = 'HKLM:\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}'
    if (-not (Test-Path $classRoot)) { return $null }
    $deviceIdLower = $DeviceID.ToLower()
    Get-ChildItem $classRoot -ErrorAction SilentlyContinue | ForEach-Object {
        $matching = (Get-ItemProperty $_.PSPath -Name 'MatchingDeviceId' -ErrorAction SilentlyContinue).MatchingDeviceId
        if ($matching -and $deviceIdLower.StartsWith($matching.ToLower())) {
            $qword = (Get-ItemProperty $_.PSPath -Name 'HardwareInformation.qwMemorySize' -ErrorAction SilentlyContinue).'HardwareInformation.qwMemorySize'
            if ($qword -and $qword -gt 0) {
                return [int64]([math]::Round($qword / 1MB))
            }
            $dword = (Get-ItemProperty $_.PSPath -Name 'HardwareInformation.MemorySize' -ErrorAction SilentlyContinue).'HardwareInformation.MemorySize'
            if ($dword -and $dword -gt 0) {
                return [int64]([math]::Round($dword / 1MB))
            }
        }
    } | Select-Object -First 1
}

# Inside the foreach over $controllers, replace the existing $vramMb computation:
$vramMb = Get-GpuVramMb -DeviceID $c.PNPDeviceID
if (-not $vramMb -and $c.AdapterRAM -gt 0) {
    $vramMb = [int64]([math]::Round([int64]$c.AdapterRAM / 1MB))
}
```

- [ ] **Step 2: lanPC SSH smoke**

```bash
ssh lanpc "powershell -NoProfile -ExecutionPolicy Bypass -File E:\code\ue-cache-manager\ps-scripts\query-gpu-driver.ps1"
```

Expected: NVIDIA RTX 3080 row shows `vram_mb: 10240` (or 10239, depending on actual hardware) instead of 4095.

- [ ] **Step 3: Commit**

```bash
git add ps-scripts/query-gpu-driver.ps1
git commit -m "fix(ps): read GPU VRAM from registry qwMemorySize to bypass WMI AdapterRAM 4GB cap"
```

---

## Task 8: SQLite migration 006 + `data::share_configs` CRUD

**Files:**
- Modify: `src-tauri/src/data/schema.rs` (add migration 006)
- Create: `src-tauri/src/data/share_configs.rs`
- Modify: `src-tauri/src/data/mod.rs`

- [ ] **Step 1: Add migration 006**

```rust
(
    "006_share_configs",
    r#"
    CREATE TABLE IF NOT EXISTS share_configs (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        host_machine_id INTEGER NOT NULL,
        share_name TEXT NOT NULL,
        unc_path TEXT NOT NULL,
        local_path TEXT NOT NULL,
        mode TEXT NOT NULL,           -- "open" | "managed"
        credential_alias TEXT,        -- NULL for Mode A
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        UNIQUE(host_machine_id, share_name),
        FOREIGN KEY (host_machine_id) REFERENCES machines(id) ON DELETE CASCADE
    );
    CREATE INDEX IF NOT EXISTS idx_share_configs_host ON share_configs(host_machine_id);
    "#,
),
```

- [ ] **Step 2: Create `data/share_configs.rs`** with `ShareConfig` struct + `insert / list_all / find_by_id / find_by_host / delete` + 5-6 tests. Mirror `data/credentials.rs` style.

- [ ] **Step 3: Update `data/mod.rs`** to export.

- [ ] **Step 4: Tests pass**

```bash
cd src-tauri && cargo test --lib data && cd ..
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/data
git commit -m "feat(rust): add share_configs table + CRUD"
```

---

## Task 9: PowerShell — Mode A share creation script

**Files:**
- Create: `ps-scripts/setup-share-mode-a.ps1`

Mode A enables the Guest account and creates an Everyone:Full share. Used in closed on-set environments where the network is trusted.

- [ ] **Step 1: Write `ps-scripts/setup-share-mode-a.ps1`**

```powershell
# Creates an open SMB share on a remote host with Everyone:Full access.
# Parameters:
#   -HostName <string>             remote host (Mode A operates against host only)
#   -ShareName <string>            e.g. "DDC"
#   -LocalPath <string>            e.g. "D:\DDC" (will be created if missing)
#   [-Username <string>] [-Password <string>]    operator's admin cred for the host
# Output: JSON { ok, unc_path, message }
# Requires: WinRM session user (or supplied credential) is admin on the remote host.

param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$ShareName,
    [Parameter(Mandatory=$true)] [string]$LocalPath,
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
        param($ShareName, $LocalPath)
        if (-not (Test-Path $LocalPath)) {
            New-Item -ItemType Directory -Path $LocalPath -Force | Out-Null
        }
        # Enable Guest account (idempotent)
        $guest = Get-LocalUser -Name 'Guest' -ErrorAction Stop
        if (-not $guest.Enabled) {
            Enable-LocalUser -Name 'Guest'
        }
        # Allow Guest to access shares (LSA setting)
        # Note: requires SecPol; reg edit equivalent:
        $regPath = 'HKLM:\SYSTEM\CurrentControlSet\Services\LanmanServer\Parameters'
        Set-ItemProperty -Path $regPath -Name 'AutoShareWks' -Value 1 -Type DWord -ErrorAction SilentlyContinue
        Set-ItemProperty -Path $regPath -Name 'RestrictNullSessAccess' -Value 0 -Type DWord -ErrorAction SilentlyContinue
        # Create or replace share
        $existing = Get-SmbShare -Name $ShareName -ErrorAction SilentlyContinue
        if ($existing) {
            Remove-SmbShare -Name $ShareName -Force
        }
        New-SmbShare -Name $ShareName -Path $LocalPath -FullAccess 'Everyone' -Description 'UECM open DDC share (Mode A)' | Out-Null
        # ACL on filesystem
        icacls $LocalPath /grant 'Everyone:(OI)(CI)F' | Out-Null
        return "\\$($env:COMPUTERNAME)\$ShareName"
    }
    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock  = $script
        ArgumentList = @($ShareName, $LocalPath)
        ErrorAction  = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $unc = "$(Invoke-Command @invokeArgs)"
    @{ ok = $true; unc_path = $unc; message = "Mode A share created: $unc" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; unc_path = ""; message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
```

- [ ] **Step 2: Commit**

```bash
git add ps-scripts/setup-share-mode-a.ps1
git commit -m "feat(ps): add Mode A SMB share creation (Guest + Everyone:Full)"
```

---

## Task 10: PowerShell — Mode B share creation script (`ddc-svc` account + share)

**Files:**
- Create: `ps-scripts/setup-share-mode-b.ps1`

Mode B is the production-grade path: dedicated `ddc-svc` local account on the host, share authorized only for that account.

- [ ] **Step 1: Write `ps-scripts/setup-share-mode-b.ps1`**

```powershell
# Creates a managed SMB share on a remote host with a dedicated ddc-svc account.
# Parameters:
#   -HostName <string>
#   -ShareName <string>
#   -LocalPath <string>
#   -SvcUsername <string>          e.g. "ddc-svc"
#   -SvcPassword <string>          24-byte random generated by the operator
#   [-Username <string>] [-Password <string>]   operator admin cred for host
# Output: JSON { ok, unc_path, message }

param(
    [Parameter(Mandatory=$true)] [string]$HostName,
    [Parameter(Mandatory=$true)] [string]$ShareName,
    [Parameter(Mandatory=$true)] [string]$LocalPath,
    [Parameter(Mandatory=$true)] [string]$SvcUsername,
    [Parameter(Mandatory=$true)] [string]$SvcPassword,
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
        param($ShareName, $LocalPath, $SvcUsername, $SvcPasswordPlain)
        if (-not (Test-Path $LocalPath)) {
            New-Item -ItemType Directory -Path $LocalPath -Force | Out-Null
        }
        $svcSecure = ConvertTo-SecureString -String $SvcPasswordPlain -AsPlainText -Force
        $existingUser = Get-LocalUser -Name $SvcUsername -ErrorAction SilentlyContinue
        if ($existingUser) {
            Set-LocalUser -Name $SvcUsername -Password $svcSecure -PasswordNeverExpires $true
        } else {
            New-LocalUser -Name $SvcUsername -Password $svcSecure -PasswordNeverExpires -AccountNeverExpires -UserMayNotChangePassword -Description 'UECM share account (Mode B)' | Out-Null
            Add-LocalGroupMember -Group 'Users' -Member $SvcUsername -ErrorAction SilentlyContinue
        }
        # NTFS ACL
        icacls $LocalPath /grant "${SvcUsername}:(OI)(CI)F" | Out-Null
        # Replace share
        $existing = Get-SmbShare -Name $ShareName -ErrorAction SilentlyContinue
        if ($existing) {
            Remove-SmbShare -Name $ShareName -Force
        }
        New-SmbShare -Name $ShareName -Path $LocalPath -FullAccess $SvcUsername -Description 'UECM managed DDC share (Mode B)' | Out-Null
        return "\\$($env:COMPUTERNAME)\$ShareName"
    }
    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $HostName
        ScriptBlock  = $script
        ArgumentList = @($ShareName, $LocalPath, $SvcUsername, $SvcPassword)
        ErrorAction  = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $unc = "$(Invoke-Command @invokeArgs)"
    @{ ok = $true; unc_path = $unc; message = "Mode B share created: $unc as $SvcUsername" } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; unc_path = ""; message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
```

- [ ] **Step 2: Commit**

```bash
git add ps-scripts/setup-share-mode-b.ps1
git commit -m "feat(ps): add Mode B SMB share creation (ddc-svc account)"
```

---

## Task 11: PowerShell — SYSTEM credential injection via PsExec

**Files:**
- Create: `vendor/PsExec64.exe` (Sysinternals binary, vendored — see prerequisites)
- Create: `vendor/Sysinternals-EULA.txt`
- Create: `ps-scripts/inject-system-credential.ps1`
- Modify: `src-tauri/tauri.conf.json` (resources include vendor/)

- [ ] **Step 1: Vendor PsExec64.exe**

Download Sysinternals PSTools from `https://download.sysinternals.com/files/PSTools.zip`, extract `PsExec64.exe`, place at `vendor/PsExec64.exe`. Also save the included EULA to `vendor/Sysinternals-EULA.txt`. Add both to git.

- [ ] **Step 2: Write `ps-scripts/inject-system-credential.ps1`**

```powershell
# Injects a credential into both the current user's Credential Manager AND
# the SYSTEM account's Credential Manager (so that services running as
# LocalSystem can use it to authenticate to a SMB share).
# Parameters:
#   -ClientHostName <string>    the client machine that needs the credential
#   -TargetHost <string>        the SMB host the credential authenticates to
#   -SvcUsername <string>       e.g. "ddc-svc"
#   -SvcPassword <string>
#   [-Username <string>] [-Password <string>]   operator admin cred for client
# Output: JSON { ok, message }
# Requires: vendored PsExec64.exe alongside this script (in same parent ps-scripts/
# OR resolved via UECM resource path).

param(
    [Parameter(Mandatory=$true)] [string]$ClientHostName,
    [Parameter(Mandatory=$true)] [string]$TargetHost,
    [Parameter(Mandatory=$true)] [string]$SvcUsername,
    [Parameter(Mandatory=$true)] [string]$SvcPassword,
    [string]$Username,
    [string]$Password,
    [string]$PsExecPath = ""
)

$ErrorActionPreference = 'Stop'

function Build-CredentialOrNull {
    param([string]$User, [string]$Pass)
    if ([string]::IsNullOrEmpty($User) -or [string]::IsNullOrEmpty($Pass)) { return $null }
    $secure = ConvertTo-SecureString -String $Pass -AsPlainText -Force
    return New-Object System.Management.Automation.PSCredential($User, $secure)
}

# Resolve PsExec path. UECM passes it explicitly; default to ../vendor/PsExec64.exe.
if ([string]::IsNullOrEmpty($PsExecPath)) {
    $PsExecPath = Join-Path (Split-Path -Parent $PSScriptRoot) 'vendor\PsExec64.exe'
}
if (-not (Test-Path $PsExecPath)) {
    @{ ok = $false; message = "PsExec64.exe not found at $PsExecPath" } | ConvertTo-Json -Compress
    exit 1
}

try {
    $script = {
        param($TargetHost, $SvcUsername, $SvcPassword, $PsExecPath)
        # 1. User-level cmdkey
        & cmdkey.exe "/add:$TargetHost" "/user:$SvcUsername" "/pass:$SvcPassword" | Out-Null
        # 2. SYSTEM-level cmdkey via PsExec -s
        # PsExec accepts the EULA on first run; -accepteula avoids the prompt.
        & $PsExecPath -accepteula -nobanner -s -i 0 cmdkey.exe "/add:$TargetHost" "/user:$SvcUsername" "/pass:$SvcPassword" | Out-Null
        # 3. Verify SYSTEM cred was set by listing under SYSTEM context
        $listOut = & $PsExecPath -accepteula -nobanner -s -i 0 cmdkey.exe /list:$TargetHost
        if ($listOut -notmatch [regex]::Escape($SvcUsername)) {
            throw "SYSTEM cred verify failed; cmdkey /list under SYSTEM did not show '$SvcUsername'"
        }
        return "user + SYSTEM creds injected for $TargetHost"
    }
    $cred = Build-CredentialOrNull -User $Username -Pass $Password
    $invokeArgs = @{
        ComputerName = $ClientHostName
        ScriptBlock  = $script
        ArgumentList = @($TargetHost, $SvcUsername, $SvcPassword, $PsExecPath)
        ErrorAction  = 'Stop'
    }
    if ($cred) { $invokeArgs['Credential'] = $cred }
    $msg = "$(Invoke-Command @invokeArgs)"
    @{ ok = $true; message = $msg } | ConvertTo-Json -Compress
}
catch {
    @{ ok = $false; message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
```

**Caveat:** `Invoke-Command` to a remote host requires PsExec64.exe to be PRESENT on that remote host (since the scriptblock executes there). For Plan 3 v1, document that the operator must pre-stage PsExec64.exe on each client target (or UECM auto-copies it via `Copy-Item -ToSession`). Add a follow-up task for auto-copy if needed.

- [ ] **Step 3: Modify `tauri.conf.json`** to bundle `vendor/`:

```json
"resources": {
  "../ps-scripts": "ps-scripts",
  "../vendor": "vendor"
}
```

- [ ] **Step 4: Commit**

```bash
git add vendor ps-scripts/inject-system-credential.ps1 src-tauri/tauri.conf.json
git commit -m "feat: vendor PsExec64 + add SYSTEM credential injection script"
```

---

## Task 12: Rust `core::shares` + `core::psexec` + `commands::shares`

**Files:**
- Create: `src-tauri/src/core/shares.rs`
- Create: `src-tauri/src/core/psexec.rs`
- Create: `src-tauri/src/commands/shares.rs`
- Modify: `src-tauri/src/core/mod.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: `core::shares`** — wraps `setup-share-mode-a.ps1` and `setup-share-mode-b.ps1` as `create_mode_a(host, share_name, local_path, opt_creds) -> ShareCreateResult` and `create_mode_b(host, share_name, local_path, svc_user, svc_pass, opt_creds)`. Both emit a `ShareCreateResult { unc_path, mode }`. Persist into `data::share_configs` from the calling layer (commands).

- [ ] **Step 2: `core::psexec`** — single function `inject_system_credential(client_host, target_host, svc_user, svc_pass, opt_creds) -> Result`. Resolves PsExec path via `powershell::script_path` cousin: `vendor_path("PsExec64.exe")`. Add `vendor_path` helper to `powershell.rs` mirroring `script_path`.

- [ ] **Step 3: `commands::shares`** —
  - `create_share(db, host_machine_id, mode: "open"|"managed", share_name, local_path, optional credential_alias) -> i64` (returns share_configs row id). For Mode B, generates a 24-byte random password (use `getrandom` crate or `rand`), passes to script, then persists credential alias to data::credentials + DPAPI.
  - `inject_share_credential_to_clients(db, share_config_id, client_machine_ids: Vec<i64>, optional credential_alias_for_clients) -> Vec<InjectionResult>` (sequential or batch via core::batch).
  - `list_shares(db) -> Vec<ShareConfig>`
  - `delete_share(db, share_config_id, also_remove_remote: bool)` — if `also_remove_remote=true`, run `remove-share.ps1` (Task 19) on the host first.

- [ ] **Step 4: Tests** — cfg(not(windows)) tests asserting all _ps-dependent paths return `UecmError::PowerShell` on macOS. Cfg(any) tests for `data::share_configs` and Rust-only logic (random password generation length/charset).

- [ ] **Step 5: Build + tests**

```bash
cd src-tauri && cargo build && cargo test --lib && cd ..
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src
git commit -m "feat: shares + psexec wrappers + create_share / inject commands"
```

---

## Task 13: Rust `core::batch` (mpsc-based fan-out + progress events)

**Files:**
- Create: `src-tauri/src/core/batch.rs`
- Create: `src-tauri/src/commands/batch.rs`
- Modify: `src-tauri/src/core/mod.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: `core::batch`** — defines `BatchEvent { machine_id, status: BatchStatus, message: Option<String> }` enum (`Running`, `Ok`, `Err`) and a `run_batch<F, T>(machine_ids, max_concurrency, op_fn)` async function. `op_fn` is a closure `(machine_id) -> impl Future<Output = Result<T, UecmError>>`. Spawns one tokio task per machine, behind a semaphore, sending events to an `mpsc::UnboundedSender<BatchEvent>`. Returns the receiver to the caller.

- [ ] **Step 2: `commands::batch`**:
  - `batch_set_env_var(db, app_handle, machine_ids, name, value, credential_alias)` — calls `core::batch::run_batch` with closures that delegate to `core::env_vars::set_with_credential`. As events arrive, emits to frontend via `app_handle.emit_all("batch-progress", event)`.
  - `batch_set_ini_key(db, app_handle, machine_ids, file_path, section, name, value, credential_alias)` — same shape.

- [ ] **Step 3: Frontend will subscribe via `listen('batch-progress', ...)`** in Task 14.

- [ ] **Step 4: Tests** — cfg(any) test for `BatchEvent` enum + mpsc plumbing on macOS using a no-op closure (verifies fan-out + ordering + concurrency cap).

- [ ] **Step 5: Build + tests**

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src
git commit -m "feat: batch fan-out with mpsc progress events"
```

---

## Task 14: Frontend service + stores for shares + batch

**Files:**
- Modify: `src/services/tauri.ts`
- Create: `src/stores/shares.ts`
- Create: `src/stores/batch.ts`
- Create: `src/__tests__/shares-store.spec.ts`
- Create: `src/__tests__/batch-store.spec.ts`
- Modify: `src/__tests__/machines-store.spec.ts` (add rename action test)

- [ ] **Step 1: tauri.ts types** — `ShareConfig`, `ShareCreateResult`, `BatchEvent`, `BatchStatus`. Service functions: `createShare`, `injectShareCredentialToClients`, `listShares`, `deleteShare`, `batchSetEnvVar`, `batchSetIniKey`.

- [ ] **Step 2: shares store** — `shares: ShareConfig[]`, `load()`, `create(...)`, `delete(...)`. ~50 lines.

- [ ] **Step 3: batch store** — `events: BatchEvent[]`, `runBatch(commandName, args)`, subscribes to `'batch-progress'` event from Tauri, accumulates into `events`, exposes `byMachine` computed grouping for table render.

- [ ] **Step 4: Tests** — 4-5 each.

- [ ] **Step 5: Commit**

```bash
git add src/services/tauri.ts src/stores src/__tests__
git commit -m "feat(frontend): shares + batch stores + tauri service extensions"
```

---

## Task 15: ShareCreateWizard.vue modal

**Files:**
- Create: `src/components/modals/ShareCreateWizard.vue`
- Create: `src/__tests__/ShareCreateWizard.spec.ts`

Multi-step wizard: pick mode → pick host → fill share params (name + local path + optional svc account name for Mode B) → preview the PowerShell that will run → execute with progress.

- [ ] **Step 1: Spec.ts (verbatim per skill convention; abbreviated below — full version mirrors DiscoveryWizard.spec.ts patterns)**

5-7 tests: renders nothing when closed; mode picker enables Next; Mode A skips svc fields; Mode B requires svc username; preview shows expected command; Create button calls `createShare` with correct args.

- [ ] **Step 2: Component** (~200 lines, splits into 4 visual steps with `currentStep: 1|2|3|4` ref, BaseModal wrapping, footer with Back/Next/Create buttons)

- [ ] **Step 3: Wire into `views/Machines.vue`** — add a "Create Share" button to the toolbar above the machine list.

- [ ] **Step 4: Test pass + commit**

```bash
git add src/components/modals/ShareCreateWizard.vue src/__tests__/ShareCreateWizard.spec.ts src/views/Machines.vue
git commit -m "feat(frontend): add ShareCreateWizard modal"
```

---

## Task 16: BatchEnvVarModal + BatchIniEditModal + BatchProgressTable

**Files:**
- Create: `src/components/batch/BatchProgressTable.vue`
- Create: `src/components/modals/BatchEnvVarModal.vue`
- Create: `src/components/modals/BatchIniEditModal.vue`
- Create: `src/__tests__/BatchProgressTable.spec.ts`
- Create: `src/__tests__/BatchEnvVarModal.spec.ts`
- Create: `src/__tests__/BatchIniEditModal.spec.ts`
- Modify: `src/views/Machines.vue` (multi-select + batch toolbar)

- [ ] **Step 1: BatchProgressTable.vue** — props `[{machine_id, hostname, status, message}]`, renders a table with status icon (✓ green / ✗ red / ↻ spinner / — gray for not-started). ~80 lines.

- [ ] **Step 2: Multi-select in Machines.vue** — checkboxes next to each machine row, "Select all" header checkbox, "X selected" indicator + 2 buttons "Batch env var" / "Batch INI". Reactive `selectedIds: Set<number>` ref. Modify the v-for `<li>` rendering.

- [ ] **Step 3: BatchEnvVarModal.vue** — props `:open` `:machineIds` `:credentialAlias`. Form: env var name + value. On Apply, call `store.batch.runBatch('batch_set_env_var', ...)` then render `BatchProgressTable` reactively until all done. Cancel closes modal but does NOT cancel in-flight ops (Plan 4 enhancement).

- [ ] **Step 4: BatchIniEditModal.vue** — same shape but for INI (file_path, section, name, value).

- [ ] **Step 5: Tests** — 4-5 each.

- [ ] **Step 6: Commit**

```bash
git add src/components src/views/Machines.vue src/__tests__
git commit -m "feat(frontend): multi-select + batch env var/INI modals + progress table"
```

---

## Task 17: Wire ShareConfig list view + delete flow

**Files:**
- Create: `src/views/Shares.vue` (or extend an existing view)
- Modify: `src/router/index.ts` (add /shares route)
- Modify: `src/components/AppShell.vue` (add Shares to activity bar) — or repurpose existing "DDC Pak" stub

- [ ] **Step 1: Decide where shares live in nav**

The existing 7-view stub layout from Plan 1 has DDC Pak / PSO Cache as separate views. Shares is upstream of DDC Pak (you create a share before you populate it with DDC Pak). Two options:

- **A**: Add a new "Shares" item between Machines and DDC Pak in activity bar (now 8 views)
- **B**: Embed Shares as a sub-section inside DDC Pak view (keep nav at 7)

Default: **A** (new Shares view). DDC Pak stub stays for Plan 5.

- [ ] **Step 2: Shares.vue** — list rows with host hostname, share name, UNC path, mode, credential alias, created_at. Per-row delete button (with confirmation, optional "also remove remote" checkbox).

- [ ] **Step 3: Tests** — 3-4 view tests.

- [ ] **Step 4: Commit**

```bash
git add src/views src/router src/components/AppShell.vue src/__tests__
git commit -m "feat(frontend): add Shares view (list + delete)"
```

---

## Task 18: lanPC E2E — full Plan 3 verification

**Files:** none (verification task).

- [ ] **Step 1: Sync + rebuild on lanPC**

```bash
ssh lanpc "p4 sync"
ssh lanpc "cd /d E:\code\ue-cache-manager && pnpm install && pnpm tauri build"
```

- [ ] **Step 2: Pre-cleanup on lanPC**

```powershell
# Run as admin on lanPC first time:
Remove-LocalUser ddc-svc -ErrorAction SilentlyContinue
Remove-SmbShare -Name DDC -Force -ErrorAction SilentlyContinue
Remove-Item D:\DDC -Recurse -Force -ErrorAction SilentlyContinue
```

- [ ] **Step 3: Launch UECM normally (no RunAs).** Verify all Plan 2 deliverables 1-7 still pass with the per-call credential pipe (Task 4 step but more thoroughly).

- [ ] **Step 4: Mode A verification**

Click "Create Share" → Mode A → host = lanPC → share name = `DDC` → local path = `D:\DDC` → credential = `UECM:winrm:LANPC` → Create.

Expected:
- Wizard shows preview, then green success with `\\lanPC\DDC`
- SSH check: `ssh lanpc "Get-SmbShare -Name DDC | Format-List Name,Path,Description; net share DDC"` — share exists
- SSH check from a different machine if available, or from lanPC itself: `Test-Path \\lanPC\DDC` returns true

Cleanup: delete share via UECM "also remove remote" → SSH verifies it's gone.

- [ ] **Step 5: Mode B verification**

Click "Create Share" → Mode B → host = lanPC → share = `DDC` → local = `D:\DDC` → svc username = `ddc-svc` → credential = `UECM:winrm:LANPC` → Create.

Expected:
- Wizard generates 24-byte password, shows preview, runs, green success
- SSH check: `ssh lanpc "Get-LocalUser ddc-svc; Get-SmbShare DDC"` — both exist
- New credential alias `UECM:share:lanPC:ddc-svc` appears in Credentials list

Then: pick "Inject share credential to clients" → select lanPC itself (acts as both host and client for E2E) → Inject.

Expected:
- PsExec64 spawns successfully
- SSH verify SYSTEM cred via PsExec on lanPC:
  ```bash
  ssh lanpc "powershell -c \"E:\code\ue-cache-manager\src-tauri\target\release\vendor\PsExec64.exe -accepteula -nobanner -s -i 0 cmdkey.exe /list:lanPC\""
  ```
  Output should mention `ddc-svc`.

- [ ] **Step 6: Verify RenderStream-Service-equivalent reads the share**

Without a real RenderStream Service, simulate by running a one-shot cmd as SYSTEM:

```bash
ssh lanpc "powershell -c \"E:\code\ue-cache-manager\src-tauri\target\release\vendor\PsExec64.exe -accepteula -nobanner -s -i 0 cmd /c 'dir \\\\lanPC\\DDC'\""
```

Expected: directory listing succeeds (proving SYSTEM has the credential and can mount the share). If it fails with "Logon failure", the SYSTEM cmdkey injection didn't work.

- [ ] **Step 7: Batch verification**

Add 192.168.10.20 a second time as `127.0.0.1` (or use `lanbipu-razer` if available). Multi-select both, click "Batch env var" → name = `UECM_BATCH_TEST` value = `42` → Apply.

Expected:
- BatchProgressTable shows 2 rows updating in real time
- Both end in green ✓
- SSH check on each:
  ```bash
  ssh lanpc 'powershell -c "[Environment]::GetEnvironmentVariable(\"UECM_BATCH_TEST\", \"Machine\")"'
  ```

Cleanup: another batch run with empty value (or manual env var removal).

- [ ] **Step 8: Final cleanup**

```powershell
# As admin on lanPC:
Remove-LocalUser ddc-svc -ErrorAction SilentlyContinue
Remove-SmbShare -Name DDC -Force -ErrorAction SilentlyContinue
Remove-Item D:\DDC -Recurse -Force -ErrorAction SilentlyContinue
[Environment]::SetEnvironmentVariable('UECM_BATCH_TEST', $null, 'Machine')
[Environment]::SetEnvironmentVariable('UE-SharedDataCachePath', $null, 'Machine')
cmdkey /delete:lanPC
& 'E:\code\ue-cache-manager\src-tauri\target\release\vendor\PsExec64.exe' -accepteula -nobanner -s -i 0 cmdkey /delete:lanPC
```

- [ ] **Step 9: Mark all 8 deliverables ✅ in summary report.**

---

## Task 19: Final integration — README + production build smoke + NSIS pre-stage

**Files:**
- Modify: `README.md`
- Optional: vendor `nsis-3.11.zip` to `vendor/nsis-3.11.zip` if you want NSIS bundling to work offline

- [ ] **Step 1: Run full test suite**

```bash
export PATH="/Users/bip.lan/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
pnpm test
cd src-tauri && cargo test && cd ..
```

Expected: ~80 frontend + ~70 backend tests pass.

- [ ] **Step 2: Production build smoke (macOS)**

```bash
pnpm tauri build
```

Verify `.dmg` + `.app` produce. NSIS will be skipped on macOS (Windows-only target).

- [ ] **Step 3: Pre-stage NSIS for lanPC** (optional but recommended)

Vendor `nsis-3.11.zip` from a mirror (e.g. `https://prdownloads.sourceforge.net/nsis/nsis-3.11.zip`). Place at `vendor/nsis-3.11.zip`. Document in README how to copy to Tauri cache dir on Windows:

```
On lanPC:
copy E:\code\ue-cache-manager\vendor\nsis-3.11.zip %LOCALAPPDATA%\tauri\cache\NSIS\nsis-3.11.zip
```

After this, `pnpm tauri build` on lanPC should produce both MSI and NSIS bundles.

- [ ] **Step 4: README update**

Replace status section + add Plan 3 capabilities + PsExec attribution + NSIS note. Approximate diff:

```
Status: Plan 3 (Elevation, SMB Shares & Cluster Batch) complete.

What's new in Plan 3:
- Per-call credential injection (no UAC required for normal ops)
- SMB share creation wizard: Mode A (open) / Mode B (managed ddc-svc)
- SYSTEM-level credential injection via vendored PsExec64
- Multi-machine batch env var / INI operations with real-time progress
- Hostname rename inline edit
- GPU VRAM correctly reports >4 GB (DXGI registry fallback)
- last_seen_at + status reactive online/offline badge

Third-party:
- PsExec64.exe (Sysinternals, redistributed under Microsoft EULA — see vendor/Sysinternals-EULA.txt)
- Optional NSIS for installer bundle (pre-stage to %LOCALAPPDATA%\tauri\cache\NSIS\)
```

- [ ] **Step 5: Verify clean repo state + commit**

```bash
git add README.md vendor/nsis-3.11.zip
git commit -m "docs: update README with Plan 3 completion + PsExec attribution + NSIS pre-stage"
```

```bash
git status
git log --oneline | head -25
```

Expected: ~19 new commits since Plan 2.

---

## Summary

At the end of Plan 3:

1. ✅ UECM admin operations work via per-call credential pipe (no RunAs Admin / no requireAdministrator manifest)
2. ✅ User can create SMB shares: Mode A (open) and Mode B (managed) with one wizard
3. ✅ SYSTEM credential injection via vendored PsExec64 enables RenderStream-Service-class clients to mount shares
4. ✅ User can multi-select machines + apply env var or INI changes batch with real-time progress
5. ✅ `refresh_machine` updates last_seen_at + status; UI online badge accurate
6. ✅ Hostname rename via inline editor in detail panel
7. ✅ GPU VRAM correctly reports real values (RTX 3080 = 10240 MB, not 4095)
8. ✅ Production build green; full E2E verified on lanPC

**Plan 4** will build on this: cluster-wide INI conflict scanning + auto-fix wizard + 11-row health check matrix.
