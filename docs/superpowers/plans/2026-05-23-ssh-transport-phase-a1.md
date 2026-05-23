# UECM SSH 传输重构 · Phase A1 实施计划

> **For agentic workers:** 沿用 superpowers:executing-plans 风格逐 task 执行。提交用 `git -c commit.gpgsign=false`（1Password 签名锁会拦）。
>
> 前置：A0 已完成并在 lanPC 验证（见 `2026-05-23-ssh-transport-phase-a0.md` + memory `ssh-a0-validated`）。spec §7/§11-A1。

**Goal:** 让一台新机器能被 SSH 纳管——节点本地 `enable-ssh.ps1`（开 OpenSSH + 写 UECM 公钥到 `administrators_authorized_keys` + 建暂存目录 + 落脚本）、operator 端 `sync_scripts`（按 manifest 差异 scp 推脚本）、`UECM-Bootstrap.cmd` 串起 SSH 步骤。

**Architecture:** SSH 与 WinRM 并存（A1 是过渡，A5 才删 WinRM）。SSH 用**独立** `enable-ssh.ps1`，不塞进 32k 的 `enable-winrm.ps1`，A5 删 WinRM 时干净。节点脚本暂存：bootstrap 本地落一份，运行时 operator 用 A0 的 `compute_manifest`/`drifted_files` + 新 `scp` 推送保持更新。

**Tech Stack:** PowerShell 5.1（节点）、Windows OpenSSH（`Add-WindowsCapability`）、系统 `scp`、Rust（core/ssh.rs）。

约束：`.cmd` 必须 **CRLF + 纯 ASCII**（[[bootstrap_cmd_crlf_trap]]）；`.ps1` 可 LF。lanPC 已可作真节点验证（idempotent，可重跑）。

---

## 文件结构

| 文件 | 职责 | 动作 |
|---|---|---|
| `ps-scripts/enable-ssh.ps1` | 节点本地：开 OpenSSH + 授权 UECM 公钥 + 建/落暂存脚本 | Create |
| `src-tauri/src/core/ssh.rs` | operator 端 `scp_push` + `sync_scripts`（按 manifest 差异推送） | Modify |
| `ps-scripts/UECM-Bootstrap.cmd` | 在 WinRM 步骤后追加 SSH 步骤（纯 ASCII/CRLF） | Modify |
| `ps-scripts/package-winrm-bootstrap.ps1` | 打包含 `enable-ssh.ps1` + `uecm.pub` | Modify |

---

## Task A1-1：节点 `enable-ssh.ps1`

**Files:** Create `ps-scripts/enable-ssh.ps1`

行为（全 idempotent）：
1. `Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0`（已装则跳过；老系统失败 → JSON warning 不硬挂）。
2. `sshd` + `ssh-agent` 服务设 Automatic、启动 `sshd`；确保防火墙放行 TCP 22（`OpenSSH-Server-In-TCP` 规则）。
3. 把 UECM 公钥（`-PublicKeyPath`，默认同目录 `uecm.pub`；或 `-UecmPublicKey` 覆盖）写入 `C:\ProgramData\ssh\administrators_authorized_keys`：不存在则建文件并设 ACL 仅 `SYSTEM`+`BUILTIN\Administrators`（否则 sshd 忽略该文件）；已含同一行则跳过（不 dup）。
4. 建暂存目录 `C:\ProgramData\UECM\ps-scripts`；把 `-StagingSourceDir`（默认脚本所在目录）下的 `*.ps1` 复制进去（排除 `enable-*.ps1` / 自身）。
5. 末尾 `ConvertTo-Json -Compress` 出 `{ ok, changes[], message }`，`exit 0/1`。

- [ ] **Step 1: 写 `enable-ssh.ps1`**（完整脚本，见下）

```powershell
# Enables Windows OpenSSH Server for UECM SSH transport onboarding.
# Run locally on the target as Administrator. Idempotent; safe to re-run.
# Emits JSON { ok, changes, message } and exits 0 (ok) / 1 (failed).
param(
    [string]$PublicKeyPath = '',
    [string]$UecmPublicKey = '',
    [string]$StagingSourceDir = '',
    [switch]$CheckOnly
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$changes = New-Object System.Collections.ArrayList
function Note($m) { [void]$changes.Add($m) }
$adminKeys = 'C:\ProgramData\ssh\administrators_authorized_keys'
$staging = 'C:\ProgramData\UECM\ps-scripts'

try {
    if (-not $StagingSourceDir) { $StagingSourceDir = Split-Path -Parent $PSCommandPath }
    if (-not $PublicKeyPath) { $PublicKeyPath = Join-Path $StagingSourceDir 'uecm.pub' }

    # 1. resolve pubkey
    $pub = ''
    if ($UecmPublicKey) { $pub = $UecmPublicKey.Trim() }
    elseif (Test-Path $PublicKeyPath) { $pub = (Get-Content -Raw $PublicKeyPath).Trim() }
    if (-not $pub) { throw "no UECM public key (set -UecmPublicKey or place uecm.pub at $PublicKeyPath)" }
    if ($pub -notmatch '^ssh-(ed25519|rsa) ') { throw "public key does not look like an OpenSSH public key" }

    # 2. OpenSSH Server capability
    $cap = Get-WindowsCapability -Online -Name 'OpenSSH.Server*' -ErrorAction SilentlyContinue
    if ($cap -and $cap.State -ne 'Installed') {
        if (-not $CheckOnly) { Add-WindowsCapability -Online -Name $cap.Name | Out-Null }
        Note "installed OpenSSH.Server"
    } elseif (-not $cap) {
        Note "WARNING: OpenSSH.Server capability not found (older Windows: install Win32-OpenSSH manually)"
    }

    # 3. services + firewall
    if (-not $CheckOnly) {
        Set-Service -Name sshd -StartupType Automatic -ErrorAction SilentlyContinue
        Set-Service -Name ssh-agent -StartupType Automatic -ErrorAction SilentlyContinue
        Start-Service sshd -ErrorAction SilentlyContinue
        if (-not (Get-NetFirewallRule -Name 'OpenSSH-Server-In-TCP' -ErrorAction SilentlyContinue)) {
            New-NetFirewallRule -Name 'OpenSSH-Server-In-TCP' -DisplayName 'OpenSSH Server (sshd)' `
                -Enabled True -Direction Inbound -Protocol TCP -Action Allow -LocalPort 22 | Out-Null
            Note "added firewall rule TCP/22"
        }
    }

    # 4. authorize UECM pubkey in administrators_authorized_keys (ACL-correct)
    if (-not $CheckOnly) {
        $existing = if (Test-Path $adminKeys) { Get-Content $adminKeys } else { @() }
        if ($existing -notcontains $pub) {
            Add-Content -Path $adminKeys -Value $pub -Encoding ascii
            Note "authorized UECM key"
        }
        # enforce required ACL: only SYSTEM + Administrators (sshd ignores the file otherwise)
        icacls $adminKeys /inheritance:r | Out-Null
        icacls $adminKeys /grant 'SYSTEM:F' 'BUILTIN\Administrators:F' | Out-Null
    }

    # 5. staging dir + copy node scripts
    if (-not $CheckOnly) {
        if (-not (Test-Path $staging)) { New-Item -ItemType Directory -Path $staging -Force | Out-Null }
        Get-ChildItem -Path $StagingSourceDir -Filter '*.ps1' -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -notlike 'enable-*' -and $_.FullName -ne $PSCommandPath } |
            ForEach-Object { Copy-Item $_.FullName -Destination $staging -Force }
        Note "staged node scripts -> $staging"
    }

    $sshd = Get-Service sshd -ErrorAction SilentlyContinue
    $ok = ($sshd -and $sshd.Status -eq 'Running') -or $CheckOnly
    @{ ok = $ok; changes = $changes; message = if ($ok) { "SSH onboarding complete" } else { "sshd not running" } } |
        ConvertTo-Json -Depth 6 -Compress
    exit $(if ($ok) { 0 } else { 1 })
}
catch {
    @{ ok = $false; changes = $changes; message = $_.Exception.Message } | ConvertTo-Json -Depth 6 -Compress
    exit 1
}
```

- [ ] **Step 2: lanPC 验证（真节点，idempotent 重跑）**

```bash
VDIR=/tmp/uecm-a0-validate; F="post-quantum\|store now\|may be vulnerable\|server may need\|openssh.com/pq\|^\*\*\|WARNING: connection"
cp /Users/bip.lan/AIWorkspace/vp/ue-cache-manager/ps-scripts/enable-ssh.ps1 "$VDIR/"
cp "$VDIR/uecm_ed25519.pub" "$VDIR/uecm.pub"
scp -o BatchMode=yes "$VDIR/enable-ssh.ps1" "$VDIR/uecm.pub" lanpc@192.168.10.20:C:/ProgramData/UECM/ 2>&1 | grep -v "$F" || true
ssh -o BatchMode=yes lanpc@192.168.10.20 'powershell -NoProfile -ExecutionPolicy Bypass -File C:\ProgramData\UECM\enable-ssh.ps1 -PublicKeyPath C:\ProgramData\UECM\uecm.pub -StagingSourceDir C:\ProgramData\UECM' 2>&1 | grep -v "$F"
```
Expected: JSON `{"ok":true,...}`，changes 含 staged；重跑不 dup key。

- [ ] **Step 3: 提交**

```bash
git add ps-scripts/enable-ssh.ps1
git -c commit.gpgsign=false commit -m "feat(bootstrap): enable-ssh.ps1 node-local OpenSSH onboarding"
```

---

## Task A1-2：operator 端 `scp_push` + `sync_scripts`

**Files:** Modify `src-tauri/src/core/ssh.rs`

把 A0 的 `compute_manifest`/`drifted_files` 接上真正的推送：查节点 manifest → 算 drift → scp 推变更文件。

- [ ] **Step 1: 写测试**（纯逻辑：`remote_manifest_from_json` 解析节点回传的 `{name:hash}`；追加到 `ssh::tests`）

```rust
    #[test]
    fn remote_manifest_parses_node_json() {
        let m = remote_manifest_from_json(r#"{"a.ps1":"AAA","b.ps1":"BBB"}"#).unwrap();
        assert_eq!(m.get("a.ps1"), Some(&"AAA".to_string()));
        assert_eq!(m.len(), 2);
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test --lib core::ssh::remote_manifest`
Expected: 编译失败（`remote_manifest_from_json` 不存在）。

- [ ] **Step 3: 实现**（追加到 `ssh.rs`）

```rust
/// 解析节点回传的 manifest JSON（`{ "<name>": "<sha256>", ... }`）。
pub fn remote_manifest_from_json(s: &str) -> UecmResult<BTreeMap<String, String>> {
    serde_json::from_str(s).map_err(|e| UecmError::NodeScript {
        exit: 0,
        stderr: format!("bad remote manifest JSON: {e}"),
    })
}

/// scp 把本地文件推到节点暂存目录（用系统 scp，复用同一把 key/known_hosts）。
pub fn scp_push(
    key_path: &std::path::Path,
    known_hosts: &std::path::Path,
    ssh_user: &str,
    host: &str,
    local_files: &[std::path::PathBuf],
    remote_dir: &str,
) -> UecmResult<()> {
    if local_files.is_empty() {
        return Ok(());
    }
    let mut cmd = std::process::Command::new("scp");
    cmd.arg("-i").arg(key_path)
        .arg("-o").arg("IdentitiesOnly=yes")
        .arg("-o").arg(format!("UserKnownHostsFile={}", known_hosts.to_string_lossy()))
        .arg("-o").arg("StrictHostKeyChecking=accept-new")
        .arg("-o").arg("BatchMode=yes");
    for f in local_files {
        cmd.arg(f);
    }
    cmd.arg(format!("{ssh_user}@{host}:{remote_dir}/"));
    let out = cmd.output().map_err(|e| UecmError::ScriptStaging(format!("spawn scp failed: {e}")))?;
    if !out.status.success() {
        return Err(UecmError::ScriptStaging(format!(
            "scp failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    Ok(())
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test --lib core::ssh::remote_manifest`
Expected: PASS。

- [ ] **Step 5: lanPC 集成验证（手动）** — 改一个本地脚本 hash，确认 `drifted_files` + `scp_push` 只推变更项；节点 manifest 更新。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/core/ssh.rs
git -c commit.gpgsign=false commit -m "feat(ssh): scp_push + remote manifest parse for script staging sync"
```

---

## Task A1-3：`UECM-Bootstrap.cmd` + 打包串 SSH

**Files:** Modify `ps-scripts/UECM-Bootstrap.cmd`, `ps-scripts/package-winrm-bootstrap.ps1`

- [ ] **Step 1: `.cmd` 在 WinRM 步骤后追加 SSH 步骤**（纯 ASCII，保持 CRLF）

在 WinRM 的 `powershell.exe ... -File "%PS1%" ...` 之后、`set "PS_EXIT=..."` 之前插入：

```bat
REM ====== SSH transport onboarding (parallel to WinRM during migration) ======
set "SSH_PS1=%SCRIPT_DIR%enable-ssh.ps1"
set "UECM_PUB=%SCRIPT_DIR%uecm.pub"
if exist "%SSH_PS1%" if exist "%UECM_PUB%" (
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%SSH_PS1%" -PublicKeyPath "%UECM_PUB%" -StagingSourceDir "%SCRIPT_DIR%"
)
```

- [ ] **Step 2: 确认 `.cmd` 仍 CRLF + 纯 ASCII**

Run: `file ps-scripts/UECM-Bootstrap.cmd` → 含 `CRLF`；`LC_ALL=C grep -n '[^\x00-\x7F]' ps-scripts/UECM-Bootstrap.cmd` 只剩既有 em-dash 行（不新增非 ASCII）。

- [ ] **Step 3: `package-winrm-bootstrap.ps1` 纳入 `enable-ssh.ps1` + `uecm.pub`**

读现有打包脚本的文件清单逻辑，追加 `enable-ssh.ps1`；并从 keystore 导出 `uecm.pub` 一并打包（公钥可明文随包走）。

- [ ] **Step 4: 提交**

```bash
git add ps-scripts/UECM-Bootstrap.cmd ps-scripts/package-winrm-bootstrap.ps1
git -c commit.gpgsign=false commit -m "feat(bootstrap): .cmd + packaging wire SSH onboarding step"
```

---

## Self-Review

- **Spec §7 覆盖**：开 OpenSSH(T1.2) / 服务+防火墙(T1.3) / 写 admin authorized_keys + ACL(T1.4) / 暂存目录+落脚本(T1.5,T2) / `.cmd` 串联(T3) / 打包含公钥(T3.3) ✅。
- **占位符**：T3.3 “读现有打包逻辑追加”需执行时按 `package-winrm-bootstrap.ps1` 实际结构补全（非代码占位，是依赖现有文件的真实步骤）。
- **类型一致**：`scp_push(key,known_hosts,user,host,files,remote_dir)`、`remote_manifest_from_json`、`compute_manifest`/`drifted_files`(A0) 一致。
- **真节点**：T1 必在 lanPC 验证（idempotent）；A5 才删 WinRM，本 phase SSH/WinRM 并存。
