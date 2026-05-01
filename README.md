# UE Cache Manager (UECM)

Cross-machine Unreal Engine cache management tool for VP/XR render clusters.

**Status:** Plan 1 (Foundation) complete. See `docs/superpowers/plans/`.

## What's working

- Tauri 2.x app shell with 7 navigable views (Dashboard / Machines / Projects / DDC Pak / PSO Cache / INI Scanner / Health Check)
- SQLite persistence with schema migrations (machines table)
- Add / list / delete machines manually via the Machines view
- PowerShell sidecar bridge (verified by Dashboard "bridge test" button)
- Builds to a single .exe / .dmg / .AppImage

## What's NOT yet implemented (next plans)

- Network discovery, UE / GPU detection (Plan 2)
- SMB share creation wizard, credential management (Plan 3)
- INI scanner, health check matrix (Plan 4)
- DDC Pak generation + distribution (Plan 5)
- PSO Cache operations + visual polish (Plan 6)

## Development

Prerequisites:
- Rust 1.75+ (https://rustup.rs/)
- Node.js 20+
- pnpm: `npm install -g pnpm`
- OS-specific Tauri prerequisites: see https://v2.tauri.app/start/prerequisites/

```bash
pnpm install
pnpm tauri dev    # launch in dev mode (hot reload)
pnpm test         # frontend unit tests (Vitest)
cd src-tauri
cargo test        # backend unit tests
cd ..
pnpm tauri build  # produce installable artifact
```

## Architecture

- `src/`: Vue 3 frontend (TypeScript, Tailwind, Pinia, Vue Router)
- `src-tauri/`: Rust backend (rusqlite, serde, tokio)
- `ps-scripts/`: PowerShell sidecar scripts (Windows-only execution)
- `docs/superpowers/specs/`: design docs
- `docs/superpowers/plans/`: implementation plans

Frontend ↔ backend communication via Tauri commands. All command handlers
in `src-tauri/src/commands/` are thin wrappers that delegate to logic in
`src-tauri/src/core/` and data in `src-tauri/src/data/`.

## Platform support

Target: Windows 10/11. Foundation in this plan builds + runs on Mac/Linux
for development convenience, but PowerShell sidecar (and future
WinRM/registry/credential features) only work on Windows. Tests gated
appropriately with `#[cfg(windows)]`.
