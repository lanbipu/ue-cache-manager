//! SSH 传输：shell out 系统 `ssh`，在节点上 `-File` 跑预置的纯脚本，stdin 喂 JSON 参数。
//! 这是 UECM 唯一做远程的地方。argv 构造与退出码映射是纯函数，可在任意平台单测。

use crate::error::{UecmError, UecmResult};
use serde::de::DeserializeOwned;
use serde::Deserialize;

/// 节点脚本暂存路径（bootstrap 推到这里）。
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
    serde_json::from_str(&raw).map_err(|e| UecmError::NodeScript {
        exit: 0,
        stderr: format!("bad JSON from node: {e} (raw: {raw})"),
    })
}

/// 拼系统 ssh 的 argv（纯函数，便于单测）。脚本正文绝不内联——只 `-File` 引用
/// 节点上预置的脚本，规避 Windows 远程命令行长度上限。
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
        "-i".into(),
        key_path.into(),
        "-o".into(),
        "IdentitiesOnly=yes".into(),
        "-o".into(),
        format!("UserKnownHostsFile={known_hosts}"),
        "-o".into(),
        "StrictHostKeyChecking=accept-new".into(),
        "-o".into(),
        "BatchMode=yes".into(),
        "-o".into(),
        "ConnectTimeout=10".into(),
        format!("{ssh_user}@{host}"),
        remote,
    ]
}

/// ssh 进程退出码 → 错误分类。255 = ssh 自身（连接/认证/host-key）；其余 = 节点脚本失败。
pub fn map_exit(code: i32, stderr: &str) -> UecmError {
    if code == 255 {
        UecmError::SshConnect(stderr.trim().to_string())
    } else {
        UecmError::NodeScript {
            exit: code,
            stderr: stderr.trim().to_string(),
        }
    }
}

/// 失败时组装错误明细。节点脚本约定可能把结构化 `{ok:false,message}` 写到 stdout
/// 后再非零退出；只取 stderr 会把这条信息丢掉，所以非空 stdout 必须纳入。
pub fn failure_detail(stdout: &str, stderr: &str) -> String {
    let stdout = stdout.trim();
    let stderr = stderr.trim();
    match (stdout.is_empty(), stderr.is_empty()) {
        (true, _) => stderr.to_string(),
        (false, true) => stdout.to_string(),
        (false, false) => format!("{stderr}\n[stdout] {stdout}"),
    }
}

use std::collections::BTreeMap;
use std::path::Path;

/// 算目录下所有 `.ps1` 文件的 SHA256（文件名 → 十六进制 hash），供节点脚本暂存
/// 漂移检测用。
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

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// 生产传输实现：用系统 ssh 在节点跑预置脚本，参数 JSON 经 stdin 喂入。
pub struct SshExecutor {
    pub key_path: PathBuf,
    pub known_hosts: PathBuf,
    pub default_user: String, // "uecm-svc"
    pub staging_root: String, // STAGING_ROOT
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
        // 参数 JSON 经 stdin 喂入（不上命令行，secret 不暴露在节点进程列表里）。
        {
            let stdin = child
                .stdin
                .as_mut()
                .ok_or_else(|| UecmError::SshConnect("open ssh stdin failed".into()))?;
            let payload = serde_json::to_vec(&script.args)
                .map_err(|e| UecmError::InvalidInput(format!("encode args: {e}")))?;
            stdin.write_all(&payload)?;
        }
        let out = child.wait_with_output()?;
        let code = out.status.code().unwrap_or(-1);
        if !out.status.success() {
            let detail = failure_detail(&Self::decode(&out.stdout), &Self::decode(&out.stderr));
            return Err(map_exit(code, &detail));
        }
        Ok(Self::decode(&out.stdout))
    }

    fn probe(&self, host: &str) -> UecmResult<ProbeResult> {
        let started = std::time::Instant::now();
        let mut args = build_ssh_args(
            &self.key_path.to_string_lossy(),
            &self.known_hosts.to_string_lossy(),
            &self.default_user,
            host,
            "noop",
            &self.staging_root,
        );
        // probe 不跑脚本：把最后的远程命令替换为一个 noop。
        if let Some(last) = args.last_mut() {
            *last = "powershell.exe -NoProfile -Command exit 0".into();
        }
        let out = Command::new("ssh")
            .args(&args)
            .output()
            .map_err(|e| UecmError::SshConnect(format!("spawn ssh failed: {e}")))?;
        let latency_ms = started.elapsed().as_millis() as i64;
        if out.status.success() {
            Ok(ProbeResult {
                ok: true,
                message: "ssh ok".into(),
                latency_ms,
            })
        } else {
            let code = out.status.code().unwrap_or(-1);
            Err(map_exit(code, &Self::decode(&out.stderr)))
        }
    }
}

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

    #[test]
    fn manifest_lists_files_with_stable_hashes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.ps1"), b"hello").unwrap();
        std::fs::write(dir.path().join("b.ps1"), b"world").unwrap();
        std::fs::write(dir.path().join("ignore.txt"), b"x").unwrap(); // 非 .ps1 不计入
        let m1 = compute_manifest(dir.path()).unwrap();
        assert_eq!(m1.len(), 2);
        assert!(m1.contains_key("a.ps1") && m1.contains_key("b.ps1"));
        std::fs::write(dir.path().join("a.ps1"), b"changed").unwrap();
        let m2 = compute_manifest(dir.path()).unwrap();
        assert_ne!(m1["a.ps1"], m2["a.ps1"]);
        assert_eq!(m1["b.ps1"], m2["b.ps1"]);
    }

    #[test]
    fn drifted_files_detects_only_changed() {
        let mut remote = std::collections::BTreeMap::new();
        remote.insert("a.ps1".to_string(), "AAA".to_string());
        remote.insert("b.ps1".to_string(), "BBB".to_string());
        let mut local = std::collections::BTreeMap::new();
        local.insert("a.ps1".to_string(), "AAA".to_string()); // 同
        local.insert("b.ps1".to_string(), "ZZZ".to_string()); // 变
        local.insert("c.ps1".to_string(), "CCC".to_string()); // 新增
        let drift = drifted_files(&local, &remote);
        assert_eq!(drift, vec!["b.ps1".to_string(), "c.ps1".to_string()]);
    }

    struct FakeExec(String);
    impl RemoteExecutor for FakeExec {
        fn run(&self, _h: &str, _s: &NodeScript) -> UecmResult<String> {
            Ok(self.0.clone())
        }
        fn probe(&self, _h: &str) -> UecmResult<ProbeResult> {
            Ok(ProbeResult {
                ok: true,
                message: "fake".into(),
                latency_ms: 1,
            })
        }
    }

    #[derive(Debug, serde::Deserialize)]
    struct Demo {
        ok: bool,
        value: i64,
    }

    #[test]
    fn run_json_parses_node_stdout() {
        let exec = FakeExec(r#"{"ok":true,"value":42}"#.to_string());
        let script = NodeScript {
            name: "x.ps1",
            args: serde_json::json!({}),
            ssh_user: None,
        };
        let d: Demo = run_json(&exec, "RENDER-01", &script).unwrap();
        assert!(d.ok && d.value == 42);
    }

    #[test]
    fn failure_detail_preserves_structured_stdout() {
        // 节点脚本把结构化失败写 stdout、stderr 为空：信息不能丢。
        let d = failure_detail(r#"{"ok":false,"message":"disk full"}"#, "");
        assert!(d.contains("disk full"));
        // stdout + stderr 都有：两者都保留。
        let d2 = failure_detail(r#"{"ok":false}"#, "winrm noise");
        assert!(d2.contains("winrm noise") && d2.contains("ok"));
        // 只有 stderr：原样。
        assert_eq!(failure_detail("", "boom"), "boom");
    }

    #[test]
    fn run_json_surfaces_bad_json_as_node_script_error() {
        let exec = FakeExec("not json".to_string());
        let script = NodeScript {
            name: "x.ps1",
            args: serde_json::json!({}),
            ssh_user: None,
        };
        let err = run_json::<Demo>(&exec, "RENDER-01", &script).unwrap_err();
        assert!(matches!(err, UecmError::NodeScript { .. }));
    }
}
