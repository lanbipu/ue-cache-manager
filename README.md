# UE Cache Manager (UECM)

Cross-machine Unreal Engine cache management tool for VP/XR render clusters.

Status: Plan 1 (Foundation) — in progress.

## Development

Prerequisites: Rust 1.75+, Node.js 20+, pnpm, Tauri prerequisites for your OS.

```bash
pnpm install
pnpm tauri dev    # launch in dev mode
pnpm test         # frontend tests
cd src-tauri && cargo test    # backend tests
pnpm tauri build  # produce installable artifact
```

See `docs/superpowers/specs/` for design documents.
See `docs/superpowers/plans/` for implementation plans.
