# UECM Zen Integration — Operator Guide

UECM Plan 7 ships **zen daemon orchestration** alongside the existing legacy
DDC pak + SMB share flows. This guide covers what UECM manages, what it
deliberately doesn't touch, and the CLI / health / scan surface an operator
uses day-to-day.

Source-of-truth references:
- Plan: `docs/superpowers/plans/2026-05-18-uecm-plan-7-zen-integration.md`
- Fact-finding (UE 5.7.4 + zen 5.8.10 on lanPC, 2026-05-18):
  `docs/research/zen-launch-mechanism.md`
- Rule schema: `docs/research/zen-ini-rules.yaml`

---

## 1. What UECM does and doesn't do for zen

**Manages (in-scope):**
- Register / unregister `(machine, port)` endpoints in UECM's `zen_endpoints`
  table. Records role (`local` / `shared_upstream`), lifecycle
  (`editor_owned` / `installed_service`), data_dir, scheme, httpserverclass.
- Render zen.lua config for a given endpoint and (optionally) write it to
  the remote host.
- Probe `/health`, `/health/info`, `/stats`, `/stats/z$` on registered
  endpoints; persist the latest CB blob + extracted flat columns.
- Detect installed zen binaries (zen.exe + zenserver.exe) and compute
  SHA256 baselines per zen build version.
- Install / uninstall / start / stop the `ZenServer` Windows service
  (cluster master nodes only; `editor_owned` endpoints are skipped).
- Manage URL ACL reservations for the zen HTTP listener.
- Enable / disable ZenShared upstream + legacy SMB / Pak cleanup against
  project `DefaultEngine.ini` files (cluster-wide fan-out).
- Surface health (zen_reachable / version_consistent / binary_intact /
  cache_provider_ready) per machine.
- Emit INI-scanner findings R012-R018 against project INIs.

**Deliberately out of scope:**
- Running `zen service install --full`. Plan §12 Red Line #1: re-installing
  zen binaries server-side is treated as untrusted upgrade, hard-blocked at
  the PS sidecar level.
- Auto-restoring legacy SMB / Pak configuration on `zen disable`. The
  `zen disable` flow is **narrow** — it removes the ZenShared key only.
  Operators wanting the prior config back should use `git restore` or the
  per-key `.bak.<timestamp>` sibling files created by `core::ini_editor`.
- Editing zen binaries on disk. Both install-path and InTree binaries are
  read-only baselines from UECM's perspective. InTree drift is logged
  (not surfaced as findings) per Plan §M4 T4.3.
- Storing zen secrets outside DPAPI. All credential storage continues to
  use the existing UECM cred workflow.

---

## 2. CLI command reference

All commands are agent-friendly via `--json`. Destructive operations require
`--yes` or `--dry-run`.

### 2.0 Per-machine UE runtime user

```
uecm-cli machine set-ue-user --machine ID --ue-user USERNAME
uecm-cli machine set-ue-user --machine ID --ue-user ""   # clear
```

Sets the Windows username that runs UE on the target machine. Required before
`zen enable --global` can resolve the `UserEngine.ini` path. The username is
stored in `machines.ue_runtime_user` (nullable TEXT) and is never used for
authentication — it only affects INI path construction.

Path resolved: `C:\Users\<ue_runtime_user>\AppData\Roaming\Unreal Engine\Engine\Config\UserEngine.ini`

### 2.1 Endpoint registry

```
uecm-cli zen list-endpoints                            # read-only
uecm-cli zen register   --machine ID --declared-port PORT
                        --scheme http
                        --role local|shared_upstream
                        [--upstream-endpoint-id ID]
                        --data-dir PATH
                        --httpserverclass asio|httpsys
                        --lifecycle editor_owned|installed_service
uecm-cli zen unregister --endpoint-id ID [--yes]
```

Idempotency: registering an existing `(machine_id, declared_port)` returns
`{inserted: false, endpoint_id: N}` and **does not overwrite** the recorded
row. To change `role` / `lifecycle` / `data_dir`, unregister and re-register.

Role state machine:
- `local`: standalone or worker pointing upstream. May have
  `upstream_endpoint_id` set.
- `shared_upstream`: cluster master. Must be `lifecycle=installed_service`
  (editor-owned sponsors would preempt a master, breaking workers). Must
  NOT have `upstream_endpoint_id`.

Datadir safety (T2.8): paths under `C:\Windows`, `C:\Program Files`,
`C:\Program Files (x86)` are rejected at register time. Win32 device
namespace prefixes (`\\?\` / `\\.\`) and drive-relative paths (`D:zen`)
are also rejected so `GetFullPath` can't redirect into a forbidden root.

### 2.2 Probing

```
uecm-cli zen status        [--machine ID | --all] [--cred-alias ALIAS]
uecm-cli zen probe         [--machine ID | --all] [--cred-alias ALIAS] [--timeout SECS]
uecm-cli zen cache-stats   [--endpoint-id ID | --all] [--timeout SECS]
uecm-cli zen detect-binary [--machine ID | --all] --cred-alias ALIAS
```

`status` is the cheap read-only view (latest probe per endpoint).
`probe` actively hits `/health`, `/health/info`, `/health/version` and
persists a `zen_probes` row. `cache-stats` does the same against `/stats`
and `/stats/z$`. `detect-binary` scans `%LOCALAPPDATA%\UnrealEngine\Common\
Zen\Install\` plus each UE install's `Engine\Binaries\Win64\` directory.

### 2.3 Lifecycle + URL ACL

```
uecm-cli zen apply-config   --endpoint-id ID --dest-path PATH --cred-alias ALIAS [--yes] [--dry-run]
uecm-cli zen lua-preview    --endpoint-id ID
uecm-cli zen service install --endpoint-id ID --cred-alias ALIAS [--yes] [--dry-run]
uecm-cli zen service start   --endpoint-id ID --cred-alias ALIAS
uecm-cli zen service stop    --endpoint-id ID --cred-alias ALIAS [--yes] [--dry-run]
uecm-cli zen service status  --endpoint-id ID --cred-alias ALIAS
uecm-cli zen service uninstall --endpoint-id ID --cred-alias ALIAS [--yes] [--dry-run]
uecm-cli zen urlacl add      --endpoint-id ID --principal USER --cred-alias ALIAS [--yes]
uecm-cli zen urlacl list     --machine ID --cred-alias ALIAS
uecm-cli zen urlacl remove   --endpoint-id ID --cred-alias ALIAS [--yes]
```

`apply-config` renders `zen.lua` and writes it via `zen-write-lua-config.ps1`
with SHA256 round-trip verification. `--dest-path` is mandatory — there is
no default. M4 will derive it automatically from `machine_zen_install.install_dir`.

`service install` / `start` / `stop` are gated on
`lifecycle_mode = installed_service`. Trying these against an
`editor_owned` endpoint returns a clear error pointing at `zen_unregister`
+ `zen_register` with the corrected lifecycle (idempotency contract means a
plain re-register loops on the no-op path).

### 2.4 Project enable / disable

**Project-level mode (per-project config via `DefaultEngine.ini`):**

```
uecm-cli zen enable  --project-id ID --machines M1,M2,...
                     --upstream-endpoint-id E
                     [--namespace ue.ddc]
                     --cred-alias ALIAS [--yes] [--dry-run]
uecm-cli zen disable --project-id ID --machines M1,M2,...
                     --cred-alias ALIAS [--yes] [--dry-run]
```

`enable` does, per target machine:
1. Resolves the project's `DefaultEngine.ini` location via `project_locations`.
2. Adds `[InstalledDerivedDataBackendGraph] ZenShared=...` pointing at the
   `--upstream-endpoint-id`'s master.
3. Removes legacy `Shared=` (SMB) and `Pak=` / `CompressedPak=` keys.
4. Clears the `UE-SharedDataCachePath` env var on both Machine and User
   scope via `zen-env-cleanup.ps1`.

Pre-flight: every machine's project_location row is resolved before any
write happens. A missing prereq aborts the whole command (no half-applied
state across the cluster).

Continue-on-failure: per-machine errors are recorded but don't stop the
fan-out. The exit code reflects whether all machines succeeded.

`disable` is **narrow** — removes only the ZenShared key, does NOT
auto-restore legacy SMB / Pak. The outcome always carries a warning to
this effect.

**Global mode (all-project config via `UserEngine.ini`):**

```
uecm-cli zen enable --global \
  --machines M1,M2,... \
  --upstream-endpoint-id E \
  [--namespace ue.ddc] \
  --cred-alias ALIAS [--yes] [--dry-run]

uecm-cli zen disable --global \
  --machines M1,M2,... \
  --cred-alias ALIAS [--yes] [--dry-run]
```

`--global` writes `ZenShared` to each machine's `UserEngine.ini` instead of a
specific project's `DefaultEngine.ini`. This applies to **all** UE 5.4+ projects
for that Windows user without per-project configuration.

Pre-flight: every `--machines` target must have `ue_runtime_user` set (see §2.0).
`--global` and `--project-id` are mutually exclusive.

`UserEngine.ini` is created if absent (`CreateIfMissing` transport flag).

### 2.5 Backend routing for ddc commands

```
uecm-cli ddc generate / verify / distribute --backend auto|legacy|zen [other flags]
```

- `--backend zen`: no-op JSON `{backend:"zen", skipped:true,
  reason:"zen handles caching natively"}`, exit 0. Use this on machines
  whose project should already be served via ZenShared.
- `--backend legacy`: existing DDC pak path runs.
- `--backend auto` (default): `core::cache_backend::resolve_for` picks:
  - project UE < 5.4 → LegacyPak
  - project UE unknown → LegacyPak (conservative)
  - project UE ≥ 5.4 + machine has 5.4+ install + reachable zen → Zen
  - otherwise → LegacyPak

Explicit `project_cache_backend` row override (per-project per-machine)
always wins over the decision table.

### 2.6 Baselines

```
uecm-cli zen baseline list   [--zen-build-version V] [--kind zen_cli|zenserver]
uecm-cli zen baseline lock   --zen-build-version V --kind K --locked-by NAME [--yes] [--dry-run]
uecm-cli zen baseline unlock --zen-build-version V --kind K [--yes] [--dry-run]
```

Sets / clears the `locked_by` marker on a `zen_binary_expected` row. Used
by R016 (install binary drift detection): a locked baseline is treated
as authoritative; mismatching install-path SHA256s raise a warning.

### 2.7 verify-rules

```
uecm-cli zen verify-rules --ue-version X.Y --ue-install PATH [--write-verified]
```

Resolves `zen-ini-rules.yaml` against a UE version, prints the effective
rule set, optionally appends the version to `verified_versions` in the
yaml. Useful when validating a new UE version's rules before rolling out
a cluster-wide `zen enable`.

DB-free: this command resolves the yaml only. Works even when the
SQLite DB is unwritable.

---

## 3. INI scanner — new zen rules

`uecm-cli ini scan` auto-enables R012-R018 when the target machine has at
least one zen endpoint registered. No flag needed.

| Rule | Severity | Triggers when |
|---|---|---|
| R012 | warning  | ZenShared key missing in `[InstalledDerivedDataBackendGraph]` of a project's `DefaultEngine.ini` while a zen endpoint exists for the machine |
| R013 | critical | ZenShared value doesn't match `(Type=Zen, Host="...", Port=..., Namespace="...")` shape |
| R014 | warning  | ZenShared.Host points at a host with no recent reachable probe |
| R015 | warning  | Legacy `Shared=` key OR `UE-SharedDataCachePath` env var coexists with ZenShared (SMB+Zen double-write) |
| R016 | warning  | Install-path `zenserver.exe` SHA256 differs from the locked baseline |
| R017 | warning  | Legacy `Pak=` / `CompressedPak=` keys still present alongside ZenShared |
| R018 | warning  | This machine's `zenserver_build_version` is the minority in a cluster (≥3 machines, strict >50% majority) |
| R026 | warning  | Global ZenShared (UserEngine.ini) and project-level ZenShared both present on same machine — project config takes precedence and may shadow the global setting |

InTree (per-UE-install) zenserver drift is logged via `tracing` but does NOT
produce a finding — Plan §M4 T4.3 explicitly treats InTree as informational.

---

## 4. Health check

`uecm-cli health check` per-machine row now includes 4 zen keys (and these
land in both the online and offline paths so the UI gets a stable layout):

- `zen_reachable` — latest probe < 5 min + `reachable=1`.
- `zen_version_consistent` — `machine_zen_install.zenserver_build_version`
  matches the cluster strict majority.
- `zen_binary_intact` — install-path SHA256 == `zen_binary_expected` baseline.
- `zen_cache_provider_ready` — latest `/stats` row's provider list contains
  `z$` (the UE DDC namespace), within the 1-hour freshness window.

If a row reads `unknown`, that's the "no data yet" state — probably no
endpoint registered / no probe run / no binary detected. Operators
distinguish this from `warning` / `critical` (= we have data and it's bad).

---

## 5. Secret redaction

Three CLI / Tauri flags are auto-redacted whenever a command line lands
in `operations.log_text`: `--access-token`, `--password`, `--api-key`.

The redactor handles:
- `--flag=value` / `--flag value` / `--flag "..."` / `--flag '...'`
- PowerShell backtick-escaped quotes inside `"..."` (`` `" ``)
- PowerShell doubled `''` inside `'...'`
- Values containing `&` / `;` / `|` shell separators
- Multi-byte UTF-8 (CJK / emoji) preserved around redactions
- Dash-prefixed secret values (`-hunter2`)
- Quoted command-line wrappers (`args '--password ...'`)
- Idempotency: `redact(redact(s)) == redact(s)`

Both the invocation string and any error text appended on failure are
scrubbed before persist.

---

## 6. Troubleshooting

### "service install" returns "already installed with different config"
The service exists with a different `--data-dir` or `zen.exe` path than
the current request. Per Plan §12 Red Line #1 we won't auto-`--full`-
reinstall. Run `zen service uninstall --endpoint-id ID --cred-alias ALIAS`
first, then `zen service install` again.

### "URL reservation already exists but is owned by 'X'"
Another principal owns the netsh URL ACL for the same prefix. UECM
refuses to silently report success since the zen service would still
fail to bind. Use `netsh http delete urlacl url=...` on the host (admin
shell) and re-add.

### "service install/start/stop requires lifecycle_mode=installed_service"
The endpoint was registered as `editor_owned`. Unregister + re-register
with `--lifecycle installed_service`. A simple re-register is a no-op
on `(machine_id, declared_port)` and doesn't update lifecycle.

### `zen detect-binary` reports "install_dir absent"
This host has never opened a UE 5.4+ editor. The `Common\Zen\Install\`
directory is created on first editor launch. Either launch UE 5.4+
once or distribute the install directory contents from a healthy node.

### Health row `zen_cache_provider_ready` flips to `warning`
Run `uecm-cli zen cache-stats --endpoint-id ID` to refresh the latest
`/stats` row. The check requires data within the last hour. If the row
shows providers without `z$`, the zen daemon is up but not serving the
UE DDC namespace — usually a sign zen was started with no project
sponsor.

### Cluster shows R018 version drift
Run `uecm-cli zen detect-binary --all --cred-alias ALIAS` to refresh
every machine's binary inventory. Cluster consistency = all machines'
`Common\Zen\Install\` directories on the same `zenserver_build_version`.
Sync the `Install\` directory from the highest-versioned host to the
outlier(s) via SMB / robocopy.

### `zen enable` fails on a subset of machines
The CLI continues on per-machine failure and reports `ok=false` for the
overall run. Per-machine `error` field in the JSON output names the
specific failure (typically: missing `project_locations` row, WinRM not
reachable, or `UE-SharedDataCachePath` env var cleanup failed at Machine
scope due to non-admin cred). Fix the failing nodes and re-run; the
operation is idempotent on the per-key INI level (`changed=false` for
machines already in the target state) and the env-var cleanup is
re-driven from `env_cleanup_planned` regardless of the INI changed flag
so partial-failure recovery doesn't get stuck.

---

## 7. Deferred items (need lanPC + UE editor)

The following M4/M5 items are deferred and will land when lanPC is back
online:

- **T4.4** `core::zen::verify` — headless editor + log watcher to
  confirm a `zen enable` actually produced the
  `LogDerivedDataCache: ZenShared ... status: OK!` line.
- **T4.6** `zen-verify-rules.ps1` — PS sidecar that drives T4.4.
- **T4.7** M4 acceptance — 5-machine fixture project run-through.
- **T5.1** retention GC empirical test (7-day window).
- **T5.4** §7.1 11-step agent-driven path end-to-end on lanPC.
- **T5.5** zen schema drift detection at release time.

What ships today is the **offline portion** of these — the resolver,
the data model, the orchestration, and unit/integration tests up to
the loopback boundary. Production wiring against a real Windows host
will follow when the test cluster comes back.
