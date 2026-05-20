# 机器纳管编排命令 Implementation Plan

> **STATUS: 已实现（2026-05-20，分支 `feature/machine-onboarding-cli`）。已落地代码为权威来源。** 执行过程中两处与下方早期片段不同，以代码为准：
> 1. **deep-scan** 改为「先逐台探活分类（probe-fail=skip / 探活后 refresh 失败=failed / not-found=failed），再对可达集合跑**一次** `ini scan` + **一次** `health run`」，保留跨机一致性（GPU/zen/cluster-majority）scope——不是在每台循环里传 `vec![id]`。
> 2. **authorize** 按用户确认做**全量 provision**：`enable_winrm_with_psexec` 新增 `full_provision: bool` 参数（透传 `-EnableSmbServer/-EnableWmi/-EnableLongPaths/-SetExecutionPolicy RemoteSigned/-PowerProfile HighPerformance` 给 `bootstrap-winrm-remote.ps1`），authorize 以 `(.., true, true)` 调用；3 个既有调用方传 `false`。并修了 `bootstrap-winrm-remote.ps1` 的用户名规范化（`<host>\user`）与 preflight 对齐。

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 `machine` 域新增 `deep-scan`（refresh+INI+health）与 `authorize`（preflight→bootstrap）两条编排命令，把现有原子能力各串成一个 CLI 动作。

**Architecture:** 纯编排 + 聚合输出，不写新后端逻辑。两个新 handler 在 `domain_machine.rs`，内部复用现有 `refresh` / `domain_ini::handle` / `domain_health::handle` / `core::preflight` / `core::bootstrap` / `core::credentials`。凭据**先解析一次**成 (user,pass) 再以 inline 形式喂子调用（绕开 `--pass-stdin` 只能读一次的坑）。

**Tech Stack:** Rust + clap derive + rusqlite，测试用 in-memory DB + `NdjsonEmitter`。

参考规格：`docs/superpowers/specs/2026-05-20-machine-onboarding-cli-design.md`

---

### Task 1: 加 args + dispatch 骨架

**Files:**
- Modify: `src-tauri/src/cli/args.rs`（`MachineAction` enum + 现有 parse 测试块）
- Modify: `src-tauri/src/cli/domain_machine.rs:13-23`（`handle` 的 match）

- [ ] **Step 1: 在 `MachineAction` 末尾（`Rename` 之后、`}` 之前）加两个变体**

```rust
    /// Deep scan a set of machines: refresh (UE/GPU) + INI scan + health, per machine.
    /// WinRM-unreachable machines are skipped (run `machine authorize` first) and the
    /// batch continues.
    DeepScan {
        #[arg(long, value_name = "M1,M2,...", value_delimiter = ',', conflicts_with = "all")]
        machine_ids: Vec<i64>,
        /// Deep-scan every machine in inventory.
        #[arg(long, conflicts_with = "machine_ids")]
        all: bool,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
    /// Authorize a set of machines for remote management: winrm preflight → bootstrap
    /// (Path B remote PsExec). Machines where Path B is not viable fall back to a USB
    /// script hint. The batch continues past per-machine failures.
    Authorize {
        #[arg(long, value_name = "M1,M2,...", value_delimiter = ',', conflicts_with = "all")]
        machine_ids: Vec<i64>,
        /// Authorize every machine in inventory.
        #[arg(long, conflicts_with = "machine_ids")]
        all: bool,
        /// Save the resolved --user/--pass-stdin credential as this DPAPI alias for reuse.
        #[arg(long, value_name = "ALIAS")]
        save_as: Option<String>,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
```

- [ ] **Step 2: 在 `domain_machine.rs` 的 `handle` match 末尾加两条 arm**

```rust
        MachineAction::DeepScan { machine_ids, all, cred } => deep_scan(ctx, machine_ids, all, &cred),
        MachineAction::Authorize { machine_ids, all, save_as, cred } => {
            authorize(ctx, machine_ids, all, save_as, &cred)
        }
```

- [ ] **Step 3: 加临时 stub 让其编译**（Task 2/3 替换真实实现）

在 `domain_machine.rs` 末尾 `#[cfg(test)]` 之前加：

```rust
fn deep_scan(_ctx: &mut Ctx<'_>, _ids: Vec<i64>, _all: bool, _cred: &crate::cli::credential_args::CredentialArgs) -> UecmResult<()> {
    Err(UecmError::OperationFailed("deep_scan not implemented".into()))
}
fn authorize(_ctx: &mut Ctx<'_>, _ids: Vec<i64>, _all: bool, _save_as: Option<String>, _cred: &crate::cli::credential_args::CredentialArgs) -> UecmResult<()> {
    Err(UecmError::OperationFailed("authorize not implemented".into()))
}
```

- [ ] **Step 4: 加 parse 测试**（放进 `args.rs` 现有 `#[cfg(test)]` 块，紧邻已有的 `MachineAction::Scan` parse 测试）

```rust
    #[test]
    fn parses_machine_deep_scan() {
        let cli = Cli::try_parse_from([
            "uecm-cli", "machine", "deep-scan", "--machine-ids", "3,4,5", "--cred-alias", "prod",
        ]).unwrap();
        match cli.command {
            Domain::Machine { action: MachineAction::DeepScan { machine_ids, all, .. } } => {
                assert_eq!(machine_ids, vec![3, 4, 5]);
                assert!(!all);
            }
            _ => panic!("expected DeepScan"),
        }
    }

    #[test]
    fn parses_machine_authorize_with_save_as() {
        let cli = Cli::try_parse_from([
            "uecm-cli", "machine", "authorize", "--all", "--user", "Administrator", "--pass-stdin", "--save-as", "prod",
        ]).unwrap();
        match cli.command {
            Domain::Machine { action: MachineAction::Authorize { all, save_as, .. } } => {
                assert!(all);
                assert_eq!(save_as.as_deref(), Some("prod"));
            }
            _ => panic!("expected Authorize"),
        }
    }
```

> 注：`Cli` / `Domain` 的确切引入路径以 `args.rs` 现有测试为准（搜 `MachineAction::Scan` 那个测试照抄 import / 解析方式）。

- [ ] **Step 5: 编译 + 跑 parse 测试**

Run: `cd src-tauri && cargo test --lib cli::args 2>&1 | tail -20`
Expected: 新增两个 parse 测试 PASS（其余不回归）。

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/cli/args.rs src-tauri/src/cli/domain_machine.rs
git commit -m "feat(cli): add machine deep-scan / authorize arg variants + dispatch stubs"
```

---

### Task 2: 抽取「凭据一次性解析」helper

**Files:**
- Modify: `src-tauri/src/cli/credential_args.rs`（加一个把 resolved (user,pass) 重新包成 inline `CredentialArgs` 的便利构造）

**Why:** 编排命令对一批机器多次调子 handler，每个子 handler 都会 `cred.resolve()`。`--pass-stdin` 只能读一次、DPAPI 每次解析有开销 → 必须解析一次、之后以 inline `--user/--pass` 形式复用。

- [ ] **Step 1: 写失败测试**（`credential_args.rs` 的 `#[cfg(test)]` 块内）

```rust
    #[test]
    fn inline_from_resolved_roundtrips_without_stdin() {
        let reused = CredentialArgs::inline(Some(("alice".into(), "pw".into())));
        let db = fresh_db();
        // resolve 不读 stdin、不碰 DPAPI，直接还原 (user, pass)
        assert_eq!(reused.resolve(&db).unwrap(), Some(("alice".into(), "pw".into())));

        let none = CredentialArgs::inline(None);
        assert!(none.resolve(&db).unwrap().is_none());
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cd src-tauri && cargo test --lib credential_args::tests::inline_from_resolved 2>&1 | tail -10`
Expected: FAIL（`CredentialArgs::inline` 不存在）。

- [ ] **Step 3: 实现 `inline`**（`impl CredentialArgs` 块内）

```rust
    /// Build a stdin-free `CredentialArgs` from an already-resolved credential.
    /// Used by orchestration commands that resolve once then fan out to many
    /// sub-handlers — calling `resolve` repeatedly would re-read `--pass-stdin`
    /// (only readable once) or re-hit DPAPI.
    pub fn inline(resolved: Option<(String, String)>) -> Self {
        match resolved {
            Some((user, pass)) => CredentialArgs {
                cred_alias: None,
                user: Some(user),
                pass: Some(pass),
                pass_stdin: false,
            },
            None => CredentialArgs {
                cred_alias: None,
                user: None,
                pass: None,
                pass_stdin: false,
            },
        }
    }
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cd src-tauri && cargo test --lib credential_args 2>&1 | tail -10`
Expected: PASS。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/cli/credential_args.rs
git commit -m "feat(cli): CredentialArgs::inline for one-shot credential reuse across sub-handlers"
```

---

### Task 3: 实现 `deep_scan`

**Files:**
- Modify: `src-tauri/src/cli/domain_machine.rs`（替换 Task 1 的 stub）
- Test: `src-tauri/src/cli/domain_machine.rs`（`#[cfg(test)]` 块）

- [ ] **Step 1: 写失败测试**（在 `domain_machine.rs` 测试块内加）

在非 Windows（开发机）上 `refresh` 的 WinRM 探活会因 `WinRM is Windows-only` 失败 → 验证 deep_scan 把每台标 skip、不让整批 Err。

```rust
    #[test]
    fn deep_scan_skips_winrm_unreachable_and_completes_batch() {
        let (db, buf) = setup();
        let emitter: Box<dyn Emitter> = Box::new(NdjsonEmitter::new(buf));
        let mut ctx = Ctx { db: Some(db), db_path: PathBuf::from(":memory:"), emitter, json_mode: true };

        add(&mut ctx, "10.0.0.1".to_string(), Some("m1".to_string())).unwrap();
        add(&mut ctx, "10.0.0.2".to_string(), Some("m2".to_string())).unwrap();

        // 无凭据；非 Windows 上 refresh 会失败 → 两台都被跳过，但整批返回 Ok。
        let cred = crate::cli::credential_args::CredentialArgs::inline(None);
        let res = deep_scan(&mut ctx, vec![1, 2], false, &cred);
        assert!(res.is_ok(), "batch must complete even when every machine is skipped");
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cd src-tauri && cargo test --lib domain_machine::tests::deep_scan_skips 2>&1 | tail -15`
Expected: FAIL（stub 返回 `not implemented` Err）。

- [ ] **Step 3: 实现 `deep_scan`**（替换 stub）

```rust
fn resolve_target_ids(db: &crate::data::Db, machine_ids: &[i64], all: bool) -> UecmResult<Vec<i64>> {
    if all {
        Ok(machines::list_all(db)?
            .into_iter()
            .filter_map(|m| m.id)
            .collect())
    } else if machine_ids.is_empty() {
        Err(UecmError::InvalidInput(
            "one of --machine-ids or --all is required".into(),
        ))
    } else {
        Ok(machine_ids.to_vec())
    }
}

fn deep_scan(
    ctx: &mut Ctx<'_>,
    machine_ids: Vec<i64>,
    all: bool,
    cred: &crate::cli::credential_args::CredentialArgs,
) -> UecmResult<()> {
    let ids = {
        let db = ctx.require_db()?;
        resolve_target_ids(db, &machine_ids, all)?
    };
    // Resolve credentials ONCE (stdin/DPAPI), reuse inline across all sub-calls.
    let sub_cred = {
        let db = ctx.require_db()?;
        crate::cli::credential_args::CredentialArgs::inline(cred.resolve(db)?)
    };

    ctx.emitter.emit_event(&Event::Started {
        task_type: "machine_deep_scan".into(),
        task_id: None,
        metadata: json!({ "machines": ids.len() }),
    }).ok();

    let mut scanned = 0usize;
    let mut skipped = 0usize;
    for id in &ids {
        // Step 1: refresh. WinRM-unreachable → skip the rest for this machine.
        match refresh(ctx, *id, &sub_cred) {
            Ok(()) => {}
            Err(e) => {
                skipped += 1;
                ctx.emitter.emit_event(&Event::Completed {
                    summary: json!({
                        "machine_id": id,
                        "step": "deep_scan",
                        "skipped": true,
                        "reason": format!("refresh failed (WinRM unreachable?): {}", e),
                        "hint": "run `uecm-cli machine authorize` to open WinRM first",
                    }),
                }).ok();
                continue;
            }
        }
        // Step 2: INI scan (reuse domain_ini handler for this machine).
        // Per spec: a sub-step error is recorded but MUST NOT abort the batch —
        // so match-and-continue, never `?`.
        if let Err(e) = crate::cli::domain_ini::handle(
            ctx,
            crate::cli::args::IniAction::Scan { machine_ids: vec![*id], cred: sub_cred.clone() },
        ) {
            ctx.emitter.emit_event(&Event::Completed {
                summary: json!({ "machine_id": id, "step": "ini_scan", "error": e.to_string() }),
            }).ok();
        }
        // Step 3: health run (reuse domain_health handler for this machine).
        if let Err(e) = crate::cli::domain_health::handle(
            ctx,
            crate::cli::args::HealthAction::Run {
                machine_ids: vec![*id],
                cidr: None,
                all: false,
                expected_local_path: String::new(),
                expected_shared_path: String::new(),
                cred: sub_cred.clone(),
            },
        ) {
            ctx.emitter.emit_event(&Event::Completed {
                summary: json!({ "machine_id": id, "step": "health_run", "error": e.to_string() }),
            }).ok();
        }
        scanned += 1;
    }

    ctx.emitter.emit_event(&Event::Completed {
        summary: json!({ "machines": ids.len(), "scanned": scanned, "skipped": skipped }),
    }).ok();
    Ok(())
}
```

> 注：实现时确认 `IniAction` / `HealthAction` 的字段与 `args.rs` 当前定义完全一致（健康 Run 有 `expected_local_path` / `expected_shared_path`）；`domain_ini` / `domain_health` 的 `handle` 是 `pub`（已确认）。`CredentialArgs` 需 `Clone`（已确认 derive）。

- [ ] **Step 4: 跑测试确认通过 + 全量回归**

Run: `cd src-tauri && cargo test --lib domain_machine 2>&1 | tail -20`
Expected: `deep_scan_skips_winrm_unreachable_and_completes_batch` PASS，其余不回归。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/cli/domain_machine.rs
git commit -m "feat(cli): implement machine deep-scan (refresh+ini+health, skip-on-unreachable)"
```

---

### Task 4: 实现 `authorize`（含纯决策函数 + --save-as）

**Files:**
- Modify: `src-tauri/src/cli/domain_machine.rs`（替换 stub + 加纯决策函数）
- Modify: `src-tauri/src/cli/domain_cred.rs`（抽 `pub(crate) fn save_resolved`，供 authorize 复用）

- [ ] **Step 1: 抽取 `save_resolved`**（`domain_cred.rs`）：把现有 `save` 中“密码已知之后”的逻辑提成可复用函数，`save` 改为调用它。

```rust
pub(crate) fn save_resolved(
    ctx: &mut Ctx<'_>,
    alias: &str,
    username: &str,
    password: &str,
    kind: &str,
) -> UecmResult<()> {
    use crate::core::credentials as core_creds;
    let _validated_kind = parse_credential_kind(kind)?;
    let username = core_creds::normalize_username_for_storage(username);
    core_creds::store(alias, &username, password)?;
    if let Err(dpapi_err) = core_creds::store_password(alias, password) {
        if let Err(rollback_err) = core_creds::delete(alias) {
            tracing::warn!(alias = %alias, error = %rollback_err, "cmdkey rollback after DPAPI failure also failed");
        }
        return Err(dpapi_err);
    }
    let db = ctx.require_db()?;
    if data_creds::find_by_alias(db, alias)?.is_some() {
        data_creds::delete_by_alias(db, alias)?;
    }
    let record = build_credential_record(alias, &username, kind)?;
    let id = data_creds::insert(db, &record)?;
    ctx.emitter.emit_event(&crate::cli::output::Event::Completed {
        summary: serde_json::json!({ "id": id, "alias": alias }),
    }).ok();
    Ok(())
}
```

然后把现有 `save` 改为：

```rust
fn save(ctx: &mut Ctx<'_>, alias: &str, user: &str, pass_inline: Option<&str>, pass_stdin: bool, kind: &str) -> UecmResult<()> {
    let password = read_password(pass_inline, pass_stdin)?;
    save_resolved(ctx, alias, user, &password, kind)
}
```

- [ ] **Step 2: 写决策函数失败测试**（`domain_machine.rs` 测试块）

```rust
    #[test]
    fn authorize_decision_maps_verdict() {
        use super::AuthorizeStep;
        assert_eq!(authorize_decision("viable"), AuthorizeStep::Bootstrap);
        assert_eq!(authorize_decision("likely_viable"), AuthorizeStep::Bootstrap);
        assert_eq!(authorize_decision("blocked"), AuthorizeStep::UsbFallback);
        assert_eq!(authorize_decision("uncertain"), AuthorizeStep::UsbFallback);
    }
```

- [ ] **Step 3: 跑测试确认失败**

Run: `cd src-tauri && cargo test --lib domain_machine::tests::authorize_decision 2>&1 | tail -10`
Expected: FAIL（`AuthorizeStep` / `authorize_decision` 未定义）。

- [ ] **Step 4: 实现决策函数 + `authorize` handler**（替换 stub）

```rust
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AuthorizeStep {
    Bootstrap,
    UsbFallback,
}

/// Pure mapping: Path B preflight verdict → next action. `viable` / `likely_viable`
/// proceed to bootstrap; everything else (`blocked` / `uncertain`) falls back to USB.
pub(crate) fn authorize_decision(verdict: &str) -> AuthorizeStep {
    match verdict {
        "viable" | "likely_viable" => AuthorizeStep::Bootstrap,
        _ => AuthorizeStep::UsbFallback,
    }
}

fn authorize(
    ctx: &mut Ctx<'_>,
    machine_ids: Vec<i64>,
    all: bool,
    save_as: Option<String>,
    cred: &crate::cli::credential_args::CredentialArgs,
) -> UecmResult<()> {
    // Credentials are REQUIRED — preflight + bootstrap need a local admin user/pass.
    let (user, pass) = {
        let db = ctx.require_db()?;
        cred.resolve(db)?.ok_or_else(|| {
            UecmError::InvalidInput(
                "machine authorize requires credentials (--cred-alias or --user/--pass-stdin)".into(),
            )
        })?
    };

    // Optional: persist the resolved credential as an alias for later reuse.
    if let Some(alias) = save_as.as_deref() {
        crate::cli::domain_cred::save_resolved(ctx, alias, &user, &pass, "winrm")?;
    }

    let ids = {
        let db = ctx.require_db()?;
        resolve_target_ids(db, &machine_ids, all)?
    };

    ctx.emitter.emit_event(&Event::Started {
        task_type: "machine_authorize".into(),
        task_id: None,
        metadata: json!({ "machines": ids.len() }),
    }).ok();

    let mut authorized = 0usize;
    let mut fallback = 0usize;
    let mut failed = 0usize;
    for id in &ids {
        let host = {
            let db = ctx.require_db()?;
            match machines::find_by_id(db, *id)? {
                Some(m) => m.ip,
                None => {
                    failed += 1;
                    ctx.emitter.emit_event(&Event::Completed {
                        summary: json!({ "machine_id": id, "error": "machine not found" }),
                    }).ok();
                    continue;
                }
            }
        };

        // Preflight (Shallow; no SCM probe) — classify verdict.
        let pf = match crate::core::preflight::preflight_path_b(&host, &user, &pass, false) {
            Ok(r) => r,
            Err(e) => {
                failed += 1;
                ctx.emitter.emit_event(&Event::Completed {
                    summary: json!({ "machine_id": id, "host": host, "step": "preflight", "error": e.to_string() }),
                }).ok();
                continue;
            }
        };

        match authorize_decision(&pf.verdict) {
            AuthorizeStep::Bootstrap => {
                // SHIPPED: full provisioning → `(&host, &user, &pass, true, true)`
                // (enable_local_account_remote_admin=true, full_provision=true).
                match crate::core::bootstrap::enable_winrm_with_psexec(&host, &user, &pass, true, true) {
                    Ok(b) if b.ok => {
                        authorized += 1;
                        ctx.emitter.emit_event(&Event::Completed {
                            summary: json!({ "machine_id": id, "host": host, "authorized": true, "message": b.message }),
                        }).ok();
                    }
                    Ok(b) => {
                        failed += 1;
                        ctx.emitter.emit_event(&Event::Completed {
                            summary: json!({ "machine_id": id, "host": host, "authorized": false, "error": b.message }),
                        }).ok();
                    }
                    Err(e) => {
                        failed += 1;
                        ctx.emitter.emit_event(&Event::Completed {
                            summary: json!({ "machine_id": id, "host": host, "step": "bootstrap", "error": e.to_string() }),
                        }).ok();
                    }
                }
            }
            AuthorizeStep::UsbFallback => {
                fallback += 1;
                ctx.emitter.emit_event(&Event::Completed {
                    summary: json!({
                        "machine_id": id,
                        "host": host,
                        "path_b_unavailable": true,
                        "verdict": pf.verdict,
                        "reason": pf.reason,
                        "hint": "Path B not viable — run `uecm-cli winrm bootstrap-script` and execute it on the machine via USB",
                    }),
                }).ok();
            }
        }
    }

    ctx.emitter.emit_event(&Event::Completed {
        summary: json!({ "machines": ids.len(), "authorized": authorized, "usb_fallback": fallback, "failed": failed }),
    }).ok();
    Ok(())
}
```

> 注：确认 `WinrmAction::BootstrapScript` 对应的 CLI 子命令名（hint 文案里写对）；确认 `crate::core::preflight::preflight_path_b` 与 `crate::core::bootstrap::enable_winrm_with_psexec` 签名（已读：`(host, user, pass, with_probe/enable_local_admin)`）。

- [ ] **Step 5: 跑决策测试 + 全量回归**

Run: `cd src-tauri && cargo test --lib domain_machine 2>&1 | tail -20`
Expected: `authorize_decision_maps_verdict` PASS，其余不回归。

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/cli/domain_machine.rs src-tauri/src/cli/domain_cred.rs
git commit -m "feat(cli): implement machine authorize (preflight->bootstrap, USB fallback, --save-as)"
```

---

### Task 5: 全量构建 + help 冒烟 + 收尾

**Files:** 无（验证）

- [ ] **Step 1: 全量 lib 测试**

Run: `cd src-tauri && cargo test --lib 2>&1 | tail -25`
Expected: 全绿，无回归。

- [ ] **Step 2: 构建 CLI 二进制**

Run: `cd src-tauri && cargo build --bin uecm-cli 2>&1 | tail -15`
Expected: 编译成功。

- [ ] **Step 3: help 冒烟（确认两条命令 + flag 渲染正确）**

Run:
```bash
./target/debug/uecm-cli machine deep-scan --help
./target/debug/uecm-cli machine authorize --help
```
Expected: `deep-scan` 显示 `--machine-ids` / `--all` / `--cred-alias` 等；`authorize` 额外显示 `--save-as`。

- [ ] **Step 4: 提交 spec + plan**

```bash
git add docs/superpowers/specs/2026-05-20-machine-onboarding-cli-design.md docs/superpowers/plans/2026-05-20-machine-onboarding-cli.md
git commit -m "docs(plan): machine onboarding CLI spec + implementation plan"
```

---

## Self-Review 结论

- **Spec 覆盖**：deep-scan（Task 3）、authorize（Task 4，含 USB 回落 + --save-as）、共享凭据一次解析（Task 2）、命令名/选择器/Windows-only（Task 1+args 文档注释）、测试策略（Task 3/4 + Task 5）。步骤 1/2（scan/add）用现有命令，spec 已列为不新增——无对应 task 是预期。
- **占位符**：无 TBD/TODO；每个改代码步骤都给了完整代码。
- **类型一致**：`AuthorizeStep` / `authorize_decision` / `resolve_target_ids` / `CredentialArgs::inline` / `save_resolved` 在定义与调用处命名一致。
- **真机验收**：preflight/bootstrap/cred-store 为 Windows-only，单元测试只覆盖纯逻辑 + 跳过/批量行为；真实 WinRM 在 lanPC 人工跑。
