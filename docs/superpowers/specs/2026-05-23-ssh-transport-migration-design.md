# UECM WinRM→SSH 全量传输迁移 · 设计 Spec

> 状态:设计稿,待用户审阅。审阅通过后由 `superpowers:writing-plans` 拆成实现计划。
> 分支/worktree:`.worktrees/ssh-transport-migration`(分支 `ssh-transport-migration`,off main `0e39fb6`)。
> 前置已完成:A0–A4(详见 `docs/superpowers/plans/2026-05-23-ssh-transport-phase-a2-readonly.md` / `-a3-a4.md`)。

## 1. 目标与终态

**目标**:把 UECM 的远程传输从 WinRM 彻底迁到 SSH,删光 WinRM 与 DPAPI,使整个项目只剩一条远程通道(`core/ssh.rs`)和一个跨平台 secret store(`core/secrets.rs`)。

**终态(done 的定义)**:
- 无 `core/winrm.rs`、无 `winrm` CLI 命令域、无 `invoke-remote.ps1` / `test-winrm.ps1`。
- 无 DPAPI:`core/credentials.rs` 的 `store_password`/`resolve_password` 删除,`cred-set.ps1`/`cred-delete.ps1`/`dpapi.ps1` 删除。
- 无 WinRM 纳管:`enable-winrm.ps1` / `bootstrap-winrm-remote.ps1` / `preflight-path-b.ps1` / `core/bootstrap.rs`(WinRM 部分)/ `core/preflight.rs` 删除;`UECM-Bootstrap.cmd` 转纯 SSH。
- 所有节点操作走 `ssh::run_json` + 节点纯脚本(`-File` + stdin JSON);连通性探测走 `ssh::probe`。
- 所有需持久化的 secret 走 `SecretStore`(AES-GCM)。
- core fn 不再带 `let _ = (user, pass)` 占位的 operator WinRM cred 参数;调用方不再 `resolve_password` 解析 operator 凭据。
- 新节点首次纳管 = 手动 `UECM-Bootstrap.cmd`(U 盘/双击,跑 `enable-ssh.ps1`)。**远程推送纳管(WinRM/PsExec)能力主动放弃**(SSH 无法连一台还没开 SSH 的机器,鸡生蛋)。
- `cargo test --lib` 全绿 + `pnpm tauri build --no-bundle` 通过 + lanPC 真节点抽验。

## 2. 这份 spec 为什么存在

A0–A4 只迁了**命令行(`cli/*`)的诊断域 + mutating 域 + inject**。A3 计划写「完成后只剩 winrm.rs/DPAPI 待 A5 删」是乐观假设、**与代码不符**:zen、machine/discovery 探测、winrm 命令域、以及**整个 Tauri UI 后端(`commands/*`)** 从没迁过,仍合理依赖 WinRM/DPAPI。A5 照字面删会炸 build(详见 memory `a5_blocked_winrm_dpapi_live`)。本 spec 基于对**整个 codebase** 的传输/凭据用法普查,给出完整、不漏的迁移路线。

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
| **CLI** | `cli/credential_args.rs`(operator `--cred-alias` 解析)、`cli/domain_cred.rs`(`cred save` → store) | operator cred,SSH 世界已 vestigial |
| **commands/\*(UI 后端,本计划范围内)** | env_vars / ini_editor / ini_scanner / batch / log_verify / shares / deploy / projects / pso / ddc_pak / bootstrap / health_check / credentials / zen(共 ~14) | 多为「解析 DPAPI 密码 → 丢给已是 SSH 的 core fn(被忽略)」=vestigial;`commands/zen`/`commands/discovery` 是真用 WinRM |
- DPAPI 实现:`cred-set.ps1` / `cred-delete.ps1` / `dpapi.ps1`(operator 本地 PS)。

### 3.4 operator 本地 PS(`powershell::run_json`,非远程传输)
- `core/credentials.rs`(DPAPI/cmdkey,4 处)→ 随 DPAPI 删。
- `core/bootstrap.rs`(bootstrap-winrm-remote)、`core/preflight.rs`(preflight-path-b)、`core/winrm.rs`(test-winrm probe)→ 随 WinRM 删。
- `cli/domain_system.rs`(echo→test-echo.ps1)、`commands/system.rs`(echo)→ **operator 本地自检,非节点操作,出范围,保留**。

### 3.5 纳管/bootstrap
- 留:`enable-ssh.ps1`、`UECM-Bootstrap.cmd`(转纯 SSH)、`package-winrm-bootstrap.ps1`(重命名/转 SSH-only 打包器)。
- 删:`enable-winrm.ps1`、`bootstrap-winrm-remote.ps1`、`preflight-path-b.ps1`。
- `UECM-Bootstrap.cmd` 现在 WinRM+SSH 双跑 → 改成只跑 `enable-ssh.ps1`。

## 4. 打法

**增量「先迁后删」,全程 build 绿**:每个 WinRM/DPAPI 消费方先迁到 SSH(winrm.rs/DPAPI 暂留),全部迁完、`grep` 活引用清零后,最后 phase 才删。每步 `cargo test`+build 可验、可 codex review、可随时停。否决 big-bang(无法逐步验证)。

## 5. 阶段分解

每个 phase 收尾:`cargo test --lib` 全绿 + `cargo check --all-targets` + 改了节点脚本的在 lanPC 真机抽验 + codex review。提交用 `git -c commit.gpgsign=false`,只新增 commit 不 amend。

### P1 — 探测迁移(小,先做)
- `cli/domain_machine.rs`(4 处)+ `commands/discovery.rs`:`winrm::probe[_with_credential](host)` → `SshExecutor::from_config()?.probe(host, None)`。
- ⚠️ 核实 `ssh::ProbeResult { ok, message, latency_ms }` 字段满足 machine/discovery 现有消费(winrm ProbeResult 可能字段不同);差异处适配。
- 不删 winrm.rs(probe 仍被 winrm 命令域引用)。

### P2 — zen 域迁移(大,可再细分)
- 先**完整 trace** zen 全部调用路径(含 3 个无字面引用的脚本 + `run_remote` 通用机制 + 参数 splat)。
- 15 个 `zen-*.ps1` 逐个改 node-pure(stdin JSON,套 A2/A3 配方 + 5 条 PS5.1 真机坑)。
- zen plumbing:`core/zen/verify.rs`、`cli/domain_zen.rs`(`run_remote` + body 构造)、`commands/zen.rs` 从 `read_script(body)+winrm::invoke(body)` 换 `ssh::run_json(&exec, host, &NodeScript{name, args})`。
- 细分建议:P2a 只读(detect-binary/probe-*/read-lockfile/service-status/urlacl-list/verify-rules)、P2b mutating(up/down/urlacl-add/remove/write-lua-config/env-cleanup)、P2c service(service-install/uninstall)。
- 改 zen 前读 `docs/zen-integration.md` + `docs/research/plan7-deferral-acceptance-2026-05-20.md`(per CLAUDE.md);grep「Codex round」防重复踩坑。
- 不删 winrm.rs。

### P3 — commands/* 去 DPAPI(UI 后端,本计划范围)
- ~14 个 `commands/*`:删 `resolve_password` 解析 + 透传给 core fn 的 vestigial operator cred(core fn 已忽略)。
- `commands/zen` / `commands/discovery`:随 P1/P2 的 core 迁移更新为 SSH。
- ⚠️ 逐个核实每处 `resolve_password` 喂进去的密码在 core 侧确实 vestigial(已验:env/ini/deploy/pso/ddc_pak 的 core fn 走 SSH/SecretStore 忽略 operator pass);若某处 core 仍真用密码,该 core fn 也要一并迁。
- **不动 Vue 视觉层**(子项目 B);只动 `commands/*.rs`(Rust transport)。
- 不删 DPAPI。

### P4 — CLI 去 operator-cred + 定 cred 域
- `cli/credential_args.rs`:删 `--cred-alias` 的 DPAPI `resolve_password` 路径(operator cred 在 SSH 世界无意义)。
- 各 migrated core fn:删 `let _ = (user, pass)` 占位参数 + `_with_credential` 变体(`env_vars`/`ini_editor`/`psexec` 等);更新 CLI/commands 调用方去掉传参。
- **`cred` 命令域决策**:`domain_cred` 现在 `cred save`→DPAPI store(kind=winrm/share)。SSH 世界 operator cred 没了、share secret 走 SecretStore(由 `share create` 管)。决策(P4 实现时定):**收缩成只读 SecretStore 别名展示 / 删掉 `cred save` 的 DPAPI 路径 / 整域删除**。倾向:删 DPAPI 路径,`cred list/delete` 若有 SecretStore 价值则保留,否则整域删。
- 不删 DPAPI 实现本身(留到 P5)。

### P5 — 拆除(删,门禁:前置完成 + grep 清零)
- **5a 删 WinRM(门禁 P1+P2 完)**:`core/winrm.rs`、`cli/domain_winrm.rs`、`invoke-remote.ps1`、`test-winrm.ps1`、`core/bootstrap.rs`(WinRM)、`core/preflight.rs`、`enable-winrm.ps1`、`bootstrap-winrm-remote.ps1`、`preflight-path-b.ps1`;`UECM-Bootstrap.cmd` 转纯 SSH;`package-winrm-bootstrap.ps1` 转 SSH-only;清 `lib.rs`/`mod.rs`/`run.rs`/`args.rs` 里的 winrm 注册 + `CredentialKind::Winrm`。
- **5b 删 DPAPI(门禁 P3+P4 完)**:`core/credentials.rs` 的 store/resolve_password、`cred-set.ps1`/`cred-delete.ps1`/`dpapi.ps1`。
- 每删一项先 `grep` 确认零活引用(沿用本次发现 A5 的闸门)。
- 收尾:全量 `cargo test`+build,lanPC 抽验一遍 health run + zen + share/distribute。

## 6. 顺序与依赖
P1 →(独立)P3 可与 P2 并行概念上,但建议串行 review。删除门禁:5a 需 P1+P2;5b 需 P3+P4。推荐执行序:**P1 → P2 → P3 → P4 → P5a → P5b**。

## 7. 实现时要钉死的开放项(spec 已标,plan/实现阶段解决)
1. zen 3 个无字面引用脚本的真实调用路径(P2 trace 第一步)。
2. `ssh::ProbeResult` 与 winrm `ProbeResult` 字段差异适配(P1)。
3. 每处 commands/CLI 的 `resolve_password` 是否真 vestigial(P3/P4 逐处核)。
4. `cred` 命令域最终去留(P4)。
5. zen 参数 splat 机制 → stdin JSON 的逐脚本映射(P2)。

## 8. 范围外
- Vue 视觉层 / Figma 重构 = 子项目 B(只动 `commands/*.rs` 的 transport,不动组件)。
- `domain_system` / `commands/system` 的本地 echo PS(operator 自检,非节点操作)。
- 节点 GPU 主动上报 / nDisplay 实时编排等(CLAUDE.md 里 listener-agent 范畴,非本次)。

## 9. 风险
- **zen 体量大**(15 脚本 + 14 core 模块 + Plan 7 历史 22 轮 review):P2 是长 pole,严格逐脚本真机验证。
- **lanPC 端到端当前不通**:Mac keystore key ≠ lanPC 授权 key + 无 uecm-svc 账号(memory `mac_keystore_key_mismatch`)。真 E2E 前需重 onboard lanPC;期间真机验证走 `ssh lanpc` 手动喂 stdin(本次 A4 验证用过的姿势)。
- **真机坑单测抓不到**:5 条 PS5.1 节点纯脚本坑(ErrorAction/null-guard/ArrayList/Split-Path/-EA Stop)+ A4 暴露的 PsExec stdout/file-redirect 类问题,每个改了脚本的 phase 必须真机抽验。
- **operator cred 删除影响 UI**:P3/P4 改 `commands/*` 签名,子项目 B 的 Vue 调用层需同步(本计划只保证 Rust 编译/命令可用;Vue 端在 B 里跟进)。

## 10. 验证策略
- 每 phase:`cargo test --lib`(基线 975)+ `cargo check --all-targets` + 改脚本的真机抽验 + codex review(`--base <上一 commit>`)。
- P5 后:全量 build + lanPC 端到端(health run + zen 全链 + share/distribute),前提 lanPC 已重 onboard。
- 最终 `grep -rn 'winrm\|resolve_password\|store_password\|invoke-remote' src-tauri/src ps-scripts` 仅余测试/注释/无害残留。
