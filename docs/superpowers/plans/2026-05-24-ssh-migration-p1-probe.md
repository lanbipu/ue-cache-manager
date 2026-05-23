# SSH 迁移 P1 — 探测迁移 winrm::probe → ssh::probe + 新建 ssh CLI 域 实现计划

> REQUIRED SUB-SKILL: superpowers:executing-plans(本会话 inline 执行)。对应 spec P1。提交 `git -c commit.gpgsign=false`,只新增 commit。
> 前置:P0 完成(lanPC 重 onboard,from_config 通)。基线 `cargo test --lib` 975。

**Goal:** 把 machine/discovery 的 `winrm::probe` 换成 `ssh::probe`(走 uecm-svc key 认证),给 `ssh::probe` 加 loopback 旁路,改 deep_scan 的 winrm 提示,并新建 `ssh` CLI 域(起步只 `probe`,为 P5a 退役 winrm 域铺路)。不删 winrm.rs。

**Architecture:** `ssh::ProbeResult` 与 `winrm::ProbeResult` 字段相同(`{ok,message,latency_ms}`),调用方 `.ok`/`.message` 无需改。`SshExecutor::probe` 是方法,需 `from_config()`。loopback 复用 `core::loopback::is_loopback_target`(winrm::probe 已用)。

---

## Task 1:ssh::probe 加 loopback 旁路 + 单测

**Files:** Modify `src-tauri/src/core/ssh.rs`(`SshExecutor::probe`,~370)

- [ ] **Step 1:在 SshExecutor::probe 开头加 loopback 旁路**

在 `fn probe(&self, host: &str, ssh_user: Option<&str>) -> UecmResult<ProbeResult> {` 的第一行(`let started = ...` 之前)插入:
```rust
        // Loopback target (operator probing its own box): there is no point
        // SSHing to self, and a real ssh-to-self can hit host-key/loopback
        // quirks. Mirror winrm::probe's bypass.
        if crate::core::loopback::is_loopback_target(host) {
            return Ok(ProbeResult {
                ok: true,
                message: "loopback target; ssh bypassed".to_string(),
                latency_ms: 0,
            });
        }
```

- [ ] **Step 2:加单测(loopback 不 spawn ssh)**

在 `core/ssh.rs` `#[cfg(test)] mod tests` 里加:
```rust
    #[test]
    fn probe_bypasses_loopback_without_spawning_ssh() {
        let exec = SshExecutor {
            key_path: std::path::PathBuf::from("/nonexistent/key"),
            known_hosts: std::path::PathBuf::from("/nonexistent/kh"),
            default_user: "uecm-svc".into(),
            staging_root: STAGING_ROOT.to_string(),
        };
        // 127.0.0.1 is loopback → returns Ok without spawning ssh (which would
        // fail on the nonexistent key).
        let p = exec.probe("127.0.0.1", None).unwrap();
        assert!(p.ok);
        assert!(p.message.contains("loopback"));
    }
```

- [ ] **Step 3:跑测试**

Run: `cd src-tauri && cargo test --lib core::ssh::tests::probe_bypasses_loopback -- --nocapture`
Expected: PASS。

- [ ] **Step 4:Commit**

```bash
git add src-tauri/src/core/ssh.rs
git -c commit.gpgsign=false commit -m "feat(ssh): loopback bypass in ssh::probe (P1)

Mirror winrm::probe: a loopback target returns Ok without spawning ssh, so the
operator probing its own box doesn't hit ssh-to-self host-key/loopback quirks.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

## Task 2:domain_machine.rs 两处 probe → ssh::probe + deep_scan 提示

**Files:** Modify `src-tauri/src/cli/domain_machine.rs`(refresh ~254-261,deep_scan ~456-475)

- [ ] **Step 1:refresh 的 probe 换 ssh**

把 `let probe_result = match &creds { Some((u,p)) => winrm::probe_with_credential(...), None => winrm::probe(...) };` 换成:
```rust
    let probe_result = {
        let exec = crate::core::ssh::SshExecutor::from_config()?;
        exec.probe(&host, None)
    };
```
并把上面 `label: "winrm probe".into()` 改为 `"ssh probe".into()`,错误文案 `"winrm probe failed: {}"` 改 `"ssh probe failed: {}"`。`creds` 若变 unused,加 `let _ = &creds;`(operator cred 在 SSH 世界 P4 才彻底清,P1 先忽略)。

- [ ] **Step 2:deep_scan 的 probe 换 ssh + 提示改 SSH**

把 deep_scan 里 `let probe_ok = match &resolved { ... winrm::probe... };` 换成(循环外先建一个 exec 复用):
- 在 deep_scan 循环**之前**加 `let ssh_exec = crate::core::ssh::SshExecutor::from_config()?;`
- 循环内:`let probe_ok = ssh_exec.probe(&host, None).map(|r| r.ok).unwrap_or(false);`
- hint `"run \`uecm-cli machine authorize\` to open WinRM first"` → `"node not reachable over SSH; onboard it via UECM-Bootstrap.cmd (or check uecm-svc/sshd)"`,reason `"WinRM unreachable"` → `"SSH unreachable"`。

- [ ] **Step 3:build + 既有测试**

Run: `cd src-tauri && cargo test --lib cli::domain_machine 2>&1 | tail -5`
Expected: 编译通过 + 既有 domain_machine 测试过(deep_scan_skips_winrm_unreachable 那条若断言文案,改断言或确认仍过)。

- [ ] **Step 4:Commit**(`feat(ssh): machine refresh/deep_scan probe via ssh::probe (P1)`)

## Task 3:commands/discovery.rs probe → ssh::probe

**Files:** Modify `src-tauri/src/commands/discovery.rs`(~74)

- [ ] **Step 1:换 probe**(line 88 已有 `from_config()`,提到 probe 前复用)

把 `match winrm::probe(&machine.ip) { ... }` 换成先建 exec 再 probe:
```rust
    let exec = crate::core::ssh::SshExecutor::from_config()?;
    match exec.probe(&machine.ip, None) {
        Ok(p) if p.ok => { data_machines::mark_seen(&db, machine_id, "online")?; }
        Ok(_) => { data_machines::mark_seen(&db, machine_id, "offline")?; return Ok(refresh_err(machine_id, false, "node unreachable over SSH")); }
        Err(e) => { data_machines::mark_seen(&db, machine_id, "offline")?; return Ok(refresh_err(machine_id, false, format!("probe failed: {}", e))); }
    }
```
并把下面原有的 `let exec = ...from_config()?;`(detect_ue 用,~88)删掉改用上面这个 exec(避免建两次);若类型/借用冲突,保留两个局部 exec 也可(from_config 幂等)。`RefreshResult.winrm_ok` 字段名**不动**(Vue 读,契约 §5.3)。

- [ ] **Step 2:build**

Run: `cd src-tauri && cargo check --all-targets 2>&1 | tail -3`
Expected: 通过。

- [ ] **Step 3:Commit**(`feat(ssh): discovery refresh_machine probe via ssh::probe (P1)`)

## Task 4:新建 ssh CLI 域(probe)

**Files:** Modify `src-tauri/src/cli/args.rs`、`src-tauri/src/cli/run.rs`、`src-tauri/src/cli/mod.rs`;Create `src-tauri/src/cli/domain_ssh.rs`

- [ ] **Step 1:args.rs 加 Domain::Ssh + SshAction**

在 `Domain` enum 里(`Winrm {...}` 附近)加:
```rust
    /// SSH transport onboarding + probe (replaces the retiring winrm domain).
    Ssh {
        #[command(subcommand)]
        action: SshAction,
    },
```
并在 `WinrmAction` enum 附近加:
```rust
#[derive(clap::Subcommand, Debug)]
pub enum SshAction {
    /// Probe a host's SSH reachability (uecm-svc key auth).
    Probe { host: String },
}
```
(`package-bootstrap`/`authorize` 在 P5a 加。)

- [ ] **Step 2:cli/mod.rs 注册模块**

加 `pub mod domain_ssh;`(在 `pub mod domain_winrm;` 附近)。

- [ ] **Step 3:Create cli/domain_ssh.rs**

```rust
//! `uecm-cli ssh <action>` handlers (SSH transport onboarding + probe).

use crate::cli::args::SshAction;
use crate::cli::output::Event;
use crate::cli::run::Ctx;
use crate::error::{UecmError, UecmResult};

pub fn handle(ctx: &mut Ctx<'_>, action: SshAction) -> UecmResult<()> {
    match action {
        SshAction::Probe { host } => probe(ctx, &host),
    }
}

fn probe(ctx: &mut Ctx<'_>, host: &str) -> UecmResult<()> {
    let exec = crate::core::ssh::SshExecutor::from_config()?;
    let result = exec.probe(host, None)?;
    if !result.ok {
        return Err(UecmError::SshConnect(format!(
            "ssh probe of {} reported failure: {}",
            host, result.message
        )));
    }
    ctx.emitter
        .emit_event(&Event::Completed {
            summary: serde_json::json!({
                "host": host,
                "ok": result.ok,
                "message": result.message,
                "latency_ms": result.latency_ms,
            }),
        })
        .ok();
    Ok(())
}
```
> 核对 `Ctx` / `Event::Completed` / `emit_event` 真实签名(读 cli/run.rs + cli/output.rs);若 `probe` 需要 db-free,见下 run.rs。`RemoteExecutor` trait 需 in-scope 才能调 `.probe`(`use crate::core::ssh::RemoteExecutor;` 或全路径)。

- [ ] **Step 4:run.rs 接 dispatch + db-free**

- import:`domain_winrm` 那行加 `domain_ssh`。
- db-free 判定(`Domain::Winrm { .. } => false` 附近)加 `Domain::Ssh { .. } => false`(probe 不需 DB)。
- dispatch(`Domain::Winrm { action } => domain_winrm::handle(...)` 附近)加 `Domain::Ssh { action } => domain_ssh::handle(&mut ctx, action),`。

- [ ] **Step 5:build + help 核对**

Run: `cd src-tauri && cargo build 2>&1 | tail -3 && ./target/debug/uecm ssh --help`
Expected: 编译通过 + `ssh probe <host>` 出现在 help。

- [ ] **Step 6:Commit**(`feat(ssh): new 'ssh' CLI domain with probe (P1)`)

## Task 5:P1 收口

- [ ] **Step 1:全量 test + grep 核对**

Run: `cd src-tauri && cargo test --lib 2>&1 | tail -3`
Expected: 全绿(数量 ≥975 + 新增 1)。
Run: `grep -rn 'winrm::probe' src-tauri/src --include='*.rs' | grep -v 'target/' | grep -v 'winrm.rs'`
Expected: **空**(machine/discovery 的 winrm::probe 调用清零;winrm.rs 内定义留到 P5)。

- [ ] **Step 2:codex review**

Run: `node "$(ls -t ~/.claude/plugins/cache/openai-codex/codex/*/scripts/codex-companion.mjs 2>/dev/null | head -1)" review --base <P1首commit的父> --wait`
原样贴;报问题先修完。

- [ ] **Step 3:lanPC 真机抽验**

`uecm-cli ssh probe <lanPC>` 经 from_config 应 ok(P0 已通 uecm-svc 登录)。因 mac 只能 scan/list,真机抽验在 lanPC 上跑 build 后的 uecm-cli,或用 `ssh uecm-svc@lanpc` 间接确认 probe 语义。标记真机验证结果。

## Self-Review(对照 spec P1)
- probe 迁移(machine refresh + deep_scan + discovery)→ Task 2/3 ✓;loopback 旁路 → Task 1 ✓;deep_scan 提示改 SSH → Task 2 ✓;新 ssh 域 probe → Task 4 ✓;winrm_ok 字段冻结 → Task 3 不改字段名 ✓;不删 winrm.rs ✓。
- 类型一致:ssh::ProbeResult 字段同 winrm,调用方 .ok/.message/.latency_ms 不变。
