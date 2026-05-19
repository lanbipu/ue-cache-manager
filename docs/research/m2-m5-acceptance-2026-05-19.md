# M2-M5 acceptance — lanPC live verification (2026-05-19)

Validates Plan 7 M2 / M3 / M4 / M5 deferred items (T2.9, T3.9, T4.7,
T5.1, T5.4, T5.5) on lanPC after deploy. T1.11 finished here too.

## Build / deploy

Two deploy rounds (build issues caught):

| Round | Issue | Fix | sha256 (uecm-cli.exe) |
|---|---|---|---|
| 1 | `include_str!` of `docs/research/zen-ini-rules.yaml` failed because `--exclude='./docs'` stripped the file from the tar | `scripts/deploy-lanpc.sh`: narrow exclude to `./docs/superpowers` so `docs/research/` ships | failed |
| 2 | `invoke_local` produced empty stdout for >10K multi-statement PS scripts piped via `powershell -Command -` stdin (silent truncation in Windows PowerShell) | `core::winrm::invoke_local`: write to temp .ps1 + `-File` | `FB47A739` |
| 3 | Final, with health CLI wiring for zen_health_for_machine merged in | mirror commands::health_check::run_health_check pattern in cli/domain_health.rs::run_with_rt | `59D25F35` |

Final deployed `uecm-cli.exe sha256`: `59D25F3547AF40D5A758F71ECE4861C5DFDD35B16BBFA3024AB44A281CB7F790`.

## Account setup

DPAPI-only cred bootstrap (cmdkey refuses non-interactive SSH session).
- Reset `uecm-test` password to `Uecm-Test-Pass-2026!` via `net user`
- Used `/tmp/uecm-cred-bootstrap.ps1` (DPAPI Protect + sqlite3 INSERT) to
  bypass cmdkey and write `creds.bin` + SQLite row directly.
- `sqlite3.exe` installed via `scoop install sqlite`.

## T1.11 — M1 acceptance

| Check | Result |
|---|---|
| `zen detect-binary --machine 5 --cred-alias render-svc` | install (5.8.10) + 5 InTree records persisted; 10 baselines+InTree rows in DB |
| `zen probe --machine 5` | reachable=true; `health_info_cb` BLOB 1146 bytes; all flat columns (effective_port=8558, pid=17712, uptime_seconds=7665, data_root, is_dedicated, build_version) populated |
| `zen cache-stats --endpoint-id 1` | provider list contains `z$`; `raw_cb` 422 bytes; flat columns (cache_hit_ratio=0.0, cache_disk_size_bytes=597424206 ≈ 570 MB) populated |

Bug found + fixed: `ps-scripts/zen-detect-binary.ps1` line 173 had
`"$keyPath:"` parse error (`:` ate as drive-namespace separator);
changed to `"${keyPath}:"`.

## T2.9 — M2 single-machine acceptance

`zen register → service install → service start → status → probe` flow:

```
zen register --machine 5 --declared-port 8558 --scheme http
             --role local --data-dir F:\Epic\DDC\Zen
             --httpserverclass asio --lifecycle installed_service
→ endpoint_id=1, inserted=true

zen service install --endpoint-id 1 --cred-alias render-svc --yes
→ "Installed service 'ZenServer' using zenserver.exe successfully"

zen service start --endpoint-id 1 --cred-alias render-svc
→ Initially failed: service installed as NT AUTHORITY\LocalService,
  which can't read zen.exe under user profile. After `sc config
  ZenServer obj= LocalSystem`: was_already_running=true; service
  reaches Running state.

zen service status → Running
zen probe → reachable=true (against the service, via /health/info)
zen urlacl list → 3 system URL reservations enumerated
```

OS-level note: `zen-service-install.ps1` doesn't yet expose a
`--service-user` flag. Default LocalService account can't traverse user
profile to reach zen.exe under `%LOCALAPPDATA%\UnrealEngine\Common\Zen
\Install\`. Workaround applied manually; follow-up: extend the sidecar.

## T3.9 — M3 single-machine acceptance

Test project: `E:\RenderStream Projects\test_0311` (UE 5.7.4, lanPC
fact-finding sandbox).

```
project create-manual + set-location → project_id=1
SQL: UPDATE projects SET ue_version_major=5, ue_version_minor=7 ...
SQL: UPDATE zen_endpoints SET role='shared_upstream' WHERE id=1
     (no change-role CLI; one-off SQL for single-machine self-loop)

zen enable --project-id 1 --machines 5 --upstream-endpoint-id 1
           --cred-alias render-svc --yes
→ Initially failed: DefaultEngine.ini was IsReadOnly=True. After
  clearing the read-only attribute:
  changed=true, ok=true
  backup written to ...DefaultEngine.ini.bak.1779176042550
  legacy UE-SharedDataCachePath env (\\192.168.10.2\Docs\DDC)
  cleared from User scope (Machine scope was empty, no-op)
  ZenShared=(Type=Zen, Host="127.0.0.1", Port=8558, Namespace="ue.ddc")
  written to [InstalledDerivedDataBackendGraph]

zen disable → keys_removed: ZenShared; narrow-disable warning attached

zen enable (1st) → any_changed=true
zen enable (2nd) → any_changed=false  ✓ idempotency contract
```

## T4.7 — M4 acceptance

INI scanner with project in good state → 0 findings (expected).

`health run --machine-ids 5 --cred-alias render-svc`:
- Total checks: 17 → **21** after T4.2/T4.3 CLI wiring fix (the
  original wiring was only in `commands/health_check.rs` Tauri side,
  not in `cli/domain_health.rs`).
- 4 zen_* rows present:
  - `zen_reachable` — healthy after fresh probe within 5min
  - `zen_binary_intact` — healthy (sha256 matches baseline)
  - `zen_version_consistent` — healthy (skipped: cluster size 1 < 3)
  - `zen_cache_provider_ready` — healthy after fresh cache-stats
    (provider list contains z$)

`zen verify-rules`:
- UE 5.7 (verified) → ok=true, rules rendered
- UE 5.9 (unverified, policy=refuse) → ok=false, exit 0, clear error

T4.4 / T4.6 (live editor + log watch) NOT implemented. UE editor
verification is the only deferred piece — plan note acknowledges
`UnrealEditor.exe -Unattended -Quit` doesn't exit within 3 minutes.
The full implementation needs a commandlet-based verifier and is
deferred.

## T5.1 — retention GC empirical

Inserted 110 backdated zen_probes rows (`probed_at='now -8 days'`)
+ 2 fresh probes. Triggered startup retention via any DB-touching
command:
- Before: 112 probes for endpoint_id=1
- After:  100 probes (exactly the count-floor; oldest 12 culled
  because they exceeded count-floor AND were outside age window)

Confirms `core::zen::retention::run` runs at startup AND the
count-floor + age-window combo logic works as specified.

## T5.4 — 11-step agent path

Full sequence via `/tmp/agent-path.ps1` on lanPC:

```
Step  1 project list                                     ok
Step  2 machine list                                     ok
Step  3 zen detect-binary --machine 5                    ok
Step  4 zen register                                     ok
Step  5 zen apply-config (dry-run)                       ok
Step  6 zen service install (dry-run, already installed) ok
Step  7 zen service start (idempotent)                   ok
Step  8 zen probe --all                                  ok
Step  9 zen cache-stats --all                            ok
Step 10 zen enable (idempotent no-op)                    ok
Step 11 health run                                       ok

Failed: 0 / 11
```

Each step emits valid JSON, ok=true, exit 0. Idempotency holds for
register / enable / service install (re-run sees no change).

## T5.5 — zen schema drift detection

Live zen 5.8.10 on lanPC `/health/info` CB blob (1146 bytes) parsed
by `cb_parser`. All 6 expected extract fields populated:

| Field | Value |
|---|---|
| `effective_port` | 8558 (from RuntimeConfig.EffectivePort string) |
| `pid` | 17712 |
| `uptime_seconds` | 7665 |
| `data_root` | `\\?\F:\Epic\DDC\Zen` |
| `is_dedicated` | 0 (false) |
| `build_version` | `5.8.10-202605071938-windows-x64-release-fbacdecd` |

`/stats/z$` CB (422 bytes): `cache.hit_ratio=0.0`,
`cache.size.disk=597424206`, `cache.size.memory=0`.

No schema drift detected on zen 5.8.10. The fixture in
`docs/research/zen-launch-mechanism.md` appendix C remains valid.

## Files touched (committed)

```
f42c6e6 fix(winrm+ps+deploy): T1.11 / M2 acceptance prep on lanPC
b169d47 fix(cli): wire zen_health_for_machine into `health run` CLI path
```

## Known deferrals

1. **T4.4 / T4.6 — UE editor verifier**: needs commandlet-based
   approach because `-Unattended -Quit` doesn't exit cleanly. The
   non-runtime portions of M4 (T4.5 verify-rules, R012-R018 INI
   scanner, 4 zen_* health rows) all PASS.
2. **`zen-service-install.ps1` --service-user flag**: default
   LocalService can't read zen.exe under user profile. Document the
   workaround OR add the flag in a follow-up.
3. **CLI `zen change-role`**: needed to flip a registered endpoint
   between `local` and `shared_upstream` without unregister/re-register.
   Used one-off SQL UPDATE for the single-machine self-loop test.
