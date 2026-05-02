# UE Cache Manager (UECM)

Cross-machine Unreal Engine cache management tool for VP/XR render clusters.

**Status:** Plan 3 (Elevation, SMB Shares & Cluster Batch) complete. See `docs/superpowers/plans/`.

## What's working

- Tauri 2.x app shell with 8 navigable views (Dashboard / Machines / Shares / Projects / DDC Pak / PSO Cache / INI Scanner / Health Check)
- SQLite persistence: machines + machine_ue_installs + machine_gpus + credentials + share_configs
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
- **Hostname rename**: inline editor in the machine detail panel.
- Builds to a single .exe / .dmg / .AppImage.

## What's NOT yet implemented (next plans)

- INI conflict scanner + auto-fix (Plan 4)
- Cluster health check matrix (Plan 4)
- DDC Pak generation + distribution (Plan 5)
- PSO Cache operations + visual polish (Plan 6)

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

Frontend ↔ backend communication via Tauri commands. All command handlers in
`src-tauri/src/commands/` are thin wrappers that delegate to logic in
`src-tauri/src/core/` and data in `src-tauri/src/data/`. All cross-machine
operations route through PowerShell sidecar via `core/winrm.rs` →
`ps-scripts/invoke-remote.ps1`.

## Platform support

Target: Windows 10/11. Foundation builds + runs on Mac/Linux for development;
PowerShell sidecar (and all WinRM/registry/credential features) only work
on Windows. Tests gated appropriately with `#[cfg(windows)]`.
