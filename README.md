# UE Cache Manager (UECM)

Cross-machine Unreal Engine cache management tool for VP/XR render clusters.

**Status:** Plan 4 (Diagnostics) and Plan 5 (DDC Pak workflow) implemented locally. lanPC E2E is deferred until `192.168.10.20` is reachable again. See `docs/superpowers/plans/`.

## What's working

- Tauri 2.x app shell with 8 navigable views (Dashboard / Machines / Shares / Projects / DDC Pak / PSO Cache / INI Scanner / Health Check)
- SQLite persistence: machines + machine_ue_installs + machine_gpus + credentials + share_configs + operations + projects + project_locations
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
- **INI Scanner**: project / user / engine INI scan, rule engine R001-R007, persisted findings, one-click apply for auto-fixable findings through the atomic backup write path.
- **Cluster Health Check**: 11-check matrix with KPI strip, cluster score, derived INI consistency + GPU/driver consistency, and remediation details per cell.
- **Diagnostic primitives**: diff code block, KPI tile, score tile, filter chip, finding hierarchy/detail, and health matrix components.
- **Diagnostic PowerShell sidecars**: `read-ini-file.ps1` for whole-file INI reads and `health-probes.ps1` for one-round-trip machine probes.
- **Hostname rename**: inline editor in the machine detail panel.
- Builds to a single .exe / .dmg / .AppImage.

## What's NOT yet implemented (next plans)

- lanPC real UE E2E for DDC Pak generation/distribution (blocked by host reachability during this implementation pass)
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
