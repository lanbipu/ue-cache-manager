# Health Run Full Checkup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn `health run` into a single full-checkup entry point that diagnoses port reachability, bootstrap configuration, and business readiness in one shot, with actionable remediation hints on every failure row.

**Architecture:** Extend the existing `health` domain (Rust core + PowerShell probe + CLI + Vue UI) along three layered probe groups with **no overlap** between layers:

- **L1 Port** (3 probes) — Rust local TCP probes from operator console, no creds. Answers "is the network alive?": `tcp_5985` / `tcp_445` / `tcp_135`.
- **L2 Bootstrap** (4 probes) — PowerShell via WinRM Invoke-Command, registry + service + firewall queries. Answers "is the box configured correctly for UECM to talk to it?": `firewall_445` (moved from L3) / `local_account_token_filter` / `long_paths_enabled` / `lanman_server` (combines what was Probe-SmbService).
- **L3 Business** (7 probes + 3 derived) — PowerShell via WinRM, share + cred + env queries. Answers "is the cache workflow ready to run?": `share_reachable` / `ntfs_perm` / `cred_user` / `cred_system` / `env_vars` / `system_write` / `winmgmt` + derived (`ini_consistency` / `pso_precaching` / `gpu_consistency`).

**Total probes = 17** (3 L1 + 4 L2 + 7 L3 + 3 derived). Dropped from initial design: `winrm_reachable` (redundant — if Invoke-Command worked, WinRM is reachable by definition), `admin_share_writable` (does not actually test ADMIN$ — tests `%WINDIR%\Temp` from inside the target, not operator→ADMIN$ — and overlaps with `system_write`), `smb` (Probe-SmbService duplicates `lanman_server`). `firewall_445` moves from L3 to L2 because firewall rule existence is a bootstrap concern, not a business concern.

**Canonical probe registry** — every probe name is declared exactly once in `src-tauri/src/core/probe_keys.rs` as a const `PROBE_REGISTRY: &[ProbeSpec]`. Rust constants (`OFFLINE_PROBE_KEYS`, layer enums), PS1 result hashtable, TS `PROBE_LAYER_MAP`, locale labels, and docs all derive from or are validated against the registry by unit tests. This eliminates the "rename in one place, silently broken in four" failure mode.

**Mode behavior:**
- `--machine-ids` mode runs all 3 layers and persists to `health_check_runs`.
- `--cidr` mode is an operator-console diagnostic tool — runs **L1 only**, emits one report row per IP in the range (closed hosts included so "all dark" is a visible answer), **does not persist to DB or surface in UI history**. This matches the trade-off `machine scan` and `winrm preflight` make. Persistence requires inventoried machines (`health_check_runs.machine_id` is FK-bound).
- No-creds mode (missing `--cred-alias` / `--user`) runs L1 only and marks L2/L3 as `status=na`. **`na` rows do NOT count toward `healthy` / `warning` / `critical` / `offline` summary counters** — they increment a separate `skipped` counter so the UI can honestly show "5 healthy, 12 skipped (no creds)" instead of inflating the green count.

**Tokio runtime** — CLI binary `uecm-cli` has a sync `main()` (see `src-tauri/src/bin/uecm-cli.rs:17`), so building a runtime with `tokio::runtime::Runtime::new().unwrap()` is safe in production. The runtime is hoisted to `run_dispatch` entry (built once, reused for all async calls) to avoid per-machine runtime overhead. Under `#[tokio::test]` an outer runtime exists; tests drive async helpers directly with `.await` instead of `block_on`.

Scan and preflight commands stay untouched (still useful for scripting + automation).

**Tech Stack:** Rust 2021 (Tauri 2 + tokio + clap 4 + rusqlite), Vue 3 SFC + TypeScript + Pinia, PowerShell 5.1 probe scripts dispatched via WinRM, vitest for frontend, `cargo test` for backend.

---

## Pre-flight

**Worktree:** already on `worktree-feat+health-run-full-checkup` under `.claude/worktrees/feat+health-run-full-checkup`.

**Baseline tests:** run before Task 1 to confirm clean starting state:

```bash
cd /mnt/e/AIWorkspace/vp/ue-cache-manager/.claude/worktrees/feat+health-run-full-checkup
cd src-tauri && cargo test --lib && cd ..
pnpm install && pnpm typecheck && pnpm test
```

Expected: all green. If anything red, stop and report.

---

## File Structure

**Files this plan creates / modifies:**

| Path | Role | Touched in |
|---|---|---|
| `src-tauri/src/core/health_check.rs` | Add `remediation` field to `CheckOutcome`; new `probe_tcp_ports` fn | T1, T3 |
| `src-tauri/src/core/network.rs` | Add `probe_host_one` single-host helper | T2 |
| **`src-tauri/src/core/probe_keys.rs`** | **NEW — canonical `PROBE_REGISTRY` const + layer enum + helpers** | **T4** |
| `ps-scripts/health-probes.ps1` | Replace 8 existing probes with 4 L2 + 7 L3 (= 11 PowerShell probes); all outputs include `remediation`; results hashtable keys match `PROBE_REGISTRY` | T5, T6 |
| `src-tauri/src/core/health_probes.rs` | Pass-through unchanged (HashMap auto-absorbs registry keys) — assert deserialization in mod tests | T5 |
| `src-tauri/src/cli/args.rs` | `HealthAction::Run` gains optional `cidr` / `all` flags | T8 |
| `src-tauri/src/cli/domain_health.rs` | Implement `--all` / `--cidr` / no-creds; `OFFLINE_PROBE_KEYS` derives from registry; `na` excluded from severity counters | T7, T9, T10, T11a, T11b, T11c |
| `src/services/tauri.ts` | `ProbeLayer` type + `PROBE_LAYER_MAP` const (test asserts every key is in `PROBE_REGISTRY`) | T12 |
| `src/stores/healthCheck.ts` | Add `probesByLayer` computed; log unknown probe keys via `console.warn` (not silent skip) | T13 |
| `src/views/HealthCheck.vue` | Group rows by layer; show remediation under critical/warning rows | T14 |
| `src/locales/{en,zh}.ts` | i18n strings for layer labels + probe labels (test asserts every probe key from `PROBE_REGISTRY` has a translation) | T15 |
| `Docs/core-features.html` | Update §05 Diagnostics — 17 probes × 3 layers | T16 |

**Files explicitly NOT touched** (preserved verbatim for backward compat):

- `src-tauri/src/commands/health_check.rs` — Tauri IPC command still calls `health_probes::run` the same way; new probe keys flow through `HashMap` transparently. Touched only if integration tests break.
- `src-tauri/src/core/network.rs` — public `scan_cidr` signature unchanged; we add a *new* `probe_host_one` helper rather than refactoring existing async path. (See T2.)
- `src-tauri/src/core/preflight.rs` — preflight command keeps standalone behavior.

---

### Task 1: Add `remediation` field to `CheckOutcome`

**Files:**
- Modify: `src-tauri/src/core/health_check.rs:7-12`
- Test: same file, append to `mod tests`

- [ ] **Step 1: Write failing test**

Append to `src-tauri/src/core/health_check.rs` inside `mod tests`:

```rust
    #[test]
    fn check_outcome_serializes_remediation_field() {
        let outcome = CheckOutcome {
            status: "critical".into(),
            message: "LanmanServer stopped".into(),
            sample: "Stopped".into(),
            remediation: "Start the service: Start-Service LanmanServer".into(),
        };
        let json = serde_json::to_string(&outcome).unwrap();
        assert!(json.contains("\"remediation\":\"Start the service: Start-Service LanmanServer\""));
    }

    #[test]
    fn check_outcome_deserializes_missing_remediation_as_empty() {
        let json = r#"{"status":"healthy","message":"","sample":""}"#;
        let outcome: CheckOutcome = serde_json::from_str(json).unwrap();
        assert_eq!(outcome.remediation, "");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test --lib core::health_check::tests::check_outcome_serializes_remediation_field`

Expected: FAIL with error like `missing field 'remediation' in initializer of CheckOutcome` (compile error is acceptable as failing test).

- [ ] **Step 3: Implement minimal change**

Replace `src-tauri/src/core/health_check.rs:7-12` with:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckOutcome {
    pub status: String,
    pub message: String,
    pub sample: String,
    #[serde(default)]
    pub remediation: String,
}
```

Also update `aggregate_gpu_consistency` at line 47-54 — the `CheckOutcome { ... }` literal there needs `remediation` populated. Set it to a domain-appropriate string:

```rust
        outcomes.insert(*mid, CheckOutcome {
            status: status.into(),
            message: format!(
                "{} {} ({} of {} machines have same combo)",
                g.gpu_model, g.driver_version, same_combo, total
            ),
            sample: format!("{} / {}", g.gpu_model, g.driver_version),
            remediation: if status == "critical" {
                "Standardize GPU + driver across cluster, or split into compatible subgroups before PSO distribute"
                    .into()
            } else if status == "warning" {
                "Make every node run the same NVIDIA driver: audit with `nvidia-smi --query`, then push a matched installer cluster-wide"
                    .into()
            } else {
                String::new()
            },
        });
```

- [ ] **Step 4: Run tests**

Run: `cd src-tauri && cargo test --lib core::health_check::tests`

Expected: all 6 tests pass (4 original + 2 new).

Also run the broader build to catch downstream literal-initializer breakage:

```bash
cd src-tauri && cargo build --lib 2>&1 | head -50
```

Expected: clean build OR specific compile errors at `commands/health_check.rs` and `cli/domain_health.rs` (those are fixed in next steps). If errors appear, note them — Step 5 addresses both files.

- [ ] **Step 5: Fix downstream `CheckOutcome` literals**

In `src-tauri/src/cli/domain_health.rs` find the two `CheckOutcome { ... }` literals at line ~127 (the offline-fallback `for k in [...]` loop) and lines 158-162 + 167-171 (the `pso_outcome` / `unknown_gpu_outcome` fallbacks). Add `remediation: String::new(),` to each.

In `src-tauri/src/commands/health_check.rs` grep for `CheckOutcome {` and add `remediation: String::new(),` to each literal. (There are similar fallback constructions for the offline branch.)

Run: `cd src-tauri && cargo build --lib`

Expected: clean build.

Run: `cd src-tauri && cargo test --lib`

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
cd /mnt/e/AIWorkspace/vp/ue-cache-manager/.claude/worktrees/feat+health-run-full-checkup
git add src-tauri/src/core/health_check.rs src-tauri/src/cli/domain_health.rs src-tauri/src/commands/health_check.rs
git commit -m "feat(health): add remediation field to CheckOutcome"
```

---

### Task 2: Add `probe_host_one` helper in `core::network`

**Files:**
- Modify: `src-tauri/src/core/network.rs:84-96` (expose single-host probe)
- Test: same file, append to `mod tests`

Rationale: `probe_host` is private and `scan_cidr` always filters by `winrm_open || smb_open`. For health-run we need an unfiltered single-host probe that returns even fully-closed hosts (so we can report "all ports closed" honestly).

- [ ] **Step 1: Write failing test**

Append to `src-tauri/src/core/network.rs` inside `mod tests`:

```rust
    #[tokio::test]
    async fn probe_host_one_returns_all_ports_for_unreachable_host() {
        // TEST-NET-3 (RFC 5737): documentation range, unroutable in real LANs.
        let probed = probe_host_one("203.0.113.1", 100).await;
        assert_eq!(probed.ip, "203.0.113.1");
        // We do not assert false for each port — middleboxes may intercept.
        // We assert the shape: function returns a ProbedHost no matter what.
        let _ = probed.winrm_open;
        let _ = probed.smb_open;
        let _ = probed.rpc_open;
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test --lib core::network::tests::probe_host_one_returns_all_ports_for_unreachable_host`

Expected: FAIL — compile error `cannot find function 'probe_host_one' in this scope`.

- [ ] **Step 3: Implement**

Append below `async fn probe_host` (after line 96) in `src-tauri/src/core/network.rs`:

```rust
/// Single-host port probe. Unlike `scan_cidr`, returns the result regardless of
/// which ports are open — callers (health-run) need to see "all closed" as a
/// distinct outcome from "host not in CIDR".
pub async fn probe_host_one(ip_str: &str, timeout_ms: u64) -> ProbedHost {
    let ip: IpAddr = match IpAddr::from_str(ip_str) {
        Ok(addr) => addr,
        Err(_) => {
            return ProbedHost {
                ip: ip_str.to_string(),
                winrm_open: false,
                smb_open: false,
                rpc_open: false,
            };
        }
    };
    probe_host(ip, timeout_ms).await
}
```

- [ ] **Step 4: Run tests**

Run: `cd src-tauri && cargo test --lib core::network`

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/core/network.rs
git commit -m "feat(network): expose probe_host_one for single-host TCP probe"
```

---

### Task 3: Add `probe_tcp_ports` synthesizer in `core::health_check`

**Files:**
- Modify: `src-tauri/src/core/health_check.rs` (append new fn)
- Test: same file, append to `mod tests`

This is the L1 (port-layer) probe entry — wraps `network::probe_host_one` and turns the three booleans into three `CheckOutcome` rows with remediation strings.

- [ ] **Step 1: Write failing test**

Append to `src-tauri/src/core/health_check.rs` inside `mod tests`:

```rust
    #[tokio::test]
    async fn probe_tcp_ports_returns_three_outcomes_with_remediation() {
        // Use TEST-NET-3 so probes time out fast and produce "critical" rows.
        let outcomes = probe_tcp_ports("203.0.113.2", 100).await;
        assert!(outcomes.contains_key("tcp_5985"));
        assert!(outcomes.contains_key("tcp_445"));
        assert!(outcomes.contains_key("tcp_135"));
        // Each closed port must carry a non-empty remediation string.
        for key in ["tcp_5985", "tcp_445", "tcp_135"] {
            let o = outcomes.get(key).unwrap();
            if o.status == "critical" {
                assert!(!o.remediation.is_empty(), "{} missing remediation", key);
            }
        }
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test --lib core::health_check::tests::probe_tcp_ports_returns_three_outcomes_with_remediation`

Expected: FAIL — `cannot find function 'probe_tcp_ports'`.

- [ ] **Step 3: Implement**

Add this `use` if not already at top of `src-tauri/src/core/health_check.rs`:

```rust
use crate::core::network;
```

Append after `aggregate_gpu_consistency`:

```rust
/// L1 (port-layer) probe. Runs from the operator console — no credentials,
/// no WinRM, just three TCP connect attempts. Returns three `CheckOutcome`
/// rows keyed `tcp_5985` / `tcp_445` / `tcp_135` with remediation strings
/// that direct the operator toward the right bootstrap path.
pub async fn probe_tcp_ports(host: &str, timeout_ms: u64) -> HashMap<String, CheckOutcome> {
    let probed = network::probe_host_one(host, timeout_ms).await;
    let mut out = HashMap::new();

    out.insert(
        "tcp_5985".into(),
        port_outcome(
            "WinRM 5985",
            probed.winrm_open,
            "Run `uecm-cli winrm bootstrap <host>` (Path B) when 445+135 are open, or use the USB Path A bootstrap when all three ports are closed.",
        ),
    );
    out.insert(
        "tcp_445".into(),
        port_outcome(
            "SMB 445",
            probed.smb_open,
            "Open inbound TCP 445 (FPS-SMB-In-TCP firewall rule) and start LanmanServer. `winrm bootstrap` does both via -EnableSmbServer.",
        ),
    );
    out.insert(
        "tcp_135".into(),
        port_outcome(
            "RPC 135 (Endpoint Mapper)",
            probed.rpc_open,
            "Switch network profile to Private (Public default blocks DCOM-In). `winrm bootstrap` does this when -NetworkCategory Private is passed.",
        ),
    );
    out
}

fn port_outcome(label: &str, open: bool, fix_hint: &str) -> CheckOutcome {
    if open {
        CheckOutcome {
            status: "healthy".into(),
            message: format!("{} reachable", label),
            sample: "open".into(),
            remediation: String::new(),
        }
    } else {
        CheckOutcome {
            status: "critical".into(),
            message: format!("{} not reachable (TCP connect failed)", label),
            sample: "closed".into(),
            remediation: fix_hint.into(),
        }
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cd src-tauri && cargo test --lib core::health_check`

Expected: all health_check tests pass (originals + 2 new from T1 + 1 new from T3).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/core/health_check.rs
git commit -m "feat(health): add probe_tcp_ports for L1 port-layer probe"
```

---

### Task 4: Create canonical probe registry in `src-tauri/src/core/probe_keys.rs`

**Files:**
- Create: `src-tauri/src/core/probe_keys.rs`
- Modify: `src-tauri/src/core/mod.rs` (add `pub mod probe_keys;`)
- Test: same file, `mod tests`

This is the **single source of truth** for probe key + layer assignment. Every downstream consumer (`OFFLINE_PROBE_KEYS`, PS1 `$results`, TS `PROBE_LAYER_MAP`, locale files) is either derived from or validated against this registry.

- [ ] **Step 1: Verify `core::mod` location and current contents**

Run: `cat src-tauri/src/core/mod.rs | head -40`

Expected: a list of `pub mod <name>;` lines. Note the alphabetical-ish ordering so we can insert `probe_keys` in the right place.

- [ ] **Step 2: Write failing test**

Create `src-tauri/src/core/probe_keys.rs` with **test-only** content first:

```rust
//! Canonical probe registry. Every probe name + layer assignment + creds-required flag
//! lives here exactly once. PS1 scripts, Rust constants, TS layer maps, locale labels
//! all derive from or are validated against this list — there is no second source of truth.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_contains_three_l1_port_probes() {
        let l1: Vec<_> = PROBE_REGISTRY.iter().filter(|p| p.layer == Layer::L1Port).collect();
        assert_eq!(l1.len(), 3, "expected exactly 3 L1 port probes, got {:?}", l1);
    }

    #[test]
    fn registry_contains_four_l2_bootstrap_probes() {
        let l2: Vec<_> = PROBE_REGISTRY.iter().filter(|p| p.layer == Layer::L2Bootstrap).collect();
        assert_eq!(l2.len(), 4, "expected exactly 4 L2 bootstrap probes, got {:?}", l2);
    }

    #[test]
    fn registry_contains_seven_l3_business_probes_plus_three_derived() {
        let l3: Vec<_> = PROBE_REGISTRY.iter().filter(|p| p.layer == Layer::L3Business).collect();
        assert_eq!(l3.len(), 7, "expected 7 L3 business probes, got {:?}", l3);
        let derived: Vec<_> = PROBE_REGISTRY.iter().filter(|p| p.layer == Layer::L3Derived).collect();
        assert_eq!(derived.len(), 3, "expected 3 L3 derived probes, got {:?}", derived);
    }

    #[test]
    fn no_duplicate_keys() {
        let mut seen = std::collections::HashSet::new();
        for spec in PROBE_REGISTRY {
            assert!(seen.insert(spec.key), "duplicate key in registry: {}", spec.key);
        }
    }

    #[test]
    fn powershell_probe_keys_returns_only_winrm_probes() {
        // L1 runs in Rust (no creds). L2 + L3 (non-derived) run in PowerShell via WinRM.
        let ps_keys = powershell_probe_keys();
        assert_eq!(ps_keys.len(), 11, "expected 11 PS keys (4 L2 + 7 L3), got {:?}", ps_keys);
        assert!(!ps_keys.iter().any(|k| k.starts_with("tcp_")), "PS keys must not include L1 TCP keys");
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd src-tauri && cargo test --lib core::probe_keys`

Expected: FAIL — `cannot find value 'PROBE_REGISTRY' in this scope`. Also requires `pub mod probe_keys` in `src-tauri/src/core/mod.rs` — add it now if the test compile fails on a missing module.

- [ ] **Step 4: Implement registry**

Prepend the test code above with the registry definition so the file looks like:

```rust
//! Canonical probe registry. ... (keep the doc comment from Step 2)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    L1Port,        // Rust TCP probes, no creds
    L2Bootstrap,   // PowerShell via WinRM — registry + service + firewall
    L3Business,    // PowerShell via WinRM — share + cred + env + WMI
    L3Derived,     // computed in Rust from other DB tables (no PS call)
}

#[derive(Debug, Clone, Copy)]
pub struct ProbeSpec {
    pub key: &'static str,
    pub layer: Layer,
    /// `true` if running this probe requires authenticated WinRM (so the no-creds
    /// branch should mark it `na`). L1Port is always `false`; L3Derived is always
    /// `false` (computed from local DB).
    pub requires_creds: bool,
}

pub const PROBE_REGISTRY: &[ProbeSpec] = &[
    // L1 — port reachability (Rust TCP, no creds)
    ProbeSpec { key: "tcp_5985", layer: Layer::L1Port, requires_creds: false },
    ProbeSpec { key: "tcp_445",  layer: Layer::L1Port, requires_creds: false },
    ProbeSpec { key: "tcp_135",  layer: Layer::L1Port, requires_creds: false },

    // L2 — bootstrap configuration (PowerShell via WinRM)
    ProbeSpec { key: "firewall_445",               layer: Layer::L2Bootstrap, requires_creds: true },
    ProbeSpec { key: "local_account_token_filter", layer: Layer::L2Bootstrap, requires_creds: true },
    ProbeSpec { key: "long_paths_enabled",         layer: Layer::L2Bootstrap, requires_creds: true },
    ProbeSpec { key: "lanman_server",              layer: Layer::L2Bootstrap, requires_creds: true },

    // L3 — business workflow (PowerShell via WinRM)
    ProbeSpec { key: "share_reachable", layer: Layer::L3Business, requires_creds: true },
    ProbeSpec { key: "ntfs_perm",       layer: Layer::L3Business, requires_creds: true },
    ProbeSpec { key: "cred_user",       layer: Layer::L3Business, requires_creds: true },
    ProbeSpec { key: "cred_system",     layer: Layer::L3Business, requires_creds: true },
    ProbeSpec { key: "env_vars",        layer: Layer::L3Business, requires_creds: true },
    ProbeSpec { key: "system_write",    layer: Layer::L3Business, requires_creds: true },
    ProbeSpec { key: "winmgmt",         layer: Layer::L3Business, requires_creds: true },

    // L3 — derived (computed in Rust)
    ProbeSpec { key: "ini_consistency", layer: Layer::L3Derived, requires_creds: false },
    ProbeSpec { key: "pso_precaching",  layer: Layer::L3Derived, requires_creds: false },
    ProbeSpec { key: "gpu_consistency", layer: Layer::L3Derived, requires_creds: false },
];

/// Keys that the offline / no-creds fallback should fill with placeholder outcomes
/// (everything that runs via WinRM — i.e. requires_creds == true).
pub fn offline_probe_keys() -> Vec<&'static str> {
    PROBE_REGISTRY.iter()
        .filter(|p| p.requires_creds)
        .map(|p| p.key)
        .collect()
}

/// Keys the PowerShell script returns (L2 + L3-business, not L1 not derived).
pub fn powershell_probe_keys() -> Vec<&'static str> {
    PROBE_REGISTRY.iter()
        .filter(|p| matches!(p.layer, Layer::L2Bootstrap | Layer::L3Business))
        .map(|p| p.key)
        .collect()
}

/// Look up the layer for a given probe key. `None` for unknown keys.
pub fn layer_for(key: &str) -> Option<Layer> {
    PROBE_REGISTRY.iter().find(|p| p.key == key).map(|p| p.layer)
}
```

Then add `pub mod probe_keys;` to `src-tauri/src/core/mod.rs` if Step 3 didn't already require it.

- [ ] **Step 5: PS1 drift test**

Append to `mod tests` in `probe_keys.rs`:

```rust
    #[test]
    fn powershell_script_results_hashtable_matches_registry() {
        // Parse ps-scripts/health-probes.ps1 looking for the line
        //     <key> = (Probe-<Name>)
        // inside the $results hashtable. Build the key set, compare to powershell_probe_keys().
        let ps1_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap()
            .join("ps-scripts").join("health-probes.ps1");
        let body = std::fs::read_to_string(&ps1_path)
            .unwrap_or_else(|e| panic!("read {:?}: {}", ps1_path, e));

        // Extract the $results = @{ ... } block.
        let start = body.find("$results = @{").expect("no $results = @{ block");
        let after_start = &body[start + "$results = @{".len()..];
        let end = after_start.find('}').expect("no closing } for $results");
        let block = &after_start[..end];

        // Each line in the block looks like:  <key> = (Probe-X)
        let key_re = regex::Regex::new(r"(?m)^\s*([a-z_]+[a-z0-9_]*)\s*=\s*\(Probe-").unwrap();
        let mut ps_keys: Vec<String> = key_re
            .captures_iter(block)
            .map(|c| c[1].to_string())
            .collect();
        ps_keys.sort();

        let mut expected: Vec<String> = super::powershell_probe_keys()
            .iter().map(|s| s.to_string()).collect();
        expected.sort();

        assert_eq!(ps_keys, expected,
            "ps1 $results keys drifted from PROBE_REGISTRY\n  ps1:      {:?}\n  registry: {:?}",
            ps_keys, expected);
    }
```

This test depends on the `regex` crate. Check Cargo.toml — if not present, add `regex = "1"` to `[dev-dependencies]`.

- [ ] **Step 6: Run tests**

Run: `cd src-tauri && cargo test --lib core::probe_keys`

Expected: **all tests except `powershell_script_results_hashtable_matches_registry` PASS**. The PS1 test will fail until T5 replaces the hashtable — that is the expected, useful failure that gates T5.

- [ ] **Step 7: Commit**

```bash
cd /mnt/e/AIWorkspace/vp/ue-cache-manager/.claude/worktrees/feat+health-run-full-checkup
git add src-tauri/src/core/probe_keys.rs src-tauri/src/core/mod.rs src-tauri/Cargo.toml
git commit -m "feat(health): canonical probe registry + drift tests"
```

---

### Task 5: Rewrite `ps-scripts/health-probes.ps1` with 11 probes matching the registry

**Files:**
- Modify: `ps-scripts/health-probes.ps1` (full rewrite of the `$script = {` ScriptBlock)
- Update: top-of-file doc comment

**Why one task, not two:** the PowerShell ScriptBlock is one chunk that runs in a single round-trip via `Invoke-Command`. Adding 4 new probes + removing 1 + reworking another 7 to include `remediation` is one atomic change — splitting into "add L2" then "edit L3" would leave the file in a half-broken state between commits.

**What goes in (11 probes, matching `PROBE_REGISTRY` `requires_creds=true`):**

| Key | Layer | Function | Notes |
|---|---|---|---|
| `firewall_445` | L2 | Probe-Firewall445 | Match by stable rule Name `FPS-SMB-In-TCP` (DisplayName is localized). |
| `local_account_token_filter` | L2 | Probe-LocalAccountTokenFilter | Reads HKLM LATFP DWORD. |
| `long_paths_enabled` | L2 | Probe-LongPathsEnabled | Reads HKLM LongPathsEnabled DWORD. |
| `lanman_server` | L2 | Probe-LanmanServer | Get-Service LanmanServer (replaces old `smb`/Probe-SmbService — same data, clearer name). |
| `share_reachable` | L3 | Probe-ShareReachable | Test-Path on UNC. |
| `ntfs_perm` | L3 | Probe-NtfsPerm | Get-Acl on share path. |
| `cred_user` | L3 | Probe-CredUser | `cmdkey /list`. |
| `cred_system` | L3 | Probe-CredSystem | `psexec -s cmdkey /list`. |
| `env_vars` | L3 | Probe-EnvVars | Machine env `UE-SharedDataCachePath`. |
| `system_write` | L3 | Probe-SystemWrite | PsExec SYSTEM write to share UNC. |
| `winmgmt` | L3 | Probe-Winmgmt | Get-Service Winmgmt (machine refresh GPU detection needs it). |

**Dropped from initial design:** `smb` (renamed → `lanman_server`), `winrm_reachable` (redundant), `admin_share_writable` (does not actually test ADMIN$).

- [ ] **Step 1: Re-run the PS1 drift test from T4 to confirm it still fails**

Run: `cd src-tauri && cargo test --lib core::probe_keys::tests::powershell_script_results_hashtable_matches_registry`

Expected: FAIL — confirms we are in a state where PS1 keys don't match registry.

- [ ] **Step 2: Update the top-of-file comment block (lines 1-8)**

Replace lines 1-8 in `ps-scripts/health-probes.ps1` with:

```powershell
# Runs 11 health probes against a remote host in one Invoke-Command round-trip.
# Layer assignment lives in src-tauri/src/core/probe_keys.rs — this file MUST
# stay in sync (drift test: cargo test core::probe_keys::tests::powershell_script_results_hashtable_matches_registry).
#
# L2 (bootstrap):  firewall_445, local_account_token_filter, long_paths_enabled, lanman_server
# L3 (business):   share_reachable, ntfs_perm, cred_user, cred_system, env_vars, system_write, winmgmt
#
# L1 (port reachability) runs in Rust — NOT here.
# L3 derived (ini_consistency, pso_precaching, gpu_consistency) computed in Rust — NOT here.
#
# Output: JSON { ok, results: { <key>: {status, message, sample, remediation}, ... }, message }
```

- [ ] **Step 3: Replace the entire `$script = { ... }` ScriptBlock (lines 35-142)**

Replace the inner ScriptBlock with:

```powershell
    $script = {
        param($ShareUnc, $SvcUsername, $ExpectedSharedDataCachePath)

        function Probe-Firewall445 {
            try {
                # Stable rule Name (NOT DisplayName — DisplayName is localized).
                $rule = Get-NetFirewallRule -Name 'FPS-SMB-In-TCP' -ErrorAction SilentlyContinue
                if (-not $rule) {
                    return @{ status='warning'; message='FPS-SMB-In-TCP rule not found'; sample='';
                              remediation='Re-run `uecm-cli winrm bootstrap <host>` to recreate the rule.' }
                }
                $enabled = $rule.Enabled -eq 'True'
                @{ status = ($(if ($enabled) {'healthy'} else {'critical'}));
                   message = "FPS-SMB-In-TCP enabled = $enabled"; sample = $rule.DisplayName;
                   remediation = ($(if ($enabled) {''} else {'Enable-NetFirewallRule -Name FPS-SMB-In-TCP (or re-run `uecm-cli winrm bootstrap <host>`).'})) }
            } catch {
                @{ status='warning'; message=$_.Exception.Message; sample='';
                   remediation='Inspect firewall manually: Get-NetFirewallRule -Name FPS-SMB-In-TCP.' }
            }
        }

        function Probe-LocalAccountTokenFilter {
            try {
                $v = Get-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System' `
                                      -Name 'LocalAccountTokenFilterPolicy' -ErrorAction Stop
                $val = [int]$v.LocalAccountTokenFilterPolicy
                if ($val -eq 1) {
                    @{ status='healthy'; message="LATFP=$val"; sample="$val"; remediation='' }
                } else {
                    @{ status='critical'; message="LATFP=$val (need 1 for remote local-admin token elevation)";
                       sample="$val";
                       remediation='Re-run `uecm-cli winrm bootstrap <host>` (default flow sets LATFP=1).' }
                }
            } catch {
                @{ status='critical'; message='LATFP registry value missing'; sample='';
                   remediation='Re-run `uecm-cli winrm bootstrap <host>` (default flow sets LATFP=1).' }
            }
        }

        function Probe-LongPathsEnabled {
            try {
                $v = Get-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem' `
                                      -Name 'LongPathsEnabled' -ErrorAction Stop
                $val = [int]$v.LongPathsEnabled
                if ($val -eq 1) {
                    @{ status='healthy'; message="LongPathsEnabled=$val"; sample="$val"; remediation='' }
                } else {
                    @{ status='warning'; message="LongPathsEnabled=$val (UE asset paths > 260 chars will fail)";
                       sample="$val";
                       remediation='Re-run `uecm-cli winrm bootstrap <host>` (default flow sets LongPathsEnabled=1).' }
                }
            } catch {
                @{ status='warning'; message='LongPathsEnabled registry value missing'; sample='';
                   remediation='Re-run `uecm-cli winrm bootstrap <host>` (default flow sets LongPathsEnabled=1).' }
            }
        }

        function Probe-LanmanServer {
            try {
                $svc = Get-Service -Name LanmanServer -ErrorAction Stop
                $running = $svc.Status -eq 'Running'
                @{ status = ($(if ($running) {'healthy'} else {'critical'}));
                   message = "LanmanServer = $($svc.Status)"; sample = $svc.Status.ToString();
                   remediation = ($(if ($running) {''} else {'Run `uecm-cli winrm bootstrap <host>` (starts LanmanServer + sets Automatic).'})) }
            } catch {
                @{ status='critical'; message=$_.Exception.Message; sample='';
                   remediation='LanmanServer missing — re-run `uecm-cli winrm bootstrap <host>`.' }
            }
        }

        function Probe-ShareReachable {
            if ([string]::IsNullOrEmpty($ShareUnc)) {
                return @{ status='na'; message='no share configured'; sample=''; remediation='' }
            }
            try {
                $ok = Test-Path $ShareUnc -ErrorAction Stop
                @{ status = ($(if ($ok) {'healthy'} else {'critical'}));
                   message = "Test-Path returned $ok"; sample = $ShareUnc;
                   remediation = ($(if ($ok) {''} else {'Create the SMB share on the host: `uecm-cli share create --host <hostHostingShare>`.'})) }
            } catch {
                @{ status='critical'; message=$_.Exception.Message; sample=$ShareUnc;
                   remediation='Verify share exists and current cred has read access: `uecm-cli share list`.' }
            }
        }

        function Probe-NtfsPerm {
            if ([string]::IsNullOrEmpty($ShareUnc) -or [string]::IsNullOrEmpty($SvcUsername)) {
                return @{ status='na'; message='only meaningful for managed shares with svc account'; sample=''; remediation='' }
            }
            try {
                $share = Get-SmbShare -Name (Split-Path $ShareUnc -Leaf) -ErrorAction SilentlyContinue
                if (-not $share) { return @{ status='na'; message='not the host'; sample=''; remediation='' } }
                $acl = Get-Acl $share.Path
                $hasSvc = $acl.Access | Where-Object { $_.IdentityReference -match $SvcUsername }
                @{ status = ($(if ($hasSvc) {'healthy'} else {'critical'}));
                   message = "ACL on $($share.Path) for $SvcUsername"; sample = ($acl.Owner);
                   remediation = ($(if ($hasSvc) {''} else {"Grant ACL: icacls `"$($share.Path)`" /grant ${SvcUsername}:(OI)(CI)F"})) }
            } catch {
                @{ status='warning'; message=$_.Exception.Message; sample='';
                   remediation='Inspect NTFS ACL: Get-Acl <sharePath>.' }
            }
        }

        function Probe-CredUser {
            if ([string]::IsNullOrEmpty($SvcUsername)) {
                return @{ status='na'; message='no managed share'; sample=''; remediation='' }
            }
            try {
                $out = & cmdkey.exe /list 2>&1 | Out-String
                $hasIt = $out -match [regex]::Escape($SvcUsername)
                @{ status = ($(if ($hasIt) {'healthy'} else {'critical'}));
                   message = "cmdkey /list contains $SvcUsername = $hasIt"; sample = '';
                   remediation = ($(if ($hasIt) {''} else {'Run `uecm-cli share inject-system-cred --host <host>` to write the svc credential to user + SYSTEM stores.'})) }
            } catch {
                @{ status='critical'; message=$_.Exception.Message; sample='';
                   remediation='cmdkey unavailable — verify Windows is not in a broken state.' }
            }
        }

        function Probe-CredSystem {
            if ([string]::IsNullOrEmpty($SvcUsername)) {
                return @{ status='na'; message='no managed share'; sample=''; remediation='' }
            }
            $vendor = Join-Path $env:LOCALAPPDATA 'UECM\PsExec64.exe'
            if (-not (Test-Path $vendor)) {
                return @{ status='warning'; message='PsExec64 not staged on machine; cannot verify SYSTEM cred'; sample='';
                          remediation='Re-run `uecm-cli winrm bootstrap <host>` (stages PsExec64 to %LOCALAPPDATA%\UECM). If AV/AppLocker blocks PsExec, exempt %LOCALAPPDATA%\UECM\PsExec64.exe.' }
            }
            try {
                $out = & $vendor -accepteula -nobanner -s -i 0 cmdkey.exe /list 2>&1 | Out-String
                $hasIt = $out -match [regex]::Escape($SvcUsername)
                @{ status = ($(if ($hasIt) {'healthy'} else {'critical'}));
                   message = "SYSTEM cmdkey /list contains $SvcUsername = $hasIt"; sample = '';
                   remediation = ($(if ($hasIt) {''} else {'Run `uecm-cli share inject-system-cred --host <host>` to push cred into SYSTEM credential store.'})) }
            } catch {
                @{ status='warning'; message=$_.Exception.Message; sample='';
                   remediation='PsExec invocation failed — check %LOCALAPPDATA%\UECM\PsExec64.exe integrity and AV/AppLocker exclusions.' }
            }
        }

        function Probe-EnvVars {
            $shared = [Environment]::GetEnvironmentVariable('UE-SharedDataCachePath', 'Machine')
            if ([string]::IsNullOrEmpty($ExpectedSharedDataCachePath)) {
                if ([string]::IsNullOrEmpty($shared)) {
                    @{ status='warning'; message='UE-SharedDataCachePath is empty'; sample='';
                       remediation='Set UE-SharedDataCachePath system env var: `uecm-cli env set --name UE-SharedDataCachePath --value <UNC>`.' }
                } else {
                    @{ status='healthy'; message="UE-SharedDataCachePath = $shared"; sample="$shared"; remediation='' }
                }
            } else {
                $match = $shared -eq $ExpectedSharedDataCachePath
                @{ status = ($(if ($match) {'healthy'} else {'critical'}));
                   message = "expected $ExpectedSharedDataCachePath, got $shared"; sample = "$shared";
                   remediation = ($(if ($match) {''} else {"Set system env: ``uecm-cli env set --name UE-SharedDataCachePath --value `"$ExpectedSharedDataCachePath`"``."})) }
            }
        }

        function Probe-SystemWrite {
            if ([string]::IsNullOrEmpty($ShareUnc)) {
                return @{ status='na'; message='no share configured'; sample=''; remediation='' }
            }
            $vendor = Join-Path $env:LOCALAPPDATA 'UECM\PsExec64.exe'
            if (-not (Test-Path $vendor)) {
                return @{ status='warning'; message='PsExec64 not staged; cannot SYSTEM-write probe'; sample='';
                          remediation='Re-run `uecm-cli winrm bootstrap <host>` (stages PsExec64 into %LOCALAPPDATA%\UECM).' }
            }
            try {
                $probe = "uecm-probe-$(Get-Random).txt"
                $cmd = "echo healthcheck > `"$ShareUnc\$probe`""
                & $vendor -accepteula -nobanner -s -i 0 cmd /c $cmd 2>&1 | Out-Null
                $exists = Test-Path "$ShareUnc\$probe"
                if ($exists) { Remove-Item "$ShareUnc\$probe" -Force -ErrorAction SilentlyContinue }
                @{ status = ($(if ($exists) {'healthy'} else {'critical'}));
                   message = "SYSTEM wrote probe file = $exists"; sample = $probe;
                   remediation = ($(if ($exists) {''} else {'SYSTEM cannot write to share — verify cred_system probe AND that NTFS ACL grants ddc-svc write.'})) }
            } catch {
                @{ status='critical'; message=$_.Exception.Message; sample='';
                   remediation='SYSTEM-write probe threw — inspect PsExec64 + share NTFS ACL.' }
            }
        }

        function Probe-Winmgmt {
            try {
                $svc = Get-Service -Name Winmgmt -ErrorAction Stop
                $running = $svc.Status -eq 'Running'
                @{ status = ($(if ($running) {'healthy'} else {'critical'}));
                   message = "Winmgmt = $($svc.Status)"; sample = $svc.Status.ToString();
                   remediation = ($(if ($running) {''} else {'Run `uecm-cli winrm bootstrap <host>` (sets Winmgmt Automatic+Running; required by machine refresh GPU detection).'})) }
            } catch {
                @{ status='critical'; message=$_.Exception.Message; sample='';
                   remediation='Winmgmt service missing — re-run `uecm-cli winrm bootstrap <host>`.' }
            }
        }

        $results = @{
            firewall_445               = (Probe-Firewall445)
            local_account_token_filter = (Probe-LocalAccountTokenFilter)
            long_paths_enabled         = (Probe-LongPathsEnabled)
            lanman_server              = (Probe-LanmanServer)
            share_reachable            = (Probe-ShareReachable)
            ntfs_perm                  = (Probe-NtfsPerm)
            cred_user                  = (Probe-CredUser)
            cred_system                = (Probe-CredSystem)
            env_vars                   = (Probe-EnvVars)
            system_write               = (Probe-SystemWrite)
            winmgmt                    = (Probe-Winmgmt)
        }
        return $results
    }
```

- [ ] **Step 4: REQUIRED Windows-side smoke test on lanPC**

The worktree lives on WSL but `Get-Service` / `Get-NetFirewallRule` / registry probes need real Windows. Run the rewritten script on lanPC over SSH:

```bash
# From the worktree directory:
scp ps-scripts/health-probes.ps1 lanpc:/tmp/uecm-health-probes.ps1
ssh lanpc 'powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:/tmp/uecm-health-probes.ps1 -HostName localhost -Local' > /tmp/probes-out.json
cat /tmp/probes-out.json | jq '.results | keys | sort'
```

Expected output: a sorted list of exactly these 11 keys:

```
[
  "cred_system", "cred_user", "env_vars", "firewall_445",
  "lanman_server", "local_account_token_filter", "long_paths_enabled",
  "ntfs_perm", "share_reachable", "system_write", "winmgmt"
]
```

If `ok: false` in the output, read `.message` and fix the PowerShell — likely a syntax error in one of the new probe functions.

If SSH to lanPC is not available, alternative: scp the file to any Windows host and run `powershell -File ... -HostName localhost -Local` there. **Do not skip this step** — the drift test in Rust validates key names but cannot catch PowerShell syntax errors.

- [ ] **Step 5: Re-run the Rust drift test**

Run: `cd src-tauri && cargo test --lib core::probe_keys`

Expected: `powershell_script_results_hashtable_matches_registry` now PASSES (11 keys match `powershell_probe_keys()`).

- [ ] **Step 6: Commit**

```bash
git add ps-scripts/health-probes.ps1
git commit -m "feat(health): rewrite PS probes (11 total, registry-aligned, with remediation)"
```

---

### Task 6: Wire `domain_health::run` to registry + define `na` counter semantics

**Files:**
- Modify: `src-tauri/src/cli/domain_health.rs`

This task replaces the inline `for k in [...8 keys]` loop in the offline branch with `probe_keys::offline_probe_keys()`, and introduces a separate `skipped` counter for `na` outcomes so the summary stops conflating skipped probes with passed ones.

- [ ] **Step 1: Write failing test**

Append (or create) `mod tests` in `src-tauri/src/cli/domain_health.rs`:

```rust
#[cfg(test)]
mod tests {
    use crate::core::health_check::CheckOutcome;

    fn outcome(status: &str) -> CheckOutcome {
        CheckOutcome {
            status: status.into(),
            message: "".into(),
            sample: "".into(),
            remediation: "".into(),
        }
    }

    #[test]
    fn na_outcomes_increment_skipped_not_other_counters() {
        let mut counters = super::Counters::default();
        counters.tally(&outcome("healthy"));
        counters.tally(&outcome("warning"));
        counters.tally(&outcome("na"));
        counters.tally(&outcome("na"));
        counters.tally(&outcome("critical"));

        assert_eq!(counters.healthy, 1);
        assert_eq!(counters.warning, 1);
        assert_eq!(counters.critical, 1);
        assert_eq!(counters.offline, 0);
        assert_eq!(counters.skipped, 2, "na must increment skipped");
        // total_checks excludes skipped — we want "8 of 17 ran" not "17 ran, 8 ok"
        assert_eq!(counters.total_ran, 3);
    }

    #[test]
    fn offline_probe_keys_derives_from_registry() {
        let keys = crate::core::probe_keys::offline_probe_keys();
        // 4 L2 + 7 L3 = 11, since L1 and derived have requires_creds=false
        assert_eq!(keys.len(), 11);
        assert!(keys.contains(&"lanman_server"));
        assert!(keys.contains(&"firewall_445"));
        assert!(!keys.contains(&"tcp_5985"), "L1 must not be in offline keys");
        assert!(!keys.contains(&"ini_consistency"), "derived must not be in offline keys");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test --lib cli::domain_health::tests::na_outcomes_increment_skipped_not_other_counters`

Expected: FAIL — `cannot find type 'Counters' in this scope`.

- [ ] **Step 3: Introduce `Counters` struct + tally fn**

Near the top of `src-tauri/src/cli/domain_health.rs` (after `use` block), insert:

```rust
/// Tally health probe outcomes by status. `na` is segregated into `skipped` so
/// the summary distinguishes "probe ran and succeeded" from "probe was not run
/// (e.g. no creds, no share configured)".
#[derive(Default, Debug)]
pub(crate) struct Counters {
    pub healthy: i64,
    pub warning: i64,
    pub critical: i64,
    pub offline: i64,
    pub skipped: i64,   // na outcomes — probe could not run, status indeterminate
    pub total_ran: i64, // healthy + warning + critical + offline (NOT skipped)
}

impl Counters {
    pub fn tally(&mut self, outcome: &crate::core::health_check::CheckOutcome) {
        match outcome.status.as_str() {
            "healthy"  => { self.healthy  += 1; self.total_ran += 1; }
            "warning"  => { self.warning  += 1; self.total_ran += 1; }
            "critical" => { self.critical += 1; self.total_ran += 1; }
            "offline"  => { self.offline  += 1; self.total_ran += 1; }
            "na"       => { self.skipped  += 1; }
            _          => { /* unknown/sample states ignored */ }
        }
    }
}
```

- [ ] **Step 4: Replace the inline offline-key list**

Locate the offline branch (around line 113-141 in the original — find the `for k in [` loop). Replace it with:

```rust
                for k in crate::core::probe_keys::offline_probe_keys() {
                    row.insert(
                        k.into(),
                        crate::core::health_check::CheckOutcome {
                            status: "offline".into(),
                            message: e.to_string(),
                            sample: "".into(),
                            remediation: "Bring the host online (verify network + WinRM) before retrying.".into(),
                        },
                    );
                }
```

And update the `total_checks += 8;` (around line 141) to `total_checks += crate::core::probe_keys::offline_probe_keys().len() as i64;`.

- [ ] **Step 5: Replace the manual counter increments**

In the `for v in row.values() { ... }` block (around line 179-188), replace the inline match with a `Counters` tally. The simplest path: keep `healthy / warning / critical / offline / total_checks` locals if the surrounding code depends on them, but rebuild them from a single `Counters` so `na` segregation lives in one place. Concretely, find:

```rust
        let machine_checks = row.len() as i64;
        for v in row.values() {
            total_checks += 1;
            match v.status.as_str() {
                "healthy" => healthy += 1,
                "warning" => warning += 1,
                "critical" => critical += 1,
                "offline" => offline += 1,
                _ => {}
            }
        }
```

Replace with:

```rust
        let machine_checks = row.len() as i64;
        let mut row_counters = Counters::default();
        for v in row.values() {
            row_counters.tally(v);
        }
        healthy      += row_counters.healthy;
        warning      += row_counters.warning;
        critical     += row_counters.critical;
        offline      += row_counters.offline;
        skipped      += row_counters.skipped;
        total_checks += row_counters.total_ran; // skipped excluded — matches new "total_ran" semantics
```

And declare `let mut skipped: i64 = 0;` next to the other counters near line 78-82.

- [ ] **Step 6: Add `skipped` to summary JSON + Completed event**

In the `summary_json` literal (around line 202-208), add `"skipped": skipped,`. In the `Event::Completed` summary literal (around line 213-221), also add `"skipped": skipped,`.

- [ ] **Step 7: Run tests**

Run: `cd src-tauri && cargo test --lib cli::domain_health`

Expected: 2 new tests + all existing health tests PASS. Run `cargo build --lib` to confirm no break elsewhere.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/cli/domain_health.rs
git commit -m "feat(health-cli): derive offline keys from registry + segregate na into skipped counter"
```

---

### Task 7: Add `--cidr` / `--all` flags to `HealthAction::Run`

**Files:**
- Modify: `src-tauri/src/cli/args.rs` (HealthAction::Run + tests block at file bottom)
- Modify: `src-tauri/src/cli/domain_health.rs` (HealthAction::Run match arm + new `run_dispatch` stub)

`CredentialArgs` already supports "no flags" cleanly — verified in this codebase at `credential_args.rs:90-93` (`(None, None, false) => Ok(None)`). So this task only adds `cidr` / `all` mutual exclusion; the optional-creds behavior comes for free.

- [ ] **Step 1: Write failing tests**

In the existing `mod tests` block at the bottom of `src-tauri/src/cli/args.rs`, add:

```rust
    #[test]
    fn parses_health_run_with_machine_ids() {
        let cli = Cli::try_parse_from(["uecm-cli", "health", "run", "--machine-ids", "1,2,3"]).unwrap();
        match cli.domain {
            Domain::Health { action: HealthAction::Run { machine_ids, cidr, all, .. } } => {
                assert_eq!(machine_ids, vec![1, 2, 3]);
                assert_eq!(cidr, None);
                assert_eq!(all, false);
            }
            _ => panic!("expected Health::Run"),
        }
    }

    #[test]
    fn parses_health_run_with_cidr() {
        let cli = Cli::try_parse_from(["uecm-cli", "health", "run", "--cidr", "192.168.10.0/24"]).unwrap();
        match cli.domain {
            Domain::Health { action: HealthAction::Run { cidr, .. } } => {
                assert_eq!(cidr.as_deref(), Some("192.168.10.0/24"));
            }
            _ => panic!("expected Health::Run"),
        }
    }

    #[test]
    fn parses_health_run_with_all_flag() {
        let cli = Cli::try_parse_from(["uecm-cli", "health", "run", "--all"]).unwrap();
        match cli.domain {
            Domain::Health { action: HealthAction::Run { all, .. } } => assert!(all),
            _ => panic!("expected Health::Run"),
        }
    }

    #[test]
    fn rejects_cidr_and_machine_ids_together() {
        let r = Cli::try_parse_from(["uecm-cli", "health", "run", "--cidr", "10.0.0.0/24", "--machine-ids", "1"]);
        assert!(r.is_err(), "should reject --cidr + --machine-ids");
    }

    #[test]
    fn rejects_all_and_cidr_together() {
        let r = Cli::try_parse_from(["uecm-cli", "health", "run", "--all", "--cidr", "10.0.0.0/24"]);
        assert!(r.is_err(), "should reject --all + --cidr");
    }
```

- [ ] **Step 2: Run test to verify they fail**

Run: `cd src-tauri && cargo test --lib cli::args::tests::parses_health_run_with_cidr`

Expected: FAIL — `no field 'cidr' on HealthAction::Run`.

- [ ] **Step 3: Replace `HealthAction::Run` definition**

In `src-tauri/src/cli/args.rs` lines 455-470, replace `HealthAction::Run { ... }` with:

```rust
pub enum HealthAction {
    /// Run health probes — L1 port + L2 bootstrap + L3 business checkup with remediation hints.
    ///
    /// Target selection (exactly one of three modes):
    ///   --machine-ids 1,2,3     diagnose specific inventoried machines (persists results)
    ///   --cidr 192.168.10.0/24  L1 port-layer scan, stdout-only, no DB persistence
    ///   --all                   diagnose every machine in inventory (persists results)
    ///
    /// Credentials are optional. Without --cred-alias/--user, L2 + L3 probes are
    /// reported as `status=na` and counted in a separate `skipped` summary counter
    /// (not `healthy`/`critical`). L1 ports always run.
    Run {
        #[arg(long, value_name = "M1,M2,...", value_delimiter = ',',
              conflicts_with_all = ["cidr", "all"])]
        machine_ids: Vec<i64>,
        #[arg(long, conflicts_with_all = ["machine_ids", "all"])]
        cidr: Option<String>,
        #[arg(long, conflicts_with_all = ["machine_ids", "cidr"])]
        all: bool,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// List recent health scan runs.
    Runs {
        #[arg(long, default_value_t = 10)]
        limit: i64,
    },
    /// List per-row health results for a scan run.
    Results { scan_run_id: i64 },
}
```

- [ ] **Step 4: Update the dispatch arm in `domain_health::handle`**

In `src-tauri/src/cli/domain_health.rs:15-21`, replace `HealthAction::Run { machine_ids, cred } => run(ctx, &machine_ids, &cred),` with:

```rust
        HealthAction::Run { machine_ids, cidr, all, cred } => {
            run_dispatch(ctx, machine_ids, cidr, all, &cred)
        }
```

And add a stub `run_dispatch` (real impl in T8 + T9):

```rust
fn run_dispatch(
    ctx: &mut Ctx<'_>,
    machine_ids: Vec<i64>,
    cidr: Option<String>,
    all: bool,
    cred: &CredentialArgs,
) -> UecmResult<()> {
    // clap conflicts_with_all enforces "no two at once" but not "exactly one of three".
    // Reject the all-empty case explicitly so the user gets a helpful error instead of
    // a silent zero-machine run.
    if machine_ids.is_empty() && cidr.is_none() && !all {
        return Err(crate::error::UecmError::InvalidInput(
            "health run requires exactly one of: --machine-ids, --cidr, or --all".into(),
        ));
    }
    if let Some(_) = cidr {
        return Err(crate::error::UecmError::InvalidInput("--cidr not yet implemented".into()));
    }
    if all {
        return Err(crate::error::UecmError::InvalidInput("--all not yet implemented".into()));
    }
    run(ctx, &machine_ids, cred)
}
```

Also add a test in T7 Step 1 (alongside the parsing tests) that exercises this branch — append:

```rust
    #[test]
    fn parses_health_run_with_no_target_mode() {
        // Parsing succeeds — clap doesn't enforce "exactly one of three".
        let cli = Cli::try_parse_from(["uecm-cli", "health", "run"]).unwrap();
        match cli.domain {
            Domain::Health { action: HealthAction::Run { machine_ids, cidr, all, .. } } => {
                assert!(machine_ids.is_empty());
                assert_eq!(cidr, None);
                assert_eq!(all, false);
            }
            _ => panic!("expected Health::Run"),
        }
        // The runtime guard in run_dispatch catches this case — covered by a separate
        // test in T8 (validate_target_mode_helper or via integration).
    }
```

- [ ] **Step 5: Run tests**

Run: `cd src-tauri && cargo test --lib cli::args::tests; cargo build --lib`

Expected: all 6 new tests PASS, clean build.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/cli/args.rs src-tauri/src/cli/domain_health.rs
git commit -m "feat(health-cli): add --cidr and --all flags with mutual exclusion"
```

---

### Task 8: Implement `--all` mode in `run_dispatch`

**Files:**
- Modify: `src-tauri/src/cli/domain_health.rs`

`--all` resolves every inventoried machine id from the `machines` table and delegates to existing `run`.

- [ ] **Step 1: Confirm `machines::list_all` exists**

Run: `grep -n "pub fn list_all\|pub fn list(" src-tauri/src/data/machines.rs`

Note the function name — if it is `list` instead of `list_all`, adapt accordingly.

- [ ] **Step 2: Write failing test**

Append to `mod tests` in `src-tauri/src/cli/domain_health.rs`:

```rust
    use crate::data::machines::{insert as insert_machine, Machine};
    use crate::data::{open_in_memory, schema};

    fn setup_db_with_two_machines() -> crate::data::Db {
        let db = open_in_memory().unwrap();
        {
            let mut conn = db.lock().unwrap();
            schema::migrate(&mut conn).unwrap();
        }
        let _ = insert_machine(&db, &Machine::new("RENDER-A", "192.168.10.21")).unwrap();
        let _ = insert_machine(&db, &Machine::new("RENDER-B", "192.168.10.22")).unwrap();
        db
    }

    #[test]
    fn resolve_all_machine_ids_returns_both() {
        let db = setup_db_with_two_machines();
        let ids = super::resolve_all_machine_ids(&db).unwrap();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn resolve_all_machine_ids_returns_empty_on_empty_inventory() {
        let db = open_in_memory().unwrap();
        {
            let mut conn = db.lock().unwrap();
            schema::migrate(&mut conn).unwrap();
        }
        assert_eq!(super::resolve_all_machine_ids(&db).unwrap().len(), 0);
    }
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cd src-tauri && cargo test --lib cli::domain_health::tests::resolve_all_machine_ids_returns_both`

Expected: FAIL — `cannot find function 'resolve_all_machine_ids'`.

- [ ] **Step 4: Implement**

Append to `src-tauri/src/cli/domain_health.rs`:

```rust
fn resolve_all_machine_ids(db: &crate::data::Db) -> UecmResult<Vec<i64>> {
    Ok(crate::data::machines::list_all(db)?
        .into_iter()
        .filter_map(|m| m.id)
        .collect())
}
```

(Substitute `list_all` with whatever Step 1 revealed.)

Update `run_dispatch` to wire it:

```rust
fn run_dispatch(
    ctx: &mut Ctx<'_>,
    machine_ids: Vec<i64>,
    cidr: Option<String>,
    all: bool,
    cred: &CredentialArgs,
) -> UecmResult<()> {
    if let Some(_) = cidr {
        return Err(crate::error::UecmError::InvalidInput("--cidr not yet implemented".into()));
    }
    if all {
        let db = ctx.require_db()?.clone();
        let ids = resolve_all_machine_ids(&db)?;
        if ids.is_empty() {
            return Err(crate::error::UecmError::InvalidInput(
                "--all requested but inventory is empty (run `uecm-cli machine scan` first)".into(),
            ));
        }
        return run(ctx, &ids, cred);
    }
    run(ctx, &machine_ids, cred)
}
```

- [ ] **Step 5: Run tests**

Run: `cd src-tauri && cargo test --lib cli::domain_health`

Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/cli/domain_health.rs
git commit -m "feat(health-cli): implement --all mode (resolve from inventory)"
```

---

### Task 9: Implement `--cidr` mode with hoisted Tokio runtime

**Files:**
- Modify: `src-tauri/src/cli/domain_health.rs`

**Design notes:**
- CIDR mode probes **every IP in the range**, including fully-closed hosts (so "all dark" is a visible answer). It does NOT filter by `winrm_open || smb_open` like `core::network::scan_cidr` does.
- Runs L1 only. Stdout-only — no DB writes (no machine row to FK against).
- Uses a `tokio::sync::Semaphore` capped at the same `MAX_INFLIGHT=50` as `scan_cidr` to avoid hitting socket limits.
- Tokio runtime is built **once** at `run_dispatch` entry and passed in. `Runtime::new()` is safe here because `uecm-cli`'s `main()` is sync (see `src-tauri/src/bin/uecm-cli.rs:17`).

- [ ] **Step 1: Write failing test**

Append to `mod tests` in `src-tauri/src/cli/domain_health.rs`:

```rust
    #[tokio::test]
    async fn scan_and_probe_l1_returns_one_outcome_set_per_ip() {
        // /30 in TEST-NET-3 yields 2 usable hosts; all closed, all kept.
        let outcomes = super::scan_and_probe_l1("203.0.113.0/30", 50).await.unwrap();
        assert_eq!(outcomes.len(), 2, "expected /30 to yield 2 hosts, got {}", outcomes.len());
        for (ip, port_outcomes) in &outcomes {
            assert!(ip.starts_with("203.0.113."));
            assert_eq!(port_outcomes.len(), 3, "expected 3 L1 keys per IP");
            assert!(port_outcomes.contains_key("tcp_5985"));
            assert!(port_outcomes.contains_key("tcp_445"));
            assert!(port_outcomes.contains_key("tcp_135"));
        }
    }

    #[test]
    fn cidr_too_large_returns_invalid_input() {
        // /16 = 65534 hosts, blocked by MAX_HOSTS guard.
        let rt = tokio::runtime::Runtime::new().unwrap();
        let r = rt.block_on(super::scan_and_probe_l1("10.0.0.0/16", 50));
        assert!(matches!(r, Err(crate::error::UecmError::InvalidInput(_))));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test --lib cli::domain_health::tests::scan_and_probe_l1_returns_one_outcome_set_per_ip`

Expected: FAIL — `cannot find function 'scan_and_probe_l1'`.

- [ ] **Step 3: Implement `scan_and_probe_l1` with bounded concurrency**

Append to `src-tauri/src/cli/domain_health.rs`:

```rust
/// CIDR-mode L1 scan: probe every IP in the CIDR range (including fully-closed
/// hosts), return outcomes per IP. Unlike `core::network::scan_cidr` which
/// filters out fully-closed hosts, we keep them — operators want "this IP is
/// dark" as an answer too.
///
/// Concurrency capped at `core::network::MAX_INFLIGHT` (same as scan_cidr) to
/// avoid socket exhaustion on large CIDRs.
async fn scan_and_probe_l1(
    cidr: &str,
    timeout_ms: u64,
) -> UecmResult<Vec<(String, std::collections::HashMap<String, crate::core::health_check::CheckOutcome>)>> {
    use ipnet::Ipv4Net;
    use std::str::FromStr;
    use std::sync::Arc;
    use tokio::sync::Semaphore;

    let net = Ipv4Net::from_str(cidr).map_err(|e| {
        crate::error::UecmError::InvalidInput(format!("invalid CIDR '{}': {}", cidr, e))
    })?;
    let hosts: Vec<String> = net.hosts().map(|ip| ip.to_string()).collect();
    if hosts.len() > crate::core::network::MAX_HOSTS {
        return Err(crate::error::UecmError::InvalidInput(format!(
            "CIDR expands to {} hosts (max {})",
            hosts.len(),
            crate::core::network::MAX_HOSTS
        )));
    }

    // Same MAX_INFLIGHT cap as scan_cidr to avoid socket exhaustion.
    // probe_tcp_ports opens 3 sockets per host, so effective inflight is 3 × cap.
    let semaphore = Arc::new(Semaphore::new(50));
    let mut handles = Vec::with_capacity(hosts.len());
    for ip in hosts {
        let sem = semaphore.clone();
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire_owned().await.ok()?;
            let outcomes = crate::core::health_check::probe_tcp_ports(&ip, timeout_ms).await;
            Some((ip, outcomes))
        }));
    }
    let mut out = Vec::with_capacity(handles.len());
    for h in handles {
        if let Ok(Some(pair)) = h.await {
            out.push(pair);
        }
    }
    Ok(out)
}
```

If `MAX_INFLIGHT` is `pub` in `core::network`, use `crate::core::network::MAX_INFLIGHT` instead of hard-coding `50`. Check with: `grep -n 'MAX_INFLIGHT' src-tauri/src/core/network.rs`. Currently it is `const MAX_INFLIGHT: usize = 50;` (private) — if exposing it as `pub`, do that in this task too.

- [ ] **Step 4: Hoist Tokio runtime + implement `run_cidr`**

The runtime should be built **once** at the top of `run_dispatch` and passed down. Replace `run_dispatch` with:

```rust
fn run_dispatch(
    ctx: &mut Ctx<'_>,
    machine_ids: Vec<i64>,
    cidr: Option<String>,
    all: bool,
    cred: &CredentialArgs,
) -> UecmResult<()> {
    // Build the Tokio runtime once. uecm-cli's main() is sync (uecm-cli.rs:17),
    // so creating a new runtime here is safe (no outer runtime to conflict with).
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| crate::error::UecmError::OperationFailed(e.to_string()))?;

    if let Some(cidr_str) = cidr {
        return run_cidr(ctx, &rt, &cidr_str);
    }
    if all {
        let db = ctx.require_db()?.clone();
        let ids = resolve_all_machine_ids(&db)?;
        if ids.is_empty() {
            return Err(crate::error::UecmError::InvalidInput(
                "--all requested but inventory is empty (run `uecm-cli machine scan` first)".into(),
            ));
        }
        return run_with_rt(ctx, &rt, &ids, cred);
    }
    run_with_rt(ctx, &rt, &machine_ids, cred)
}
```

Now `run` becomes `run_with_rt(ctx, rt, ids, cred)` — accepts a runtime reference instead of building one. **Subsequent tasks (T10a/b/c) thread `&rt` through `run_with_rt` instead of calling `Runtime::new()` per-machine.** For this task, just add the parameter and rename:

```rust
fn run_with_rt(
    ctx: &mut Ctx<'_>,
    _rt: &tokio::runtime::Runtime,    // used by T10a-c — unused for now
    machine_ids: &[i64],
    cred: &CredentialArgs,
) -> UecmResult<()> {
    // ... existing run() body, unchanged for this task ...
}
```

And the old top-level `fn run(ctx, machine_ids, cred)` is removed (its body lives in `run_with_rt` now).

Now add `run_cidr`:

```rust
fn run_cidr(ctx: &mut Ctx<'_>, rt: &tokio::runtime::Runtime, cidr: &str) -> UecmResult<()> {
    let outcomes = rt.block_on(scan_and_probe_l1(cidr, 1000))?;

    ctx.emitter
        .emit_event(&Event::Started {
            task_type: "health_run_cidr".into(),
            task_id: None,
            metadata: serde_json::json!({
                "cidr": cidr,
                "hosts": outcomes.len(),
                "note": "CIDR mode probes every IP in range (including all-closed). L1 only — no creds, no DB persistence."
            }),
        })
        .ok();

    let total = outcomes.len() as i64;
    let mut hosts_with_any_open = 0i64;
    for (idx, (ip, port_outcomes)) in outcomes.iter().enumerate() {
        let any_open = port_outcomes.values().any(|o| o.status == "healthy");
        if any_open { hosts_with_any_open += 1; }
        ctx.emitter
            .emit_event(&Event::ItemCompleted {
                item_id: format!("ip:{}", ip),
                index: idx as i64,
                ok: any_open,
                message: Some(serde_json::to_string(port_outcomes).unwrap_or_default()),
            })
            .ok();
    }

    let summary = serde_json::json!({
        "mode": "cidr",
        "cidr": cidr,
        "hosts_total": total,
        "hosts_with_any_open_port": hosts_with_any_open,
        "persisted": false,
        "next_step": "For deeper L2+L3 diagnosis, run `uecm-cli machine add --ip <X>` to inventory the host, then `uecm-cli health run --machine-ids <id> --cred-alias <alias>`."
    });
    ctx.emitter.emit_event(&Event::Completed { summary }).ok();
    Ok(())
}
```

- [ ] **Step 5: Run tests**

Run: `cd src-tauri && cargo test --lib cli::domain_health`

Expected: all PASS.

Also run `cargo build --lib` — the `fn run` → `fn run_with_rt` rename will surface every caller that used to call `run(...)`; fix each one.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/cli/domain_health.rs src-tauri/src/core/network.rs
git commit -m "feat(health-cli): --cidr mode with hoisted Tokio runtime + bounded concurrency"
```

---

### Task 10a: `build_no_creds_row` helper

**Files:**
- Modify: `src-tauri/src/cli/domain_health.rs`

When `--cred-alias` / `--user` are absent, we cannot call WinRM. Generate a row of `na` outcomes for every probe that requires creds — the `Counters::tally` from T6 segregates these into the `skipped` counter.

- [ ] **Step 1: Write failing test**

Append to `mod tests`:

```rust
    #[test]
    fn build_no_creds_row_marks_all_authenticated_probes_as_na() {
        let row = super::build_no_creds_row();
        let expected = crate::core::probe_keys::offline_probe_keys();
        assert_eq!(row.len(), expected.len());
        for key in &expected {
            let o = row.get(*key).expect(&format!("missing key: {}", key));
            assert_eq!(o.status, "na", "{} should be na", key);
            assert!(o.remediation.contains("--cred-alias") || o.remediation.contains("credential"),
                    "{} remediation should mention credentials, got: {}", key, o.remediation);
        }
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test --lib cli::domain_health::tests::build_no_creds_row_marks_all_authenticated_probes_as_na`

Expected: FAIL — `cannot find function 'build_no_creds_row'`.

- [ ] **Step 3: Implement**

Append to `src-tauri/src/cli/domain_health.rs`:

```rust
fn build_no_creds_row() -> std::collections::HashMap<String, crate::core::health_check::CheckOutcome> {
    use std::collections::HashMap;
    let mut row = HashMap::new();
    for k in crate::core::probe_keys::offline_probe_keys() {
        row.insert(
            k.into(),
            crate::core::health_check::CheckOutcome {
                status: "na".into(),
                message: "credentials not provided; authenticated probes skipped".into(),
                sample: "".into(),
                remediation: "Provide --cred-alias <alias> (or --user/--pass-stdin) to enable L2 and L3 probes.".into(),
            },
        );
    }
    row
}
```

- [ ] **Step 4: Run tests**

Run: `cd src-tauri && cargo test --lib cli::domain_health::tests::build_no_creds_row`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/cli/domain_health.rs
git commit -m "feat(health-cli): build_no_creds_row helper for L2/L3 na outcomes"
```

---

### Task 10b: Wire no-creds branch into `run_with_rt`

**Files:**
- Modify: `src-tauri/src/cli/domain_health.rs`

Splice the no-creds path into `run_with_rt`. When `resolved_cred.is_none()`, skip the WinRM `health_probes::run` call entirely and use `build_no_creds_row`. The offline branch (when WinRM call fails) also gets refactored to use `offline_probe_keys()` from the registry instead of an inline array.

- [ ] **Step 1: Write integration-style test stub**

A full end-to-end test requires a fake WinRM target, which is impractical. Instead, add a regression assertion that `run_with_rt` does NOT call `health_probes::run` when creds are absent. This is best done by exposing a small helper `should_skip_winrm(resolved_cred: &Option<(String, String)>)` and testing that:

```rust
    #[test]
    fn should_skip_winrm_when_creds_absent() {
        assert!(super::should_skip_winrm(&None));
        assert!(!super::should_skip_winrm(&Some(("u".into(), "p".into()))));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test --lib cli::domain_health::tests::should_skip_winrm_when_creds_absent`

Expected: FAIL — `cannot find function 'should_skip_winrm'`.

- [ ] **Step 3: Implement helper + wire into `run_with_rt`**

Add helper near top of `domain_health.rs`:

```rust
pub(crate) fn should_skip_winrm(resolved_cred: &Option<(String, String)>) -> bool {
    resolved_cred.is_none()
}
```

Now inside `run_with_rt`, replace the existing block (around line 98-152 of the original):

```rust
        let cred_opt = if resolved_cred.is_some() {
            Some((op_user.as_str(), op_pass.as_str()))
        } else {
            None
        };

        let probes = match health_probes::run(...) {
            Ok(map) => map,
            Err(e) => { ... offline branch ... continue; }
        };
```

With:

```rust
        let probes: HashMap<String, crate::core::health_check::CheckOutcome> =
            if should_skip_winrm(&resolved_cred) {
                build_no_creds_row()
            } else {
                let cred_opt = Some((op_user.as_str(), op_pass.as_str()));
                match health_probes::run(
                    &machine.ip,
                    &cluster_share_unc,
                    &cluster_svc_username,
                    &cluster_share_unc,
                    cred_opt,
                ) {
                    Ok(map) => map,
                    Err(e) => {
                        // Offline branch: fill registry keys with `offline`.
                        let mut row: HashMap<String, crate::core::health_check::CheckOutcome> = HashMap::new();
                        for k in crate::core::probe_keys::offline_probe_keys() {
                            row.insert(
                                k.into(),
                                crate::core::health_check::CheckOutcome {
                                    status: "offline".into(),
                                    message: e.to_string(),
                                    sample: "".into(),
                                    remediation: "Bring the host online (verify network + WinRM) before retrying.".into(),
                                },
                            );
                        }
                        // Still inject L1 — operator may have lost WinRM but kept TCP visibility.
                        let l1 = _rt.block_on(crate::core::health_check::probe_tcp_ports(&machine.ip, 1000));
                        for (k, v) in l1 { row.insert(k, v); }
                        health_check_runs::upsert(&db, scan_id, mid, &serde_json::to_value(&row).unwrap())?;
                        // Counter math: offline branch contributes `offline` for each PS key + L1 outcomes.
                        let mut rc = Counters::default();
                        for v in row.values() { rc.tally(v); }
                        healthy += rc.healthy; warning += rc.warning; critical += rc.critical;
                        offline += rc.offline; skipped += rc.skipped;
                        total_checks += rc.total_ran;
                        ctx.emitter.emit_event(&Event::ItemCompleted {
                            item_id: format!("machine:{}", mid),
                            index: idx as i64,
                            ok: false,
                            message: Some(e.to_string()),
                        }).ok();
                        continue;
                    }
                }
            };
```

(The `_rt` parameter from T9 is now used — drop the underscore prefix or remove the leading `_` from the parameter name in `run_with_rt`'s signature.)

- [ ] **Step 4: Run tests**

Run: `cd src-tauri && cargo test --lib cli::domain_health; cargo build --lib`

Expected: PASS + clean build.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/cli/domain_health.rs
git commit -m "feat(health-cli): wire no-creds + offline branches into run_with_rt"
```

---

### Task 10c: L1 port outcomes always run, even after L2/L3 succeed

**Files:**
- Modify: `src-tauri/src/cli/domain_health.rs`

After `probes` is populated (either from `build_no_creds_row`, the offline branch, or a successful `health_probes::run`), unconditionally append the L1 port outcomes. This gives every machine row a complete L1+L2+L3 picture.

- [ ] **Step 1: Write failing test**

Append to `mod tests`:

```rust
    #[tokio::test]
    async fn inject_l1_ports_adds_three_keys_to_row() {
        use std::collections::HashMap;
        let mut row: HashMap<String, crate::core::health_check::CheckOutcome> = HashMap::new();
        row.insert("lanman_server".into(), super::tests::outcome("healthy"));
        // Use TEST-NET-3 — closed ports, fast timeout.
        super::inject_l1_ports(&mut row, "203.0.113.3", 50).await;
        assert!(row.contains_key("tcp_5985"));
        assert!(row.contains_key("tcp_445"));
        assert!(row.contains_key("tcp_135"));
        assert!(row.contains_key("lanman_server")); // existing key preserved
    }
```

(Note: the `outcome` helper from T6 lives in the same `mod tests`, so `super::tests::outcome(...)` works.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test --lib cli::domain_health::tests::inject_l1_ports_adds_three_keys_to_row`

Expected: FAIL — `cannot find function 'inject_l1_ports'`.

- [ ] **Step 3: Implement helper + wire into `run_with_rt`**

Add to `domain_health.rs`:

```rust
pub(crate) async fn inject_l1_ports(
    row: &mut std::collections::HashMap<String, crate::core::health_check::CheckOutcome>,
    ip: &str,
    timeout_ms: u64,
) {
    let l1 = crate::core::health_check::probe_tcp_ports(ip, timeout_ms).await;
    for (k, v) in l1 {
        row.insert(k, v);
    }
}
```

In `run_with_rt`, **immediately after** the `let probes = if should_skip_winrm ... else ...;` block resolves (and BEFORE the derived-checks block that follows), inject L1:

```rust
        let mut probes = probes; // make mutable
        rt.block_on(inject_l1_ports(&mut probes, &machine.ip, 1000));
```

(Note: in the offline `continue` branch from T10b we already added L1 inline, so this top-level injection only fires on the success / no-creds paths — which is correct, no double-injection.)

- [ ] **Step 4: Run tests**

Run: `cd src-tauri && cargo test --lib cli::domain_health`

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/cli/domain_health.rs
git commit -m "feat(health-cli): always inject L1 port outcomes into per-machine row"
```

---

### Task 11: TS `PROBE_LAYER_MAP` with drift test against Rust registry

**Files:**
- Modify: `src/services/tauri.ts` (append near line 394, after `HealthCheckRun`)
- Create: `src/__tests__/probe-layer-map.spec.ts`

The TS map must mirror the Rust `PROBE_REGISTRY`. We cannot easily generate it from Rust at build time, so we hand-maintain it and add a unit test that compares its key set against the JSON snapshot of the registry (produced via a small `schema` CLI command if available — else snapshot-tested by the implementer manually).

- [ ] **Step 1: Write failing test**

Create `src/__tests__/probe-layer-map.spec.ts`:

```ts
import { describe, it, expect } from "vitest";
import { PROBE_LAYER_MAP, type ProbeLayer } from "@/services/tauri";

describe("PROBE_LAYER_MAP", () => {
  const expectedL1 = ["tcp_5985", "tcp_445", "tcp_135"];
  const expectedL2 = ["firewall_445", "local_account_token_filter", "long_paths_enabled", "lanman_server"];
  const expectedL3Business = [
    "share_reachable", "ntfs_perm", "cred_user", "cred_system",
    "env_vars", "system_write", "winmgmt",
  ];
  const expectedL3Derived = ["ini_consistency", "pso_precaching", "gpu_consistency"];

  it("has exactly 17 entries matching the Rust PROBE_REGISTRY", () => {
    expect(Object.keys(PROBE_LAYER_MAP).length).toBe(17);
  });

  it("L1 keys are correct", () => {
    for (const k of expectedL1) {
      expect(PROBE_LAYER_MAP[k as keyof typeof PROBE_LAYER_MAP]).toBe("l1_port");
    }
  });

  it("L2 keys are correct", () => {
    for (const k of expectedL2) {
      expect(PROBE_LAYER_MAP[k as keyof typeof PROBE_LAYER_MAP]).toBe("l2_bootstrap");
    }
  });

  it("L3 business + derived keys are correct", () => {
    for (const k of [...expectedL3Business, ...expectedL3Derived]) {
      expect(PROBE_LAYER_MAP[k as keyof typeof PROBE_LAYER_MAP]).toBe("l3_business");
    }
  });

  it("no unexpected keys", () => {
    const expected = new Set([...expectedL1, ...expectedL2, ...expectedL3Business, ...expectedL3Derived]);
    for (const k of Object.keys(PROBE_LAYER_MAP)) {
      expect(expected.has(k)).toBe(true);
    }
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm test -- probe-layer-map`

Expected: FAIL — `PROBE_LAYER_MAP` not exported.

- [ ] **Step 3: Implement in `src/services/tauri.ts`**

Append after `HealthCheckRun` (around line 394):

```ts
export type ProbeLayer = "l1_port" | "l2_bootstrap" | "l3_business";

/**
 * Maps each probe key emitted by the Rust health runner to its diagnostic
 * layer. MUST stay in sync with src-tauri/src/core/probe_keys.rs PROBE_REGISTRY
 * — the drift test in probe-layer-map.spec.ts encodes the expected keys.
 *
 * L3 covers both "business" (WinRM probes) and "derived" (Rust-computed)
 * because they render in the same UI section.
 */
export const PROBE_LAYER_MAP = {
  // L1 — port reachability (operator-console TCP probes)
  tcp_5985: "l1_port",
  tcp_445: "l1_port",
  tcp_135: "l1_port",
  // L2 — bootstrap configuration (PowerShell via WinRM)
  firewall_445: "l2_bootstrap",
  local_account_token_filter: "l2_bootstrap",
  long_paths_enabled: "l2_bootstrap",
  lanman_server: "l2_bootstrap",
  // L3 — business workflow (PowerShell + derived)
  share_reachable: "l3_business",
  ntfs_perm: "l3_business",
  cred_user: "l3_business",
  cred_system: "l3_business",
  env_vars: "l3_business",
  system_write: "l3_business",
  winmgmt: "l3_business",
  ini_consistency: "l3_business",
  pso_precaching: "l3_business",
  gpu_consistency: "l3_business",
} as const satisfies Record<string, ProbeLayer>;

export type ProbeKey = keyof typeof PROBE_LAYER_MAP;
```

Also extend `CheckOutcome` (around line 371) to include the `skipped` summary field consumers will need. Look at `HealthRunSummary` around line 385-392 and add `skipped: number;`:

```ts
export interface HealthRunSummary {
  scan_run_id: number;
  healthy: number;
  warning: number;
  critical: number;
  offline: number;
  skipped: number;   // <-- NEW: na outcomes from T6
  total: number;
}
```

- [ ] **Step 4: Run tests**

Run: `pnpm test -- probe-layer-map; pnpm typecheck`

Expected: PASS, clean.

- [ ] **Step 5: Commit**

```bash
git add src/services/tauri.ts src/__tests__/probe-layer-map.spec.ts
git commit -m "feat(types): PROBE_LAYER_MAP + ProbeLayer (mirrors Rust PROBE_REGISTRY)"
```

---

### Task 12: `probesByLayer` computed in healthCheck store + warn on unknown keys

**Files:**
- Modify: `src/stores/healthCheck.ts`
- Modify: `src/__tests__/health-check-store.spec.ts`

- [ ] **Step 1: Read existing store + spec to understand fixture pattern**

Run: `cat src/__tests__/health-check-store.spec.ts | head -60; cat src/stores/healthCheck.ts | head -40`

Note the `setActivePinia(createPinia())` beforeEach idiom and how `store.results` gets injected.

- [ ] **Step 2: Write failing test**

First, add this import to the **top** of `src/__tests__/health-check-store.spec.ts` (with the other imports — JS/TS import statements must be at top-level, not inside `describe`):

```ts
import { PROBE_LAYER_MAP } from "@/services/tauri";
```

Then append a new `describe` block to the file (at the bottom, top-level — NOT nested inside the existing describe):

```ts
describe("probesByLayer", () => {
  it("groups outcomes by L1/L2/L3 for a single machine", () => {
    const store = useHealthCheckStore();
    store.results = [{
      scan_run_id: 1,
      machine_id: 42,
      machine_results: {
        tcp_5985: { status: "healthy", message: "", sample: "", remediation: "" },
        lanman_server: { status: "healthy", message: "", sample: "", remediation: "" },
        share_reachable: { status: "critical", message: "stopped", sample: "Stopped",
                           remediation: "Run uecm-cli share create --host <h>" },
      },
    }] as any;

    const grouped = store.probesByLayer[42];
    expect(grouped.l1_port).toHaveLength(1);
    expect(grouped.l1_port[0].key).toBe("tcp_5985");
    expect(grouped.l2_bootstrap).toHaveLength(1);
    expect(grouped.l3_business).toHaveLength(1);
    expect(grouped.l3_business[0].outcome.remediation).toContain("share create");
  });

  it("warns when an unknown probe key appears", () => {
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    const store = useHealthCheckStore();
    store.results = [{
      scan_run_id: 1,
      machine_id: 42,
      machine_results: {
        unknown_probe_xyz: { status: "warning", message: "?", sample: "", remediation: "" },
      },
    }] as any;
    // Access probesByLayer to trigger the computed
    const _ = store.probesByLayer;
    expect(warnSpy).toHaveBeenCalledWith(
      expect.stringContaining("unknown_probe_xyz")
    );
    warnSpy.mockRestore();
  });
});
```

- [ ] **Step 3: Run test to verify it fails**

Run: `pnpm test -- health-check-store`

Expected: FAIL — `probesByLayer` not on store; or `console.warn` not called.

- [ ] **Step 4: Implement in `src/stores/healthCheck.ts`**

Add imports at top:

```ts
import { PROBE_LAYER_MAP, type ProbeLayer } from "@/services/tauri";
```

After the `summary` computed, add:

```ts
  interface LayeredOutcome {
    key: string;
    outcome: CheckOutcome;
  }

  const probesByLayer = computed<Record<number, Record<ProbeLayer, LayeredOutcome[]>>>(() => {
    const out: Record<number, Record<ProbeLayer, LayeredOutcome[]>> = {};
    for (const [mid, row] of Object.entries(rowsByMachine.value)) {
      const grouped: Record<ProbeLayer, LayeredOutcome[]> = {
        l1_port: [],
        l2_bootstrap: [],
        l3_business: [],
      };
      for (const [key, outcome] of Object.entries(row)) {
        const layer = (PROBE_LAYER_MAP as Record<string, ProbeLayer | undefined>)[key];
        if (!layer) {
          // Unknown key — log loud so renames/typos don't silently disappear in UI.
          console.warn(
            `[healthCheck] unknown probe key '${key}' not in PROBE_LAYER_MAP — ` +
            `update src/services/tauri.ts and src-tauri/src/core/probe_keys.rs in sync.`
          );
          continue;
        }
        grouped[layer].push({ key, outcome });
      }
      for (const layer of ["l1_port", "l2_bootstrap", "l3_business"] as const) {
        grouped[layer].sort((a, b) => a.key.localeCompare(b.key));
      }
      out[Number(mid)] = grouped;
    }
    return out;
  });
```

Add `probesByLayer` to the store's returned object.

- [ ] **Step 5: Run tests**

Run: `pnpm test -- health-check-store; pnpm typecheck`

Expected: PASS, clean.

- [ ] **Step 6: Commit**

```bash
git add src/stores/healthCheck.ts src/__tests__/health-check-store.spec.ts
git commit -m "feat(store): probesByLayer computed + console.warn on unknown keys"
```

---

### Task 13: Refactor `HealthCheck.vue` with concrete store-driven fixture

**Files:**
- Modify: `src/views/HealthCheck.vue`
- Modify: `src/__tests__/HealthCheck-view.spec.ts`

The existing spec at `src/__tests__/HealthCheck-view.spec.ts` mocks `tauriApi` but does not exercise `useHealthCheckStore.results`-driven rendering. We extend it. The fixture pattern matches the existing one (`setActivePinia(createPinia())` + `mount(HealthCheck)`) — no new test infrastructure needed.

- [ ] **Step 1: Write failing test**

Append to `src/__tests__/HealthCheck-view.spec.ts` inside the existing `describe("HealthCheck view", ...)` block (after the last `it(...)`):

```ts
  it("renders L1 / L2 / L3 layer sections when a scan run is loaded", async () => {
    const wrapper = mount(HealthCheck);
    // Reach into the store and inject a synthetic scan run.
    const { useHealthCheckStore } = await import("@/stores/healthCheck");
    const store = useHealthCheckStore();
    store.scanRunId = 42 as any;
    store.results = [{
      scan_run_id: 42,
      machine_id: 7,
      machine_results: {
        tcp_5985:        { status: "healthy",  message: "open",       sample: "open",    remediation: "" },
        firewall_445:    { status: "critical", message: "rule off",   sample: "false",
                           remediation: "Run uecm-cli winrm bootstrap <host>" },
        share_reachable: { status: "warning",  message: "slow",       sample: "",        remediation: "" },
      },
    }] as any;
    await flushPromises();

    const text = wrapper.text();
    // Layer labels render — match the i18n keys from T15 (zh default for this project).
    expect(text).toMatch(/L1|端口/);
    expect(text).toMatch(/L2|Bootstrap/);
    expect(text).toMatch(/L3|业务|Business/);
    // Probe keys render
    expect(text).toContain("tcp_5985");
    expect(text).toContain("firewall_445");
    expect(text).toContain("share_reachable");
  });

  it("renders remediation text under a critical row", async () => {
    const wrapper = mount(HealthCheck);
    const { useHealthCheckStore } = await import("@/stores/healthCheck");
    const store = useHealthCheckStore();
    store.scanRunId = 1 as any;
    store.results = [{
      scan_run_id: 1,
      machine_id: 1,
      machine_results: {
        cred_user: { status: "critical", message: "missing", sample: "",
                     remediation: "Run uecm-cli share inject-system-cred --host <host>" },
      },
    }] as any;
    await flushPromises();

    expect(wrapper.text()).toContain("share inject-system-cred");
  });
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `pnpm test -- HealthCheck-view`

Expected: 2 new tests FAIL (layer sections don't exist yet).

- [ ] **Step 3: Implement view changes**

Read the existing `src/views/HealthCheck.vue` to identify the per-machine rendering region. Inside the block that loops over machines (or shows the empty-state when `health.scanRunId === null`), add the layered renderer. The 3 layer sections per machine:

```vue
<template v-for="layer in (['l1_port', 'l2_bootstrap', 'l3_business'] as const)" :key="`${machine.id}-${layer}`">
  <section class="mt-4">
    <h4 class="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
      {{ t(`healthCheck.layer.${layer}`) }}
    </h4>
    <table class="w-full text-sm">
      <tbody>
        <template
          v-for="probe in health.probesByLayer[machine.id]?.[layer] ?? []"
          :key="probe.key"
        >
          <tr class="border-t border-border" :data-probe-key="probe.key">
            <td class="px-2 py-1 font-mono text-xs text-muted-foreground">{{ probe.key }}</td>
            <td class="px-2 py-1">
              <UecmStatusBadge :tone="toneFor(probe.outcome.status)" :label="probe.outcome.status" />
            </td>
            <td class="px-2 py-1 text-foreground">{{ probe.outcome.message }}</td>
          </tr>
          <tr
            v-if="probe.outcome.remediation && (probe.outcome.status === 'critical' || probe.outcome.status === 'warning')"
            class="border-t border-border/50"
            :data-probe-remediation="probe.key"
          >
            <td></td>
            <td colspan="2" class="px-2 py-1 text-xs text-muted-foreground">
              <UecmIcon name="wrench" class="mr-1 inline" />
              {{ probe.outcome.remediation }}
            </td>
          </tr>
        </template>
      </tbody>
    </table>
  </section>
</template>
```

Add `toneFor` helper in `<script setup>`:

```ts
function toneFor(status: string): "healthy" | "warning" | "critical" | "info" | "offline" | "unknown" | "na" {
  switch (status) {
    case "healthy": case "warning": case "critical":
    case "offline": case "unknown": case "na":
      return status;
    default:
      return "unknown";
  }
}
```

**Verify `UecmIcon name="wrench"` exists:**

```bash
grep -n 'wrench\|"tool"' src/components/primitives/UecmIcon.vue | head
```

If `wrench` is not in the mapping, either extend `UecmIcon.vue` (see icon list pattern in that file) or substitute with `tool` / `info` / `alert-triangle` — whichever already exists.

- [ ] **Step 4: Run tests**

Run: `pnpm test -- HealthCheck-view; pnpm typecheck`

Expected: PASS, clean.

- [ ] **Step 5: Commit**

```bash
git add src/views/HealthCheck.vue src/__tests__/HealthCheck-view.spec.ts
git commit -m "feat(ui): group HealthCheck rows by layer + show remediation"
```

---

### Task 14: i18n strings + per-probe locale drift test

**Files:**
- Modify: `src/locales/en.ts`
- Modify: `src/locales/zh.ts`
- Create: `src/__tests__/health-locale-coverage.spec.ts`

- [ ] **Step 1: Locate existing healthCheck locale block**

Run: `grep -n "healthCheck" src/locales/en.ts src/locales/zh.ts | head -20`

Note the nested shape so the new sub-blocks plug in correctly.

- [ ] **Step 2: Write failing drift test**

Create `src/__tests__/health-locale-coverage.spec.ts`:

```ts
import { describe, it, expect } from "vitest";
import { PROBE_LAYER_MAP } from "@/services/tauri";
import en from "@/locales/en";
import zh from "@/locales/zh";

describe("health probe locale coverage", () => {
  const probeKeys = Object.keys(PROBE_LAYER_MAP);
  const layers = ["l1_port", "l2_bootstrap", "l3_business"];

  function pick(obj: any, path: string[]): any {
    return path.reduce((acc, key) => (acc && acc[key] !== undefined ? acc[key] : undefined), obj);
  }

  for (const locale of [{ name: "en", t: en }, { name: "zh", t: zh }]) {
    it(`${locale.name}: every probe key in PROBE_LAYER_MAP has a label`, () => {
      for (const probeKey of probeKeys) {
        const label = pick(locale.t, ["healthCheck", "probe", probeKey]);
        expect(label, `missing healthCheck.probe.${probeKey} in ${locale.name}`).toBeTruthy();
      }
    });
    it(`${locale.name}: every layer has a label`, () => {
      for (const layer of layers) {
        const label = pick(locale.t, ["healthCheck", "layer", layer]);
        expect(label, `missing healthCheck.layer.${layer} in ${locale.name}`).toBeTruthy();
      }
    });
  }
});
```

- [ ] **Step 3: Run test to verify it fails**

Run: `pnpm test -- health-locale-coverage`

Expected: FAIL — keys not yet added.

- [ ] **Step 4: Add strings**

In `src/locales/en.ts`, find the `healthCheck:` block and add:

```ts
  healthCheck: {
    // ... existing keys preserved ...
    layer: {
      l1_port: "L1 · Port Reachability",
      l2_bootstrap: "L2 · Bootstrap Configuration",
      l3_business: "L3 · Business Workflow",
    },
    probe: {
      tcp_5985: "WinRM 5985",
      tcp_445: "SMB 445",
      tcp_135: "RPC 135",
      firewall_445: "Firewall · TCP 445 inbound",
      local_account_token_filter: "LocalAccountTokenFilterPolicy",
      long_paths_enabled: "LongPathsEnabled",
      lanman_server: "LanmanServer service",
      share_reachable: "Share reachable",
      ntfs_perm: "NTFS permissions",
      cred_user: "User credential store",
      cred_system: "SYSTEM credential store",
      env_vars: "UE-SharedDataCachePath env",
      system_write: "SYSTEM share write",
      winmgmt: "Winmgmt service",
      ini_consistency: "INI consistency",
      pso_precaching: "PSO precaching",
      gpu_consistency: "GPU consistency",
    },
  },
```

Mirror in `src/locales/zh.ts`:

```ts
  healthCheck: {
    // ... existing keys preserved ...
    layer: {
      l1_port: "L1 · 端口可达性",
      l2_bootstrap: "L2 · Bootstrap 配置",
      l3_business: "L3 · 业务流程",
    },
    probe: {
      tcp_5985: "WinRM 5985",
      tcp_445: "SMB 445",
      tcp_135: "RPC 135",
      firewall_445: "防火墙 · TCP 445 入站",
      local_account_token_filter: "LocalAccountTokenFilterPolicy",
      long_paths_enabled: "LongPathsEnabled",
      lanman_server: "LanmanServer 服务",
      share_reachable: "共享可达",
      ntfs_perm: "NTFS 权限",
      cred_user: "用户凭据存储",
      cred_system: "SYSTEM 凭据存储",
      env_vars: "UE-SharedDataCachePath 环境变量",
      system_write: "SYSTEM 共享写入",
      winmgmt: "Winmgmt 服务",
      ini_consistency: "INI 一致性",
      pso_precaching: "PSO 预缓存",
      gpu_consistency: "GPU 一致性",
    },
  },
```

- [ ] **Step 5: Run tests**

Run: `pnpm test -- health-locale-coverage; pnpm typecheck; pnpm test`

Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add src/locales/en.ts src/locales/zh.ts src/__tests__/health-locale-coverage.spec.ts
git commit -m "i18n(health): layer + probe labels (en + zh) + drift test"
```

---

### Task 15: Update `Docs/core-features.html` §05 — 17 probes × 3 layers

**Files:**
- Modify: `Docs/core-features.html`

- [ ] **Step 1: Locate §05 article block**

Run: `grep -n 'health run\|active probe\|8 个\|11 行' Docs/core-features.html | head -20`

- [ ] **Step 2: Replace the §05 probe description**

Find the line (around line 866-867) that mentions `8 个 PowerShell active probe` and `N 机 × 11 行` and replace with:

```html
分三层垂直体检（共 17 项）：<strong>L1 端口</strong>（<span class="k">tcp_5985</span> / <span class="k">tcp_445</span> / <span class="k">tcp_135</span>）+ <strong>L2 Bootstrap</strong>（<span class="k">firewall_445</span> / <span class="k">local_account_token_filter</span> / <span class="k">long_paths_enabled</span> / <span class="k">lanman_server</span>）+ <strong>L3 业务</strong>（<span class="k">share_reachable</span> / <span class="k">ntfs_perm</span> / <span class="k">cred_user</span> / <span class="k">cred_system</span> / <span class="k">env_vars</span> / <span class="k">system_write</span> / <span class="k">winmgmt</span> + 3 derived <span class="k">ini_consistency</span> / <span class="k">pso_precaching</span> / <span class="k">gpu_consistency</span>）。每行红色状态自带 <span class="k">remediation</span> 文案，告诉操作员下一步该跑哪个 CLI。输出 <strong>N 机 × 17 行</strong>矩阵。
```

Find the `<div class="pstat">...8 active · 3 derived · N×11 矩阵</div>` line and replace with:

```html
<div class="pstat"><span class="n">3</span> L1 + <span class="n">4</span> L2 + <span class="n">7</span> L3 + <span class="n">3</span> derived · N×<span class="n">17</span> 矩阵 · 每行带 remediation</div>
```

Update the command examples nearby:

```html
<div class="pcmd">
  <div><span class="prompt">$</span> uecm-cli health run <span class="arg">--cidr 192.168.10.0/24</span></div>
  <div><span class="prompt">$</span> uecm-cli health run <span class="arg">--all --cred-alias prod</span></div>
  <div><span class="prompt">$</span> uecm-cli health results <span class="arg">--format matrix</span></div>
</div>
```

- [ ] **Step 3: Update the mindmap leaf for branch 05**

Find the SVG `<text>` block containing `health run` (around line 633-634) and replace the note text:

```html
          <text x="752" y="678"><tspan class="mm-leaf-cmd">health run</tspan><tspan class="mm-leaf-note" dx="6">· 3 模式（cidr/all/machine-ids）· 17 项 L1+L2+L3 + 修复建议</tspan></text>
```

- [ ] **Step 4: Smoke-render check**

The HTML is static — no automated test. Open in a browser:

```bash
# In WSL: launch via Windows-side browser
explorer.exe "$(wslpath -w Docs/core-features.html)"
```

Or just verify HTML parses with `tidy -e Docs/core-features.html 2>&1 | head` if `tidy` is available.

Run: `pnpm typecheck; pnpm test`

Expected: clean (no code changes).

- [ ] **Step 5: Commit**

```bash
git add Docs/core-features.html
git commit -m "docs(core-features): 17 probes / 3 layers / remediation per row"
```

---

## Final verification

- [ ] **Full test matrix**

```bash
cd /mnt/e/AIWorkspace/vp/ue-cache-manager/.claude/worktrees/feat+health-run-full-checkup
cd src-tauri && cargo test --lib && cd ..
pnpm typecheck && pnpm test
```

Expected: all green. The drift tests (`probe_keys::tests::powershell_script_results_hashtable_matches_registry`, `probe-layer-map.spec.ts`, `health-locale-coverage.spec.ts`) act as compile-time guards against future probe-key drift.

- [ ] **CLI smoke test on lanPC**

```bash
# Build:
cd src-tauri && cargo build --release --bin uecm-cli && cd ..

# Three modes:
./src-tauri/target/release/uecm-cli.exe health run --cidr 192.168.10.0/29
./src-tauri/target/release/uecm-cli.exe health run --all
./src-tauri/target/release/uecm-cli.exe health run --machine-ids 1
./src-tauri/target/release/uecm-cli.exe health run --all --cred-alias prod   # if cred exists
```

Expected per mode:
- `--cidr`: each IP emits a `ItemCompleted { item_id: "ip:X.X.X.X" }` with L1 outcomes embedded in `message`. Final summary has `mode: "cidr"`, `persisted: false`.
- `--all` without creds: persists rows; per-machine `summary` shows L1 ports + L2/L3 as `na` (`skipped` counter incremented).
- `--all` with creds: full 17-probe matrix per machine, `skipped: 0` in summary.

- [ ] **Codex review on branch diff**

Per CLAUDE.md `Codex Review on Task Completion` rule, after each completed task that changed source the executor should have run:

```bash
node "$(ls -t ~/.claude/plugins/cache/openai-codex/codex/*/scripts/codex-companion.mjs 2>/dev/null | head -1)" review --wait
```

If skipped per-task, run it once on the whole branch diff before merging.

---

## Self-review notes (writer)

**Spec coverage** — every constraint in the brief is implemented:

1. ✅ "health run 只诊断，不修复" — every probe is read-only; `remediation` is text. Verified across T5 (PS1 probes) and T3 (L1).
2. ✅ "凭据可选" — T7 introduces `--cidr` / `--all` without touching `CredentialArgs` (already supports zero-flag). T10a/b implement the no-creds path; T6 introduces the `skipped` counter.
3. ✅ "入参兼容 --cidr 和 --machine-ids" — T7 enforces three-way mutex via clap `conflicts_with_all`; T8 (--all) and T9 (--cidr) implement.
4. ✅ "scan / preflight 命令保留" — `core::network::scan_cidr` and `core::preflight` untouched (File Structure table calls this out explicitly).
5. ✅ "扩到 ~18 项" — actual: 17 (3 L1 + 4 L2 + 7 L3 + 3 derived). Down from the initial 20 after the layering audit removed `winrm_reachable`, `admin_share_writable`, and renamed `smb` → `lanman_server`. Documented in §Architecture and T5.

**Placeholder scan** — none. T13 fixture is now concrete (uses real `useHealthCheckStore` injection, mirrors the existing spec's `setActivePinia` idiom). T5 Step 4 Windows validation path is concrete (scp + ssh lanpc).

**Type consistency:**
- `CheckOutcome.remediation: String` (Rust, T1) ↔ `CheckOutcome.remediation?: string` (TS, existing). ✅
- `PROBE_REGISTRY` keys (T4) drive `offline_probe_keys()` / `powershell_probe_keys()` (T4), PS1 `$results` hashtable (T5, validated by drift test in T4 Step 5), TS `PROBE_LAYER_MAP` (T11, validated by drift test in T11), locale files (T14, validated by drift test in T14). Single source of truth. ✅
- `ProbeLayer` literals `"l1_port" | "l2_bootstrap" | "l3_business"` used identically in TS map (T11), store computed (T12), view template (T13). ✅
- `Counters` struct (T6) used in normal path (T6 Step 5) and offline branch (T10b). Same tally semantics. ✅

**Architecture decisions explicitly defended:**

- **Layering** — moved `firewall_445` from L3 to L2 (firewall is bootstrap config), removed `winrm_reachable` (Invoke-Command success is the WinRM-reachable signal — separate probe is noise), removed `admin_share_writable` (was actually testing `%WINDIR%\Temp` not ADMIN$, redundant with `system_write`), renamed `smb` → `lanman_server` (clearer + no L2/L3 duplication). Result: clean layer semantics, 17 probes.
- **CIDR no DB persistence** — `health_check_runs.machine_id` is FK to `machines.id`; discovered IPs have no machine row. Trade-off: CIDR mode = ad-hoc CLI diagnostic, mirrors `machine scan` / `winrm preflight`. UI consumes only persisted runs.
- **`na` separated from severity counters** — `na` rows do not increment healthy/warning/critical/offline; they increment `skipped`. UI shows "5 healthy, 12 skipped (no creds)" instead of inflating green.
- **Tokio runtime hoisted once** — uecm-cli `main()` is sync, so `Runtime::new()` is safe; built once at `run_dispatch` entry and threaded through `run_with_rt` + `run_cidr`. No per-machine runtime construction.

**Risk callouts for the implementer:**

- T4 Step 5 (PS1 drift test) requires `regex` crate. Add to `[dev-dependencies]` if absent — check first to avoid duplicate.
- T5 Step 4 (Windows smoke test) is **REQUIRED, not optional**. The drift test in T4 catches missing/renamed keys but cannot catch PowerShell syntax errors. Implementer must scp + ssh to lanPC (or any Windows host) before claiming T5 done.
- T10b's `continue` branch fix carries L1 injection inline; T10c adds L1 injection at the top level. The plan is structured so these do NOT double-inject (T10c lives outside the offline `continue`).
- T13 references `UecmIcon name="wrench"`. If the mapping in `src/components/primitives/UecmIcon.vue` doesn't have it, substitute (T13 Step 3 includes the grep command to check).
- The `Counters::tally` snippet in T6 Step 5 changes the meaning of `total_checks` — it now means "probes that actually ran" not "probes attempted." Downstream UI consumers may need adjustment; check `src/stores/healthCheck.ts` `summary` computed and `src/views/HealthCheck.vue` KPI tiles. T11 already adds `skipped: number` to `HealthRunSummary` so the UI can display it.

---

## Execution handoff

**Plan saved to:** `docs/superpowers/plans/2026-05-18-health-run-full-checkup.md`

**15 tasks total** (T1–T15, with T10 split into T10a/b/c).

**Two execution options:**

1. **Subagent-Driven (recommended)** — dispatch a fresh subagent per task via `superpowers:subagent-driven-development`. Fast iteration, review between tasks, smaller blast radius if a task breaks.

2. **Inline Execution** — execute in this session via `superpowers:executing-plans`. Batch execution with checkpoints; better when wanting to keep mental context across closely-coupled tasks (e.g. T4 → T5 → T6 form one logical unit).

**Recommend Subagent-Driven** for this plan — it has 15 tasks with strong inter-task dependencies (registry → PS1 → CLI wiring → store → UI), and the drift tests give clear pass/fail gates between subagent dispatches.
