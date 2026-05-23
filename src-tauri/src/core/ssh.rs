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
}
