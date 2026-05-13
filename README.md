# UE Cache Manager (UECM)

Cross-machine Unreal Engine cache management tool for VP/XR render clusters.

**Status:** v1.0 feature-complete locally for Plans 1-6. lanPC E2E remains deferred until `192.168.10.20` is reachable again. See `docs/superpowers/plans/`.

## What's working

- Tauri 2.x app shell with 8 navigable views (Dashboard / Machines / Shares / Projects / DDC Pak / PSO Cache / INI Scanner / Health Check)
- SQLite persistence: machines + machine_ue_installs + machine_gpus + credentials + share_configs + operations + projects + project_locations + PSO cache metadata
- **Network discovery**: scan a CIDR, probe ports 5985 (WinRM) and 445 (SMB), add reachable hosts
- **Per-machine refresh**: probe WinRM, then read installed UE versions (registry) + GPU model + driver version (WMI). Updates `last_seen_at` + `status`; UI shows live online/offline badge. UE list persists even when GPU detect fails.
- **GPU VRAM** read from display class `qwMemorySize` registry value, bypassing the WMI `AdapterRAM` 4 GB cap (RTX 3080 reports 10240 MB).
- **Credential management**: aliases stored in SQLite + cmdkey (transparent SMB auth) + DPAPI-encrypted blob at `%LOCALAPPDATA%\UECM\creds.bin` (per-call WinRM auth).
- **Per-call credential injection**: every WinRM operation can take a credential alias; UECM resolves the password from DPAPI and forwards to `Invoke-Command -Credential`. UECM itself launches without `requireAdministrator`.
- **Single-machine env var / INI editing**: read/write system-level env vars and INI keys on a remote host with auto-backup.
- **SMB share creation wizard**:
  - Mode A — open Guest + Everyone:Full, for trusted-LAN environments.
  - Mode B — dedicated `ddc-svc` local account on the host, share authorized only to that account; SYSTEM credential injection via vendored PsExec64 lets LocalSystem services (e.g. RenderStream Service) mount the share.
- **Cluster batch operations**: multi-select machines + apply env var / INI key to all of them; real-time per-machine progress (✓ / ✗ / ↻) via mpsc fan-out + Tauri `batch-progress` events; capped at 8 concurrent.
- **Project inventory**: discover `*.uproject` files on Windows machines, group them by logical project identity, and maintain per-machine path mappings.
- **DDC Pak workflow**: generate DDC pak files through a reusable UE runner, verify `.ddp` output, cancel active UE processes, and distribute verified pak files through Robocopy fan-out.
- **PSO Cache workflow**: verify PSO precaching CVars (R008-R010), collect `*.upipelinecache` / `*.stablepc.csv` through UE `-game` mode, persist collected files, and distribute them with GPU-mismatch guardrails.
- **GPU/driver consistency matrix**: shared backend module + frontend matrix for PSO safety checks and Health Check #11.
- **WinRM bootstrap onboarding**: package and run the first-contact bootstrap scripts for workgroup/IP Windows targets, including operator-side TrustedHosts setup.
- **Hostname rename**: inline editor in the machine detail panel.
- **INI Scanner (Plan 4)**: cross-machine scan of project / user-level / engine INI files. Pure-Rust rule engine with seven rules (R001-R007) covering hardcoded DDC `Path=`, user-level DDC overrides, mapped-drive paths, deprecated CVars, missing env vars, and the healthy-path baseline. One-click apply with auto-backup that re-uses Plan 2's atomic write. Hierarchy + 3-column diagnostic detail (What / Why / Symptom) + before/after diff via `UecmCodeBlock`.
- **Cluster Health Check (Plan 4)**: 11-row × N-machine matrix. Eight active probes packaged into one PowerShell round-trip per machine (`smb`, `firewall_445`, `share_reachable`, `ntfs_perm`, `cred_user`, `cred_system`, `env_vars`, `system_write`) plus three derived checks (`ini_consistency` from latest INI scan, `pso_precaching` from project `ConsoleVariables.ini`, `gpu_consistency` pure-Rust aggregator over `machine_gpus`). SYSTEM-cred + SYSTEM-write columns emphasized. Detail panel shows What / Symptom / How-to-fix + last probe output.
- **Visual polish baseline**: shared empty/loading/error state block, v1.0 Dashboard KPIs, PSO Cache file explorer, and GPU matrix surface.
- Builds to a single .exe / .dmg / .AppImage.

## Deferred / not yet implemented

- lanPC real UE E2E for DDC Pak generation/distribution (blocked by host reachability during this implementation pass)
- lanPC real UE E2E for PSO collection/distribution (blocked by host reachability during this implementation pass)
- Automated PSO camera flythrough scripting
- Multi-machine distributed PSO collection
- AMD / Intel GPU E2E validation

## Third-party / vendored

- `vendor/PsExec64.exe` — Sysinternals PsExec 2.43, redistributed under the Microsoft EULA (see `vendor/Sysinternals-EULA.txt`). Bundled by Tauri as a resource at runtime; resolved via `<exe-dir>\vendor\PsExec64.exe`.
- NSIS bundling (Windows MSI/NSIS installers) is optional. If `pnpm tauri build` reports a download timeout from China, pre-stage the NSIS zip:
  ```
  copy E:\code\ue-cache-manager\vendor\nsis-3.11.zip %LOCALAPPDATA%\tauri\cache\NSIS\nsis-3.11.zip
  ```
  Then re-run the build.

## Development

Prerequisites:
- Rust 1.75+ (https://rustup.rs/)
- Node.js 20+, pnpm
- OS-specific Tauri prerequisites: see https://v2.tauri.app/start/prerequisites/

```bash
pnpm install
pnpm tauri dev
pnpm test
cd src-tauri
cargo test
cd ..
pnpm tauri build
```

End-to-end verification of WinRM-dependent features (discovery refresh, env var, INI edit) requires a Windows test host with `Enable-PSRemoting` enabled. macOS dev builds + tests work, but PowerShell-dependent unit tests are gated `#[cfg(windows)]`.

## Architecture

- `src/`: Vue 3 frontend (TypeScript, Tailwind, Pinia, Vue Router)
- `src-tauri/`: Rust backend (rusqlite, serde, tokio, ipnet)
- `ps-scripts/`: PowerShell sidecar scripts (Windows-only execution)
- `docs/superpowers/specs/`: design docs
- `docs/superpowers/plans/`: implementation plans
- `docs/superpowers/changelog/`: release notes

Frontend ↔ backend communication via Tauri commands. All command handlers in
`src-tauri/src/commands/` are thin wrappers that delegate to logic in
`src-tauri/src/core/` and data in `src-tauri/src/data/`. All cross-machine
operations route through PowerShell sidecar via `core/winrm.rs` →
`ps-scripts/invoke-remote.ps1`.

Primary product/design reference: `docs/superpowers/specs/2026-05-01-uecm-design.md`.

## Release notes

See `docs/superpowers/changelog/2026-05-05-v1.0.md`.

## Platform support

Target: Windows 10/11. Foundation builds + runs on Mac/Linux for development;
PowerShell sidecar (and all WinRM/registry/credential features) only work
on Windows. Tests gated appropriately with `#[cfg(windows)]`.
