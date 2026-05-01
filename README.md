# UE Cache Manager (UECM)

Cross-machine Unreal Engine cache management tool for VP/XR render clusters.

**Status:** Plan 2 (Discovery & Single-Machine Config) complete. See `docs/superpowers/plans/`.

## What's working

- Tauri 2.x app shell with 7 navigable views
- SQLite persistence: machines + machine_ue_installs + machine_gpus + credentials
- **Network discovery**: scan a CIDR, probe ports 5985 (WinRM) and 445 (SMB), add reachable hosts
- **Per-machine refresh**: probe WinRM, then read installed UE versions (registry) + GPU model + driver version (WMI)
- **Credential management**: store WinRM credentials via cmdkey + Windows Credential Manager; aliases tracked in SQLite
- **Single-machine env var config**: read/write system-level env vars on a remote host (e.g. `UE-SharedDataCachePath`)
- **Single-machine INI editing**: read [section] keys + set a single key with auto-backup (`<file>.uecm-bak-<unix-ts>`)
- Builds to a single .exe / .dmg / .AppImage

## What's NOT yet implemented (next plans)

- SMB share creation wizard (Mode A / Mode B), SYSTEM credential injection (Plan 3)
- Cluster batch configuration pushes (Plan 3)
- INI conflict scanner + auto-fix (Plan 4)
- Cluster health check matrix (Plan 4)
- DDC Pak generation + distribution (Plan 5)
- PSO Cache operations + visual polish (Plan 6)

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
