# SSH 迁移 P4 — CLI 去 operator-cred + AuthMethod + deploy 凭据门 + 凭据子系统 + secret 域

> REQUIRED SUB-SKILL: executing-plans(inline + subagent for bulk）。对应 spec P4。提交 `git -c commit.gpgsign=false`，只新增 commit。
> 前置：P0/P1/P2/P3 完成。**契约 §5**：不删/不改名任何 Tauri 命令；命令签名保留 optional cred 参数作 shim；不碰 Vue（`src/**`）；响应字段名冻结。

**Goal:** 把 operator WinRM 凭据从 CLI/core 彻底拆掉——删 `AuthMethod` enum + `auth_method` 参数线、删非-winrm 的 `_with_credential` core 变体并把调用方改无-cred、去掉 deploy 的 `creds.ok_or_else("required")` 门、`save_credential` 把密码从 DPAPI repoint 到 `SecretStore::put`、新建 `secret` CLI 域直管 SecretStore。**不删 DPAPI / winrm.rs 实现本体**（P5）。全程 build 绿、`cargo test --lib` 绿、命令签名/响应兼容。

**Architecture:** 所有节点传输已是 SSH key auth（`uecm-svc`），operator 凭据在 leaf 已被忽略（P0–P3 后 core fn 的 `_with_credential` 变体里密码不再被消费，或已有无-cred 版）。P4 把这层「解析了但没人用」的 operator cred 从上到下删干净；真用密码（share svc / SMB）已在 P3 走 SecretStore，不动。`secret` 域给 terminal/agent 直接管 SecretStore（CLAUDE.md「所有功能 CLI 暴露」）。

**Tech Stack:** Rust / clap-derive / serde / SQLite（`data::credentials` 元数据）/ `core::secrets::SecretStore`（AES-GCM）。

---

## 契约钉死（执行时不得违反）

- **Tauri 命令签名不变**：`commands/*` 的 `operator_credential_alias: Option<String>`、`credential_alias: String` 等参数**保留为接受-忽略 shim**（Vue 仍传）。只删函数体里的解析/透传，不删参数。
- **响应字段冻结**：`RefreshResult.winrm_ok`、`WinrmBootstrapResult.*` 等 Vue 读的字段不碰。
- **`CredentialKind::Winrm` 变体保留**（6+ 向导 `kind==='winrm'` 过滤）；`from_sql` 对未知 kind 容错（不硬报错）。
- **`listCredentials` 读 SQLite**（元数据，与删 DPAPI 无关）。
- winrm.rs 的 9 个 `_with_credential`（`probe_with_credential`/`invoke_with_credential`×2/`invoke_json_with_credential`）属 **P5a**，P4 不动。

---

## 分批执行（每批：改 → `cargo build` + `cargo test --lib` 绿 → commit → codex review `--base <上一 commit>`）

### Batch 1 — `CredentialArgs`：删 `AuthMethod` + DPAPI resolve 路径
**文件：** `cli/credential_args.rs`（核心）+ 所有构造 `CredentialArgs{...}` 字面量的点。

1. 删 `enum AuthMethod` + `impl AuthMethod`（`as_str`）。
2. 删 `CredentialArgs.auth_method` 字段 + 其 `#[arg(long ...)]`（即移除 `--auth-method` flag）。
3. `inline(resolved, auth_method)` → `inline(resolved)`（删第二参）。
4. `resolve()`：删 `cred_alias` 分支里的 `crate::core::credentials::resolve_password(alias)?`。operator cred 已无人消费——`--cred-alias` 改为**接受-忽略**：`cred_alias` 分支直接 `return Ok(None)`（保留 flag 不破坏脚本，但不再解析密码）；`preflight` 里 `cred_alias` 的「存在性校验」可保留（仍查 SQLite 元数据存在）或一并简化为 `Ok(())`——实现时核（倾向保留存在性校验，错别名早报）。inline `--user/--pass/--pass-stdin` 路径**保留**（仍返回 `(user,pass)`，无害）。
5. 全仓 `grep "auth_method"`：每个 `CredentialArgs{ ..., auth_method: ... }` 字面量删该字段；每个 `.inline(x, cred.auth_method)` → `.inline(x)`；每个把 `cred.auth_method.as_str()` 当实参传下游的（domain_zen 的 `invoke_*(... auth_method)` 形参链）一并删（见 Batch 2 同步处理 domain_zen 形参）。
6. 删 credential_args.rs 测试里引用 `auth_method` / `AuthMethod` 的部分（保留其余断言）。

**验收：** `grep -rn "AuthMethod\|auth_method" src/` → 0（含 winrm.rs：winrm.rs 是否用 auth_method？实测——winrm.rs `invoke_with_credential` 的 `auth_method: &str` 形参若存在则属 P5a，**保留**；P4 只删 CLI 侧 `AuthMethod` enum 与传递。执行时区分 enum（删）vs winrm.rs 的 `&str` 形参（P5a 留）。

### Batch 2 — 删非-winrm 的 `_with_credential` core 变体 + 调用方改无-cred
**core 变体（删定义）：** `core/env_vars.rs`（`set_with_credential`/`get_with_credential`）、`core/ini_editor.rs`（`read_section_with_credential`/`set_key_with_credential`/`remove_key_with_credential`/`set_backend_field_with_credential`）、`core/ini_apply.rs`（5）、`core/zen/enable.rs`（11）、`core/zen/verify.rs`（1）、`core/discovery.rs`（1）。
**调用方改无-cred：** `cli/domain_env.rs`、`cli/domain_ini.rs`、`cli/domain_zen.rs`、`commands/env_vars.rs`、`commands/ini_editor.rs`、`commands/ini_scanner.rs`、`lib.rs`（核实 4 处是什么——可能是 re-export 或 Tauri 注册）。

逐个：调用 `X_with_credential(host, ..., u, p)` → `X(host, ...)`（无-cred 版）。无-cred 版不存在的（ini_apply/zen/enable/verify/discovery 若只有 `_with_credential` 版），把该变体本体改成无-cred（删 `user/pass` 形参 + 删函数体里对它们的使用——大多已是 `let _ = (user,pass);` 占位）。删所有 `let _ = (user, pass)` / `let _ = (creds, auth_method)` 占位。domain_zen 的 `invoke_*(..., creds, auth_method)` 形参链删 `creds`/`auth_method` 形参。

> 逐 core fn 核实：no-cred 版已 SSH 化且签名匹配（env_vars/ini_editor 已确认有 `set`/`get`/`read_section`/`set_key`）。

**验收：** `grep -rn "_with_credential" src/` → 仅剩 `core/winrm.rs`（9，P5a）。

### Batch 3 — deploy 凭据门
**文件：** `core/deploy_workflow.rs`（`WriteBackendGraph` ~324、`SetPsoCvars` ~400）。

删 `let (u,p) = creds.ok_or_else(|| "credentials required")?;`，把 `ini_editor::set_backend_field_with_credential(host, ..., u, p)` → `set_backend_field(host, ...)`、`set_key_with_credential(host, ..., u, p)` → `set_key(host, ...)`（无-cred；这俩在 Batch 2 已成无-cred 版）。`creds` 形参在 deploy 链上若全无消费则删（核实 `apply_one`/调用链签名）；Tauri/CLI deploy 命令签名留 shim。

**验收：** deploy 路径不再 `ok_or_else("creds")`；`creds: None` 不再运行时崩。lanPC 抽验一条 deploy 步骤（WriteBackendGraph 或 SetPsoCvars）。

### Batch 4 — 凭据子系统：`save_credential` repoint + from_sql 容错
**文件：** `commands/credentials.rs`（`save_credential` 26/32）、`data/credentials.rs`（`from_sql` 24-34）。

1. `save_credential`：删 `core_creds::store(...)`（cmdkey）+ `core_creds::store_password(...)`（DPAPI）→ 改 `SecretStore::from_config()?.put(&alias, &password)?`；保留 SQLite 元数据 insert（kind/username/alias）；删 DPAPI 失败回滚那段（SecretStore 单写，无双写回滚需求）。**Windows-only cmdkey/DPAPI 不再触碰**——save 变跨平台。
2. `data/credentials.rs::from_sql`：未知 kind 不再硬 `Err`，容错（落到 `Winrm` 或新增 `Unknown`/保守保留原字符串——实测现有 enum，倾向把未知映射到一个不破坏列表的变体并保留 `Winrm`）。
3. `list_credentials` 已读 SQLite（核实，不改）。
4. `cli/domain_cred.rs` 的 `save`（83 用 cmdkey/DPAPI）：同步 repoint 到 SecretStore::put（CLI/Tauri parity），或在 Batch 5 由 `secret set` 取代后标退役——实现时定（倾向 repoint 保留 `cred save` 可用）。

**验收：** `save_credential` 不再调 DPAPI/cmdkey；mac 上 save 不报 Windows-only；`list_credentials` 仍列旧 winrm 行（from_sql 不炸）。

### Batch 5 — 新建 `secret` CLI 域
**文件：** `cli/args.rs`（`Domain::Secret` + `SecretAction`）、`cli/mod.rs`、`cli/run.rs`（dispatch）、新建 `cli/domain_secret.rs`、`core/secrets.rs`（按需加 `list`）。

- `secret set <alias> [--value V | stdin]`：`SecretStore::put`。无 `--value` 读 stdin（一行，`\r\n` trim；对齐 `--pass-stdin` 写法）。
- `secret get <alias>`：`SecretStore::get` → 打印（emit_result；注意密钥输出——只在显式 get 时打印明文，文档标注）。
- `secret list`：列别名。源用 **SQLite `credentials` 元数据别名**（与 listCredentials 同源）或给 `SecretStore` 加 `list()`（返回 keys，不返 secret）——倾向后者（SecretStore 自洽，`pub fn list(&self) -> UecmResult<Vec<String>>` 返回 `read_all()?.keys()`）。
- `secret delete <alias>`：`SecretStore::delete`（best-effort + destructive::check `--yes/--dry-run`，套 `cred delete` 的 destructive 模式）。
- 走现有 `Ctx`/`Emitter`/`destructive` 机制；非 Windows 可用（SecretStore 跨平台）。
- `cred` 域：保留只读兼容（`cred list`）；`cred save` 的 DPAPI 写在 Batch 4 已 repoint，或提示用 `secret set`——实现时定，不删 `cred` 域命令（CLI 不退化）。

**验收：** `uecm-cli secret set/get/list/delete` 端到端（mac 可跑，SecretStore 跨平台）；`uecm-cli secret --help` flag 正确。

---

## 顺序与依赖
Batch 1 →（解锁字段删除）→ Batch 2 →（无-cred 版就位）→ Batch 3。Batch 4/5 独立，可在 1–3 后任意序。每批独立 build 绿 + commit + review。

## 验收（对照 spec P4 + Stop-hook）
- `cargo test --lib` 绿（基线 ~972）；`cargo build` 仅 pre-existing warning。
- `grep -rn "_with_credential" src/` → 仅 `core/winrm.rs`（P5a）。
- `grep -rn "AuthMethod\|auth_method" src/` → 0（或仅 winrm.rs 的 `&str` 形参，P5a）。
- `save_credential` 不再 DPAPI/cmdkey；`secret` 域可用。
- codex `--base` 无 blocker。
- **Stop-hook grep 张力**（收尾如实报）：`winrm::probe`（`domain_winrm.rs` 的 `winrm probe` 命令）+ `resolve_password`（`commands/bootstrap.rs` 远程推送、`core/secrets.rs` 的 transitional fallback、`credentials.rs` 定义/测试）按 spec 属 **P5a/P5b**，P4 后不为零——P4 收尾向用户摆明，由用户在 P5 门决定是否前移。
- lanPC：env set/get 或 ini 一条 + 一条 deploy 步骤 + `secret set/get` 抽验。
