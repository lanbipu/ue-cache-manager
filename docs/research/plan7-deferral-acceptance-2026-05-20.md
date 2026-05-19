# Plan 7 — deferral acceptance + branch-wide fix sweep (2026-05-19/20)

Continues `m2-m5-acceptance-2026-05-19.md`. Resolves the three deferrals
listed there and documents the cross-cutting fixes Codex surfaced when
the entire `worktree-plan7-zen-integration` branch was reviewed against
`main` (rather than per-commit).

## Three deferrals — resolved

### T41 — `uecm-cli zen change-role` / Tauri `zen_change_role`

Flip a registered endpoint between `local` and `shared_upstream` without
the unregister/re-register dance.

Acceptance on lanPC (machine_id=5, endpoint_id=1, current role
`shared_upstream`):

| Command | Result |
|---|---|
| `--new-role local --dry-run --yes` | `ok`, preflight emits role transition plan |
| `--new-role shared_upstream --dry-run --yes` (no-op) | `ok` |
| `--new-role shared_upstream --upstream-endpoint-id 1` | rejected: `shared_upstream endpoint must not have an upstream_endpoint_id` |

`validate_change_role` runs read-only on `--dry-run`; the apply path
shares `validate_change_role_tx` with the dry-run validator so the two
can't drift.

### T42 — `zen-service-install.ps1 --service-user / --service-pass`

`zen.exe service install` on Windows hard-codes
`CreateService(..., "NT AUTHORITY\\LocalService", NULL)` and ignores its
own `-u`/`-p` install options (verified against
`/Users/bip.lan/AIWorkspace/vp/zen/src/zenutil/service.cpp:441-453`).
Forwarding the flags to zen is a no-op — we patch SCM via `sc.exe
config obj= ... password= ...` AFTER `zen service install` lands.

Hard rules baked into the sidecar after Codex review:
1. Built-in alias canonicalization (`LocalSystem` /
   `NT AUTHORITY\LocalService` / `NT AUTHORITY\NetworkService`) before
   `sc.exe`.
2. Non-built-in account without `-ServicePassword` rejected BEFORE
   `zen service install` runs, so a missing password never leaves a
   half-installed LocalService row.
3. `sc.exe config` failure auto-rolls back via `zen service uninstall`;
   the message switches to `"rollback FAILED — manual uninstall required"`
   when the rollback itself errors.
4. CIM-or-registry account read (`Get-ServiceAccount`) so hosts with
   WMI/CIM disabled by policy still get a correct comparison.

Rust-side mirrors the same rules at dry-run time
(`validate_service_account_pair` + `is_builtin_service_account`) so
`--dry-run` rejects unworkable plans instead of approving them.

Acceptance cases (lanPC, all `--dry-run`):

| Input | Result |
|---|---|
| `--service-user LocalSystem` | accepted (built-in, no password needed) |
| `--service-user .\uecm-test` (no password) | rejected with built-in-account hint |
| `--service-pass somepass` (no user) | rejected with require-user message |
| `--service-pass ""` (empty) | reported as `service_pass_supplied: false` in preview |

### T43 — `uecm-cli zen verify-rules --run-editor` / Tauri `zen_verify_rules`

Headless UE editor launch via WinRM that tails the editor log for the
ZenShared OK proof line, kills the editor on match/timeout, returns a
structured outcome.

Argument-validation acceptance (no editor needed):

| Input | Result |
|---|---|
| `--ue-version 5.7 --ue-install <UE_5.7>` (resolve-only) | `ok`, `verify_outcome: null` |
| `--machine 5` (no `--run-editor`) | rejected: verifier-only flag requires `--run-editor` |
| `--timeout-seconds 60` (no `--run-editor`) | rejected: same guard |
| `--expected-port 0` (or 99999) | rejected: port out of 1..=65535 |

Live editor run on `test_0311.uproject` (UE 5.7, zen running on lanPC),
240 s timeout:

```json
{
  "ok": true,
  "matched": true,
  "matched_host": "127.0.0.1",
  "matched_port": 8558,
  "matched_namespace": "ue.ddc",
  "elapsed_sec": 12,
  "killed": true,
  "match_line": "[2026.05.19-12.51.02:253][  0]LogDerivedDataCache: Display: ZenShared: Using ZenServer HTTP service at 127.0.0.1 with namespace ue.ddc status: OK!."
}
```

Three real-world issues surfaced during live runs and fixed:
1. `Get-Content` returns System.String values with ETS noteproperties
   (`PSPath`, `PSDrive`, ...) attached. `ConvertTo-Json -Depth 6` then
   emits `null` for the property. Cast each line to `[string]` to strip
   the noteproperties.
2. UE5 `-log -stdout` writes each entry twice (once `[time][frame]`,
   once ISO timestamp). `Find-MatchedPort` saw the duplicate as
   "multiple instances" and refused under the Codex P2 ambiguity guard.
   Dedupe on `(host, port)`.
3. ZenShared logs the configured host (`127.0.0.1`) while the local zen
   instance binds to `[::1]`; strict host equality refused to correlate.
   Added a single-distinct-instance fallback when no host match exists,
   keeping the multi-instance ambiguity protection.

## Codex branch-vs-main findings (rounds 14-22)

After every commit Codex was reviewing only the per-commit diff. Once
the deferral three landed, the next review pass switched to
`branch diff against main` and surfaced 10+ cross-cutting design gaps
that survived the per-commit reviews. Every finding was a real
operator-hit path; all P1/P2/P3 fixed.

| Round | File | Issue | Fix |
|---|---|---|---|
| 14 | `ini_diagnostics_zen.rs` | R015/R017 cleanup recommended `Remove` for legacy `Shared`/`Pak`/`CompressedPak` regardless of ZenShared state | gate on ZenShared configured, else `Manual` |
| 14 | `ini_scanner.rs` | `resolve(...).ok()` dropped all R012-R015 findings when `unverified_policy: refuse` (every UE not in verified_versions) | new `resolve_for_diagnostics` bypasses refuse for read-only callers |
| 14 | `core/zen/enable.rs` | host/namespace not validated before INI template substitution; `"`/`)`/`,`/newline in namespace produced malformed `ZenShared=(...)` | `validate_zen_field` against conservative charset before substitution |
| 15 | `core/ini_editor.rs` | local-loopback writer used `==` for key match while remote PS sidecar used case-insensitive; mixed-case existing key survived `Remove` while `changed=true` reported | `key_matches` switched to `eq_ignore_ascii_case` |
| 16 | `core/zen/endpoint.rs` | register accepted drive-relative/root-relative `data_dir`; lua-preview / apply-config wrote them straight into `server.datadir`, zen resolved against process CWD | require drive-abs or UNC at register |
| 16 | `commands/health_check.rs` + CLI | `zen_health_for_machine` error path only emitted `zen_reachable`, dropping 3 of 4 stable-layout keys | emit `unknown` for all 4 zen_* keys in both online + offline error branches |
| 17 | `core/health_check.rs` | remediation strings pointed at non-existent CLI syntax (`zen register --host`, `zen baseline insert`) | rewrite to use real `--machine <ID>` form and point at `zen detect-binary` |
| 18 | `core/winrm.rs` | temp `.ps1` script with inline secrets (`-ServicePassword`) could survive panic/abort between create and explicit `remove_file` | `TempScriptGuard` RAII Drop ensures cleanup on every unwind path |
| 18 | `commands/health_check.rs` offline path | `if let Ok zen_rows` silently dropped all 4 keys on DB error | same `unknown` fallback as online path |
| 18 | `core/health_check.rs` | `zen probe <endpoint>` remediation — no such positional | use real `--machine {id}` form |
| 19 | `core/health_check.rs` | machines with no zen endpoint registered were flagged `critical` — false alarm on legacy/non-zen/not-yet-configured hosts | gate at entry: no endpoint → all 4 keys `na` (segregated into `skipped` by `tally_summary`) |
| 20 | `cli/domain_zen.rs` | `validate_data_dir_safe` (lua-render path) didn't require fully-qualified absolute; relative paths from pre-existing DB rows leaked into `server.datadir` | add drive-abs / UNC check symmetric with sibling validators |
| 20 | `ini_diagnostics_zen.rs` | R016 finding hard-coded `file_path: "<machine:zenserver.exe>"`; multi-machine scans produced indistinguishable rows | use `file.path` (per-machine `<machine:{id}>` marker) |
| 21 | `core/health_check.rs` + `core/ini_scanner.rs` | `check_zen_version_consistent` and R018 computed cluster majority from every install row in the DB; an unrelated cluster could outvote the scan target | thread `cluster_scope: Option<&[i64]>` through, production callers pass current machine_ids |

## Round 22 — Codex hit usage limit

`Codex error: You've hit your usage limit. ... try again at 3:20 AM.`

Self-review of the round-21 cluster_scope diff (the most recent unreviewed
commit) found 0 functional bugs and 1 cosmetic (duplicate doc paragraph,
fixed in `0f20c6e`). Filter logic, all 5 production callers, test
fixtures, regression test all verified by hand.

## Tests

```
test result: ok. 838 passed; 0 failed; 0 ignored; 0 measured;
0 filtered out; finished in 6.73s
```

12+ new regression tests added across the round 14-21 fixes — every
codex finding either has a unit test demonstrating the bug or relies on
an existing test that newly exercises the fixed branch.

## Commit history (this round of work)

```
0f20c6e docs(zen): merge duplicate doc comment on zen_health_for_machine
1cd3377 fix(zen): Codex round-21 P2 — scope cluster majority to the active scan
ff3352f fix(zen): Codex round-20 P2 — strict data_dir in lua-render + per-machine R016
d31c815 fix(zen): Codex round-19 P2 — `na` for machines not opted into zen
7c2dc48 fix(zen): Codex round-18 P2/P3 — temp script cleanup + complete offline health
384fe64 fix(zen): Codex round-17 P3 — health remediation text uses real CLI commands
bc5a400 fix(zen): two Codex round-16 P2 — strict data_dir + complete health row
d02f45d fix(zen): case-insensitive local INI key match (Codex round-15 P2)
e58a582 fix(zen): three Codex round-14 P2 findings on the full branch diff
c7defb3 fix(zen-verify-rules): three correctness issues found during lanPC T43 acceptance
c69ce82 fix(zen): empty --service-pass shows as not-supplied in dry-run preview
9390fb8 feat(zen): Plan 7 deferral three — change-role / service-user / UE editor verifier
```

## Known follow-ups (Plan 8 candidates)

1. **`core::winrm::invoke_local` secret-in-script-body**: the RAII guard
   guarantees cleanup, but secrets still live in `%TEMP%` for the
   PowerShell child's lifetime. Eliminating that exposure requires
   reworking every sidecar that consumes a secret to accept it via
   stdin or env var — out of scope for Plan 7.
2. **Codex round 22 re-run**: after the 3:20 AM quota reset, run a
   final review pass to confirm self-review didn't miss anything in
   the cluster_scope diff.
3. **Tauri UI wiring** for the new commands (`zen_change_role`,
   `zen_verify_rules`) — the CLI is operational; the GUI surface for
   these specific actions hasn't been built yet.
