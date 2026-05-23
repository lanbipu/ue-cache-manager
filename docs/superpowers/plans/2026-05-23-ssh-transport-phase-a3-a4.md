# UECM SSH 传输重构 · Phase A3 + A4（mutating/工作流域 + 授权）实施计划

> 用 superpowers:executing-plans 风格逐域执行。提交 `git -c commit.gpgsign=false`。每域 lanPC 真节点验证。
> 前置：A0/A1/A2 完成验证。配方见 `2026-05-23-ssh-transport-phase-a2-readonly.md`（含 5 条节点纯脚本通用坑）。
> Secret store 决策见 memory `ssh-a3-secretstore-decision`。

**Goal:** 把剩余 mutating/工作流域的 sidecar 迁到 SSH 节点纯脚本；建跨平台 `core/secrets.rs`（替代 DPAPI）；迁移授权（SYSTEM cmdkey 注入）。完成后只剩 winrm.rs/DPAPI/PsExec-remote 待 A5 删。

**Architecture:** 同 A2 配方（self-remoting sidecar → 节点纯 + stdin args；调用方 leaf 注入 exec / 深嵌内部 from_config）。新增 `core/secrets.rs`（AES-GCM 加密文件，key 0600）。mutating 域的大文件传输是节点↔节点 SMB（operator 不碰字节）。

---

## 域分类 + 顺序

**第一组：无 secret，纯套 A2 配方（先做）**
| 域 | sidecar | core fn |
|---|---|---|
| env_vars | setx-machine.ps1 / getx-machine.ps1 | env_vars.rs（set/get machine env） |
| local_cache | create-local-cache-dir.ps1 | local_cache.rs |
| ini_editor | read-ini-section.ps1 / write-ini-key.ps1 / set-backend-field.ps1 | ini_editor.rs（6 处调用） |
| ddc_pak | generate-ddc-pak.ps1 / verify-pak-output.ps1 | ddc_pak.rs |
| ue_runner | start-ue-process.ps1 / stop-ue-process.ps1 / tail-ue-log.ps1 | ue_runner.rs（长任务，见下） |

**第二组：建 secret store + secret-touching 域（后做）**
| 域 | 说明 |
|---|---|
| `core/secrets.rs` | AES-GCM 加密文件 store（先建，下面被依赖） |
| shares | setup-share-mode-a/b.ps1：Mode B 生成 ddc-svc 密码 → 存 secrets.rs + 配置节点 share |
| pak_distribute / pso_collect | 节点↔节点 SMB 拉取，operator 从 secrets.rs 取 SMB 凭据传给目标节点（经 stdin） |
| A4 授权 | inject-system-credential.ps1：节点纯 + ssh + 节点本地 PsExec -s 写 SYSTEM cmdkey（材料来自 secrets.rs） |

---

## Task 组 1：无 secret 域（每域 = A2 配方）

每个 task 步骤同 A2（配方 A 节点纯化 + 配方 B/C 调用方 + 配方 D 验证）。务必套 5 条通用坑（ErrorAction 别 Stop / null-guard / ArrayList 非 Generic.List / 不用 Split-Path -LiteralPath -Parent / fail-hard 读用 -EA Stop）。

- [ ] **A3-1 env_vars**：setx-machine.ps1（set，stdin {Name,Value}）+ getx-machine.ps1（get，stdin {Name}）节点纯化；env_vars.rs 的 set/get fn 切 ssh::run_json（leaf，注入 exec）；上游改。lanPC 验证：set UECM_TEST → get 回 → 删（可逆）。commit。
- [ ] **A3-2 local_cache**：create-local-cache-dir.ps1 节点纯化（stdin {LocalPath}，含 icacls）；local_cache.rs 切 ssh；lanPC 验证建测试目录 + 清理。commit。
- [ ] **A3-3 ddc_pak**：generate-ddc-pak.ps1 + verify-pak-output.ps1 节点纯化；ddc_pak.rs（2 处）切 ssh。lanPC 验证（小 pak 或 verify 一个已有文件）。commit。
- [ ] **A3-4 ini_editor**：read-ini-section/write-ini-key/set-backend-field.ps1 节点纯化（write 类用 -EA Stop fail-hard）；ini_editor.rs（6 处）切 ssh。lanPC 验证写一个 temp .ini key 回读。commit。
- [ ] **A3-5 ue_runner**：start-ue-process/stop-ue-process/tail-ue-log.ps1 节点纯化。**长任务注意**：start-ue 是后台启动（不等退出）、tail 是读日志——SshExecutor.run 等 ssh 命令返回，start-ue 脚本本身应快速返回(Start-Process -PassThru 拿 PID 即返回，不 WaitForExit)。ue_runner.rs（3 处）切 ssh。lanPC 验证：tail 一个已有日志（start/stop 需真 UE，标 TODO 真机手测）。commit。

## Task 组 2：secret store + secret-touching 域

- [ ] **A3-6 `core/secrets.rs`**：AES-GCM-256 加密文件 store。

```rust
// Cargo.toml: aes-gcm = "0.10"
// core/secrets.rs
use crate::error::{UecmError, UecmResult};
use aes_gcm::{aead::{Aead, KeyInit, OsRng, rand_core::RngCore}, Aes256Gcm, Key, Nonce};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// 跨平台 secret store：AES-GCM 加密文件，key 在配置目录 0600 文件。
/// 替代 DPAPI（Windows-only）。存 managed-share / SMB / 服务 secret。
pub struct SecretStore { dir: PathBuf }

impl SecretStore {
    pub fn from_config() -> UecmResult<Self> {
        Ok(Self { dir: crate::startup::resolve_config_dir()? })
    }
    fn key_path(&self) -> PathBuf { self.dir.join("uecm_secrets.key") }
    fn store_path(&self) -> PathBuf { self.dir.join("uecm_secrets.bin") }

    fn load_or_create_key(&self) -> UecmResult<[u8; 32]> {
        let kp = self.key_path();
        if kp.exists() {
            let b = std::fs::read(&kp)?;
            let mut k = [0u8; 32];
            if b.len() != 32 { return Err(UecmError::Configuration("bad secrets key length".into())); }
            k.copy_from_slice(&b);
            Ok(k)
        } else {
            std::fs::create_dir_all(&self.dir)?;
            let mut k = [0u8; 32];
            OsRng.fill_bytes(&mut k);
            std::fs::write(&kp, k)?;
            #[cfg(unix)]
            { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&kp, std::fs::Permissions::from_mode(0o600))?; }
            Ok(k)
        }
    }

    fn read_all(&self) -> UecmResult<BTreeMap<String, String>> {
        let sp = self.store_path();
        if !sp.exists() { return Ok(BTreeMap::new()); }
        let key = self.load_or_create_key()?;
        let blob = std::fs::read(&sp)?;
        if blob.len() < 12 { return Err(UecmError::Configuration("secrets store too short".into())); }
        let (nonce, ct) = blob.split_at(12);
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
        let pt = cipher.decrypt(Nonce::from_slice(nonce), ct)
            .map_err(|_| UecmError::Configuration("secrets decrypt failed".into()))?;
        serde_json::from_slice(&pt).map_err(|e| UecmError::Configuration(format!("secrets parse: {e}")))
    }

    fn write_all(&self, map: &BTreeMap<String, String>) -> UecmResult<()> {
        let key = self.load_or_create_key()?;
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);
        let pt = serde_json::to_vec(map).map_err(|e| UecmError::Configuration(format!("secrets serialize: {e}")))?;
        let ct = cipher.encrypt(Nonce::from_slice(&nonce), pt.as_ref())
            .map_err(|_| UecmError::Configuration("secrets encrypt failed".into()))?;
        let mut out = nonce.to_vec();
        out.extend_from_slice(&ct);
        std::fs::write(self.store_path(), out)?;
        Ok(())
    }

    pub fn put(&self, alias: &str, secret: &str) -> UecmResult<()> {
        let mut m = self.read_all()?;
        m.insert(alias.to_string(), secret.to_string());
        self.write_all(&m)
    }
    pub fn get(&self, alias: &str) -> UecmResult<Option<String>> {
        Ok(self.read_all()?.get(alias).cloned())
    }
    pub fn delete(&self, alias: &str) -> UecmResult<()> {
        let mut m = self.read_all()?;
        m.remove(alias);
        self.write_all(&m)
    }
}
```
测试：put→get→delete round-trip（tempdir via UECM_DB_PATH）；加密后文件非明文（grep secret 不命中）。commit。

- [ ] **A3-7 shares**：setup-share-mode-a/b.ps1 节点纯化；shares.rs Mode B 生成密码 → `SecretStore::put` + 配置节点。lanPC 验证（Mode A 开放共享，或 Mode B 建 ddc-svc + 存 secret）。commit。
- [ ] **A3-8 pak_distribute / pso_collect**：distribute-pak-file/distribute-pso-cache.ps1 节点纯化（stdin 含 SourceUnc + SMB 凭据，凭据来自 `SecretStore::get`）；节点↔节点 SMB 拉取不变。lanPC 验证（小文件节点间分发）。commit。
- [ ] **A4 inject-system-credential**：节点纯化 + ssh + 节点本地 PsExec -s 写 SYSTEM cmdkey（材料来自 SecretStore）；psexec.rs/对应调用方切 ssh。lanPC 验证。commit。

## Task 组 3：收口

- [ ] grep 确认无 secret 域 + secret 域均脱离 powershell::run_json（仅剩 winrm.rs/bootstrap/preflight/credentials 待 A5）。
- [ ] cargo test --lib 全绿 + build；lanPC 跑一次完整 health run + 一次 share/distribute 端到端。
- [ ] 提交收口。

---

## Self-Review
- **范围**：A3 第一组 5 域(无 secret)+ secrets.rs + 第二组 3 域 + A4。覆盖 spec §11 A3+A4。
- **secret store**：完整代码给出（AES-GCM + 0600 key），非占位。
- **复杂域已标注**：pak_distribute/pso_collect(节点间 SMB)、ue_runner(长任务/Start-Process 不等待)——非纯机械，单独设计。
- **类型一致**：沿用 A2 的 NodeScript/RemoteExecutor/ScriptOutput/from_config 模式 + 新 SecretStore put/get/delete。
- **A5/B 不在本 plan**（A5 破坏性压轴；B 是 UI 子项目 Figma-first）。
