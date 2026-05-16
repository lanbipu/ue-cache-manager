# UECM CLI — Plan 2: Config Management Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship 4 new CLI domains (`cred` / `env` / `ini` / `share`) so AI / human callers can run the full "save creds → env set → ini edit → share create" path against a UE cluster without the WebView. `env set` / `ini set` / `ini remove` get `--hosts a,b,c` batch fan-out.

**Architecture:** Plan 1's `Ctx` / `Emitter` / `needs_db` / startup module / clap derive idiom — all reused unchanged. New shared structs `CredentialArgs` and `HostArgs` flatten into every subcommand that needs them. CLI handlers call `core::*` / `data::*` directly; **no `core::*` extension** — every `*_with_credential` variant already exists.

**Tech Stack:** Rust 1.75+, clap 4.5 derive (`#[command(flatten)]`), tokio current-thread runtime for batch operations, sha2 for value SHA prefix in NDJSON metadata.

**Relates to:**
- [2026-05-17-cli-config-management-design.md](../specs/2026-05-17-cli-config-management-design.md) — Plan 2 spec
- [2026-05-16-cli-plan-1-foundation-discovery.md](./2026-05-16-cli-plan-1-foundation-discovery.md) — Plan 1 (foundation)

---

## File Structure

### New files

```
src-tauri/src/cli/credential_args.rs  ← shared CredentialArgs + resolve()
src-tauri/src/cli/host_args.rs        ← shared HostArgs + require_one()
src-tauri/src/cli/domain_cred.rs      ← cred list / save / delete handlers
src-tauri/src/cli/domain_env.rs       ← env get / set (single + batch)
src-tauri/src/cli/domain_ini.rs       ← ini read / set / remove (single + batch)
src-tauri/src/cli/domain_share.rs     ← share list / forget / create / inject-system-cred
```

### Modified files

```
src-tauri/Cargo.toml             ← add sha2 dep
src-tauri/src/cli/mod.rs         ← 6 new mod declarations
src-tauri/src/cli/args.rs        ← extend Domain enum with 4 new variants + 4 ActionEnums
src-tauri/src/cli/run.rs         ← 4 new dispatch arms + extend needs_db()
src-tauri/tests/cli_smoke.rs     ← 3 new smoke tests
```

### Untouched

- `src-tauri/src/core/*` — all required `*_with_credential` variants already exist
- `src-tauri/src/data/*` — all required CRUD functions already exist
- `src-tauri/src/commands/*` — Tauri-only, not touched
- `src-tauri/src/bin/uecm-cli.rs`, `startup.rs`, frontend, ps-scripts, vendor

---

## Phase 0 — Shared argument helpers

### Task 0.1: `CredentialArgs` shared struct

**Files:**
- Create: `src-tauri/src/cli/credential_args.rs`
- Modify: `src-tauri/src/cli/mod.rs`

- [ ] **Step 1: Add `sha2` dependency to Cargo.toml**

Append under `[dependencies]` in `src-tauri/Cargo.toml`:

```toml
sha2 = "0.10"
```

Run `cargo build --bin uecm-cli` once to populate Cargo.lock. Expected: build succeeds.

- [ ] **Step 2: Create `src-tauri/src/cli/credential_args.rs`**

```rust
//! Shared `--cred-alias` / `--user --pass[--pass-stdin]` argument set, used by
//! every subcommand that authenticates against a remote host.

use crate::data::credentials as data_creds;
use crate::data::Db;
use crate::error::{UecmError, UecmResult};
use clap::Args;
use std::io::{self, BufRead};

#[derive(Args, Debug, Clone)]
pub struct CredentialArgs {
    /// Resolve credentials from a saved DPAPI alias.
    #[arg(long, value_name = "ALIAS", group = "cred")]
    pub cred_alias: Option<String>,

    /// Inline username; use with --pass or --pass-stdin.
    #[arg(long, value_name = "USER", group = "cred", requires = "secret")]
    pub user: Option<String>,

    /// Inline password. Leaks into shell history — prefer --pass-stdin
    /// or --cred-alias.
    #[arg(long, value_name = "PASS", group = "secret", conflicts_with = "pass_stdin")]
    pub pass: Option<String>,

    /// Read password from stdin (one line, \r\n trimmed).
    #[arg(long, group = "secret", conflicts_with = "pass")]
    pub pass_stdin: bool,
}

impl CredentialArgs {
    /// Resolve to `(username, password)` if any credential was supplied;
    /// `None` means inherit the caller's Kerberos/NTLM context.
    pub fn resolve(&self, db: &Db) -> UecmResult<Option<(String, String)>> {
        if let Some(alias) = &self.cred_alias {
            let user = data_creds::find_by_alias(db, alias)?
                .ok_or_else(|| {
                    UecmError::InvalidInput(format!("credential alias '{}' not found", alias))
                })?
                .username;
            let pass = crate::core::credentials::resolve_password(alias)?;
            return Ok(Some((user, pass)));
        }
        match (&self.user, &self.pass, self.pass_stdin) {
            (Some(u), Some(p), false) => Ok(Some((u.clone(), p.clone()))),
            (Some(u), None, true) => {
                let mut line = String::new();
                io::stdin().lock().read_line(&mut line).map_err(|e| {
                    UecmError::InvalidInput(format!("read password from stdin: {}", e))
                })?;
                let pass = line.trim_end_matches(['\r', '\n']).to_string();
                Ok(Some((u.clone(), pass)))
            }
            (None, None, false) => Ok(None),
            // Other combos are blocked by clap groups (`secret` / `cred`).
            // If clap lets one through, treat as configuration error.
            _ => Err(UecmError::InvalidInput(
                "inconsistent credential flags".into(),
            )),
        }
    }
}
```

- [ ] **Step 3: Declare module in `cli/mod.rs`**

In `src-tauri/src/cli/mod.rs`, add `pub mod credential_args;` after the existing `pub mod` declarations. Do not add a re-export — handlers will use the full path `crate::cli::credential_args::CredentialArgs`.

- [ ] **Step 4: Add unit tests inside `credential_args.rs`**

Append to the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{open_in_memory, schema};

    fn fresh_db() -> Db {
        let db = open_in_memory().unwrap();
        {
            let mut conn = db.lock().unwrap();
            schema::migrate(&mut conn).unwrap();
        }
        db
    }

    #[test]
    fn resolve_returns_none_when_no_flags_given() {
        let args = CredentialArgs {
            cred_alias: None,
            user: None,
            pass: None,
            pass_stdin: false,
        };
        let db = fresh_db();
        assert!(args.resolve(&db).unwrap().is_none());
    }

    #[test]
    fn resolve_inline_user_pass() {
        let args = CredentialArgs {
            cred_alias: None,
            user: Some("alice".into()),
            pass: Some("hunter2".into()),
            pass_stdin: false,
        };
        let db = fresh_db();
        assert_eq!(args.resolve(&db).unwrap(), Some(("alice".into(), "hunter2".into())));
    }

    #[test]
    fn resolve_unknown_alias_returns_invalid_input() {
        let args = CredentialArgs {
            cred_alias: Some("nope".into()),
            user: None,
            pass: None,
            pass_stdin: false,
        };
        let db = fresh_db();
        let r = args.resolve(&db);
        assert!(matches!(r, Err(UecmError::InvalidInput(_))));
    }
}
```

- [ ] **Step 5: Run tests + commit**

```bash
cd /Users/bip.lan/AIWorkspace/vp/ue-cache-manager/.claude/worktrees/feat+cli-architecture
cargo test --manifest-path src-tauri/Cargo.toml --lib cli::credential_args::
```
Expected: 3 tests pass.

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/cli/credential_args.rs src-tauri/src/cli/mod.rs
git commit -m "feat(cli): shared CredentialArgs with resolve()"
```

---

### Task 0.2: `HostArgs` shared struct

**Files:**
- Create: `src-tauri/src/cli/host_args.rs`
- Modify: `src-tauri/src/cli/mod.rs`

- [ ] **Step 1: Create `src-tauri/src/cli/host_args.rs`**

```rust
//! `--host` (single) vs `--hosts a,b,c` (batch) mutually-exclusive flag set
//! used by `env set` / `ini set` / `ini remove`.

use crate::error::{UecmError, UecmResult};
use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct HostArgs {
    /// Single host. Mutually exclusive with --hosts.
    #[arg(long, group = "target", value_name = "HOST")]
    pub host: Option<String>,

    /// Comma-separated host list. Mutually exclusive with --host.
    #[arg(
        long,
        group = "target",
        value_name = "H1,H2,...",
        value_delimiter = ','
    )]
    pub hosts: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HostTarget {
    Single(String),
    Batch(Vec<String>),
}

impl HostArgs {
    /// Require exactly one of --host / --hosts. `clap` group already enforces
    /// mutex; this catches the "neither supplied" case.
    pub fn require_one(&self) -> UecmResult<HostTarget> {
        match (&self.host, &self.hosts) {
            (Some(h), None) => Ok(HostTarget::Single(h.clone())),
            (None, Some(hs)) if !hs.is_empty() => Ok(HostTarget::Batch(hs.clone())),
            (None, Some(_)) => Err(UecmError::InvalidInput(
                "--hosts requires at least one host".into(),
            )),
            (None, None) => Err(UecmError::InvalidInput(
                "one of --host or --hosts is required".into(),
            )),
            (Some(_), Some(_)) => unreachable!("clap group 'target' enforces mutex"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_host_returns_single_variant() {
        let args = HostArgs { host: Some("a".into()), hosts: None };
        assert_eq!(args.require_one().unwrap(), HostTarget::Single("a".into()));
    }

    #[test]
    fn hosts_list_returns_batch_variant() {
        let args = HostArgs { host: None, hosts: Some(vec!["a".into(), "b".into()]) };
        match args.require_one().unwrap() {
            HostTarget::Batch(v) => assert_eq!(v, vec!["a".to_string(), "b".to_string()]),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn empty_returns_invalid_input() {
        let args = HostArgs { host: None, hosts: None };
        assert!(matches!(args.require_one(), Err(UecmError::InvalidInput(_))));
    }

    #[test]
    fn empty_hosts_vec_returns_invalid_input() {
        let args = HostArgs { host: None, hosts: Some(vec![]) };
        assert!(matches!(args.require_one(), Err(UecmError::InvalidInput(_))));
    }
}
```

- [ ] **Step 2: Declare module in `cli/mod.rs`**

Add `pub mod host_args;` next to `pub mod credential_args;`.

- [ ] **Step 3: Run tests + commit**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib cli::host_args::
```
Expected: 4 tests pass.

```bash
git add src-tauri/src/cli/host_args.rs src-tauri/src/cli/mod.rs
git commit -m "feat(cli): shared HostArgs with --host / --hosts mutex"
```

---

## Phase 1 — Argument tree + dispatch wiring

### Task 1.1: Extend `cli::args` with 4 new domains

**Files:**
- Modify: `src-tauri/src/cli/args.rs`
- Modify: `src-tauri/src/cli/mod.rs`
- Create: 4 stub handler files

- [ ] **Step 1: Extend `Domain` enum + add 4 ActionEnums in `cli/args.rs`**

Open `src-tauri/src/cli/args.rs`. Add 4 new variants to `Domain` (after the existing `Winrm` variant):

```rust
#[derive(Subcommand, Debug)]
pub enum Domain {
    System { #[command(subcommand)] action: SystemAction },
    Machine { #[command(subcommand)] action: MachineAction },
    Winrm { #[command(subcommand)] action: WinrmAction },

    /// Credential storage (DPAPI + cmdkey + SQLite metadata).
    Cred { #[command(subcommand)] action: CredAction },
    /// Read / write system-level environment variables on remote hosts.
    Env { #[command(subcommand)] action: EnvAction },
    /// Read / write / remove single INI keys on remote hosts.
    Ini { #[command(subcommand)] action: IniAction },
    /// SMB share inventory + creation + SYSTEM credential injection.
    Share { #[command(subcommand)] action: ShareAction },
}
```

Append the 4 new ActionEnums to the same file (top-level, after `WinrmAction`):

```rust
// ---------- cred ----------
#[derive(Subcommand, Debug)]
pub enum CredAction {
    /// List saved credential aliases.
    List,
    /// Save a credential (cmdkey + DPAPI + SQLite metadata).
    Save {
        #[arg(long)]
        alias: String,
        #[arg(long)]
        user: String,
        /// Password (leaks into shell history; prefer --pass-stdin).
        #[arg(long, group = "secret", conflicts_with = "pass_stdin")]
        pass: Option<String>,
        /// Read password from stdin (one line).
        #[arg(long, group = "secret", conflicts_with = "pass")]
        pass_stdin: bool,
        /// Credential kind label stored alongside the alias.
        #[arg(long, default_value = "winrm")]
        kind: String,
    },
    /// Delete a credential alias.
    Delete {
        alias: String,
    },
}

// ---------- env ----------
#[derive(Subcommand, Debug)]
pub enum EnvAction {
    /// Read an environment variable on a single host.
    Get {
        #[arg(long)]
        host: String,
        #[arg(long)]
        name: String,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// Write an environment variable on one or more hosts.
    Set {
        #[command(flatten)]
        target: crate::cli::host_args::HostArgs,
        #[arg(long)]
        name: String,
        #[arg(long)]
        value: String,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
}

// ---------- ini ----------
#[derive(Subcommand, Debug)]
pub enum IniAction {
    /// Read all keys from one INI section on a single host.
    Read {
        #[arg(long)]
        host: String,
        #[arg(long)]
        file: String,
        #[arg(long)]
        section: String,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// Write a single INI key on one or more hosts.
    Set {
        #[command(flatten)]
        target: crate::cli::host_args::HostArgs,
        #[arg(long)]
        file: String,
        #[arg(long)]
        section: String,
        #[arg(long)]
        key: String,
        #[arg(long)]
        value: String,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// Remove a single INI key on one or more hosts.
    Remove {
        #[command(flatten)]
        target: crate::cli::host_args::HostArgs,
        #[arg(long)]
        file: String,
        #[arg(long)]
        section: String,
        #[arg(long)]
        key: String,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
}

// ---------- share ----------
#[derive(Subcommand, Debug)]
pub enum ShareAction {
    /// List share configs in the local inventory.
    List,
    /// Forget a share config (LOCAL inventory only; remote SMB share is NOT removed).
    Forget {
        id: i64,
        #[arg(long)]
        yes: bool,
    },
    /// Create an SMB share (Mode A = open Guest+Everyone; Mode B = dedicated ddc-svc).
    Create {
        #[arg(long, value_name = "a|b")]
        mode: String,
        #[arg(long)]
        host: String,
        #[arg(long)]
        share: String,
        #[arg(long)]
        local_path: String,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// Inject the share's SYSTEM-context credential on a client machine so
    /// LocalSystem services (e.g. RenderStream) can mount the share.
    InjectSystemCred {
        #[arg(long)]
        client_host: String,
        #[arg(long)]
        target_host: String,
        #[arg(long, default_value = "ddc-svc")]
        svc_user: String,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
}
```

- [ ] **Step 2: Add 4 unit tests to the existing `mod tests` block in `args.rs`**

Append inside `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn parses_cred_save_with_alias_and_user() {
        let cli = Cli::try_parse_from([
            "uecm-cli", "cred", "save",
            "--alias", "winrm-admin",
            "--user", "Administrator",
            "--pass-stdin",
        ]).unwrap();
        match cli.command {
            Domain::Cred { action: CredAction::Save { alias, user, pass, pass_stdin, .. } } => {
                assert_eq!(alias, "winrm-admin");
                assert_eq!(user, "Administrator");
                assert_eq!(pass, None);
                assert!(pass_stdin);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn cred_save_rejects_both_pass_and_pass_stdin() {
        let r = Cli::try_parse_from([
            "uecm-cli", "cred", "save",
            "--alias", "a", "--user", "u",
            "--pass", "p", "--pass-stdin",
        ]);
        assert!(r.is_err());
    }

    #[test]
    fn env_set_rejects_both_host_and_hosts() {
        let r = Cli::try_parse_from([
            "uecm-cli", "env", "set",
            "--host", "a", "--hosts", "b,c",
            "--name", "X", "--value", "Y",
        ]);
        assert!(r.is_err());
    }

    #[test]
    fn env_set_accepts_hosts_list() {
        let cli = Cli::try_parse_from([
            "uecm-cli", "env", "set",
            "--hosts", "a,b,c",
            "--name", "X", "--value", "Y",
        ]).unwrap();
        match cli.command {
            Domain::Env { action: EnvAction::Set { target, name, value, .. } } => {
                assert_eq!(target.hosts, Some(vec!["a".into(), "b".into(), "c".into()]));
                assert_eq!(name, "X");
                assert_eq!(value, "Y");
            }
            _ => panic!("wrong variant"),
        }
    }
```

- [ ] **Step 3: Create 4 stub handler files**

Create each of these with a stub `handle` that returns `OperationFailed`. They'll be filled in Phase 2-5.

`src-tauri/src/cli/domain_cred.rs`:
```rust
//! `uecm-cli cred <action>` handlers — implemented in Phase 2.

use crate::cli::args::CredAction;
use crate::cli::run::Ctx;
use crate::error::{UecmError, UecmResult};

pub fn handle(_ctx: &mut Ctx<'_>, _action: CredAction) -> UecmResult<()> {
    Err(UecmError::OperationFailed("cred: not yet implemented".into()))
}
```

Same shape for `domain_env.rs` (`EnvAction`), `domain_ini.rs` (`IniAction`), `domain_share.rs` (`ShareAction`).

- [ ] **Step 4: Declare 4 new modules in `cli/mod.rs`**

In `src-tauri/src/cli/mod.rs`, after the existing 3 `domain_*` declarations:

```rust
pub mod domain_cred;
pub mod domain_env;
pub mod domain_ini;
pub mod domain_share;
```

- [ ] **Step 5: Build + run tests**

```bash
cargo build --manifest-path src-tauri/Cargo.toml --bin uecm-cli
cargo test --manifest-path src-tauri/Cargo.toml --lib cli::args::
```
Expected: build clean; 8 cli::args tests pass (4 from Plan 1 + 4 new).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/cli/args.rs src-tauri/src/cli/mod.rs \
        src-tauri/src/cli/domain_cred.rs src-tauri/src/cli/domain_env.rs \
        src-tauri/src/cli/domain_ini.rs src-tauri/src/cli/domain_share.rs
git commit -m "feat(cli): args + stub handlers for cred/env/ini/share domains"
```

---

### Task 1.2: Dispatch wiring + `needs_db`

**Files:**
- Modify: `src-tauri/src/cli/run.rs`

- [ ] **Step 1: Extend `needs_db()` in `run.rs`**

Find `fn needs_db` and replace the body:

```rust
fn needs_db(cmd: &Domain) -> bool {
    use crate::cli::args::{MachineAction, SystemAction};
    match cmd {
        // `machine scan` is a stateless network probe — no DB writes.
        Domain::Machine { action } => !matches!(action, MachineAction::Scan { .. }),
        // `system echo` and the path-printing variants don't open DB;
        // only `migrate-db` does.
        Domain::System { action } => matches!(action, SystemAction::MigrateDb),
        // None of the WinRM commands touch SQLite.
        Domain::Winrm { .. } => false,
        // Plan 2 — all of these read or write SQLite (cred alias resolution,
        // share inventory, env/ini history via operations log).
        Domain::Cred { .. } => true,
        Domain::Env { .. } => true,
        Domain::Ini { .. } => true,
        Domain::Share { .. } => true,
    }
}
```

- [ ] **Step 2: Add 4 dispatch arms in `run::run`**

In the `match cli.command { ... }` block inside `pub fn run`, add 4 arms after the existing `Domain::Winrm` arm:

```rust
    let result = match cli.command {
        Domain::System { action } => domain_system::handle(&mut ctx, action),
        Domain::Machine { action } => domain_machine::handle(&mut ctx, action),
        Domain::Winrm { action } => domain_winrm::handle(&mut ctx, action),
        Domain::Cred { action } => domain_cred::handle(&mut ctx, action),
        Domain::Env { action } => domain_env::handle(&mut ctx, action),
        Domain::Ini { action } => domain_ini::handle(&mut ctx, action),
        Domain::Share { action } => domain_share::handle(&mut ctx, action),
    };
```

- [ ] **Step 3: Extend the `use` for the new domain modules**

Update the existing `use crate::cli::{domain_machine, domain_system, domain_winrm};` to:

```rust
use crate::cli::{
    domain_cred, domain_env, domain_ini, domain_machine, domain_share,
    domain_system, domain_winrm,
};
```

- [ ] **Step 4: Build + smoke test**

```bash
cargo build --manifest-path src-tauri/Cargo.toml --bin uecm-cli
src-tauri/target/debug/uecm-cli --json cred list
```
Expected: stdout has one line with `"kind":"error","code":"operation_failed","message":"...cred: not yet implemented"`; exit 1. This proves dispatch works.

```bash
src-tauri/target/debug/uecm-cli --help | head -25
```
Expected: clap lists 7 subcommands (`system`, `machine`, `winrm`, `cred`, `env`, `ini`, `share`).

- [ ] **Step 5: Run all tests + commit**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: previous test count + 8 new (3 credential_args + 4 host_args + 4 args parsing minus 3 dupe in count — actual delta should be roughly +11; just confirm all green).

```bash
git add src-tauri/src/cli/run.rs
git commit -m "feat(cli): wire dispatch + needs_db for cred/env/ini/share"
```

---

## Phase 2 — `cred` domain

### Task 2.1: `cred list`

**Files:**
- Modify: `src-tauri/src/cli/domain_cred.rs`

- [ ] **Step 1: Replace the stub with dispatch + `list` handler**

```rust
//! `uecm-cli cred <action>` handlers.

use crate::cli::args::CredAction;
use crate::cli::run::Ctx;
use crate::cli::EmitSerialize;
use crate::data::credentials as data_creds;
use crate::error::{UecmError, UecmResult};

pub fn handle(ctx: &mut Ctx<'_>, action: CredAction) -> UecmResult<()> {
    match action {
        CredAction::List => list(ctx),
        CredAction::Save { .. } => Err(UecmError::OperationFailed("save: pending Task 2.2".into())),
        CredAction::Delete { .. } => {
            Err(UecmError::OperationFailed("delete: pending Task 2.3".into()))
        }
    }
}

fn list(ctx: &mut Ctx<'_>) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let rows = data_creds::list_all(db)?;
    ctx.emitter.emit_result(&rows).ok();
    Ok(())
}
```

If `data::credentials::list_all` is named differently (grep `src-tauri/src/data/credentials.rs` to confirm), use the actual name. Common candidates: `list_all`, `list`, `all`.

- [ ] **Step 2: Smoke test against fresh DB**

```bash
cargo build --manifest-path src-tauri/Cargo.toml --bin uecm-cli
TMP_DB="$(mktemp -t uecm-test.XXXXXX.sqlite)"
UECM_DB_PATH="$TMP_DB" src-tauri/target/debug/uecm-cli --json cred list
```
Expected: stdout has `[]` (empty array, fresh DB).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/cli/domain_cred.rs
git commit -m "feat(cli): cred list"
```

---

### Task 2.2: `cred save` — transactional invariants

**Files:**
- Modify: `src-tauri/src/cli/domain_cred.rs`

Spec §5.1.1 makes this a 5-step MUST. Mirror `commands/credentials.rs::save_credential` exactly.

- [ ] **Step 1: Read existing UI handler for sequence reference**

```bash
sed -n '16,55p' src-tauri/src/commands/credentials.rs
```
Confirm the 5-step shape (normalize → cmdkey store → DPAPI store w/ rollback → find_by_alias replace → insert).

- [ ] **Step 2: Replace `CredAction::Save` arm + implement `save`**

In `domain_cred.rs`, change the dispatch:

```rust
        CredAction::Save { alias, user, pass, pass_stdin, kind } => {
            save(ctx, &alias, &user, pass.as_deref(), pass_stdin, &kind)
        }
```

Add helper functions:

```rust
use std::io::{self, BufRead};

fn read_password(pass_inline: Option<&str>, pass_stdin: bool) -> UecmResult<String> {
    if let Some(p) = pass_inline {
        return Ok(p.to_string());
    }
    if pass_stdin {
        let mut line = String::new();
        io::stdin().lock().read_line(&mut line).map_err(|e| {
            UecmError::InvalidInput(format!("read password from stdin: {}", e))
        })?;
        return Ok(line.trim_end_matches(['\r', '\n']).to_string());
    }
    Err(UecmError::InvalidInput(
        "either --pass or --pass-stdin is required".into(),
    ))
}

fn save(
    ctx: &mut Ctx<'_>,
    alias: &str,
    user: &str,
    pass_inline: Option<&str>,
    pass_stdin: bool,
    kind: &str,
) -> UecmResult<()> {
    use crate::core::credentials as core_creds;
    use crate::data::credentials::CredentialRecord;

    let password = read_password(pass_inline, pass_stdin)?;
    let username = core_creds::normalize_username_for_storage(user);

    // Step 1+2: cmdkey first. If this fails, nothing else gets written.
    core_creds::store(alias, &username, &password)?;

    // Step 3: DPAPI. If it fails, roll back cmdkey before propagating.
    if let Err(dpapi_err) = core_creds::store_password(alias, &password) {
        if let Err(rollback_err) = core_creds::delete(alias) {
            tracing::warn!(
                alias = %alias,
                error = %rollback_err,
                "cmdkey rollback after DPAPI failure also failed"
            );
        }
        return Err(dpapi_err);
    }

    // Step 4+5: SQLite. Replace if alias already exists, else insert.
    let db = ctx.require_db()?;
    if data_creds::find_by_alias(db, alias)?.is_some() {
        data_creds::delete_by_alias(db, alias)?;
    }
    let record = CredentialRecord {
        id: None,
        alias: alias.to_string(),
        kind: kind.parse().unwrap_or_default(),
        username,
    };
    let id = data_creds::insert(db, &record)?;

    ctx.emitter
        .emit_event(&crate::cli::output::Event::Completed {
            summary: serde_json::json!({ "id": id, "alias": alias }),
        })
        .ok();
    Ok(())
}
```

Note on `kind.parse().unwrap_or_default()`: `CredentialRecord::kind` is likely an enum. Grep:

```bash
grep -nE "pub enum CredentialKind|pub struct CredentialRecord" src-tauri/src/data/credentials.rs
```

If `CredentialKind` has `impl FromStr` and a `Default`, the line above works. If not, replace with a hardcoded `CredentialKind::Winrm` (or whatever the default variant is). Pick what compiles.

- [ ] **Step 3: Add unit tests at the bottom of `domain_cred.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::output::{Emitter, NdjsonEmitter};
    use crate::data::{open_in_memory, schema};

    fn make_ctx<'a>(buf: &'a mut Vec<u8>, db: &'a crate::data::Db) -> Ctx<'a> {
        let emitter: Box<dyn Emitter> = Box::new(NdjsonEmitter::new(buf));
        Ctx {
            db: Some(db.clone()),
            db_path: std::path::PathBuf::from(":memory:"),
            emitter,
            json_mode: true,
        }
    }

    fn fresh_db() -> crate::data::Db {
        let db = open_in_memory().unwrap();
        {
            let mut conn = db.lock().unwrap();
            schema::migrate(&mut conn).unwrap();
        }
        db
    }

    #[cfg(not(windows))]
    #[test]
    fn save_returns_powershell_error_when_cmdkey_unavailable() {
        let db = fresh_db();
        let mut buf: Vec<u8> = Vec::new();
        let mut ctx = make_ctx(&mut buf, &db);
        let result = save(&mut ctx, "alias", "u", Some("p"), false, "winrm");
        // cmdkey is the first remote step; on non-Windows it fails as PowerShell.
        assert!(matches!(result, Err(UecmError::PowerShell(_))));
        // SQLite must remain empty: no metadata written when cmdkey fails.
        assert_eq!(data_creds::list_all(&db).unwrap().len(), 0);
    }
}
```

Note: `Db = Arc<Mutex<Connection>>` so `.clone()` is cheap. If `Ctx::db` is `Option<Db>` rather than `Option<&Db>`, the `make_ctx` above is correct as written.

- [ ] **Step 4: Build + test**

```bash
cargo build --manifest-path src-tauri/Cargo.toml --bin uecm-cli
cargo test --manifest-path src-tauri/Cargo.toml --lib cli::domain_cred::
```
Expected: 1 test passes on macOS; on Windows the test is skipped.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/cli/domain_cred.rs
git commit -m "feat(cli): cred save with 5-step transactional sequence"
```

---

### Task 2.3: `cred delete` — best-effort cleanup

**Files:**
- Modify: `src-tauri/src/cli/domain_cred.rs`

- [ ] **Step 1: Replace `CredAction::Delete` arm**

```rust
        CredAction::Delete { alias } => delete(ctx, &alias),
```

Add helper:

```rust
fn delete(ctx: &mut Ctx<'_>, alias: &str) -> UecmResult<()> {
    use crate::core::credentials as core_creds;

    // 1. cmdkey delete — keep result, propagate at the end.
    let cm_result = core_creds::delete(alias);

    // 2. SQLite delete — environment error if this fails, propagate now.
    let db = ctx.require_db()?;
    data_creds::delete_by_alias(db, alias)?;

    // 3. DPAPI best-effort — orphan key is harmless (never resolved).
    if let Err(e) = core_creds::delete_password(alias) {
        tracing::warn!(
            alias = %alias,
            error = %e,
            "DPAPI delete_password failed; orphan entry will remain in creds.bin"
        );
    }

    // 4. Surface cmdkey result so user knows if Credential Manager is dirty.
    cm_result.map(|_| {
        let _ = ctx.emitter.emit_event(&crate::cli::output::Event::Completed {
            summary: serde_json::json!({ "alias": alias, "deleted": true }),
        });
    })
}
```

- [ ] **Step 2: Smoke test**

```bash
cargo build --manifest-path src-tauri/Cargo.toml --bin uecm-cli
TMP_DB="$(mktemp -t uecm-test.XXXXXX.sqlite)"
UECM_DB_PATH="$TMP_DB" src-tauri/target/debug/uecm-cli --json cred delete nonexistent
```
Expected on macOS: error event with `code: "powershell_failed"` (cmdkey can't run); exit 4. SQLite delete succeeds silently because the row didn't exist. On Windows: completed event with `deleted:true` if alias didn't exist (cmdkey returns OK on no-op).

- [ ] **Step 3: Run all lib tests + commit**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib
```
Expected: green.

```bash
git add src-tauri/src/cli/domain_cred.rs
git commit -m "feat(cli): cred delete with best-effort cleanup"
```

---

## Phase 3 — `env` domain

### Task 3.1: `env get` (single host)

**Files:**
- Modify: `src-tauri/src/cli/domain_env.rs`

- [ ] **Step 1: Replace the stub with dispatch + `get` handler**

```rust
//! `uecm-cli env <action>` handlers.

use crate::cli::args::EnvAction;
use crate::cli::credential_args::CredentialArgs;
use crate::cli::host_args::HostTarget;
use crate::cli::run::Ctx;
use crate::cli::EmitSerialize;
use crate::core::env_vars;
use crate::error::{UecmError, UecmResult};
use serde::Serialize;

pub fn handle(ctx: &mut Ctx<'_>, action: EnvAction) -> UecmResult<()> {
    match action {
        EnvAction::Get { host, name, cred } => get(ctx, &host, &name, &cred),
        EnvAction::Set { .. } => Err(UecmError::OperationFailed("set: pending Task 3.2/3.3".into())),
    }
}

#[derive(Serialize)]
struct EnvGetOut<'a> {
    host: &'a str,
    name: &'a str,
    value: Option<String>,
}

fn get(ctx: &mut Ctx<'_>, host: &str, name: &str, cred: &CredentialArgs) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let creds = cred.resolve(db)?;
    let value = match creds {
        Some((u, p)) => env_vars::get_with_credential(host, name, &u, &p)?,
        None => env_vars::get(host, name)?,
    };
    ctx.emitter
        .emit_result(&EnvGetOut { host, name, value })
        .ok();
    Ok(())
}
```

- [ ] **Step 2: Smoke test (macOS — expected fail)**

```bash
cargo build --manifest-path src-tauri/Cargo.toml --bin uecm-cli
src-tauri/target/debug/uecm-cli --json env get --host 192.0.2.1 --name PATH
```
Expected on macOS: error event `code: "powershell_failed"`, exit 4. On Windows reaching a real host with admin creds: `{"host":"...","name":"PATH","value":"C:\\..."}`.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/cli/domain_env.rs
git commit -m "feat(cli): env get (single host)"
```

---

### Task 3.2: `env set` (single host)

**Files:**
- Modify: `src-tauri/src/cli/domain_env.rs`

- [ ] **Step 1: Add `set` arm dispatch + helper for single host**

Add a SHA helper at the top of the file:

```rust
use sha2::{Digest, Sha256};

fn value_sha256_prefix(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    hex::encode(&digest[..4]) // 8 hex chars
}
```

`hex` may not be in deps yet. Check `Cargo.toml`; if absent, replace the body with a manual loop:

```rust
fn value_sha256_prefix(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    let mut out = String::with_capacity(8);
    for b in &digest[..4] {
        use std::fmt::Write as _;
        write!(out, "{:02x}", b).unwrap();
    }
    out
}
```

Replace the `Set { .. }` arm:

```rust
        EnvAction::Set { target, name, value, cred } => {
            use crate::cli::host_args::HostArgs;
            // Keep `target` around so we can match the target enum.
            let t = HostArgs { host: target.host.clone(), hosts: target.hosts.clone() }
                .require_one()?;
            match t {
                HostTarget::Single(h) => set_single(ctx, &h, &name, &value, &cred),
                HostTarget::Batch(_) => Err(UecmError::OperationFailed(
                    "batch set: pending Task 3.3".into(),
                )),
            }
        }
```

Add `set_single`:

```rust
fn set_single(
    ctx: &mut Ctx<'_>,
    host: &str,
    name: &str,
    value: &str,
    cred: &CredentialArgs,
) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let creds = cred.resolve(db)?;
    match creds {
        Some((u, p)) => env_vars::set_with_credential(host, name, value, &u, &p)?,
        None => env_vars::set(host, name, value)?,
    }
    // Redaction contract (spec §8): never echo `value`.
    ctx.emitter
        .emit_event(&crate::cli::output::Event::Completed {
            summary: serde_json::json!({
                "host": host,
                "name": name,
                "value_len": value.chars().count(),
                "value_sha256_prefix": value_sha256_prefix(value),
            }),
        })
        .ok();
    Ok(())
}
```

- [ ] **Step 2: Smoke test (macOS expected fail)**

```bash
cargo build --manifest-path src-tauri/Cargo.toml --bin uecm-cli
src-tauri/target/debug/uecm-cli --json env set \
  --host 192.0.2.1 --name MY_VAR --value some-value
```
Expected: error event `powershell_failed`, exit 4. Verify the error message **does NOT contain** `some-value` — redaction holds.

```bash
src-tauri/target/debug/uecm-cli --json env set \
  --host 192.0.2.1 --name MY_VAR --value SECRET123 2>&1 | grep -c SECRET123
```
Expected: `0` (the value must not leak into stdout or stderr).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/cli/domain_env.rs
git commit -m "feat(cli): env set single host (with redaction)"
```

---

### Task 3.3: `env set --hosts` batch with NDJSON

**Files:**
- Modify: `src-tauri/src/cli/domain_env.rs`

- [ ] **Step 1: Replace the batch arm with a real handler**

Replace the `HostTarget::Batch(_)` arm in the `Set` dispatch:

```rust
                HostTarget::Batch(hs) => set_batch(ctx, &hs, &name, &value, &cred),
```

Add the batch helper. Use a synchronous loop because `env_vars::set_with_credential` is sync (not async); the spec's "max concurrency 8" is a soft cap that doesn't add value for purely sync remote calls within a Tokio current-thread runtime. Document this:

```rust
fn set_batch(
    ctx: &mut Ctx<'_>,
    hosts: &[String],
    name: &str,
    value: &str,
    cred: &CredentialArgs,
) -> UecmResult<()> {
    use crate::cli::output::Event;

    let db = ctx.require_db()?;
    let creds = cred.resolve(db)?;
    let total = hosts.len() as i64;

    ctx.emitter
        .emit_event(&Event::Started {
            task_type: "env_set".into(),
            task_id: None,
            metadata: serde_json::json!({
                "hosts": total,
                "name": name,
                "value_len": value.chars().count(),
                "value_sha256_prefix": value_sha256_prefix(value),
            }),
        })
        .ok();

    let mut ok_count: i64 = 0;
    let mut fail_count: i64 = 0;
    // Sequential — env_vars::set_with_credential is blocking and there's no
    // real win from spawning tasks when each call blocks on PowerShell. If
    // future profiling shows wall-clock pain we can revisit with core::batch::run_batch.
    for (idx, host) in hosts.iter().enumerate() {
        ctx.emitter
            .emit_event(&Event::ItemStarted {
                item_id: host.clone(),
                index: idx as i64,
                total,
            })
            .ok();

        let res = match &creds {
            Some((u, p)) => env_vars::set_with_credential(host, name, value, u, p),
            None => env_vars::set(host, name, value),
        };

        match res {
            Ok(()) => {
                ok_count += 1;
                ctx.emitter
                    .emit_event(&Event::ItemCompleted {
                        item_id: host.clone(),
                        index: idx as i64,
                        ok: true,
                        message: None,
                    })
                    .ok();
            }
            Err(e) => {
                fail_count += 1;
                // Redact: if the message contains our value, sub in a marker.
                // Belt-and-suspenders; the underlying script shouldn't echo
                // values, but we don't audit them all.
                let mut msg = e.to_string();
                if value.len() >= 4 && msg.contains(value) {
                    msg = msg.replace(value, "[REDACTED:value]");
                }
                ctx.emitter
                    .emit_event(&Event::ItemCompleted {
                        item_id: host.clone(),
                        index: idx as i64,
                        ok: false,
                        message: Some(msg),
                    })
                    .ok();
            }
        }
    }

    ctx.emitter
        .emit_event(&Event::Completed {
            summary: serde_json::json!({
                "hosts": total,
                "ok": ok_count,
                "failed": fail_count,
            }),
        })
        .ok();

    if fail_count > 0 {
        return Err(UecmError::OperationFailed(format!(
            "{}/{} hosts failed env set",
            fail_count, total
        )));
    }
    Ok(())
}
```

- [ ] **Step 2: Add unit test for the batch event lifecycle**

Append at the bottom of `domain_env.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::output::{Emitter, NdjsonEmitter};
    use crate::data::{open_in_memory, schema, Db};

    fn make_ctx<'a>(buf: &'a mut Vec<u8>, db: &'a Db) -> Ctx<'a> {
        let emitter: Box<dyn Emitter> = Box::new(NdjsonEmitter::new(buf));
        Ctx {
            db: Some(db.clone()),
            db_path: std::path::PathBuf::from(":memory:"),
            emitter,
            json_mode: true,
        }
    }

    fn fresh_db() -> Db {
        let db = open_in_memory().unwrap();
        {
            let mut conn = db.lock().unwrap();
            schema::migrate(&mut conn).unwrap();
        }
        db
    }

    #[cfg(not(windows))]
    #[test]
    fn set_hosts_emits_full_lifecycle_with_no_value_leak() {
        let db = fresh_db();
        let mut buf: Vec<u8> = Vec::new();
        let mut ctx = make_ctx(&mut buf, &db);
        let cred = CredentialArgs {
            cred_alias: None, user: None, pass: None, pass_stdin: false,
        };
        let secret = "SECRET-VALUE-XYZ-NEVER-LEAK";
        let _ = set_batch(
            &mut ctx,
            &["192.0.2.1".into(), "192.0.2.2".into()],
            "DUMMY",
            secret,
            &cred,
        );
        let s = String::from_utf8(buf).unwrap();
        // Started + 2 item_started + 2 item_completed + completed = 6 lines.
        assert_eq!(s.lines().count(), 6, "stream: {}", s);
        assert!(s.contains("\"kind\":\"started\""));
        assert!(s.contains("\"kind\":\"item_started\""));
        assert!(s.contains("\"kind\":\"item_completed\""));
        assert!(s.contains("\"kind\":\"completed\""));
        // Redaction MUST hold even on the error path.
        assert!(!s.contains(secret), "value leaked into NDJSON: {}", s);
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib cli::domain_env::
```
Expected on macOS: 1 test passes.

- [ ] **Step 4: E2E smoke**

```bash
src-tauri/target/debug/uecm-cli --json env set \
  --hosts 192.0.2.1,192.0.2.2 \
  --name X --value SECRET-TOPSECRET 2>&1 | tee /tmp/batch.ndjson
grep -c SECRET-TOPSECRET /tmp/batch.ndjson
```
Expected: 0 occurrences of `SECRET-TOPSECRET`; the stream has `started`, two `item_started`/`item_completed`, and a `completed` event.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/cli/domain_env.rs
git commit -m "feat(cli): env set --hosts batch with NDJSON and redaction"
```

---

## Phase 4 — `ini` domain

### Task 4.1: `ini read` / `ini set` / `ini remove` (single host)

**Files:**
- Modify: `src-tauri/src/cli/domain_ini.rs`

- [ ] **Step 1: Replace the stub with dispatch + 3 single-host handlers**

```rust
//! `uecm-cli ini <action>` handlers.

use crate::cli::args::IniAction;
use crate::cli::credential_args::CredentialArgs;
use crate::cli::host_args::HostTarget;
use crate::cli::run::Ctx;
use crate::cli::EmitSerialize;
use crate::core::ini_editor;
use crate::error::{UecmError, UecmResult};
use serde::Serialize;
use sha2::{Digest, Sha256};

fn value_sha256_prefix(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    let mut out = String::with_capacity(8);
    for b in &digest[..4] {
        use std::fmt::Write as _;
        write!(out, "{:02x}", b).unwrap();
    }
    out
}

pub fn handle(ctx: &mut Ctx<'_>, action: IniAction) -> UecmResult<()> {
    match action {
        IniAction::Read { host, file, section, cred } => {
            read(ctx, &host, &file, &section, &cred)
        }
        IniAction::Set { target, file, section, key, value, cred } => {
            let t = target.require_one()?;
            match t {
                HostTarget::Single(h) => set_single(ctx, &h, &file, &section, &key, &value, &cred),
                HostTarget::Batch(_) => Err(UecmError::OperationFailed(
                    "batch ini set: pending Task 4.2".into(),
                )),
            }
        }
        IniAction::Remove { target, file, section, key, cred } => {
            let t = target.require_one()?;
            match t {
                HostTarget::Single(h) => remove_single(ctx, &h, &file, &section, &key, &cred),
                HostTarget::Batch(_) => Err(UecmError::OperationFailed(
                    "batch ini remove: pending Task 4.2".into(),
                )),
            }
        }
    }
}

#[derive(Serialize)]
struct IniReadOut<'a> {
    host: &'a str,
    file: &'a str,
    section: &'a str,
    keys: Vec<ini_editor::IniKey>,
}

fn read(
    ctx: &mut Ctx<'_>,
    host: &str,
    file: &str,
    section: &str,
    cred: &CredentialArgs,
) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let creds = cred.resolve(db)?;
    let keys = match creds {
        Some((u, p)) => ini_editor::read_section_with_credential(host, file, section, &u, &p)?,
        None => ini_editor::read_section(host, file, section)?,
    };
    ctx.emitter
        .emit_result(&IniReadOut { host, file, section, keys })
        .ok();
    Ok(())
}

fn set_single(
    ctx: &mut Ctx<'_>,
    host: &str,
    file: &str,
    section: &str,
    key: &str,
    value: &str,
    cred: &CredentialArgs,
) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let creds = cred.resolve(db)?;
    match creds {
        Some((u, p)) => ini_editor::set_key_with_credential(host, file, section, key, value, &u, &p)?,
        None => ini_editor::set_key(host, file, section, key, value)?,
    }
    ctx.emitter
        .emit_event(&crate::cli::output::Event::Completed {
            summary: serde_json::json!({
                "host": host,
                "file": file,
                "section": section,
                "key": key,
                "value_len": value.chars().count(),
                "value_sha256_prefix": value_sha256_prefix(value),
            }),
        })
        .ok();
    Ok(())
}

fn remove_single(
    ctx: &mut Ctx<'_>,
    host: &str,
    file: &str,
    section: &str,
    key: &str,
    cred: &CredentialArgs,
) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let creds = cred.resolve(db)?;
    let (u, p) = creds.ok_or_else(|| {
        UecmError::InvalidInput(
            "ini remove requires credentials (--cred-alias or --user --pass)".into(),
        )
    })?;
    ini_editor::remove_key_with_credential(host, file, section, key, &u, &p)?;
    ctx.emitter
        .emit_event(&crate::cli::output::Event::Completed {
            summary: serde_json::json!({
                "host": host,
                "file": file,
                "section": section,
                "key": key,
                "removed": true,
            }),
        })
        .ok();
    Ok(())
}
```

Notes:
- `ini_editor::remove_key_with_credential` exists; the host-only `remove_key` may not. If not, require credentials for remove (as above) — that's spec-consistent because `remove` is destructive and we want explicit auth.
- The `IniKey` struct from `core::ini_editor` derives `Serialize` (verify with `grep -nE "pub struct IniKey" src-tauri/src/core/ini_editor.rs` — should show a `#[derive(Serialize, ...)]` line).

- [ ] **Step 2: Smoke test (macOS expected fail)**

```bash
cargo build --manifest-path src-tauri/Cargo.toml --bin uecm-cli
src-tauri/target/debug/uecm-cli --json ini set \
  --host 192.0.2.1 --file C:\\test.ini --section S --key K --value SECRET-INI 2>&1 | grep -c SECRET-INI
```
Expected: 0 (redaction holds).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/cli/domain_ini.rs
git commit -m "feat(cli): ini read / set / remove (single host)"
```

---

### Task 4.2: `ini set --hosts` + `ini remove --hosts` batch

**Files:**
- Modify: `src-tauri/src/cli/domain_ini.rs`

- [ ] **Step 1: Replace the 2 `HostTarget::Batch(_)` arms with real handlers**

In the `handle` match, change:

```rust
                HostTarget::Batch(hs) => set_batch(ctx, &hs, &file, &section, &key, &value, &cred),
```

and:

```rust
                HostTarget::Batch(hs) => remove_batch(ctx, &hs, &file, &section, &key, &cred),
```

Add helpers:

```rust
fn set_batch(
    ctx: &mut Ctx<'_>,
    hosts: &[String],
    file: &str,
    section: &str,
    key: &str,
    value: &str,
    cred: &CredentialArgs,
) -> UecmResult<()> {
    use crate::cli::output::Event;

    let db = ctx.require_db()?;
    let creds = cred.resolve(db)?;
    let total = hosts.len() as i64;

    ctx.emitter
        .emit_event(&Event::Started {
            task_type: "ini_set".into(),
            task_id: None,
            metadata: serde_json::json!({
                "hosts": total,
                "file": file,
                "section": section,
                "key": key,
                "value_len": value.chars().count(),
                "value_sha256_prefix": value_sha256_prefix(value),
            }),
        })
        .ok();

    let mut ok_count: i64 = 0;
    let mut fail_count: i64 = 0;
    for (idx, host) in hosts.iter().enumerate() {
        ctx.emitter
            .emit_event(&Event::ItemStarted {
                item_id: host.clone(),
                index: idx as i64,
                total,
            })
            .ok();
        let res = match &creds {
            Some((u, p)) => ini_editor::set_key_with_credential(host, file, section, key, value, u, p),
            None => ini_editor::set_key(host, file, section, key, value),
        };
        match res {
            Ok(()) => {
                ok_count += 1;
                ctx.emitter
                    .emit_event(&Event::ItemCompleted {
                        item_id: host.clone(),
                        index: idx as i64,
                        ok: true,
                        message: None,
                    })
                    .ok();
            }
            Err(e) => {
                fail_count += 1;
                let mut msg = e.to_string();
                if value.len() >= 4 && msg.contains(value) {
                    msg = msg.replace(value, "[REDACTED:value]");
                }
                ctx.emitter
                    .emit_event(&Event::ItemCompleted {
                        item_id: host.clone(),
                        index: idx as i64,
                        ok: false,
                        message: Some(msg),
                    })
                    .ok();
            }
        }
    }
    ctx.emitter
        .emit_event(&Event::Completed {
            summary: serde_json::json!({
                "hosts": total,
                "ok": ok_count,
                "failed": fail_count,
            }),
        })
        .ok();
    if fail_count > 0 {
        return Err(UecmError::OperationFailed(format!(
            "{}/{} hosts failed ini set",
            fail_count, total
        )));
    }
    Ok(())
}

fn remove_batch(
    ctx: &mut Ctx<'_>,
    hosts: &[String],
    file: &str,
    section: &str,
    key: &str,
    cred: &CredentialArgs,
) -> UecmResult<()> {
    use crate::cli::output::Event;

    let db = ctx.require_db()?;
    let creds = cred.resolve(db)?;
    let (u, p) = creds.ok_or_else(|| {
        UecmError::InvalidInput(
            "ini remove --hosts requires credentials (--cred-alias or --user --pass)".into(),
        )
    })?;
    let total = hosts.len() as i64;

    ctx.emitter
        .emit_event(&Event::Started {
            task_type: "ini_remove".into(),
            task_id: None,
            metadata: serde_json::json!({
                "hosts": total,
                "file": file,
                "section": section,
                "key": key,
            }),
        })
        .ok();

    let mut ok_count: i64 = 0;
    let mut fail_count: i64 = 0;
    for (idx, host) in hosts.iter().enumerate() {
        ctx.emitter
            .emit_event(&Event::ItemStarted {
                item_id: host.clone(),
                index: idx as i64,
                total,
            })
            .ok();
        match ini_editor::remove_key_with_credential(host, file, section, key, &u, &p) {
            Ok(()) => {
                ok_count += 1;
                ctx.emitter
                    .emit_event(&Event::ItemCompleted {
                        item_id: host.clone(),
                        index: idx as i64,
                        ok: true,
                        message: None,
                    })
                    .ok();
            }
            Err(e) => {
                fail_count += 1;
                ctx.emitter
                    .emit_event(&Event::ItemCompleted {
                        item_id: host.clone(),
                        index: idx as i64,
                        ok: false,
                        message: Some(e.to_string()),
                    })
                    .ok();
            }
        }
    }
    ctx.emitter
        .emit_event(&Event::Completed {
            summary: serde_json::json!({
                "hosts": total,
                "ok": ok_count,
                "failed": fail_count,
            }),
        })
        .ok();
    if fail_count > 0 {
        return Err(UecmError::OperationFailed(format!(
            "{}/{} hosts failed ini remove",
            fail_count, total
        )));
    }
    Ok(())
}
```

- [ ] **Step 2: Smoke + redaction test**

```bash
cargo build --manifest-path src-tauri/Cargo.toml --bin uecm-cli
src-tauri/target/debug/uecm-cli --json ini set \
  --hosts 192.0.2.1,192.0.2.2 \
  --file C:\\test.ini --section S --key K --value INI-SECRET-XYZ 2>&1 | grep -c INI-SECRET-XYZ
```
Expected: 0.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/cli/domain_ini.rs
git commit -m "feat(cli): ini set/remove --hosts batch with redaction"
```

---

## Phase 5 — `share` domain

### Task 5.1: `share list` + `share forget`

**Files:**
- Modify: `src-tauri/src/cli/domain_share.rs`

- [ ] **Step 1: Confirm data fn names**

```bash
grep -nE "^pub fn" src-tauri/src/data/share_configs.rs
```
Expected names: `insert`, `list_all`, `delete` (or close variants). Use what exists.

- [ ] **Step 2: Replace stub with dispatch + list + forget**

```rust
//! `uecm-cli share <action>` handlers.

use crate::cli::args::ShareAction;
use crate::cli::credential_args::CredentialArgs;
use crate::cli::run::Ctx;
use crate::cli::EmitSerialize;
use crate::data::share_configs as data_shares;
use crate::error::{UecmError, UecmResult};

pub fn handle(ctx: &mut Ctx<'_>, action: ShareAction) -> UecmResult<()> {
    match action {
        ShareAction::List => list(ctx),
        ShareAction::Forget { id, yes } => forget(ctx, id, yes),
        ShareAction::Create { .. } => Err(UecmError::OperationFailed(
            "share create: pending Task 5.2/5.3".into(),
        )),
        ShareAction::InjectSystemCred { .. } => Err(UecmError::OperationFailed(
            "share inject-system-cred: pending Task 5.4".into(),
        )),
    }
}

fn list(ctx: &mut Ctx<'_>) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let rows = data_shares::list_all(db)?;
    ctx.emitter.emit_result(&rows).ok();
    Ok(())
}

fn forget(ctx: &mut Ctx<'_>, id: i64, yes: bool) -> UecmResult<()> {
    if !yes {
        return Err(UecmError::InvalidInput(
            "share forget is destructive; pass --yes to confirm. \
             Note: the remote SMB share is NOT removed by this command — \
             use ssh + Remove-SmbShare for that."
                .into(),
        ));
    }
    let db = ctx.require_db()?;
    data_shares::delete(db, id)?;
    ctx.emitter
        .emit_event(&crate::cli::output::Event::Completed {
            summary: serde_json::json!({
                "id": id,
                "forgotten": true,
                "note": "local inventory only; remote share still active",
            }),
        })
        .ok();
    Ok(())
}
```

- [ ] **Step 3: Smoke test on fresh DB**

```bash
cargo build --manifest-path src-tauri/Cargo.toml --bin uecm-cli
TMP_DB="$(mktemp -t uecm-test.XXXXXX.sqlite)"
UECM_DB_PATH="$TMP_DB" src-tauri/target/debug/uecm-cli --json share list
```
Expected: `[]`.

```bash
UECM_DB_PATH="$TMP_DB" src-tauri/target/debug/uecm-cli --json share forget 1
```
Expected: error `invalid_input` (missing `--yes`).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/cli/domain_share.rs
git commit -m "feat(cli): share list + share forget (local-only delete)"
```

---

### Task 5.2: `share create` (Mode A + Mode B)

**Files:**
- Modify: `src-tauri/src/cli/domain_share.rs`

- [ ] **Step 1: Read core::shares signatures**

```bash
sed -n '1,150p' src-tauri/src/core/shares.rs
```
Confirm `create_mode_a` / `create_mode_b` / `generate_svc_password` exist and parameter order.

- [ ] **Step 2: Replace `ShareAction::Create` arm + implement create**

```rust
        ShareAction::Create { mode, host, share, local_path, cred } => {
            create(ctx, &mode, &host, &share, &local_path, &cred)
        }
```

Add helper:

```rust
fn create(
    ctx: &mut Ctx<'_>,
    mode: &str,
    host: &str,
    share: &str,
    local_path: &str,
    cred: &CredentialArgs,
) -> UecmResult<()> {
    use crate::core::shares;
    use crate::data::share_configs::ShareConfig;

    let db = ctx.require_db()?;
    let creds = cred.resolve(db)?;
    let (op_user, op_pass) = match &creds {
        Some((u, p)) => (Some(u.as_str()), Some(p.as_str())),
        None => (None, None),
    };

    let (result, mode_value, credential_alias, svc_user_persisted): (_, &str, Option<String>, Option<String>) = match mode {
        "a" | "A" => {
            let r = shares::create_mode_a(host, share, local_path, op_user, op_pass)?;
            (r, "A", cred.cred_alias.clone(), None)
        }
        "b" | "B" => {
            let svc_user = "ddc-svc";
            let svc_pass = shares::generate_svc_password();
            let r = shares::create_mode_b(
                host,
                share,
                local_path,
                svc_user,
                &svc_pass,
                op_user,
                op_pass,
            )?;
            // Persist the svc credential into DPAPI + SQLite so future
            // injection / Robocopy fan-out can resolve it.
            let svc_alias = format!("share-{}-{}", host, share);
            crate::core::credentials::store(&svc_alias, svc_user, &svc_pass)?;
            if let Err(dpapi_err) = crate::core::credentials::store_password(&svc_alias, &svc_pass) {
                let _ = crate::core::credentials::delete(&svc_alias);
                return Err(dpapi_err);
            }
            (r, "B", Some(svc_alias.clone()), Some(svc_user.to_string()))
        }
        other => {
            return Err(UecmError::InvalidInput(format!(
                "unknown share mode '{}'; expected 'a' or 'b'",
                other
            )))
        }
    };

    // Persist to share_configs. Resolve the host's machine_id; if the host
    // isn't in the inventory yet, surface that as InvalidInput — the UI
    // doesn't allow stranded shares either.
    let host_machine = crate::data::machines::find_by_ip(db, host)?
        .or_else(|| {
            // Fallback: find by hostname (some users add machines without IP).
            crate::data::machines::list_all(db)
                .ok()
                .and_then(|rows| rows.into_iter().find(|m| m.hostname == host))
        })
        .ok_or_else(|| {
            UecmError::InvalidInput(format!(
                "host '{}' is not in the machine inventory; run `machine add` first",
                host
            ))
        })?;

    let machine_id = host_machine.id.expect("machine from DB has id");

    let config = ShareConfig {
        id: None,
        host_machine_id: machine_id,
        share_name: share.to_string(),
        unc_path: result.unc_path.clone(),
        local_path: local_path.to_string(),
        mode: mode_value.to_string(),
        credential_alias,
    };
    let id = data_shares::insert(db, &config)?;

    ctx.emitter
        .emit_event(&crate::cli::output::Event::Completed {
            summary: serde_json::json!({
                "id": id,
                "host": host,
                "share": share,
                "unc_path": result.unc_path,
                "mode": mode_value,
                "svc_user": svc_user_persisted,
            }),
        })
        .ok();
    Ok(())
}
```

If `data::share_configs::ShareConfig` field names differ, grep `src-tauri/src/data/share_configs.rs` and align. The plan's contract: insert a row capturing host_machine_id / share_name / unc_path / local_path / mode / credential_alias.

- [ ] **Step 3: Smoke test (macOS expected fail at PS layer)**

```bash
cargo build --manifest-path src-tauri/Cargo.toml --bin uecm-cli
TMP_DB="$(mktemp -t uecm-test.XXXXXX.sqlite)"
# Add the host first so the machine_id lookup doesn't trip
UECM_DB_PATH="$TMP_DB" src-tauri/target/debug/uecm-cli --json machine add --ip 192.0.2.1
UECM_DB_PATH="$TMP_DB" src-tauri/target/debug/uecm-cli --json share create \
  --mode a --host 192.0.2.1 --share TEST --local-path 'C:\Temp\test'
```
Expected on macOS: error event `code: "powershell_failed"` from the underlying `setup-share-mode-a.ps1` call; exit 4. The DB write doesn't run because we propagate before insert.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/cli/domain_share.rs
git commit -m "feat(cli): share create (Mode A + Mode B with svc cred persistence)"
```

---

### Task 5.3: `share inject-system-cred`

**Files:**
- Modify: `src-tauri/src/cli/domain_share.rs`

- [ ] **Step 1: Replace stub arm**

```rust
        ShareAction::InjectSystemCred { client_host, target_host, svc_user, cred } => {
            inject_system_cred(ctx, &client_host, &target_host, &svc_user, &cred)
        }
```

Add helper:

```rust
fn inject_system_cred(
    ctx: &mut Ctx<'_>,
    client_host: &str,
    target_host: &str,
    svc_user: &str,
    cred: &CredentialArgs,
) -> UecmResult<()> {
    let db = ctx.require_db()?;
    let creds = cred.resolve(db)?;
    let (op_user, op_pass) = match &creds {
        Some((u, p)) => (Some(u.as_str()), Some(p.as_str())),
        None => (None, None),
    };

    // Look up the svc password from the share's persisted alias (Mode B).
    let svc_alias_guess = format!("share-{}-{}", target_host, svc_user);
    let svc_pass = crate::core::credentials::resolve_password(&svc_alias_guess)
        .map_err(|e| {
            UecmError::InvalidInput(format!(
                "no DPAPI entry for '{}': {}. Inject expects a Mode B share \
                 created via this CLI; if the share predates Plan 2, \
                 re-create or recover the svc password manually.",
                svc_alias_guess, e
            ))
        })?;

    let message = crate::core::psexec::inject_system_credential(
        client_host,
        target_host,
        svc_user,
        &svc_pass,
        op_user,
        op_pass,
    )?;

    ctx.emitter
        .emit_event(&crate::cli::output::Event::Completed {
            summary: serde_json::json!({
                "client_host": client_host,
                "target_host": target_host,
                "svc_user": svc_user,
                "message": message,
            }),
        })
        .ok();
    Ok(())
}
```

Note: the alias key scheme `share-<host>-<svc_user>` must match Task 5.2. If you used a different scheme there, change both.

- [ ] **Step 2: Smoke test (macOS expected fail)**

```bash
cargo build --manifest-path src-tauri/Cargo.toml --bin uecm-cli
src-tauri/target/debug/uecm-cli --json share inject-system-cred \
  --client-host 192.0.2.1 --target-host 192.0.2.2
```
Expected: error event `invalid_input` (no DPAPI entry for nonexistent share); exit 2.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/cli/domain_share.rs
git commit -m "feat(cli): share inject-system-cred"
```

---

## Phase 6 — Integration

### Task 6.1: Extend `tests/cli_smoke.rs`

**Files:**
- Modify: `src-tauri/tests/cli_smoke.rs`

- [ ] **Step 1: Add 3 new integration tests**

Append after the existing tests:

```rust
#[test]
fn cred_list_on_fresh_db_returns_empty_array() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_string_lossy().to_string();
    let out = std::process::Command::new(bin())
        .env("UECM_DB_PATH", &path)
        .args(["--json", "cred", "list"])
        .output()
        .expect("spawn");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(stdout.trim_end()).unwrap();
    assert_eq!(v, serde_json::Value::Array(vec![]));
}

#[test]
fn env_set_without_target_returns_invalid_input() {
    let out = std::process::Command::new(bin())
        .args(["--json", "env", "set", "--name", "X", "--value", "Y"])
        .output()
        .expect("spawn");
    assert!(!out.status.success());
    // clap exit code (2) for missing required arg group; underlying error
    // type is InvalidInput which also maps to 2.
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn env_set_does_not_leak_value_to_stderr() {
    // macOS will fail at the powershell layer, but redaction must hold.
    let secret = "MY-VERY-SECRET-VALUE-DEF-456";
    let out = std::process::Command::new(bin())
        .args([
            "--json", "env", "set",
            "--host", "192.0.2.1",
            "--name", "X",
            "--value", secret,
        ])
        .output()
        .expect("spawn");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!combined.contains(secret), "value leaked: {}", combined);
}
```

- [ ] **Step 2: Build the binary, then run integration tests**

```bash
cargo build --manifest-path src-tauri/Cargo.toml --bin uecm-cli
cargo test --manifest-path src-tauri/Cargo.toml --test cli_smoke
```
Expected: 6 tests pass (3 from Plan 1 + 3 new).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/tests/cli_smoke.rs
git commit -m "test(cli): smoke for cred list / env set arg validation / redaction"
```

---

### Task 6.2: lanPC deploy + end-to-end validation + changelog

**Files:** none (deploy + verify + changelog write)

- [ ] **Step 1: Release build on macOS**

```bash
cargo build --manifest-path src-tauri/Cargo.toml --release --bin uecm-cli
```
Expected: clean release build.

- [ ] **Step 2: Sync source to lanPC and rebuild**

```bash
cd /Users/bip.lan/AIWorkspace/vp/ue-cache-manager/.claude/worktrees/feat+cli-architecture
COPYFILE_DISABLE=1 tar czf /tmp/uecm-cli-plan2-src.tgz \
  --exclude='node_modules' --exclude='target' --exclude='dist' \
  --exclude='src-tauri/target' --exclude='.git' \
  src-tauri/ ps-scripts/ vendor/
scp /tmp/uecm-cli-plan2-src.tgz lanpc@192.168.10.20:E:/uecm-plan4-test/
```

On lanPC:
```powershell
cd E:\uecm-plan4-test
tar xzf uecm-cli-plan2-src.tgz
cd src-tauri
cargo build --release --bin uecm-cli
copy E:\uecm-plan4-test\src-tauri\target\release\uecm-cli.exe C:\Tools\UECM\uecm-cli.exe /Y
```

`cargo build --bin uecm-cli` is **not** the same as `pnpm tauri build` — it only compiles the CLI target, no frontend dev server, no Tauri capabilities loader.

- [ ] **Step 3: lanPC end-to-end — capability ② happy path**

On lanPC (PowerShell):

```powershell
# 1. Save admin credential (assumes lanPC\Administrator + a known password).
"<password>" | C:\Tools\UECM\uecm-cli.exe cred save `
  --alias winrm-admin --user 'lanPC\Administrator' --pass-stdin --json

# 2. List to confirm.
C:\Tools\UECM\uecm-cli.exe --json cred list

# 3. Set a probe env var (using lanPC itself as target; loopback bypass).
C:\Tools\UECM\uecm-cli.exe --json env set --host lanPC `
  --name UECM_PLAN2_TEST --value plan2-ok --cred-alias winrm-admin

# 4. Read it back.
C:\Tools\UECM\uecm-cli.exe --json env get --host lanPC `
  --name UECM_PLAN2_TEST --cred-alias winrm-admin

# 5. Read an existing INI section (e.g. base engine ini if available).
$ue58 = "D:\Program Files\Epic Games\UE_5.8\Engine\Config\BaseEngine.ini"
C:\Tools\UECM\uecm-cli.exe --json ini read --host lanPC `
  --file $ue58 --section DerivedDataBackendGraph --cred-alias winrm-admin

# 6. Create a test share on lanPC itself.
mkdir C:\Temp\plan2-share -Force | Out-Null
C:\Tools\UECM\uecm-cli.exe --json share create --mode a `
  --host lanPC --share UECM_PLAN2 --local-path C:\Temp\plan2-share `
  --cred-alias winrm-admin

# 7. Confirm the inventory row.
C:\Tools\UECM\uecm-cli.exe --json share list
```

Expected outputs:
- Step 1: `{"kind":"completed","summary":{"id":<n>,"alias":"winrm-admin"}}`
- Step 2: a JSON array with one element
- Step 3: completed event; **stdout MUST NOT contain `plan2-ok`** as raw value (only `value_len:7` and `value_sha256_prefix`)
- Step 4: `{"host":"lanPC","name":"UECM_PLAN2_TEST","value":"plan2-ok"}` — this is the ONE place value can appear (single get)
- Step 5: a list of keys for the section, or empty array if section absent
- Step 6: completed event with `unc_path`
- Step 7: array with one element

- [ ] **Step 4: Redaction sanity grep on lanPC**

```powershell
C:\Tools\UECM\uecm-cli.exe --json env set --host lanPC `
  --name UECM_PLAN2_REDACT --value SECRET-LANPC-TEST --cred-alias winrm-admin |
  Select-String "SECRET-LANPC-TEST"
```
Expected: no output (redaction holds in human + JSON streams).

- [ ] **Step 5: UI cross-check**

Launch `C:\Tools\UECM\uecm.exe`. Verify:
- `winrm-admin` alias is visible in the Credentials view
- `UECM_PLAN2` share row is visible in the Shares view
- DB path the UI sees matches `uecm-cli --json system db-path`

- [ ] **Step 6: Write changelog**

Create `docs/superpowers/changelog/2026-05-17-cli-plan-2.md`:

```markdown
# UECM CLI Plan 2 — Config Management

**Date:** 2026-05-17
**Branch:** worktree-feat+cli-architecture
**Spec:** [2026-05-17-cli-config-management-design.md](../specs/2026-05-17-cli-config-management-design.md)
**Plan:** [2026-05-17-cli-plan-2-config-management.md](../plans/2026-05-17-cli-plan-2-config-management.md)

## What shipped

Four new CLI domains so AI clients and humans can run the full DDC-path
configuration loop without the WebView:

- `cred` — list / save (5-step transactional) / delete (best-effort)
- `env` — get (single) / set (single + `--hosts` batch with redaction)
- `ini` — read (single) / set (single + batch) / remove (single + batch)
- `share` — list / forget (LOCAL inventory only) / create (Mode A/B) /
  inject-system-cred

Two shared argument structs flatten into every subcommand that needs
them: `CredentialArgs` (`--cred-alias` / `--user --pass` /
`--user --pass-stdin`) and `HostArgs` (`--host` / `--hosts` mutex).

## Contract additions

- NDJSON redaction: `env set` / `ini set` never echo the raw value in
  metadata, only `name`, `value_len`, `value_sha256_prefix`. Error
  messages get a substring-replace pass as belt-and-suspenders.
- `share forget` is the only inventory-deleting command name; the
  destructive `share remove` (with `Remove-SmbShare` + `Remove-LocalUser`
  + cmdkey clear) is parked for a future plan.
- `cred save` mirrors the UI's 5-step sequence (normalize → cmdkey →
  DPAPI w/ rollback → replace-by-alias → SQLite insert) so DPAPI and
  SQLite never split-brain.
- `cred delete` matches the UI's best-effort cleanup order
  (cmdkey result preserved → SQLite hard delete → DPAPI best-effort →
  return cmdkey result).

## Validated on lanPC

- macOS dev build + Windows release build both clean
- Tauri main binary build unaffected
- 7-step capability ② loop on lanPC (cred save → list → env set →
  env get → ini read → share create → share list)
- Redaction grep: NDJSON / stdout / stderr contain neither value nor
  SHA preimage
- UI sees `winrm-admin` alias and `UECM_PLAN2` share row written by CLI

## Out of scope (deferred)

- INI cluster scan (`scan` / `findings` / `apply` / `skip` / `runs`)
  — Plan 3 (capability ③)
- Credential-aware `machine refresh` — Plan 3 along with the other
  Plan 1 leftovers
- DDC pak / PSO collect / project discovery / health / GPU matrix
- SIGINT graceful cancel for batch
- Shell completion / MCP / daemon
- `share remove` (the destructive variant)
```

Commit:
```bash
git add docs/superpowers/changelog/2026-05-17-cli-plan-2.md
git commit -m "docs: CLI Plan 2 changelog"
```

---

## Summary of tasks

| Phase | Task | Description |
|---|---|---|
| 0 | 0.1 | `CredentialArgs` + sha2 dep |
| 0 | 0.2 | `HostArgs` + mutex |
| 1 | 1.1 | Args tree extension + 4 stub handler files |
| 1 | 1.2 | Dispatch + `needs_db` wiring |
| 2 | 2.1 | `cred list` |
| 2 | 2.2 | `cred save` (5-step transactional) |
| 2 | 2.3 | `cred delete` (best-effort cleanup) |
| 3 | 3.1 | `env get` (single host) |
| 3 | 3.2 | `env set` (single host with redaction) |
| 3 | 3.3 | `env set --hosts` batch (NDJSON + redaction) |
| 4 | 4.1 | `ini read / set / remove` (single host) |
| 4 | 4.2 | `ini set / remove --hosts` batch |
| 5 | 5.1 | `share list` + `share forget` |
| 5 | 5.2 | `share create` (Mode A + Mode B with svc DPAPI) |
| 5 | 5.3 | `share inject-system-cred` |
| 6 | 6.1 | Integration tests (3 new smoke) |
| 6 | 6.2 | lanPC deploy + end-to-end + changelog |

**Out of scope for this plan** (per spec §3):
- INI cluster scan / apply / findings flow — Plan 3
- Credential-aware `machine refresh` — Plan 3
- `ddc` / `pso` / `project` / `health` / `gpu` domains — Plan 3+
- SIGINT graceful cancel for batch
- `share remove` (the destructive variant with remote SMB cleanup)

## Known assumptions

Several handlers reference data-layer functions (`data::credentials::list_all` / `find_by_alias` / `delete_by_alias` / `insert`; `data::share_configs::list_all` / `delete` / `insert`; `data::machines::find_by_ip`). Plan author checked that each exists by the names used here, but **the implementing agent must grep and confirm** the exact signatures (return types, parameter order) before pasting handler code. The behavior contract is stable; only the spelling may need a one-line fix.

Same applies to `core::shares::create_mode_a` / `create_mode_b` (param order: `host, share, local_path, [svc_user, svc_pass,] op_user?, op_pass?`) and `core::psexec::inject_system_credential` (the exact param tuple).

## Redaction contract recap (spec §8)

For every NDJSON event emitted by `env set` / `ini set` / `ini remove`:

- `metadata` may include `name`, `value_len`, `value_sha256_prefix` (first 4 bytes of SHA-256, 8 hex chars), `section`, `key`, `file`, `hosts` count
- `metadata` MUST NOT include `value` raw
- `item_started` / `item_completed` / `progress` carry no `value` either
- Error messages that contain the raw value (e.g. PS layer echoed it) get a `str.replace(value, "[REDACTED:value]")` pass at the handler level
- `env get` is the single command allowed to surface a value, and is single-host only
