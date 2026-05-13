# UECM WinRM Bootstrap Script Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build first-contact Windows target bootstrap that enables UECM remote management without requiring SSH.

**Architecture:** The target-side script is idempotent and runs locally as Administrator on unmanaged Windows machines. UECM also exposes an automatic path that uses `ADMIN$` + `PsExec64.exe` when those Windows admin channels are already reachable. If that path fails, the UI falls back to the same local script for USB, shared-folder, image, `GPO`, `Intune`, `SCCM`, or `RMM` onboarding.

**Tech Stack:** PowerShell 5.1+, Windows `WinRM`, Windows Firewall `NetSecurity`, WSMan provider, UECM `ps-scripts` Tauri resource bundle.

---

## Scope

- Create a local target-machine bootstrap script for flow C: no existing remote management channel.
- Add automatic first-contact bootstrap from the Machines detail view when `ADMIN$` / `RPC` / `Service Control Manager` can be used.
- Return the local script as fallback when automatic bootstrap is not possible.
- Document every permission or security setting the script can change.
- Document deployment options: USB, shared folder, image template, `GPO`, `Intune`, `SCCM`, and `RMM`.
- Do not add SSH support.
- Do not enable `Basic` auth, `CredSSP`, unencrypted WinRM, or WinRM HTTPS `5986`.

## Files

- Create: `ps-scripts/enable-winrm.ps1`
  - Runs once on the target Windows machine.
  - Emits JSON `{ ok, message, changed, state }`.
- Create: `ps-scripts/bootstrap-winrm-remote.ps1`
  - Runs on the UECM operator machine.
  - Copies `enable-winrm.ps1` through `ADMIN$` and runs it with bundled `PsExec64.exe`.
- Create: `src-tauri/src/core/bootstrap.rs`
  - Wraps automatic bootstrap and exposes local fallback script text.
- Create: `src-tauri/src/commands/bootstrap.rs`
  - Adds Tauri commands for automatic bootstrap and script export.
- Modify: `src/components/machines/MachineDetail.vue`
  - Adds first-contact bootstrap panel for non-online machines.
- Modify: `src/stores/machines.ts`, `src/services/tauri.ts`
  - Adds frontend API/store plumbing.
- Create: `ps-scripts/package-winrm-bootstrap.ps1`
  - Copies `enable-winrm.ps1` to a USB-friendly package as `UECM-Bootstrap-WinRM.ps1`.
  - Writes a plain `README.txt` with exact commands.
- Create: `docs/bootstrap/winrm-bootstrap.md`
  - Chinese operator documentation.
  - Lists all permissions/settings the script can change.
- Create: `docs/superpowers/plans/2026-05-09-uecm-winrm-bootstrap-script.md`
  - This implementation plan.

## Permission Contract

Default script run:

- Starts `WinRM`.
- Sets `WinRM` startup type to `Automatic`.
- Runs `Enable-PSRemoting -Force -SkipNetworkProfileCheck`.
- Runs `winrm quickconfig -q`.
- Enables firewall rules in display group `Windows Remote Management`.
- Converts active `Public` network profiles to `Private` unless `-NetworkCategory Skip` is passed.
- Verifies `Test-WSMan localhost`.

Optional script switches:

- `-AllowedRemoteAddress <ip-or-cidr[]>`
  - Restricts Windows Remote Management firewall rules to the supplied remote addresses.
- `-TrustedHosts <host[]>`
  - Sets `WSMan:\localhost\Client\TrustedHosts` on the machine running the script.
  - This is mainly for an operator/source machine, not required for a target-only machine.
- `-EnableLocalAccountRemoteAdmin`
  - Sets `HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System\LocalAccountTokenFilterPolicy = 1`.
  - Required only for workgroup deployments that use local Administrator accounts for remote admin operations.

Explicit non-goals:

- Does not install a persistent UECM agent.
- Does not enable SSH.
- Does not enable `Basic` authentication.
- Does not enable `CredSSP`.
- Does not set `AllowUnencrypted`.
- Does not change global `ExecutionPolicy`.

---

### Task 1: Add Target Bootstrap Script

**Files:**
- Create: `ps-scripts/enable-winrm.ps1`

- [x] **Step 1: Add the PowerShell script**

Create `ps-scripts/enable-winrm.ps1` with these behaviors:

- Require Administrator for non-`-CheckOnly` runs.
- Normalize active `Public` networks to `Private` by default.
- Enable `WinRM` service/listener/firewall.
- Support optional firewall remote-address restriction.
- Support optional `TrustedHosts`.
- Support optional workgroup local-admin remote token policy.
- Return machine-readable JSON.

- [x] **Step 2: Parse-check the script**

Run:

```bash
pwsh -NoProfile -Command '$null = [scriptblock]::Create((Get-Content -Raw ps-scripts/enable-winrm.ps1)); "parse ok"'
```

Expected:

```text
parse ok
```

If `pwsh` is unavailable on macOS, defer runtime verification to Windows and run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\ps-scripts\enable-winrm.ps1 -CheckOnly
```

Expected: JSON with `ok=true` when `Test-WSMan localhost` succeeds; JSON with `ok=false` and exit code `1` when the local WSMan probe is not reachable.

### Task 2: Add USB Package Helper

**Files:**
- Create: `ps-scripts/package-winrm-bootstrap.ps1`

- [x] **Step 1: Add the packaging helper**

Create a helper that copies `ps-scripts/enable-winrm.ps1` to:

```text
<OutputDirectory>\UECM-Bootstrap-WinRM.ps1
```

and writes:

```text
<OutputDirectory>\README.txt
```

The README must include the basic admin command and workgroup variant:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\UECM-Bootstrap-WinRM.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\UECM-Bootstrap-WinRM.ps1 -EnableLocalAccountRemoteAdmin
```

- [x] **Step 2: Parse-check the helper**

Run:

```bash
pwsh -NoProfile -Command '$null = [scriptblock]::Create((Get-Content -Raw ps-scripts/package-winrm-bootstrap.ps1)); "parse ok"'
```

Expected:

```text
parse ok
```

### Task 3: Add Deployment Documentation

**Files:**
- Create: `docs/bootstrap/winrm-bootstrap.md`

- [x] **Step 1: Document the hard boundary**

Explain that if the target has no `SSH`, no `WinRM`, no `SMB ADMIN$`/`RPC`, no domain/MDM/RMM path, then UECM needs one-time local bootstrap.

- [x] **Step 2: Document exact permission changes**

Document the default changes and optional switches from the Permission Contract section.

- [x] **Step 3: Document deployment functions**

Document:

- USB bootstrap package.
- Shared-folder/manual package.
- Golden image.
- `GPO` startup script.
- `Intune` / `SCCM` / `RMM`.
- Post-bootstrap validation from the operator machine.

### Task 4: Verify Package Surface

**Files:**
- Test: `ps-scripts/enable-winrm.ps1`
- Test: `ps-scripts/bootstrap-winrm-remote.ps1`
- Test: `ps-scripts/package-winrm-bootstrap.ps1`
- Test: `src-tauri/src/core/bootstrap.rs`
- Test: `src-tauri/src/commands/bootstrap.rs`
- Test: `src/components/machines/MachineDetail.vue`
- Test: `docs/bootstrap/winrm-bootstrap.md`

- [x] **Step 1: Run shell checks**

Run:

```bash
git diff --check
```

Expected: no output.

- [x] **Step 2: Confirm Tauri resource inclusion**

Run:

```bash
rg -n '"../ps-scripts": "ps-scripts"' src-tauri/tauri.conf.json
```

Expected: one matching line, confirming the new scripts are included in production bundles through the existing resource mapping.

- [x] **Step 3: Review final diff**

Run:

```bash
git diff -- ps-scripts/enable-winrm.ps1 ps-scripts/bootstrap-winrm-remote.ps1 ps-scripts/package-winrm-bootstrap.ps1 src-tauri/src/core/bootstrap.rs src-tauri/src/commands/bootstrap.rs src/components/machines/MachineDetail.vue docs/bootstrap/winrm-bootstrap.md docs/superpowers/plans/2026-05-09-uecm-winrm-bootstrap-script.md
```

Expected: only bootstrap scripts, backend/frontend bootstrap wiring, tests, documentation, and plan files changed.

## Self-Review

- Spec coverage: covers automatic first-contact bootstrap, local fallback script, permission list, and deployment functions requested by the user.
- Placeholder scan: no placeholder markers or undefined future work required for this branch.
- Type consistency: script names and command examples use `enable-winrm.ps1` and packaged `UECM-Bootstrap-WinRM.ps1` consistently.

## Verification Evidence

- `pnpm test --run src/__tests__/tauri-service.spec.ts src/__tests__/machines-store.spec.ts src/__tests__/MachineDetail.spec.ts`
- `pnpm build`
- `git diff --check`
- On `lanpc` via `SSH`: PowerShell parse check for `enable-winrm.ps1`, `bootstrap-winrm-remote.ps1`, and `package-winrm-bootstrap.ps1`.
- On `lanpc` via `SSH`: `powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\ps-scripts\enable-winrm.ps1 -CheckOnly`
- On `lanpc` via `SSH`: `cargo test --manifest-path src-tauri/Cargo.toml bootstrap --lib`
- On `lanpc` via `SSH`: `cargo check --manifest-path src-tauri/Cargo.toml`
- On `lanpc` via `SSH`: `package-winrm-bootstrap.ps1` generated `UECM-Bootstrap-WinRM.ps1` and `README.txt`.
