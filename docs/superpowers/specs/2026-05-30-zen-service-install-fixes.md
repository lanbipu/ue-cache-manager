# Spec: zen-service-install.ps1 Bug Fixes (Codex P2)

**Date:** 2026-05-30
**Status:** Fixed (commit d53bcab)
**Source:** Codex review against main — 2 P2 findings in `ps-scripts/zen-service-install.ps1`

---

## 1. Problem A — Idempotency drift for `--port` / `--http` flags

### Root cause

The existing-service idempotency check (lines ~329–331 before fix) only compared:
- exe path (`ZenExePath`)
- data directory (`--data-dir`)
- service account (`ServiceUser`)

It did **not** compare `--port` or `--http`. After the zen-global feature (T2026-05-30) wired `--port` and `--http` into the SCM `ImagePath` via registry patch, a re-install with the same `data-dir` but a different `port` or `httpserverclass` would:
1. Return `already_installed=true` (false positive idempotency)
2. Leave the SCM `ImagePath` pointing at the old port
3. Zen continues listening on the wrong endpoint after service restart

### Fix

After the existing token-parsing loop that extracts `--data-dir`, add a second pass for `--port` and `--http`:

```powershell
$existingPort = $null
$existingHttp = $null
for ($i = 0; $i -lt $tokens.Count; $i++) {
    $t = $tokens[$i].ToString()
    if ($t -ieq '--port' -and ($i + 1) -lt $tokens.Count) {
        $existingPort = $tokens[$i + 1].ToString()
    }
    if ($t -ieq '--http' -and ($i + 1) -lt $tokens.Count) {
        $existingHttp = $tokens[$i + 1].ToString()
    }
}

$requestedPort = if ([string]::IsNullOrWhiteSpace($Port)) { $null } else { $Port.Trim() }
$requestedHttp = if ([string]::IsNullOrWhiteSpace($HttpServerClass)) { $null } else { $HttpServerClass.Trim() }

$portMatches = ($null -eq $requestedPort) -or ($existingPort -ieq $requestedPort)
$httpMatches = ($null -eq $requestedHttp) -or ($existingHttp -ieq $requestedHttp)

$matchesExpected = ($existingExe -eq $expectedExe) -and
                   ($null -ne $existingDir) -and
                   ($existingDir -eq $expectedDir) -and
                   $portMatches -and $httpMatches
```

**Semantics:** `$null` requested port/http = "no constraint" — services installed before the zen-global feature (which had no `--port` / `--http` in `ImagePath`) still satisfy the idempotency check. Only explicit port/http requests are compared.

### Impact if unpatched

- `zen service install` called with port 8559 on a machine already running on 8558: returns `ok=true already_installed=true` without updating the service, user gets no error, Zen stays on 8558.

---

## 2. Problem B — Unquoted `--data-dir` in ImagePath binpath patch

### Root cause

The ImagePath patch (lines ~553–560 before fix) built the runtime args as:

```powershell
$runtimeArgs = "--data-dir $normalizedDataDir"
```

`$normalizedDataDir` is inserted without quoting. The resulting `ImagePath` for a path containing spaces looks like:

```
"C:\...\zenserver.exe" --data-dir F:\UE Cache\Zen --port 8558 --http asio
```

SCM parses this as:
```
argv[0] = C:\...\zenserver.exe
argv[1] = --data-dir
argv[2] = F:\UE           ← WRONG: only first segment
argv[3] = Cache\Zen       ← stray arg, ignored
argv[4] = --port
...
```

Zen receives `F:\UE` as `data-dir`, which doesn't exist → service fails to start.

### Fix

Wrap `$normalizedDataDir` in escaped double-quotes:

```powershell
$runtimeArgs = "--data-dir ""$normalizedDataDir"""
```

The resulting ImagePath for the example above:

```
"C:\...\zenserver.exe" --data-dir "F:\UE Cache\Zen" --port 8558 --http asio
```

SCM now correctly passes `F:\UE Cache\Zen` as a single token.

### Impact if unpatched

- Any data dir with spaces (valid Windows path) silently produces a broken service that fails to start. The script reports `binpath_patched=true` but the service command line is incorrect.

---

## 3. Files changed

| File | Change |
|---|---|
| `ps-scripts/zen-service-install.ps1` | Both fixes applied. Deploy to `C:\ProgramData\UECM\ps-scripts\` via `scripts/deploy-lanpc.sh` (requires admin). |

## 4. No spec changes needed

Neither fix changes the external API (JSON input/output shape, Rust caller code). Both are PS-internal correctness fixes. The Rust layer in `src-tauri/src/commands/zen.rs` and `src-tauri/src/cli/domain_zen.rs` is unchanged.
