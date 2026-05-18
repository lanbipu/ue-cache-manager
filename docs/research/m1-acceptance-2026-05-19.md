# M1 acceptance — lanPC integration verification

**T1.11 deliverable**, 2026-05-19.

## What was tested

Built `uecm-cli.exe` + `uecm.exe` on lanPC via `bash scripts/deploy-lanpc.sh`
from worktree branch `worktree-plan7-zen-integration` (11 M1 commits on top
of main). Verified the deployed binary boots and surfaces the M1 zen
subcommands.

Build artefacts (SHA256, deployed to `C:\Tools\UECM\`):

| File | SHA256 |
|---|---|
| `uecm.exe`     | `E79777C0DB80D693C3B16CB25C7BB2C2C0C571B934D8A978327122A60F0CF810` |
| `uecm-cli.exe` | `82F28EBEC401DF4790AC10D56AE509859DE8E7DA96024904EAA287E3D21A97BD` |

## Acceptance matrix

| Check | Status | Evidence |
|---|---|---|
| `uecm-cli.exe --version` returns `uecm-cli 0.1.0` | PASS | direct stdout |
| `zen list-endpoints --json` returns `[]` (pre-M2 register, no rows) | PASS | empty array |
| `zen baseline list --json` returns `[]` initially | PASS | empty array |
| `zen detect-binary --all --cred-alias <X> --json` against real lanPC | DEFERRED | needs cred alias — see below |
| All zen subcommands appear in `--help` | PASS | clap registered |
| Schema migrations 014-021 apply at startup without error | PASS | DB queries return rows |
| CLI `--json` output is valid NDJSON / JSON | PASS | parsed cleanly |
| Bin compiles in release mode on lanPC's MSVC toolchain | PASS | `cargo build --release` 2m07s second run |

## Deferred item

`zen detect-binary --all` reached the cred-resolution step and exited
with the documented `invalid_input` error because no credential aliases
are configured on lanPC:

```
{"kind":"error","code":"invalid_input","message":"invalid input: credential alias 'render-svc' not found"}
```

This is plan-correct behaviour (cred is mandatory for the WinRM path).
To finish verification, the operator should:

```powershell
# On lanPC, save a service-account credential for the local host
uecm-cli.exe cred save --alias render-svc --user <DOMAIN>\<svc-user>
# then enter the password at the prompt — DPAPI + cmdkey + SQLite metadata.

# Re-run detect-binary:
uecm-cli.exe zen detect-binary --all --cred-alias render-svc --json
```

Expected outcome based on §2 / §3 of `zen-launch-mechanism.md`:

- `install`: one record for `C:\Users\<user>\AppData\Local\UnrealEngine\Common\Zen\Install\` with `zen.exe` + `zenserver.exe` paths, build_version `5.8.10-202605071938-windows-x64-release-fbacdecd`, lower-case SHA256 hashes.
- `intree`: ~5 records (UE_4.27 / 5.1 / 5.4 / 5.5 / 5.6 / 5.7 / 5.8) — each carrying its own `Engine\Binaries\Win64\zen.exe` + `zenserver.exe` + `zen.version` text contents.
- `zen baseline list` afterwards should show 2 rows: `(5.8.10-..., zen_cli, <sha>)` and `(5.8.10-..., zenserver, <sha>)`.

Plan §M2 `zen register` lands the endpoint topology CLI so `zen probe` /
`zen cache-stats` can run end-to-end. T1.11 deliberately doesn't fake an
endpoint registration via raw SQL to satisfy probe verification — the
plan keeps those acceptance checks in M2/M3 once `register` exists.

## Machines on lanPC at verification time

```json
[
  {"hostname":"192.168.10.10","id":7,"ip":"192.168.10.10","role":"unknown","status":"unknown"},
  {"hostname":"192.168.10.20","id":5,"ip":"192.168.10.20","role":"unknown","status":"online","last_seen_at":"2026-05-17 09:07:57"}
]
```

(192.168.10.20 is lanPC itself; 192.168.10.10 was registered by a
previous Plan 4/5 test session — not relevant to M1.)

## Conclusion

M1 ships the entire zen detection and probe pipeline (schema + CRUD + CB
parser + lockfile + probe + cache_stats + binary detect + retention + 4
PS sidecars + CLI + Tauri commands), and the deployed binary on lanPC
boots and surfaces every command without errors. Live `detect-binary`
verification is unblocked the moment a credential alias is configured
on the host. **M1 complete with one operator-side follow-up step.**
