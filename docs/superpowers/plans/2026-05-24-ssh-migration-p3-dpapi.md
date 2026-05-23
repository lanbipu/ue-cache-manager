# SSH 迁移 P3 — 杀 DPAPI(commands 去解析 + 真用密码迁 SecretStore)实现计划

> REQUIRED SUB-SKILL: executing-plans(inline + subagent for bulk)。对应 spec P3。提交 `git -c commit.gpgsign=false`。
> 前置:P0/P1/P2 完成。**契约 §5**:不删/不改名任何 Tauri 命令;命令签名保留 optional cred 参数作 shim;不碰 Vue。

**Goal:** 删 `commands/*` 内部的 `resolve_password`/`resolve_operator_creds` 解析(DPAPI),vestigial 处改调无 cred 的 core fn、命令签名不变(shim);真用密码处(SMB/share-svc)改走 `SecretStore`。不删 DPAPI 实现本身(P5b)。build green + UI 命令签名/响应不变。

**Architecture:** 已迁移的 core fn 多有「无 cred」版(`env_vars::set` vs `set_with_credential`)走 SSH。vestigial 命令改调无 cred 版即去 DPAPI。真 secret 用 `core::secrets::SecretStore::get(alias)`。

---

## 分类(grep 实测)

### A. vestigial — 删 resolve_password,改调无-cred core fn,命令签名留 shim
逐个:把 `let password = core_credentials::resolve_password(&credential_alias)?;` + `core::X_with_credential(host, ..., &cred.username, &password)` 改成 `core::X(host, ...)`(无 cred 版)。**保留命令的 `credential_alias`/`cred_alias` 参数**(接受-忽略 shim,Vue 仍传)。涉及:
- `commands/env_vars.rs`(51 set, 69 get → `env_vars::set`/`get`)
- `commands/ini_editor.rs`(61 read, 88 set → `ini_editor::read_section`/`set_key`)
- `commands/ini_scanner.rs`(90, 283)
- `commands/batch.rs`(22)
- `commands/health_check.rs`(76)
- `commands/log_verify.rs`(24)
- `commands/projects.rs`(31 + 69 resolve_operator_creds)

> 核实每个无-cred core fn 存在且签名匹配(它们在 A2/A3 已迁 SSH)。若某 core fn 只有 `_with_credential` 版,P3 调它并传空/忽略(P4 删变体)——优先调真正无 cred 的版本。

### B. operator-cred vestigial — 删 resolve_operator_creds,core fn 已忽略
`ddc_pak`(233/384/420)、`pso`(109/303)、`shares`(80/202)、`projects`(69)的 `resolve_operator_creds(...)` → op_user/op_pass 喂给已忽略它们的 core/distribute/share fn。删解析,传 None / 不传;命令签名留 `operator_credential_alias` shim。

### C. 真用密码 → SecretStore(非 vestigial,真迁)
- `commands/ddc_pak.rs` source-SMB(84 的 helper + 420-422):`source_smb_credential_alias` 解析的是 **SMB 拉取凭据**,node robocopy 真用。改走 `pak_distribute::resolve_source_smb`(SecretStore),对齐 `commands/pso.rs` 已有做法(读 pso.rs 现状抄)。
- `commands/shares.rs:200` `inject_share_credential_to_clients`:`resolve_password(svc_alias)` 读的是 **share svc 密码**(Mode B),inject 真用。改 `SecretStore::get(svc_alias)`。注意 `create_share`(commands/shares.rs)历史用 DPAPI 存,需确认 svc 密码在 SecretStore(若 commands 的 create_share 仍 DPAPI 存,P3 顺带把它改成 SecretStore::put,对齐 cli/domain_share)。
- `commands/pso.rs` source-SMB(若还有 DPAPI 路径)同 ddc_pak。

### D. 不碰(P5a / P4 / 已 deferred)
- `commands/bootstrap.rs:32`(resolve_password→`enable_winrm_with_psexec`):WinRM 远程推送,P5a repoint,**P3 不动**。
- `commands/zen.rs:864`(cred.resolve):跨域 vestigial,P4 统一删(CLI/Tauri parity),**P3 不动**。
- `core/credentials.rs` resolve/store_password 实现:P5b 删。

## 执行
1. **vestigial 批(A+B)**:委派 subagent(机械,~10 文件),要求 build + cargo test --lib 绿、命令签名不变、无新 warning。
2. **真 secret(C)**:自己迁(ddc_pak/pso source-SMB→SecretStore、shares svc→SecretStore + create_share 对齐),逐处核实密码来源。
3. 收尾:cargo test 全绿 + build + codex review(`--base <P3首commit父>`)+ lanPC 抽验(env set/get 或 ini 一条经 commands... 注:commands 是 Tauri,CLI 验对应 cli 域已在;commands 验靠 build + 命令签名核对 + 可选 Tauri dev)。

## 验收(对照 spec P3)
- `grep resolve_password src-tauri/src/commands` 仅剩 D 类(bootstrap/zen)。
- `grep resolve_operator_creds src-tauri/src/commands` 清零(或仅 D)。
- 真 secret 处走 SecretStore::get。
- 命令签名(含 operator_credential_alias / credential_alias)不变;响应字段不变。
- cargo test --lib 全绿;build green;codex 无 blocker。

## 注意
- §5 契约:命令签名是 Vue API 边界,**只删内部 resolve,不删参数**。
- C 类逐处真测密码来源(spec review 教训:别把真用当 vestigial)。
- bootstrap/zen(D)留到对应 phase。
