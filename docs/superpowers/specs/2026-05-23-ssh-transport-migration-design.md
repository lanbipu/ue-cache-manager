# UECM WinRM→SSH 全量传输迁移 · 设计 Spec

> 状态:设计稿(v2,已纳入 Codex adversarial review),待用户审阅。审阅通过后由 `superpowers:writing-plans` 拆成实现计划。
> 分支/worktree:`.worktrees/ssh-transport-migration`(分支 `ssh-transport-migration`,off main `0e39fb6`)。
> 前置已完成:A0–A4(详见 `docs/superpowers/plans/2026-05-23-ssh-transport-phase-a2-readonly.md` / `-a3-a4.md`)。

## 1. 目标与终态

**目标**:把 UECM 的远程传输从 WinRM 彻底迁到 SSH,删光 WinRM 与 DPAPI,使整个项目只剩一条远程通道(`core/ssh.rs`)和一个跨平台 secret store(`core/secrets.rs`)——**且全程不破坏已发布的 Tauri UI**。

**终态(done 的定义)**:
- 无 `core/winrm.rs`、无 `winrm` CLI 命令域、无 `invoke-remote.ps1` / `test-winrm.ps1`。
- 无 DPAPI:`core/credentials.rs` 的 `store_password`/`resolve_password` 删除,`cred-set.ps1`/`cred-delete.ps1`/`dpapi.ps1` 删除。
- 无 WinRM 纳管:`enable-winrm.ps1` / `bootstrap-winrm-remote.ps1` / `preflight-path-b.ps1` / `core/bootstrap.rs`(WinRM 部分)/ `core/preflight.rs` 删除;`UECM-Bootstrap.cmd` 转纯 SSH。
- 所有节点操作走 `ssh::run_json` + 节点纯脚本(`-File` + stdin JSON);连通性探测走 `ssh::probe`。
- 所有需持久化的 secret 走 `SecretStore`(AES-GCM)。
- **core fn + CLI** 不再带 operator WinRM cred 参数 / 不再 `resolve_password`。**Tauri 命令(`commands/*`)保持向后兼容**:保留现有 optional cred 参数(接受但不再使用),不删/不改名命令——直到子项目 B 更新 Vue 调用层(见 §5 兼容契约)。
- 新节点首次纳管 = 手动 `UECM-Bootstrap.cmd`(U 盘/双击,跑 `enable-ssh.ps1`)。**远程推送纳管(WinRM/PsExec)能力主动放弃**(SSH 无法连一台还没开 SSH 的机器)。
- `cargo test --lib` 全绿 + `pnpm tauri build --no-bundle` 通过 + lanPC 真节点抽验 + **受影响 Tauri 命令的 UI/命令级 smoke 验证**。

## 2. 这份 spec 为什么存在

A0–A4 只迁了**命令行(`cli/*`)的诊断域 + mutating 域 + inject**。A3 计划写「完成后只剩 winrm.rs/DPAPI 待 A5 删」是乐观假设、**与代码不符**:zen、machine/discovery 探测、winrm 命令域、以及**整个 Tauri UI 后端(`commands/*`)** 从没迁过,仍合理依赖 WinRM/DPAPI。A5 照字面删会炸 build(详见 memory `a5_blocked_winrm_dpapi_live`)。本 spec 基于对**整个 codebase** 的传输/凭据用法普查,给出完整、不漏、且不破坏 UI 的迁移路线。

## 3. 完整清单(深度普查产出)

### 3.1 已在 SSH(A0–A4,不动)
- CLI 诊断域(A2):command-line-scan / consistency / renderstream / ini-read / health / project-discovery / ddc-file-stats / ue-log。
- CLI mutating 域(A3):env_vars / local_cache / ddc_pak / ini_editor / ue_runner / shares / distribute(pak+pso)。
- A4:inject-system-credential(+ option C:PsExec 经 onboarding 装到 `C:\ProgramData\UECM\`)。
- 基础设施:`core/ssh.rs`(run/run_json/probe/scp + auto-staging)、`core/secrets.rs`(SecretStore)、`enable-ssh.ps1`。
- 节点纯脚本(23 个,stdin JSON)。

### 3.2 还在 WinRM(`core/winrm.rs` 活引用)
| 块 | 位置 | 机制 | 规模 |
|---|---|---|---|
| **zen 域** | 15 个 `zen-*.ps1` + `core/zen/*.rs`(verify.rs)+ `cli/domain_zen.rs` + `commands/zen.rs` | `read_script(name)` → 参数 splat 进 body(`& { body } @params`) → `run_remote` → `winrm::invoke(body)`(经 `invoke-remote.ps1` 远程跑整脚本体) | **大** |
| **machine 探测** | `cli/domain_machine.rs`(`winrm::probe` / `probe_with_credential`,4 处) | 端口 5985 连通性 | 小 |
| **discovery 探测** | `commands/discovery.rs`(`winrm::probe`) | 同上 | 小 |
| **winrm 命令域** | `cli/domain_winrm.rs`(probe/bootstrap/preflight)+ `invoke-remote.ps1` + `test-winrm.ps1` | WinRM 纳管工具本身 | 中 |

zen 15 脚本:detect-binary / down / env-cleanup / probe-cache-stats / probe-health / read-lockfile / service-install / service-status / service-uninstall / up / urlacl-add / urlacl-list / urlacl-remove / verify-rules / write-lua-config。
⚠️ 其中 probe-cache-stats / probe-health / read-lockfile 在 Rust 里**无直接字面引用**(疑似经通用 `run_remote` + 动态脚本名调用)。**P2 第一步必须完整 trace zen 全部调用路径**,确保 15 个一个不漏。

### 3.3 还在 DPAPI(`core/credentials.rs` store/resolve_password)
| 层 | 文件 | 性质 |
|---|---|---|
| **CLI** | `cli/credential_args.rs`(operator `--cred-alias` 解析)、`cli/domain_cred.rs`(`cred save` → store) | operator cred,SSH 世界已 vestigial,可干净移除 |
| **commands/\*(UI 后端,本计划范围内)** | env_vars / ini_editor / ini_scanner / batch / log_verify / shares / deploy / projects / pso / ddc_pak / bootstrap / health_check / credentials / zen(共 ~14) | 多为「解析 DPAPI 密码 → 丢给已是 SSH 的 core fn(被忽略)」=vestigial;`commands/zen`/`commands/discovery` 是真用 WinRM |
- DPAPI 实现:`cred-set.ps1` / `cred-delete.ps1` / `dpapi.ps1`(operator 本地 PS)。

### 3.4 Vue 前端调用面(决定兼容契约,见 §5)
Vue stores 经 `tauriApi` 把 cred 别名传给 Tauri 命令:`stores/shares.ts`、`stores/pso.ts`、`stores/ddcPak.ts`(operator + source_smb)、`stores/batch.ts`、`stores/projects.ts`、`lib/deployApi.ts`(`deploy_ddc_run` 带 credentialAlias)、`stores/credentials.ts`(`listCredentials` + 凭据页,UI 文案明说「只列 WinRM 凭据」)。**这些是 Vue↔Rust 的 API 边界,改 Rust 命令签名/删命令会让 UI 运行时崩(`cargo test/check` 抓不到)。**

### 3.5 operator 本地 PS(`powershell::run_json`,非远程传输)
- `core/credentials.rs`(DPAPI/cmdkey,4 处)→ 随 DPAPI 删。
- `core/bootstrap.rs`(bootstrap-winrm-remote)、`core/preflight.rs`(preflight-path-b)、`core/winrm.rs`(test-winrm probe)→ 随 WinRM 删。
- `cli/domain_system.rs`(echo→test-echo.ps1)、`commands/system.rs`(echo)→ **operator 本地自检,非节点操作,出范围,保留**。

### 3.6 纳管/bootstrap
- 留:`enable-ssh.ps1`、`UECM-Bootstrap.cmd`(转纯 SSH)、`package-winrm-bootstrap.ps1`(重命名/转 SSH-only 打包器)。
- 删:`enable-winrm.ps1`、`bootstrap-winrm-remote.ps1`、`preflight-path-b.ps1`。
- `UECM-Bootstrap.cmd` 现在 WinRM+SSH 双跑 → 改成只跑 `enable-ssh.ps1`。

## 4. 打法

**增量「先迁后删」,全程 build 绿 + UI 不破**:每个 WinRM/DPAPI 消费方先迁到 SSH(winrm.rs/DPAPI 暂留),全部迁完、`grep` 活引用清零后,最后 phase 才删。每步 `cargo test`+build 可验、可 codex review、可随时停。否决 big-bang(无法逐步验证)。

## 5. Vue↔Tauri 命令兼容契约(回应 Codex adversarial review · NO-SHIP)

**问题**:P3/P4 杀 DPAPI/operator-cred 会动 `commands/*`;若同时改 Tauri 命令的对外契约,Vue 调用层(§3.4)会运行时崩,而 backend 检查(cargo test/check)全绿——掩盖故障。

**契约(本迁移的硬约束)**:
1. **本迁移完全不修改 Vue**(`src/**`)。靠保持 Tauri 命令 API 向后兼容来保证 UI 不破,而非依赖 Vue 跟改。
2. **杀 DPAPI ≠ 改命令签名**:P3 只删 `commands/*` 内部的 `resolve_password` 调用(DPAPI 死);**保留命令现有 optional cred 参数**(`operator_credential_alias` 等)在签名里,接受但不再使用(thin compat shim)。Vue 继续传这些字段→命令照收→忽略→不崩(不依赖 serde 忽略未知字段这种脆弱前提)。
3. **不删、不改名任何 Tauri 命令**;`commands/credentials.rs` 的 `listCredentials` 等**必须持续返回合法结果**(WinRM/DPAPI 删除后,凭据列表要么改读 SecretStore 别名、要么优雅返回空,**绝不报错**)。
4. **可干净移除 operator-cred 的只有 core fn 与 CLI**(我们控全部 caller)。Tauri 命令层的参数/控件清理 + Vue store/视觉清理 = **子项目 B**,本迁移留好向后兼容供 B 自行排期。
5. **验证门禁**:进入 P5 删除前,枚举每个受 P3/P4 影响的 Tauri 命令 + 其 Vue invoke 调用点,逐一确认命令仍存在且签名兼容,并对这些命令做 UI/命令级 smoke 验证(Tauri dev 跑一遍关键流,或直接 invoke 这些命令断言不报错)。

## 6. 阶段分解

每个 phase 收尾:`cargo test --lib` 全绿 + `cargo check --all-targets` + 改了节点脚本的在 lanPC 真机抽验 + codex review。提交用 `git -c commit.gpgsign=false`,只新增 commit 不 amend。

### P1 — 探测迁移(小,先做)
- `cli/domain_machine.rs`(4 处)+ `commands/discovery.rs`:`winrm::probe[_with_credential](host)` → `SshExecutor::from_config()?.probe(host, None)`。
- ⚠️ 核实 `ssh::ProbeResult { ok, message, latency_ms }` 字段满足 machine/discovery 现有消费(winrm ProbeResult 可能字段不同);差异处适配。
- `commands/discovery.rs` 命令签名不变(契约 §5)。不删 winrm.rs(probe 仍被 winrm 命令域引用)。

### P2 — zen 域迁移(大,可再细分)
- 先**完整 trace** zen 全部调用路径(含 3 个无字面引用的脚本 + `run_remote` 通用机制 + 参数 splat)。
- 15 个 `zen-*.ps1` 逐个改 node-pure(stdin JSON,套 A2/A3 配方 + 5 条 PS5.1 真机坑)。
- zen plumbing:`core/zen/verify.rs`、`cli/domain_zen.rs`(`run_remote` + body 构造)、`commands/zen.rs` 从 `read_script(body)+winrm::invoke(body)` 换 `ssh::run_json(&exec, host, &NodeScript{name, args})`。
- `commands/zen.rs` 命令签名保持兼容(契约 §5):内部换 SSH,对 Vue 的命令接口不变。
- 细分建议:P2a 只读(detect-binary/probe-*/read-lockfile/service-status/urlacl-list/verify-rules)、P2b mutating(up/down/urlacl-add/remove/write-lua-config/env-cleanup)、P2c service(service-install/uninstall)。
- 改 zen 前读 `docs/zen-integration.md` + `docs/research/plan7-deferral-acceptance-2026-05-20.md`(per CLAUDE.md);grep「Codex round」防重复踩坑。
- 不删 winrm.rs。

### P3 — 杀 DPAPI(commands 内部去解析,签名不变)
- ~14 个 `commands/*`:删 `resolve_password` 解析 + 透传给 core fn 的 vestigial operator cred(core fn 已忽略);**保留命令签名里的 optional cred 参数为接受-忽略的 shim**(契约 §5.2)。
- `commands/zen` / `commands/discovery`:随 P1/P2 的 core 迁移更新为 SSH,签名不变。
- ⚠️ 逐个核实每处 `resolve_password` 喂进去的密码在 core 侧确实 vestigial(已验:env/ini/deploy/pso/ddc_pak 的 core fn 走 SSH/SecretStore 忽略 operator pass);若某处 core 仍真用密码,该 core fn 也要一并迁。
- 不删 DPAPI 实现本身(留到 P5)。

### P4 — CLI 去 operator-cred + 定 cred 域(不破 UI)
- `cli/credential_args.rs`:删 `--cred-alias` 的 DPAPI `resolve_password` 路径(operator cred 在 SSH 世界无意义)。
- 各 migrated core fn:删 `let _ = (user, pass)` 占位参数 + `_with_credential` 变体(`env_vars`/`ini_editor`/`psexec` 等);更新 **CLI** 调用方去掉传参;**Tauri 命令层维持 shim**(契约 §5)。
- **`cred` 命令域决策(区分两层)**:
  - **CLI `domain_cred`**(我们控):删 `cred save` 的 DPAPI store 路径;`cred list/delete` 若对 SecretStore 有价值则保留指向 SecretStore,否则删 CLI 子命令。
  - **Tauri `commands/credentials.rs`**(UI 用):`listCredentials` 等**保留且持续可用**(改读 SecretStore 别名或优雅空列表),不报错(契约 §5.3)。
- 不删 DPAPI 实现本身(留到 P5)。

### P5 — 拆除(删,门禁:前置完成 + grep 清零 + UI smoke)
- **进入门禁**:执行契约 §5.5 的 Vue-caller 审计 + 受影响 Tauri 命令 smoke 验证,确认无命令被删/改名、`listCredentials` 等仍工作。
- **5a 删 WinRM(门禁 P1+P2 完)**:`core/winrm.rs`、`cli/domain_winrm.rs`、`invoke-remote.ps1`、`test-winrm.ps1`、`core/bootstrap.rs`(WinRM)、`core/preflight.rs`、`enable-winrm.ps1`、`bootstrap-winrm-remote.ps1`、`preflight-path-b.ps1`;`UECM-Bootstrap.cmd` 转纯 SSH;`package-winrm-bootstrap.ps1` 转 SSH-only;清 `lib.rs`/`mod.rs`/`run.rs`/`args.rs` 里的 winrm 注册。`CredentialKind::Winrm` 的去留要确保不破 `listCredentials`(保留 enum 变体为 no-op 或迁移既有行)。
- **5b 删 DPAPI(门禁 P3+P4 完)**:`core/credentials.rs` 的 store/resolve_password、`cred-set.ps1`/`cred-delete.ps1`/`dpapi.ps1`。
- 每删一项先 `grep` 确认零活引用(沿用本次发现 A5 的闸门)。
- 收尾:全量 `cargo test`+build,lanPC 抽验一遍 health run + zen + share/distribute,+ 受影响 Tauri 命令 smoke。

## 7. 顺序与依赖
推荐执行序:**P1 → P2 → P3 → P4 → P5a → P5b**。删除门禁:5a 需 P1+P2;5b 需 P3+P4 + §5.5 UI 验证。

## 8. 实现时要钉死的开放项(spec 已标,plan/实现阶段解决)
1. zen 3 个无字面引用脚本的真实调用路径(P2 trace 第一步)。
2. `ssh::ProbeResult` 与 winrm `ProbeResult` 字段差异适配(P1)。
3. 每处 commands/CLI 的 `resolve_password` 是否真 vestigial(P3/P4 逐处核)。
4. CLI `cred` 子命令最终去留 + Tauri `listCredentials` 在 WinRM/DPAPI 删除后的数据来源(SecretStore 别名 / 空列表)(P4)。
5. zen 参数 splat 机制 → stdin JSON 的逐脚本映射(P2)。
6. 确认 `CredentialKind::Winrm` 移除不破坏 `commands/credentials.rs` + Vue 凭据页(P5a)。

## 9. 范围内 / 范围外(边界,回应 Codex)
**范围内**:`core/*`、`cli/*`、`commands/*.rs`(Rust transport + Tauri 命令实现,**保持 API 向后兼容**)、`ps-scripts/*`、纳管脚本、`core/secrets.rs`。
**范围外(子项目 B)**:`src/**`(Vue 组件/stores/视觉/Figma 重构),包括清理 Vue 里已失效的 cred-alias 控件与 invoke payload、以及最终从 Tauri 命令签名删掉 shim 参数。本迁移留好向后兼容供 B 排期。
**范围外(其它)**:`domain_system`/`commands/system` 的本地 echo PS;节点 GPU 主动上报 / nDisplay 实时编排(CLAUDE.md listener-agent 范畴)。

## 10. 风险
- **zen 体量大**(15 脚本 + 14 core 模块 + Plan 7 历史 22 轮 review):P2 是长 pole,严格逐脚本真机验证。
- **UI 静默崩(Codex NO-SHIP)**:已用 §5 兼容契约消解——本迁移不碰 Vue、Tauri 命令保持向后兼容、P5 前 UI smoke 门禁。残余风险:某命令的参数语义变化未被 shim 覆盖→靠 §5.5 审计兜底。
- **lanPC 端到端当前不通**:Mac keystore key ≠ lanPC 授权 key + 无 uecm-svc 账号(memory `mac_keystore_key_mismatch`)。真 E2E 前需重 onboard lanPC;期间真机验证走 `ssh lanpc` 手动喂 stdin(本次 A4 验证用过的姿势)。
- **真机坑单测抓不到**:5 条 PS5.1 节点纯脚本坑(ErrorAction/null-guard/ArrayList/Split-Path/-EA Stop)+ A4 暴露的 PsExec stdout/file-redirect 类问题,每个改了脚本的 phase 必须真机抽验。

## 11. 验证策略
- 每 phase:`cargo test --lib`(基线 975)+ `cargo check --all-targets` + 改脚本的真机抽验 + codex review(`--base <上一 commit>`)。
- 改 Tauri 命令的 phase(P1/P3):额外确认命令签名向后兼容(§5)。
- P5 前:§5.5 Vue-caller 审计 + 受影响 Tauri 命令 smoke(Tauri dev 跑关键 UI 流 或 直接 invoke 断言不报错)。
- P5 后:全量 build + lanPC 端到端(health run + zen 全链 + share/distribute,前提 lanPC 已重 onboard)。
- 最终 `grep -rn 'winrm\|resolve_password\|store_password\|invoke-remote' src-tauri/src ps-scripts` 仅余测试/注释/无害残留。
