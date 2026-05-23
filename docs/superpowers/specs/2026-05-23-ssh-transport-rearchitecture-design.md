# UECM SSH 传输重构设计

- **日期**：2026-05-23
- **状态**：设计已定稿（已并入 Codex adversarial review 反馈 v2）
- **目标**：把 UE Cache Manager 的远程驱动方式从「本机 powershell.exe + WinRM 中继」彻底换成「shell out 系统 ssh + 节点暂存纯脚本」，使 operator 端跨平台（macOS / Windows 同一份代码），并退役 WinRM。

> **v2 修订**（Codex adversarial review，2026-05-23）：①不再用 `-EncodedCommand` 传脚本正文（撞 Windows 命令行长度上限）→ 改节点侧脚本暂存 + `-File`；②区分「transport 认证密钥」与「secondary 服务/共享 secret」，后者保留并迁移，不随 DPAPI 一起删；③UI 范围扩大——request schema 里的 credential_alias 必须同步改，A/B 子项目要协调，过渡期加兼容 adapter。

---

## 1. 背景与动机

### 现状
- UECM 所有远程操作的实现方式是：**operator 本机 spawn 一个 `powershell.exe`，跑一段 `.ps1` sidecar**；远程操作时 sidecar 内部再用 `Invoke-Command -ComputerName $HostName` 连到节点。
- 53 个 sidecar 里 26 个「自带远程」；36 处本机 powershell 调用散在 21 个 core 模块；`core/winrm.rs` 是远程入口；`core/credentials.rs`（299 行）用 DPAPI + cmdkey 存 WinRM 账密别名。
- 后果：operator 必须是 Windows。

### 这个中继方式漏出来的已知问题（迁移要顺带消除的）
- GBK/CP936 乱码解码（`powershell.rs:20`）
- stdin 大脚本被截断、只能落临时文件（`winrm.rs` T1.11）
- 临时 `.ps1` 明文带密码（Codex round-18）
- Negotiate/SSO 回退补丁 `--auth-method`（commit `6de9bb9`）
- 每个操作冷起一个 powershell.exe，无 session 复用

### 已确认的设计决策（brainstorm 结论）
1. **终态：SSH 单一传输**，WinRM / powershell.exe 中继彻底退役（不保留双轨）。
2. **认证：UECM 自管一把专用 SSH key**（不复用 1Password agent）。
3. **传输实现：shell out 到系统 `ssh`**，`StrictHostKeyChecking=accept-new` TOFU + `-i 专用私钥` + 独立 `known_hosts`。
4. **范围：引擎（CLI + Rust core）+ Tauri UI 端到端**；实施拆成两个子项目（引擎先、UI 后），设计一份覆盖全貌。
5. **sidecar：重构成节点纯脚本**，删除所有 `Invoke-Command` 远程包装，远程只剩 SSH transport 一处。

### 不在范围 / 明确放弃
- **operator 远程 first-contact 纳管**（PsExec + ADMIN$/RPC 那条 Windows-only 通道）：放弃。新机器纳管统一走「节点本地双击 `UECM-Bootstrap.cmd`」（顺手开 SSH + 授权 uecm 公钥 + 推脚本包），与 operator 是 Mac 还是 Windows 无关。
- 不引入常驻 agent（沿用 CLAUDE.md 既定）。

---

## 2. 目标架构

```
CLI (uecm-cli)  ┐
                ├─→ domain 逻辑 (core/*) ─→ RemoteExecutor (SSH) ─→ 节点暂存纯脚本.ps1 ─→ 渲染节点
Tauri UI 命令   ┘                              │            │
                            KeyStore（transport key + known_hosts）
                            SecretStore（跨平台加密的 secondary 服务/共享 secret）
```

**核心原则**：远程这件事**只在传输模块一个地方发生**。
- domain 逻辑：组装「脚本名 + 参数」→ 调 executor.run() → 解析 JSON；不碰远程/认证细节。
- 节点脚本：只在本地干活、吐 JSON；不含 `Invoke-Command`/operator 凭据逻辑。
- 传输模块：唯一知道怎么连、怎么认证、怎么暂存脚本。

### 变化点

| 现在 | 重构后 |
|---|---|
| `core/winrm.rs`（spawn powershell.exe 跑 invoke-remote.ps1） | **`core/ssh.rs`** — SSH 传输 + 脚本暂存（shell out 系统 ssh / scp） |
| `core/powershell.rs`（本机 sidecar 执行 + 编码解码） | 退役；保留 `decode_subprocess_output`（GBK 兜底）挪进 ssh 模块 |
| `core/credentials.rs`（DPAPI + cmdkey + WinRM 账密别名） | 拆成 **`core/keystore.rs`**（transport key）+ **`core/secrets.rs`**（跨平台 secondary secret store，保留迁移） |
| 53 个自带远程 sidecar | 53 个**节点暂存纯脚本**（删 Invoke-Command 包装，运行时 `-File` 引用） |
| bootstrap 走 WinRM/PsExec | bootstrap 开 OpenSSH + 写 uecm 公钥 + 推脚本包 |

---

## 3. SSH 传输模块 `core/ssh.rs`

### 3.1 对外接口（domain 层只看到这个）

为可测性引入薄 trait（seam），只有一个生产实现 + 一个测试 fake：

```rust
pub trait RemoteExecutor {
    fn run(&self, host: &str, script: &NodeScript) -> UecmResult<String>;   // 返回 stdout
    fn probe(&self, host: &str) -> UecmResult<ProbeResult>;                  // 连通性 + 认证探活
}

pub struct SshExecutor { /* key 路径、known_hosts 路径、默认 user、暂存根路径 */ }
impl RemoteExecutor for SshExecutor { ... }                                  // 唯一生产实现

pub fn run_json<T: DeserializeOwned>(exec: &dyn RemoteExecutor, host: &str, script: &NodeScript) -> UecmResult<T>;
```

```rust
pub struct NodeScript {
    pub name: &'static str,                 // 已预置在节点的脚本名，如 "health-probes.ps1"
    pub args: serde_json::Value,            // 入参（含 secret），运行时经 stdin JSON 传给节点
    pub ssh_user: Option<String>,           // None = 默认 uecm-svc
}
```

> **不再把脚本正文塞进远程命令行**（v2 修订，见 §3.3）。脚本预置在节点，运行时只 `-File` 引用脚本名 + stdin 传 JSON 参数。

### 3.2 内部实现：拼一条系统 ssh 命令

```
ssh -i <config>/uecm_ed25519 \
    -o IdentitiesOnly=yes \
    -o UserKnownHostsFile=<config>/known_hosts \
    -o StrictHostKeyChecking=accept-new \
    -o BatchMode=yes \
    -o ConnectTimeout=10 \
    <ssh_user>@<host> \
    "powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\ProgramData\UECM\ps-scripts\<name>"
```

参数（JSON）经该远程命令的 **stdin** 喂入，节点脚本 prologue 读 stdin 解析。

### 3.3 关键设计点

- **脚本预置在节点，运行时用 `-File` 引用，不把正文放进命令行**（Codex adversarial review 修订点）：
  - **为什么不用 `-EncodedCommand` 传正文**：Windows 远程命令行有长度上限（`CreateProcessW` ~32767 字符，经 cmd 更低）。现有待迁移脚本里 `health-probes.ps1` ≈44.5k、`zen-service-install.ps1` ≈70.7k、`zen-verify-rules.ps1` ≈71.3k 字符，转 UTF-16LE + base64 后约 ×2.67（→ 119k~190k），**远超上限，PowerShell 启动前就失败**；且 `winrm.rs` 早记录过「~10K 多语句脚本走 stdin 会被静默截断」才改用 temp `-File`。命令行传正文对真实域脚本根本走不通。
  - **方案：节点侧脚本暂存（staging）**。bootstrap + `sync-scripts` 步骤把 `ps-scripts/` 整包推到节点固定路径 `C:\ProgramData\UECM\ps-scripts\`，带 SHA256 manifest；传输前比对 manifest，漂移就重推（系统 `scp`/sftp，Mac/Windows 都有）。运行时只 `-File <staged>\<name>`，**零正文传输、零命令行长度风险、零截断**。
  - **测试硬要求**：迁移测试必须覆盖**当前最大的 sidecar**（zen 那两个 ~71k）跑在**真实 Windows sshd** 上，确认 staging + `-File` 成立。
- **所有入参（含 secret）走节点进程的 stdin，JSON 一次喂入**：节点脚本 prologue 读 `[Console]::In.ReadToEnd() | ConvertFrom-Json` 绑定参数。args 体积小（不含脚本正文），stdin 不进命令行，secret 不暴露在节点进程列表里。
- **编码**：节点仍是 Windows PowerShell 5.1，脚本 prologue 照旧 `[Console]::OutputEncoding=UTF8; chcp 65001` 吐 UTF-8 JSON；传输侧 UTF-8 解，GBK 兜底保留。
- **TOFU 首连**：`accept-new` 自动信任并钉指纹进 UECM 自己的 `known_hosts`；之后指纹变化 ssh 拒连（防 MITM），错误原样上抛。
- **退出码分类**：ssh `255` → `UecmError::SshConnect`（连接/认证/host-key，附 stderr）；其余非零 → `UecmError::NodeScript { exit, stderr }`。
- **超时**：`ConnectTimeout=10` + 整体 wait 超时（可配）。
- **loopback**：operator 自管时保留本地直跑 powershell.exe 快路径，不绕 ssh；纯 Mac operator 不存在「管自己」。

---

## 4. 凭据：拆成两套独立体系（v2 核心修订）

> Codex 指出：旧 `credentials.rs` 不只服务 operator→WinRM，还存「Mode B share 生成的 `ddc-svc` 密码」等 secondary secret，被 SYSTEM cmdkey 注入 / DDC/PSO 分发复用。**不能整体删除**。明确拆成两套：

### 4.1 Transport 认证 `core/keystore.rs`（替代 operator→节点的 WinRM 账密）

- **密钥类型**：ed25519。
- **存储**：应用配置目录（`creds.bin` 同处，`<data_dir>/<APP_IDENTIFIER>/`）：私钥 `uecm_ed25519`（unix `0600`/Windows 收紧 ACL）、公钥 `uecm_ed25519.pub`、`known_hosts`。
- **接口**：`ensure_keypair()` / `public_key()` / `private_key_path()` / `known_hosts_path()` / `rotate()`。
- **节点身份**：`host + ssh_user`，默认 user = `uecm-svc`；`machines` 表新增 `ssh_user` 列（迁移加列，默认 NULL=用 uecm-svc）。
- **删除**：DPAPI 对 **operator WinRM 认证条目** 的存取、`--auth-method`。

### 4.2 Secondary secret store `core/secrets.rs`（保留 + 迁移，绝不随 DPAPI 一起删）

- **存什么**：Mode B managed share 的 `ddc-svc` 服务密码、SYSTEM cmdkey 注入材料、节点↔节点 SMB 拉取凭据等——这些是**节点服务/共享 secret，不是 operator 认证**。
- **跨平台存储**：DPAPI 是 Windows-only，operator 可能是 Mac，所以换成**跨平台加密 secret store**（加密文件，密钥放 OS keychain：macOS Keychain / Windows Credential Manager / Linux secret-service）。
- **迁移策略（关键，回答 Codex 的「升级后密码不可恢复」）**：因为 SSH-only 终态**本就要求每个节点重新纳管**，managed share secret 在重纳管时**重新 provision（rotate）**并写入新 store——**不尝试在 Mac 上解 Windows DPAPI blob**（跨平台解不了）。旧 `creds.bin` 仅在 Windows operator 上保留只读，用于过渡期对照 / rollback，不作为迁移前置。
- **保留语义**：`share_configs.credential_alias` 等指向 secondary secret 的引用**保留**，只是后端解析从 DPAPI 改成 `secrets.rs`。
- **rotation / rollback / 幂等重放**：`rotate(alias)` 重新生成并重注入节点 cmdkey；重纳管/重注入操作幂等（同 alias 重复跑结果一致）；失败保留旧值不破坏现有访问。

### 4.3 边界澄清
- operator→节点：**只**用 4.1 的 ssh key。
- 节点 SYSTEM 服务→共享：用 4.2 的 secondary secret（经 §6 注入）。两者代码与文档都不混。

---

## 5. 节点纯脚本重构（53 个 sidecar）

### 5.1 统一改造模式

从「自带远程」改成「节点本地纯脚本」，运行时由传输层 `-File` 调用：

```powershell
# prologue（所有节点脚本统一）：设编码 + 从 stdin 读参数
[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; chcp 65001 | Out-Null
$ErrorActionPreference = 'Stop'
$p = [Console]::In.ReadToEnd() | ConvertFrom-Json

# body：直接干活（已在节点上，无需 Invoke-Command），用 $p.<字段>
# 结尾吐 JSON：@{ ok = $true; ... } | ConvertTo-Json -Compress
```

### 5.2 删除项
- 所有 `Invoke-Command` / `-ComputerName` / `-HostName` 远程包装。
- 所有 `Build-CredentialOrNull` + operator 会话 `-Username/-Password`（operator 认证现在是 ssh 层的事）。
- `invoke-remote.ps1`（被 SSH transport 取代）。

### 5.3 保留的「次级凭据」例外（经 §4.2 store + stdin JSON 传）
- `distribute-pak-file.ps1` / `distribute-pso-cache.ps1`：节点挂**对端节点** SMB 共享拉文件，需 SMB 凭据（`SourceSmbUser/Pass`），经 stdin 传入，不上命令行。大文件传输仍是**节点↔节点 SMB**，operator（Mac）全程不碰字节。

### 5.4 暂存与版本
- `ps-scripts/` 整包随 bootstrap 推到节点 `C:\ProgramData\UECM\ps-scripts\`；带 `manifest.sha256`。
- 传输前 `sync-scripts` 比对 manifest；UECM 升级 / 脚本变更后首次连接自动重推变更文件。

---

## 6. 授权 / 节点 SYSTEM 凭据（「开通与授权」板块的节点部分）

- `inject-system-credential.ps1` 改成节点纯脚本：经 ssh 在节点上跑，`PsExec64 -s` **在节点本地**提到 SYSTEM 上下文，`cmdkey` 写入 host-specific 凭据（来自 §4.2 store），使 SYSTEM 上下文 UE 服务能访问 DDC 共享。
- **PsExec64.exe 投送**：仍 vendored；`scp -i <key>` 推到节点（或 bootstrap 预置），经 ssh 触发本地执行。这里 PsExec 是「节点本地提权」，非远程认证。
- 注入幂等：同节点同 alias 重复跑结果一致；失败不破坏既有凭据。

---

## 7. 纳管 / Bootstrap 重设计

`UECM-Bootstrap.cmd`（节点本地双击）步骤：

1. **开 OpenSSH Server**：`Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0`（Windows 自带能力）。Server 2016 等老系统降级为单独装 Win32-OpenSSH（文档注明）。
2. **sshd 服务设 Automatic + 启动**；防火墙放行 TCP 22。
3. **建 `uecm-svc` 本地管理员**（沿用现有逻辑 + 现有明文凭据约定）。
4. **写 UECM 公钥到 `C:\ProgramData\ssh\administrators_authorized_keys`**，设正确 ACL（仅 `SYSTEM` + `Administrators`）。原因：`uecm-svc` 是管理员，Windows OpenSSH 对管理员组成员用这个**共享**文件，不是 per-user 的——常见踩坑点。
5. **推 `ps-scripts/` 包到 `C:\ProgramData\UECM\ps-scripts\`**（含 manifest），供运行时 `-File`。
6. 不改 sshd 默认 shell（无需；ssh 命令显式调 `powershell.exe`）。

**公钥分发**：UECM 公钥随 bootstrap 包携带（公钥可明文随包走）。UI/CLI 提供「导出公钥」。

**删除**：`bootstrap-winrm-remote.ps1`、`enable-winrm.ps1`、`preflight-path-b.ps1`、`core/preflight.rs` 的 Path B、`enable_winrm_with_psexec`。

**约束沿用**：`.cmd` 必须 CRLF + 纯 ASCII，中文留 `README.txt`（CLAUDE.md 既定）。

---

## 8. CLI / Tauri UI 变更

### 8.1 CLI（`src-tauri/src/cli/args.rs`）
- `winrm` 域 → **`ssh` 域**（`probe` / `exec`）。
- `cred` 域 → **`key` 域**：`key show` / `key path` / `key rotate`。删 password 别名 add/remove（**仅指 operator 认证别名**；secondary secret 的管理另列，见 8.3）。
- 全域：删 `--auth-method`；operator `--cred-alias` 移除。新增可选 `--ssh-user`（默认 `uecm-svc`）。
- `--help` 同步更新（CLAUDE.md 要求核对 args.rs）。

### 8.2 Tauri UI / API contract（v2 扩大范围）

> Codex 指出：`ScanInisRequest`、`RunHealthCheckRequest`、INI/Health wizard、DDC/PSO 调用都带 `credential_alias` / `operator_credential_alias`。「主视图不动」前提不成立。

- **Request schema 审计 + 改造**（B 子项目首要任务）：逐个 Tauri command request struct（`src-tauri/src/commands/*` + `src/services/tauri.ts` + 各 Vue wizard / store / test）：
  - 移除 **operator** `credential_alias` / `operator_credential_alias`（认证已由 ssh 承担）。
  - **保留**真正指向 §4.2 secondary SMB/share secret 的 alias 字段。
- **Key 管理界面**：替代凭据别名界面；展示 uecm 公钥 + 一键复制、key 状态、rotate。
- **纳管向导**：① 复制公钥 → ② 节点双击 `UECM-Bootstrap.cmd` → ③ 自动授权 + 推脚本包。
- **过渡期兼容 adapter**：A5 删 operator alias 解析前，后端对仍传来的 operator alias **接受但忽略**（不 500），直到 B 改完 schema；adapter 在 B 完成后移除。
- 遵守 `.claude/rules/figma-design-system.md`（UI 改动先 Figma → 确认 → 改 Vue）。

### 8.3 secondary secret 管理入口
- managed share / service secret 不随 operator 凭据别名删除；保留独立的 CLI/UI 管理入口（list / rotate），后端走 §4.2 `secrets.rs`。

---

## 9. 错误处理

- ssh `255` → `UecmError::SshConnect`（附 ssh stderr）。
- 远程命令非零 → `UecmError::NodeScript { exit, stderr }`。
- **host key 变化（TOFU 冲突）** → 单独明确错误，原样透出（绝不静默）。
- 超时 → `UecmError::Timeout`。
- staging 失败（scp/manifest）→ `UecmError::ScriptStaging`，明确提示节点脚本未就绪。
- **节点脚本契约**：一律 `{ ok: bool, ... }` JSON；写操作做字节/长度交叉核对（沿用 zen `ok:true` 不可独信的 Codex 教训）。
- 编码：UTF-8 主，GBK 兜底。

---

## 10. 测试策略

- **传输 argv 构造**：纯函数，任意平台单测（key/known_hosts 选项、`-File` 路径、退出码映射）。
- **staging**：manifest 比对 / 漂移检测逻辑单测；**最大 sidecar（~71k）在真实 Windows sshd 上跑通**为 phase 验收硬条件。
- **domain 逻辑**：靠 `RemoteExecutor` fake（预置 JSON）在 Mac/CI 单测——针对现状「几乎无 mock」的改进。
- **secondary secret 迁移**：rotate / 重注入 / rollback 幂等性单测。
- **集成**：每个迁移 phase 收尾在 lanPC 真节点验证。

---

## 11. 迁移分阶段（两个子项目）

> 终态 SSH-only；迁移期短暂并存旧路（脚手架），最后删净。每 phase 收尾在 lanPC 真节点验证。**A5 与 B 的 credential-alias 改动必须协调**（先 B 改 schema + adapter，后 A5 删解析）。

### 子项目 A：引擎（CLI + core）
- **A0**：`core/ssh.rs`（SshExecutor + RemoteExecutor trait + 退出码/编码）+ **脚本暂存机制（sync-scripts + SHA256 manifest + scp）** + `core/keystore.rs` + argv/manifest 单测 + `probe`。machines 表加 `ssh_user` 迁移。
- **A1**：`UECM-Bootstrap.cmd` 加 SSH（开 OpenSSH + 写 `administrators_authorized_keys` + 推脚本包 + 建 uecm-svc）；lanPC + razer 验证一台机被 SSH 纳管，且**最大 zen 脚本能 `-File` 跑通**。
- **A2**：迁移**只读诊断**域到节点纯脚本 + ssh：machine / discovery / health / ini-read / project / command-line-scan / consistency / ddc-file-stats / renderstream / ue-log。
- **A3**：建 `core/secrets.rs`（跨平台 secret store）+ 迁移**变更/工作流**域：ini-write / share（含 Mode B secret re-provision）/ ddc-pak / pak-distribute / pso-collect / ue-runner / env / local-cache / zen。
- **A4**：迁移**授权**（inject-system-credential / SYSTEM cmdkey）到 ssh + 节点本地 PsExec，接 `secrets.rs`。
- **A5**：删除 `core/winrm.rs`、`core/powershell.rs` 中继、DPAPI **operator 认证**层、PsExec 远程 first-contact、`enable-winrm.ps1`/`preflight-path-b.ps1`；清 `--auth-method`/operator `--cred-alias`（与 B 协调后）。**保留 secrets.rs**。

### 子项目 B：Tauri UI / API
- **B0**：API request schema 审计 + 兼容 adapter（后端忽略 operator alias 不报错）。
- **B1**：Key 管理界面（替代凭据别名）。
- **B2**：纳管向导改造（公钥复制 + 双击 bootstrap + 推包文案）。
- **B3**：逐项改 request struct / Vue wizard / store / test，去 operator alias、留 secondary SMB alias；与 A5 协调后移除 adapter。

---

## 12. 风险与取舍

- **大脚本命令行上限**（已解决）：靠节点脚本暂存 + `-File`，不再走命令行传正文；验收强制覆盖最大 sidecar。
- **secondary secret 迁移**（已设计）：靠重纳管时 re-provision + 跨平台 secret store，不解旧 DPAPI blob；过渡期旧 creds.bin 只读对照。
- **A/B 脱节风险**（已设计）：兼容 adapter + 先 B schema 后 A5 删解析的顺序约束。
- **operator 远程 first-contact 能力丧失**：统一靠节点本地双击 bootstrap（已接受）。
- **Windows operator 失去域 SSO**：改 ssh key（home lab 本就如此）。
- **节点仍跑 PowerShell 5.1**：保留编码处理（成本低）；后续可选 pwsh 7 升级，非本次前提。
- **Server 2016 等老系统**：OpenSSH Server 需单独装；bootstrap 注明降级路径。
- **节点全量重新纳管**：SSH-only 固有成本，已接受。
- **改造量大**：53 脚本 + ~36 调用点 + UI schema 审计；靠分 phase + 真节点回归 + fake executor 单测控制。
