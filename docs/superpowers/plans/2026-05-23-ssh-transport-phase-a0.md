# UECM SSH 传输重构 · Phase A0 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **本环境提交注意**：1Password 作为 git 签名器，agent 锁定时 `git commit` 会报 `1Password: failed to fill whole buffer`。本计划所有 commit 命令都用 `git -c commit.gpgsign=false commit ...` 规避。

**Goal:** 给 UECM 落地 SSH 传输的地基——`RemoteExecutor` trait + `SshExecutor`（shell out 系统 ssh）+ 专用 key 管理 + 节点脚本暂存机制 + machines.ssh_user，全部带单测，不动任何现有 WinRM 调用点。

**Architecture:** 新增 `core/ssh.rs`（传输 + 暂存）与 `core/keystore.rs`（专用 ed25519 key，靠 shell out `ssh-keygen` 生成，不引 crypto crate）。错误类型加 4 个变体。machines 表加 `ssh_user` 列。本 phase 只建地基与单测，**不改 domain 调用方、不删 winrm.rs**——保证现有功能零回归。

**Tech Stack:** Rust（rusqlite 0.31 / serde_json / sha2 0.10 / base64 0.22 / directories 5.0 已在 Cargo）、系统 `ssh` + `ssh-keygen` + `scp`、SQLite。

参考 spec：`docs/superpowers/specs/2026-05-23-ssh-transport-rearchitecture-design.md`（§3 传输 / §4.1 keystore / §5.4 暂存 / §10 测试 / §11 A0）。

---

## 文件结构

| 文件 | 职责 | 动作 |
|---|---|---|
| `src-tauri/src/error.rs` | 加 `SshConnect` / `NodeScript` / `Timeout` / `ScriptStaging` 变体 + 映射 | Modify |
| `src-tauri/src/startup.rs` | 加 `resolve_config_dir()`（key/known_hosts 落点） | Modify |
| `src-tauri/src/data/schema.rs` | 追加迁移 `022_machines_ssh_user` | Modify |
| `src-tauri/src/data/machines.rs` | 加 `get_ssh_user` / `set_ssh_user` | Modify |
| `src-tauri/src/core/keystore.rs` | 专用 ed25519 key 生成/路径/导出公钥 | Create |
| `src-tauri/src/core/ssh.rs` | `NodeScript` / `RemoteExecutor` trait / `SshExecutor` / argv builder / 退出码映射 / probe / 暂存 manifest | Create |
| `src-tauri/src/core/mod.rs` | 注册 `keystore` / `ssh` 模块 | Modify |

---

## Task 1：错误类型加 4 个变体

**Files:**
- Modify: `src-tauri/src/error.rs`

- [ ] **Step 1: 写失败测试**（追加到 `error.rs` 的 `#[cfg(test)] mod tests`）

```rust
    #[test]
    fn ssh_connect_error_serializes_with_code() {
        let err = UecmError::SshConnect("port 22 refused".to_string());
        let payload: ErrorPayload = err.into();
        assert_eq!(payload.code, "SSH_CONNECT");
        assert!(payload.message.contains("port 22 refused"));
    }

    #[test]
    fn node_script_error_carries_exit_and_stderr() {
        let err = UecmError::NodeScript { exit: 3, stderr: "boom".to_string() };
        let payload: ErrorPayload = err.into();
        assert_eq!(payload.code, "NODE_SCRIPT");
        assert!(payload.message.contains("3"));
        assert!(payload.message.contains("boom"));
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p uecm --lib error::tests -- --nocapture`
Expected: 编译失败（`SshConnect` / `NodeScript` 变体不存在）。

- [ ] **Step 3: 加变体 + 两处映射 + Serialize**

在 `enum UecmError` 末尾（`Configuration` 之后）加：

```rust
    #[error("ssh connect failed: {0}")]
    SshConnect(String),

    #[error("node script failed (exit {exit}): {stderr}")]
    NodeScript { exit: i32, stderr: String },

    #[error("operation timed out: {0}")]
    Timeout(String),

    #[error("script staging failed: {0}")]
    ScriptStaging(String),
```

在 `impl From<UecmError> for ErrorPayload` 的 `match` 末尾加：

```rust
            UecmError::SshConnect(_) => "SSH_CONNECT",
            UecmError::NodeScript { .. } => "NODE_SCRIPT",
            UecmError::Timeout(_) => "TIMEOUT",
            UecmError::ScriptStaging(_) => "SCRIPT_STAGING",
```

在 `impl Serialize for UecmError` 的 `match self` 末尾加同样四行（注意这里匹配 `self`，写法相同）。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p uecm --lib error::tests`
Expected: PASS（含原有 3 个 + 新 2 个）。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/error.rs
git -c commit.gpgsign=false commit -m "feat(error): add SshConnect/NodeScript/Timeout/ScriptStaging variants"
```

---

## Task 2：配置目录 helper

**Files:**
- Modify: `src-tauri/src/startup.rs`

- [ ] **Step 1: 写失败测试**（追加到 `startup.rs` 的测试模块；若无则新建 `#[cfg(test)] mod tests`）

```rust
    #[test]
    fn config_dir_follows_db_path_override() {
        std::env::set_var("UECM_DB_PATH", "/tmp/uecm-test-abc/uecm.sqlite");
        let dir = resolve_config_dir().unwrap();
        assert_eq!(dir, std::path::PathBuf::from("/tmp/uecm-test-abc"));
        std::env::remove_var("UECM_DB_PATH");
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p uecm --lib startup::`
Expected: 编译失败（`resolve_config_dir` 不存在）。

- [ ] **Step 3: 实现 `resolve_config_dir`**

在 `startup.rs` 加（紧跟 `resolve_db_path` 之后）：

```rust
/// Resolves the UECM config directory (where the SSH key, public key, and
/// known_hosts live). Mirrors `resolve_db_path`'s parent so a `UECM_DB_PATH`
/// test override keeps key material next to the test DB.
pub fn resolve_config_dir() -> UecmResult<PathBuf> {
    if let Ok(override_path) = env::var("UECM_DB_PATH") {
        if let Some(parent) = Path::new(&override_path).parent() {
            return Ok(parent.to_path_buf());
        }
    }
    let base = BaseDirs::new().ok_or_else(|| {
        UecmError::Configuration("failed to resolve user base directories".into())
    })?;
    Ok(base.data_dir().join(APP_IDENTIFIER))
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p uecm --lib startup::config_dir_follows_db_path_override`
Expected: PASS。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/startup.rs
git -c commit.gpgsign=false commit -m "feat(startup): add resolve_config_dir helper"
```

---

## Task 3：machines.ssh_user 迁移 + 读写

**Files:**
- Modify: `src-tauri/src/data/schema.rs`
- Modify: `src-tauri/src/data/machines.rs`

- [ ] **Step 1: 写失败测试**（追加到 `machines.rs` 测试模块；参考其现有 `mem_db()` / 建库辅助；若辅助名不同，照该文件已有测试的建库方式）

```rust
    #[test]
    fn ssh_user_round_trips_and_defaults_none() {
        let db = test_db();                       // 沿用本文件现有测试建库辅助
        let id = insert(&db, &Machine::new("RENDER-09", "192.168.10.29")).unwrap();
        assert_eq!(get_ssh_user(&db, id).unwrap(), None);
        set_ssh_user(&db, id, Some("uecm-svc")).unwrap();
        assert_eq!(get_ssh_user(&db, id).unwrap(), Some("uecm-svc".to_string()));
        set_ssh_user(&db, id, None).unwrap();
        assert_eq!(get_ssh_user(&db, id).unwrap(), None);
    }
```

> 注：`test_db()` 用本文件已有测试的建库函数名（打开 in-memory + `schema::migrate`）。若现有测试用的是别的名字（如 `mem_db()` / `setup()`），改成那个。

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p uecm --lib machines::`
Expected: 编译失败（`get_ssh_user` / `set_ssh_user` 不存在）。

- [ ] **Step 3: 追加迁移**

在 `schema.rs` 的 `MIGRATIONS` 数组**末尾**（`021_...` tuple 之后）追加新 tuple——必须是新 name，绝不改已有 tuple：

```rust
    (
        "022_machines_ssh_user",
        r#"
        ALTER TABLE machines ADD COLUMN ssh_user TEXT;
        "#,
    ),
```

- [ ] **Step 4: 实现读写**（加到 `machines.rs`，放在 `rename` 附近）

```rust
/// Returns the per-machine SSH login user, or None when unset (caller uses
/// the default `uecm-svc`).
pub fn get_ssh_user(db: &Db, id: i64) -> UecmResult<Option<String>> {
    let conn = db.lock().unwrap();
    let user: Option<String> = conn
        .query_row("SELECT ssh_user FROM machines WHERE id = ?", params![id], |r| r.get(0))
        .map_err(UecmError::from)?;
    Ok(user)
}

/// Sets (or clears, with None) the per-machine SSH login user.
pub fn set_ssh_user(db: &Db, id: i64, user: Option<&str>) -> UecmResult<()> {
    let conn = db.lock().unwrap();
    let n = conn.execute(
        "UPDATE machines SET ssh_user = ? WHERE id = ?",
        params![user, id],
    )?;
    if n == 0 {
        return Err(UecmError::InvalidInput(format!("no machine row with id {id}")));
    }
    Ok(())
}
```

- [ ] **Step 5: 跑测试确认通过**

Run: `cargo test -p uecm --lib machines::ssh_user_round_trips_and_defaults_none`
Expected: PASS。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/data/schema.rs src-tauri/src/data/machines.rs
git -c commit.gpgsign=false commit -m "feat(machines): add ssh_user column (migration 022) + accessors"
```

---

## Task 4：KeyStore（专用 ed25519 key，shell out ssh-keygen）

**Files:**
- Create: `src-tauri/src/core/keystore.rs`
- Modify: `src-tauri/src/core/mod.rs`

- [ ] **Step 1: 注册模块**

`core/mod.rs` 按字母序加一行：`pub mod keystore;`（在 `pub mod ini_scanner;` 与 `pub mod local_cache;` 之间，紧贴现有顺序即可）。

- [ ] **Step 2: 写失败测试**（写进 `keystore.rs` 末尾的 `#[cfg(test)] mod tests`）

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn ensure_keypair_generates_then_is_idempotent() {
        let dir = tempdir().unwrap();
        let ks = KeyStore::at(dir.path());
        assert!(!ks.private_key_path().exists());
        ks.ensure_keypair().unwrap();
        assert!(ks.private_key_path().exists());
        assert!(ks.public_key_path().exists());
        let pub1 = ks.public_key().unwrap();
        // 第二次调用不重生成（公钥不变）
        ks.ensure_keypair().unwrap();
        assert_eq!(pub1, ks.public_key().unwrap());
        assert!(pub1.starts_with("ssh-ed25519 "));
    }
}
```

- [ ] **Step 3: 跑测试确认失败**

Run: `cargo test -p uecm --lib keystore::`
Expected: 编译失败（`KeyStore` 不存在）。

- [ ] **Step 4: 实现 `keystore.rs`**

```rust
//! UECM 专用 SSH 传输密钥。靠 shell out `ssh-keygen` 生成 ed25519 keypair
//! （ssh-keygen 与系统 ssh 一起安装，Mac/Windows 都有），不引入 crypto crate。
//! 私钥 / 公钥 / known_hosts 都落在应用配置目录。

use crate::error::{UecmError, UecmResult};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct KeyStore {
    dir: PathBuf,
}

impl KeyStore {
    /// 用显式目录构造（生产传 `startup::resolve_config_dir()?`，测试传 tempdir）。
    pub fn at(dir: &Path) -> Self {
        Self { dir: dir.to_path_buf() }
    }

    pub fn private_key_path(&self) -> PathBuf {
        self.dir.join("uecm_ed25519")
    }
    pub fn public_key_path(&self) -> PathBuf {
        self.dir.join("uecm_ed25519.pub")
    }
    pub fn known_hosts_path(&self) -> PathBuf {
        self.dir.join("known_hosts")
    }

    /// 私钥不存在则用 ssh-keygen 生成（空 passphrase，非交互）。已存在则 no-op。
    pub fn ensure_keypair(&self) -> UecmResult<()> {
        let key = self.private_key_path();
        if key.exists() {
            return Ok(());
        }
        std::fs::create_dir_all(&self.dir)?;
        let out = Command::new("ssh-keygen")
            .arg("-t").arg("ed25519")
            .arg("-f").arg(&key)
            .arg("-N").arg("")            // 空 passphrase
            .arg("-C").arg("uecm")
            .arg("-q")
            .output()
            .map_err(|e| UecmError::Configuration(format!("spawn ssh-keygen failed: {e}")))?;
        if !out.status.success() {
            return Err(UecmError::Configuration(format!(
                "ssh-keygen failed: {}",
                String::from_utf8_lossy(&out.stderr)
            )));
        }
        Ok(())
    }

    /// 读取并返回公钥串（含算法前缀，如 `ssh-ed25519 AAAA... uecm`）。
    pub fn public_key(&self) -> UecmResult<String> {
        let s = std::fs::read_to_string(self.public_key_path())?;
        Ok(s.trim().to_string())
    }
}
```

- [ ] **Step 5: 跑测试确认通过**

Run: `cargo test -p uecm --lib keystore::ensure_keypair_generates_then_is_idempotent`
Expected: PASS（Mac/Linux 上 ssh-keygen 存在，真生成一对 key）。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/core/keystore.rs src-tauri/src/core/mod.rs
git -c commit.gpgsign=false commit -m "feat(keystore): UECM ed25519 transport key via ssh-keygen"
```

---

## Task 5：NodeScript + RemoteExecutor trait + ssh argv builder

**Files:**
- Create: `src-tauri/src/core/ssh.rs`
- Modify: `src-tauri/src/core/mod.rs`

- [ ] **Step 1: 注册模块**

`core/mod.rs` 加一行：`pub mod ssh;`（按字母序在 `pub mod shares;` 之后）。

- [ ] **Step 2: 写失败测试**（写进 `ssh.rs` 末尾 `#[cfg(test)] mod tests`）

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_ssh_args_uses_key_known_hosts_and_file() {
        let args = build_ssh_args(
            "/cfg/uecm_ed25519",
            "/cfg/known_hosts",
            "uecm-svc",
            "RENDER-01",
            "health-probes.ps1",
            r"C:\ProgramData\UECM\ps-scripts",
        );
        assert!(args.contains(&"-i".to_string()));
        assert!(args.contains(&"/cfg/uecm_ed25519".to_string()));
        assert!(args.iter().any(|a| a == "UserKnownHostsFile=/cfg/known_hosts"));
        assert!(args.iter().any(|a| a == "StrictHostKeyChecking=accept-new"));
        assert!(args.iter().any(|a| a == "BatchMode=yes"));
        assert!(args.contains(&"uecm-svc@RENDER-01".to_string()));
        // 远程命令用 -File 引用暂存脚本，绝不内联正文
        let remote = args.last().unwrap();
        assert!(remote.contains(r"-File C:\ProgramData\UECM\ps-scripts\health-probes.ps1"));
        assert!(remote.contains("powershell.exe -NoProfile -ExecutionPolicy Bypass"));
        assert!(!remote.contains("-EncodedCommand"));
    }

    #[test]
    fn map_exit_distinguishes_connect_from_script_failure() {
        match map_exit(255, "ssh: connect to host RENDER-01 port 22: Connection refused") {
            UecmError::SshConnect(m) => assert!(m.contains("Connection refused")),
            other => panic!("expected SshConnect, got {other:?}"),
        }
        match map_exit(3, "node side blew up") {
            UecmError::NodeScript { exit, stderr } => {
                assert_eq!(exit, 3);
                assert!(stderr.contains("blew up"));
            }
            other => panic!("expected NodeScript, got {other:?}"),
        }
    }
}
```

- [ ] **Step 3: 跑测试确认失败**

Run: `cargo test -p uecm --lib ssh::`
Expected: 编译失败（`build_ssh_args` / `map_exit` 不存在）。

- [ ] **Step 4: 实现类型 + 纯函数**

```rust
//! SSH 传输：shell out 系统 `ssh`，在节点上 `-File` 跑预置的纯脚本，stdin 喂 JSON 参数。
//! 这是 UECM 唯一做远程的地方。argv 构造与退出码映射是纯函数，可在任意平台单测。

use crate::error::{UecmError, UecmResult};
use serde::de::DeserializeOwned;
use serde::Deserialize;

/// 节点暂存路径（bootstrap 推到这里）。
pub const STAGING_ROOT: &str = r"C:\ProgramData\UECM\ps-scripts";

/// 一次远程调用：引用节点上预置的脚本名 + 参数（含 secret，运行时经 stdin JSON 传）。
pub struct NodeScript {
    pub name: &'static str,
    pub args: serde_json::Value,
    pub ssh_user: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProbeResult {
    pub ok: bool,
    pub message: String,
    pub latency_ms: i64,
}

/// 传输抽象。生产实现是 `SshExecutor`；测试用 fake 注入预置 JSON。
pub trait RemoteExecutor {
    fn run(&self, host: &str, script: &NodeScript) -> UecmResult<String>;
    fn probe(&self, host: &str) -> UecmResult<ProbeResult>;
}

/// 跑脚本并把 stdout 解析成 JSON。
pub fn run_json<T: DeserializeOwned>(
    exec: &dyn RemoteExecutor,
    host: &str,
    script: &NodeScript,
) -> UecmResult<T> {
    let raw = exec.run(host, script)?;
    serde_json::from_str(&raw)
        .map_err(|e| UecmError::NodeScript { exit: 0, stderr: format!("bad JSON: {e} (raw: {raw})") })
}

/// 拼系统 ssh 的 argv（纯函数，便于单测）。
pub fn build_ssh_args(
    key_path: &str,
    known_hosts: &str,
    ssh_user: &str,
    host: &str,
    script_name: &str,
    staging_root: &str,
) -> Vec<String> {
    let remote = format!(
        "powershell.exe -NoProfile -ExecutionPolicy Bypass -File {staging_root}\\{script_name}"
    );
    vec![
        "-i".into(), key_path.into(),
        "-o".into(), "IdentitiesOnly=yes".into(),
        "-o".into(), format!("UserKnownHostsFile={known_hosts}"),
        "-o".into(), "StrictHostKeyChecking=accept-new".into(),
        "-o".into(), "BatchMode=yes".into(),
        "-o".into(), "ConnectTimeout=10".into(),
        format!("{ssh_user}@{host}"),
        remote,
    ]
}

/// ssh 进程退出码 → 错误分类。255 = ssh 自身（连接/认证/host-key）；其余 = 节点脚本失败。
pub fn map_exit(code: i32, stderr: &str) -> UecmError {
    if code == 255 {
        UecmError::SshConnect(stderr.trim().to_string())
    } else {
        UecmError::NodeScript { exit: code, stderr: stderr.trim().to_string() }
    }
}
```

- [ ] **Step 5: 跑测试确认通过**

Run: `cargo test -p uecm --lib ssh::`
Expected: PASS（两个纯函数测试）。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/core/ssh.rs src-tauri/src/core/mod.rs
git -c commit.gpgsign=false commit -m "feat(ssh): NodeScript/RemoteExecutor trait + argv builder + exit mapping"
```

---

## Task 6：SshExecutor.run（真 shell out + stdin JSON + 编码解码）

**Files:**
- Modify: `src-tauri/src/core/ssh.rs`

- [ ] **Step 1: 写测试**（用 fake executor 验证 `run_json` 解析契约；真 ssh 执行留 lanPC 集成验证。追加到 `ssh::tests`）

```rust
    struct FakeExec(String);
    impl RemoteExecutor for FakeExec {
        fn run(&self, _h: &str, _s: &NodeScript) -> UecmResult<String> { Ok(self.0.clone()) }
        fn probe(&self, _h: &str) -> UecmResult<ProbeResult> {
            Ok(ProbeResult { ok: true, message: "fake".into(), latency_ms: 1 })
        }
    }

    #[derive(serde::Deserialize)]
    struct Demo { ok: bool, value: i64 }

    #[test]
    fn run_json_parses_node_stdout() {
        let exec = FakeExec(r#"{"ok":true,"value":42}"#.to_string());
        let script = NodeScript { name: "x.ps1", args: serde_json::json!({}), ssh_user: None };
        let d: Demo = run_json(&exec, "RENDER-01", &script).unwrap();
        assert!(d.ok && d.value == 42);
    }

    #[test]
    fn run_json_surfaces_bad_json_as_node_script_error() {
        let exec = FakeExec("not json".to_string());
        let script = NodeScript { name: "x.ps1", args: serde_json::json!({}), ssh_user: None };
        let err = run_json::<Demo>(&exec, "RENDER-01", &script).unwrap_err();
        assert!(matches!(err, UecmError::NodeScript { .. }));
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p uecm --lib ssh::run_json`
Expected: 编译/运行失败（fake 与 `run_json` 已具备 → 实际此步可能直接编译通过；若通过则跳到 Step 4 补 `SshExecutor`）。

- [ ] **Step 3: 实现 `SshExecutor`**（追加到 `ssh.rs`，trait 定义之后）

```rust
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// 生产传输实现：用系统 ssh 在节点跑预置脚本。
pub struct SshExecutor {
    pub key_path: PathBuf,
    pub known_hosts: PathBuf,
    pub default_user: String,           // "uecm-svc"
    pub staging_root: String,           // STAGING_ROOT
}

impl SshExecutor {
    /// GBK 兜底解码（节点 PowerShell 5.1 在中文系统可能吐 CP936 stderr）。
    fn decode(bytes: &[u8]) -> String {
        match std::str::from_utf8(bytes) {
            Ok(s) => s.to_string(),
            Err(_) => encoding_rs::GBK.decode(bytes).0.into_owned(),
        }
    }
}

impl RemoteExecutor for SshExecutor {
    fn run(&self, host: &str, script: &NodeScript) -> UecmResult<String> {
        let user = script.ssh_user.as_deref().unwrap_or(&self.default_user);
        let args = build_ssh_args(
            &self.key_path.to_string_lossy(),
            &self.known_hosts.to_string_lossy(),
            user,
            host,
            script.name,
            &self.staging_root,
        );
        let mut child = Command::new("ssh")
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| UecmError::SshConnect(format!("spawn ssh failed: {e}")))?;
        // 参数 JSON 经 stdin 喂入（不上命令行，secret 不暴露）。
        {
            let stdin = child.stdin.as_mut()
                .ok_or_else(|| UecmError::SshConnect("open ssh stdin failed".into()))?;
            let payload = serde_json::to_vec(&script.args)
                .map_err(|e| UecmError::InvalidInput(format!("encode args: {e}")))?;
            stdin.write_all(&payload).map_err(UecmError::from)?;
        }
        let out = child.wait_with_output().map_err(UecmError::from)?;
        let code = out.status.code().unwrap_or(-1);
        if !out.status.success() {
            return Err(map_exit(code, &Self::decode(&out.stderr)));
        }
        Ok(Self::decode(&out.stdout))
    }

    fn probe(&self, host: &str) -> UecmResult<ProbeResult> {
        let started = std::time::Instant::now();
        let user = &self.default_user;
        let mut args = build_ssh_args(
            &self.key_path.to_string_lossy(),
            &self.known_hosts.to_string_lossy(),
            user, host, "noop", &self.staging_root,
        );
        // probe 不跑脚本：替换最后的远程命令为一个 noop。
        if let Some(last) = args.last_mut() {
            *last = "powershell.exe -NoProfile -Command exit 0".into();
        }
        let out = Command::new("ssh").args(&args).output()
            .map_err(|e| UecmError::SshConnect(format!("spawn ssh failed: {e}")))?;
        let latency_ms = started.elapsed().as_millis() as i64;
        if out.status.success() {
            Ok(ProbeResult { ok: true, message: "ssh ok".into(), latency_ms })
        } else {
            let code = out.status.code().unwrap_or(-1);
            Err(map_exit(code, &Self::decode(&out.stderr)))
        }
    }
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p uecm --lib ssh::`
Expected: PASS（fake 路径全过；SshExecutor 编译通过）。

- [ ] **Step 5: lanPC 集成验证（手动，记录结果）**

在 lanPC（真节点已开 OpenSSH + 授权 uecm 公钥后，A1 完成才能跑；A0 阶段先标记 TODO，A1 后补跑）：构造 `SshExecutor`，对一个最大脚本（`zen-verify-rules.ps1`）`-File` 跑通，确认无命令行长度报错。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/core/ssh.rs
git -c commit.gpgsign=false commit -m "feat(ssh): SshExecutor run/probe via system ssh + stdin JSON args"
```

---

## Task 7：脚本暂存 manifest（SHA256 + 漂移检测）

**Files:**
- Modify: `src-tauri/src/core/ssh.rs`

- [ ] **Step 1: 写失败测试**（追加到 `ssh::tests`）

```rust
    #[test]
    fn manifest_lists_files_with_stable_hashes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.ps1"), b"hello").unwrap();
        std::fs::write(dir.path().join("b.ps1"), b"world").unwrap();
        let m1 = compute_manifest(dir.path()).unwrap();
        assert_eq!(m1.len(), 2);
        assert!(m1.contains_key("a.ps1") && m1.contains_key("b.ps1"));
        // 改一个文件 → 该项 hash 变，另一项不变
        std::fs::write(dir.path().join("a.ps1"), b"changed").unwrap();
        let m2 = compute_manifest(dir.path()).unwrap();
        assert_ne!(m1["a.ps1"], m2["a.ps1"]);
        assert_eq!(m1["b.ps1"], m2["b.ps1"]);
    }

    #[test]
    fn drifted_files_detects_only_changed() {
        use std::collections::BTreeMap;
        let mut remote = BTreeMap::new();
        remote.insert("a.ps1".to_string(), "AAA".to_string());
        remote.insert("b.ps1".to_string(), "BBB".to_string());
        let mut local = BTreeMap::new();
        local.insert("a.ps1".to_string(), "AAA".to_string());   // 同
        local.insert("b.ps1".to_string(), "ZZZ".to_string());   // 变
        local.insert("c.ps1".to_string(), "CCC".to_string());   // 新增
        let drift = drifted_files(&local, &remote);
        assert_eq!(drift, vec!["b.ps1".to_string(), "c.ps1".to_string()]);
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p uecm --lib ssh::manifest`
Expected: 编译失败（`compute_manifest` / `drifted_files` 不存在）。

- [ ] **Step 3: 实现 manifest 逻辑**（追加到 `ssh.rs`）

```rust
use std::collections::BTreeMap;
use std::path::Path;

/// 算目录下所有 `.ps1` 文件的 SHA256（文件名 → 十六进制 hash）。
pub fn compute_manifest(dir: &Path) -> UecmResult<BTreeMap<String, String>> {
    use sha2::{Digest, Sha256};
    let mut map = BTreeMap::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("ps1") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let bytes = std::fs::read(&path)?;
        let hash = Sha256::digest(&bytes);
        map.insert(name, format!("{:x}", hash));
    }
    Ok(map)
}

/// 对比本地与节点 manifest，返回需要重推的文件名（变更 + 新增），排序稳定。
pub fn drifted_files(
    local: &BTreeMap<String, String>,
    remote: &BTreeMap<String, String>,
) -> Vec<String> {
    let mut out: Vec<String> = local
        .iter()
        .filter(|(name, hash)| remote.get(*name) != Some(*hash))
        .map(|(name, _)| name.clone())
        .collect();
    out.sort();
    out
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p uecm --lib ssh::`
Expected: PASS。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/core/ssh.rs
git -c commit.gpgsign=false commit -m "feat(ssh): script staging manifest (sha256) + drift detection"
```

---

## Task 8：A0 收口验证

**Files:** 无新增（只跑全量）。

- [ ] **Step 1: 全量编译 + 测试**

Run: `cargo test -p uecm --lib`
Expected: 全绿（含原有测试 + A0 新增）；无 warning 级别的 unused（keystore/ssh 暂未被 domain 调用，但有自带测试引用，不会触发 dead_code）。

- [ ] **Step 2: 确认零回归**

Run: `cargo build -p uecm`
Expected: 编译通过；`winrm.rs` / `credentials.rs` / 所有 domain 调用方未改动。

- [ ] **Step 3: 提交收口（若有 lint/格式微调）**

```bash
git add -A src-tauri/src
git -c commit.gpgsign=false commit -m "chore(ssh): Phase A0 transport+keystore foundation green" || echo "nothing to commit"
```

---

## Self-Review

- **Spec 覆盖**（对 §11 A0 逐条）：`core/ssh.rs`=Task5-7 ✅；`SshExecutor`+`RemoteExecutor` trait=Task5-6 ✅；脚本暂存机制（manifest+drift）=Task7 ✅（scp 实推留 A1，因需真节点）；`core/keystore.rs`=Task4 ✅；machines.ssh_user 迁移=Task3 ✅；argv/manifest 单测=Task5/7 ✅；probe=Task6 ✅。
- **占位符扫描**：无 TBD/TODO 代码步；唯一标注「留 A1 / lanPC」的是 Task6-Step5 真 ssh 执行——这是合理的环境依赖（Mac 上无真 Windows 节点），不是计划缺口；逻辑/纯函数全部本 phase 单测覆盖。
- **类型一致性**：`NodeScript{name,args,ssh_user}`、`RemoteExecutor{run,probe}`、`ProbeResult{ok,message,latency_ms}`、`build_ssh_args(...)`、`map_exit(...)`、`compute_manifest`/`drifted_files`、`KeyStore::at/ensure_keypair/public_key`、`get/set_ssh_user`、`resolve_config_dir` 在各 task 间签名一致。
- **范围**：A0 只建地基 + 单测，不动 domain 调用方、不删 winrm——零回归，可独立编译测试。A1-A5 / 子项目 B 在 spec 已分解，各自将出独立 plan。
