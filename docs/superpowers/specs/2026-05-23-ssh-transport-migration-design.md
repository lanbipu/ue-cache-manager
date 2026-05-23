# UECM WinRM→SSH 全量传输迁移 · 设计 Spec

> 状态:设计稿 **v3**(已纳入 Codex adversarial review + max-effort /code-review 5-angle 发现),待用户审阅。审阅通过后由 `superpowers:writing-plans` 拆成实现计划。
> 分支/worktree:`.worktrees/ssh-transport-migration`(分支 `ssh-transport-migration`,off main `0e39fb6`)。
> 前置已完成:A0–A4(详见 `docs/superpowers/plans/2026-05-23-ssh-transport-phase-a2-readonly.md` / `-a3-a4.md`)。
> v2→v3 主要修订:补全 v2 漏掉的活 WinRM/DPAPI consumer(`commands/bootstrap`、`machine authorize/deep_scan`、`AuthMethod`、`network.winrm_open`、health 文案/taxonomy、`cred-list.ps1`、`save_credential`);修正 v2 错误的「vestigial」判断(`ddc_pak` source-SMB、`deploy` 凭据门、`shares` inject svc-pass 都真用密码);zen 改「12 传输脚本 + 3 reqwest-HTTP 孤儿」;P1 补 probe 语义(loopback/认证语义);新增 **P0** 把 `enable-ssh.ps1` 做成完整 onboarder;凭据子系统给具体终态。

## 1. 目标与终态

**目标**:把 UECM 远程传输从 WinRM 彻底迁到 SSH,删光 WinRM 与 DPAPI,使 core / CLI / transport 只剩一条远程通道(`core/ssh.rs`)和一个跨平台 secret store(`core/secrets.rs`)——**且全程不破坏已发布的 Tauri UI**。

**术语澄清(回应 review)**:
- 「退役 WinRM」= 删 `winrm.rs` + WinRM 命令域 + WinRM **远程推送纳管**(PsExec 推到全新节点)。
- **PsExec 本身保留**:A4 的 inject-system-credential 在**节点本地**用 PsExec 写 SYSTEM cmdkey,这条 SSH 路径**不动**。「放弃远程推送纳管」≠「删 PsExec」。

**终态(done 的定义,诚实版)**:
- 无 `core/winrm.rs`、无 `winrm` CLI 命令域、无 `invoke-remote.ps1` / `test-winrm.ps1`。
- 无 DPAPI:`core/credentials.rs` 的 `store_password`/`resolve_password`/`list_aliases`(cmdkey 读)删除,`cred-set.ps1`/`cred-delete.ps1`/`cred-list.ps1`/`dpapi.ps1` 删除。
- 无 WinRM 纳管核心:`enable-winrm.ps1` / `bootstrap-winrm-remote.ps1` / `preflight-path-b.ps1` / `core/bootstrap.rs` / `core/preflight.rs` 删除。
- **`enable-ssh.ps1` 成为完整独立 onboarder**(建 `uecm-svc` 账号 + SMB/LongPaths/HighPerformance/ExecutionPolicy 节点 prep),`UECM-Bootstrap.cmd` 转纯 SSH。
- 所有节点操作走 `ssh::run_json` + 节点纯脚本;连通性探测走 `ssh::probe`(含 loopback 旁路)。
- 所有需持久化的 secret 走 `SecretStore`;SQLite `credentials` 表仅存别名元数据(无密码)。
- **CLI 覆盖不退化(各 phase 落地见 §6:P1 起 `ssh` 域、P4 `secret` 域、P5a 退役 `winrm`+`machine authorize`)**:每个迁移域保留其 CLI 命令;退役的 `winrm` 命令域 + `machine authorize` 由**新 `ssh` 域**(`package-bootstrap`/`probe`/`authorize`)替代;退役的 `cred` 域由**新 `secret` 域**(`set`/`get`/`list`/`delete` SecretStore)替代。terminal/agent 能从命令行驱动纳管 + 管密钥(CLAUDE.md「所有功能 CLI 暴露」)。
- **core fn + CLI** 不再带 operator WinRM cred 参数 / 不再 `resolve_password`。**Tauri 命令(`commands/*`)保持向后兼容**:保留 optional cred 参数 shim,**WinRM-原生命令(`bootstrap_winrm` 等)repoint 成 SSH 信息 shim 而非删除**(见 §5)。Tauri 表面参数清理 + Vue 清理 = 子项目 B。
- `cargo test --lib` 全绿 + `pnpm tauri build --no-bundle` 通过 + lanPC 真节点抽验 + 受影响 Tauri 命令 UI/命令级 smoke。

**「done」不等于「每一行 WinRM 痕迹消失」**:Tauri 命令层的 shim 参数、Vue 里 `kind==='winrm'` 过滤与 `winrm_ok` 字段、`network.winrm_open`(5985 scan,`machine scan` 仍用)等显式留给子项目 B / 后续,本迁移用兼容契约托住、不删(见 §9)。

## 2. 这份 spec 为什么存在

A0–A4 只迁了 CLI 诊断域 + mutating 域 + inject。v1/v2 仍**漏掉活 consumer**(正是上次失败模式),经两轮 review 坐实:`commands/bootstrap`(UI 后端 WinRM 命令,Vue 在用)、`cli/domain_machine::authorize/deep_scan`、`AuthMethod` 线、`network.winrm_open`、health 文案/probe taxonomy、`cred-list.ps1`、`save_credential`(真 DPAPI writer);并把 `ddc_pak`/`deploy`/`shares-inject` 误判成 vestigial(其实真用密码)。v3 基于对整个 codebase 的完整普查 + 代码层面坐实,给出不漏、不误判、不破坏 UI 的路线。

## 3. 完整清单(普查 + 两轮 review 坐实)

### 3.1 已在 SSH(A0–A4,不动)
CLI 诊断域(A2 · 8)、CLI mutating 域(A3:env/local-cache/ddc-pak/ini/ue_runner/shares/distribute)、A4 inject(节点本地 PsExec 保留)。基础设施:`core/ssh.rs`、`core/secrets.rs`、`enable-ssh.ps1`、23 个 node-pure 脚本。

### 3.2 还在 WinRM(`core/winrm.rs` + 远程推送纳管,活引用)
| 块 | 位置(file:line) | 机制 | 处置 phase |
|---|---|---|---|
| **zen 域(传输)** | 12 个 `zen-*.ps1`(经 `read_script`+`run_remote`)+ `core/zen/verify.rs:119`、`cli/domain_zen.rs`(`run_remote`)+ `commands/zen.rs:429` | ship-body over WinRM + 参数 splat | P2 |
| **zen probe/stats/lockfile** | `core/zen/probe.rs:54`、`cache_stats.rs:63`(reqwest HTTP);`zen-probe-cache-stats/probe-health/read-lockfile.ps1`(**0 Rust 引用**) | 不经 WinRM,in-process HTTP / 孤儿脚本 | P2(处置孤儿,非迁移) |
| **machine 探测** | `cli/domain_machine.rs:255,256,458,461`(`winrm::probe`) | 端口连通 | P1 |
| **discovery 探测** | `commands/discovery.rs:74`(`winrm::probe`) | 同上 | P1 |
| **machine authorize/deep_scan** | `cli/domain_machine.rs:627,645`(`preflight_path_b`)、`:665`(`enable_winrm_with_psexec`);`args.rs:213` `MachineAction::Authorize` | WinRM 远程推送纳管(CLI) | P5(退役) |
| **Tauri WinRM 命令** | `commands/bootstrap.rs:9`(`get_winrm_bootstrap_script`)、`:14`(`bootstrap_winrm`),注册 `lib.rs:43-44`,**Vue 在用** `stores/machines.ts:130,151` | WinRM 远程推送纳管(UI) | P5(repoint 成 SSH 信息 shim) |
| **winrm 命令域** | `cli/domain_winrm.rs`(probe/bootstrap/preflight)+ `invoke-remote.ps1`+`test-winrm.ps1` | WinRM 工具本身 | P5(删) |
| **AuthMethod 线** | `cli/credential_args.rs:14`(`enum AuthMethod`)+ `auth_method` 参数(zen ~20 + commands/zen ~12 + env/ini/share/machine) | 仅喂 `winrm::invoke_with_credential` | P4(删) |
| **WinRM 端口语义** | `core/network.rs:19,38`(`winrm_open`/`PORT_WINRM=5985`)、`cli/output.rs:22,256`(HostProbe 输出字段)、`core/health_check.rs:89-106`(remediation 文案)、`core/probe_keys.rs:31`(`tcp_5985` + L2/L3 taxonomy) | 5985 探测 + 文案 | 见 §9(多数留给后续/B;文案在 P5 顺手更) |

zen 12 传输脚本:detect-binary / down / env-cleanup / service-install / service-status / service-uninstall / up / urlacl-add / urlacl-list / urlacl-remove / verify-rules / write-lua-config。**全是 `param()` 块,需逐个改 node-pure(stdin JSON),非"换 plumbing"**。

### 3.3 还在 DPAPI / cmdkey
| 性质 | 位置 | 处置 |
|---|---|---|
| **operator cred 解析(vestigial)** | `cli/credential_args.rs:125`、`commands/*` 中喂给已 SSH 化 core fn 的(env/ini/ini_scanner/batch/log_verify/projects/health_check) | P3/P4 删解析,签名留 shim |
| **真用密码(v2 误判为 vestigial,实为真迁)** | `commands/ddc_pak.rs:421`→source-SMB(node robocopy 用,`pak_distribute.rs:324-327`);`commands/shares.rs:200` `inject_share_credential_to_clients`→读 **svc** 别名密码(share secret 非 operator);`core/deploy_workflow.rs:324,400`(WriteBackendGraph/SetPsoCvars `ok_or_else("creds required")`) | P3(SMB/svc 改 SecretStore,套 pso 已有做法)/ P4(deploy 凭据门) |
| **DPAPI writer** | `commands/credentials.rs:26,32` `save_credential`(`store`+`store_password`);`cli/domain_cred.rs:83` | P4 repoint→SecretStore |
| **cmdkey/DPAPI 实现** | `core/credentials.rs`(store/resolve/list_aliases,4 处 `powershell::run_json`)、`cred-set/cred-delete/cred-list/dpapi.ps1` | P5b 删 |
| **DB 解析** | `data/credentials.rs:24-34` `from_sql` 对未知 kind **硬报错**;`CredentialKind::Winrm` | P4/P5:保留 Winrm 变体 + from_sql 容错 |
| **health-probes cmdkey** | `ps-scripts/health-probes.ps1`(SYSTEM `cmdkey /list` 验证 svc cred,A4 已节点纯) | 不删(cmdkey 验证仍有效),P5 更 remediation 文案 |

### 3.4 Vue 调用面(决定兼容契约)
- 传 cred 别名(shim 兜住,不崩):`stores/{shares,pso,ddcPak,batch,projects,credentials}.ts`、`lib/deployApi.ts`。
- **WinRM-原生命令(必须 repoint,不能删)**:`bootstrapWinrm`/`getWinrmBootstrapScript`(`stores/machines.ts:130,151` + `services/tauri.ts:614-627`,机器详情 bootstrap 面板);返回类型 `WinrmBootstrapResult`(Vue 读 `.ok/.message/.manual_script`)。
- **save**:`saveCredential`(`stores/credentials.ts:25`,CredentialDialog)。**list**:`listCredentials`(读 SQLite 元数据,DPAPI 无关)。
- **`kind==='winrm'` 过滤**:6+ 向导(ProjectDiscovery/PsoCollect/ShareCreate/DdcPak/PsoDistribute + MachineDetailTabs)——删 Winrm 变体会清空选择器。
- **响应字段**:`RefreshResult.winrm_ok`(`stores/machines.ts:112` 读)、`ProbedHost.winrm_open`(DiscoveryWizard 渲染)。

### 3.5 operator 本地 PS / 纳管(非节点远程传输)
- 随 DPAPI 删:`core/credentials.rs` 4 处。随 WinRM 删:`core/bootstrap.rs`、`core/preflight.rs`、`winrm::probe`(test-winrm)。
- 保留(出范围):`domain_system`/`commands/system` echo 自检。
- 纳管:留 `enable-ssh.ps1`(P0 扩成完整 onboarder)、`UECM-Bootstrap.cmd`(转纯 SSH)、`package-winrm-bootstrap.ps1`(转 SSH-only 打包器);删 `enable-winrm.ps1`/`bootstrap-winrm-remote.ps1`/`preflight-path-b.ps1`。

## 4. 打法
**增量「先迁后删」,全程 build 绿 + UI 不破**:每个消费方先迁 SSH(winrm.rs/DPAPI 暂留),全迁完、`grep` 活引用清零、§5 UI 审计过后,最后 phase 才删。每步 `cargo test`+build+真机抽验+codex review。提交 `git -c commit.gpgsign=false`,只新增 commit。

## 5. Vue↔Tauri 命令兼容契约(回应两轮 review)
1. **本迁移完全不修改 Vue**(`src/**`);UI 不破靠 Tauri 命令 API 向后兼容(请求**和响应**双向)。
2. **杀 DPAPI ≠ 改签名**:删 `commands/*` 内部 `resolve_password`,保留 optional cred 参数 shim;**非 Option(必填 `String`)的 cred 参数**(env_vars/batch/ini_editor/ini_scanner/health_check/bootstrap/zen 等)保留为接受-忽略 shim(逐命令核实是否需用其 username)。
3. **不删/不改名命令、不改响应字段名**:`RefreshResult.winrm_ok`、`WinrmBootstrapResult.{ok,message,manual_script}` 等 Vue 读的字段**冻结**(§11 最终 grep 清理**不得**碰这些 Vue-facing 名)。
4. **WinRM-原生命令 repoint 而非删**:`bootstrap_winrm` → 返回 graceful 结果(远程推送已退役,提示用 `UECM-Bootstrap.cmd`),`get_winrm_bootstrap_script` → 返回 SSH 纳管脚本/说明;二者保留 `WinrmBootstrapResult` 兼容形状(或在 `core/ssh` 侧新增等价类型供其返回)。
5. **凭据子系统终态**(消除 v2 hand-waving):`listCredentials` 继续读 **SQLite `credentials` 表**(元数据,DPAPI 无关→不受删 DPAPI 影响);`save_credential` 把密码从 DPAPI repoint 到 `SecretStore::put`(SQLite 存元数据);**保留 `CredentialKind::Winrm` 变体 + `from_sql` 容错**(不破 6+ 向导的 `kind==='winrm'` 过滤与旧行解码);需真密码处(ddc_pak SMB / shares svc)从 `SecretStore::get` 取。**SecretStore 无需新增 list API**(列表源是 SQLite)。
6. **验证门禁**:进 P5 前,枚举每个受影响 Tauri 命令 + Vue invoke 点,确认命令在、签名/响应兼容,并对这些命令做 UI/命令级 smoke。

## 6. 阶段分解
每 phase 收尾:`cargo test --lib`(基线 975)+ `cargo check --all-targets` + 改脚本的 lanPC 真机抽验 + codex review(`--base <上一 commit>`)。

### P0 —(新)`enable-ssh.ps1` 完整 onboarder + 重 onboard lanPC(前置)
- 把 `enable-winrm.ps1` 里**非 WinRM** 的节点 prep 折进 `enable-ssh.ps1`:建 `uecm-svc` 本地管理员账号(`UecmRender@2026`)、SMB server enable、LongPaths、HighPerformance、ExecutionPolicy RemoteSigned。
- 理由:`SshExecutor` 硬编码以 `uecm-svc` 登录(`ssh.rs:286`);今天该账号由 `enable-winrm.ps1` 创建。删 WinRM bootstrap 前 `enable-ssh.ps1` 必须能独立把节点准备到「SSH 连得上 + 渲染可用」。
- **重 onboard lanPC**(推当前 Mac uecm.pub + 建 uecm-svc):解 memory `mac_keystore_key_mismatch`,**解锁后续所有 phase 的真机 E2E 验证**(否则验证门禁不可满足)。
- 验证:lanPC 用纯 SSH 路径(只跑新 `enable-ssh.ps1`)重纳管后,`ssh uecm-svc@lanpc` 通 + 一条 SSH 域命令端到端通。

### P1 — 探测迁移
- `cli/domain_machine.rs`(4 处)+ `commands/discovery.rs`:`winrm::probe` → `SshExecutor::from_config()?.probe(host, None)`。
- **补 probe 语义**(review 坐实,非仅字段——两结构字段相同):
  - 给 `ssh::probe` 加 **loopback 旁路**(`is_loopback_target` → 直接 ok,对齐 `winrm.rs:18-23`),否则 lanPC 自探会真 SSH-to-self(memory `lanpc_ntlm_loopback`)。
  - `ssh::probe` 是 `uecm-svc` 认证登录(非 `Test-WSMan` 无认证连通):只开了 WinRM 没开 SSH 的节点会判 offline——P0 之后这是**预期**(WinRM 退役),但 `deep_scan` 的「run `machine authorize` 开 WinRM」提示要改成 SSH 纳管提示。
  - `Ok(ok=false)` vs `Err`:winrm 返 `Ok{ok:false}`,ssh 返 `Err`;`Ok(_)=>offline` 分支变 dead,offline 经 `Err` 仍达成,核实落库一致。
- 命令签名/响应不变(`winrm_ok` 字段冻结,契约 §5.3)。不删 winrm.rs。
- **新建 `ssh` CLI 域(args.rs `Domain::Ssh` + `SshAction`)** 起步:`ssh probe <host>`(走 `ssh::probe`,作 `winrm probe` 的替代)。`package-bootstrap`/`authorize` 在 P5a 补全(那时一并退役 `winrm` 域)。这样 CLI 纳管能力在 P5 删 `winrm` 域前已就位,不留真空。

### P2 — zen 域迁移
- **先完整 trace**:确认 12 传输脚本(literal name → `run_remote`)+ 3 孤儿(`probe-cache-stats/probe-health/read-lockfile` = 0 Rust 引用,功能在 `core/zen/{probe,cache_stats}.rs` 走 reqwest HTTP)。**孤儿不属于传输迁移**:删除或标 manual-debug,不改成 node-pure。
- 12 个传输 `zen-*.ps1` **逐个 param()→`[Console]::In.ReadToEnd()|ConvertFrom-Json` 重写**(套 A2/A3 配方 + 5 条 PS5.1 真机坑;`[Parameter(Mandatory)]` 在无 TTY 下会挂,必须改);每个真机抽验。
- plumbing:`core/zen/verify.rs`、`cli/domain_zen.rs`(`run_remote`/body 构造)、`commands/zen.rs` 换 `ssh::run_json(NodeScript)`;命令签名兼容。
- **长任务**:`verify-rules`(编辑器最长 300s)、`service-install`(SCM)单 SSH 会话——加 `ServerAliveInterval` keepalive 到 `build_ssh_args`,真机长跑验证。
- 改 zen 前读 `docs/zen-integration.md` + `docs/research/plan7-deferral-acceptance-2026-05-20.md`;grep「Codex round」。
- 不删 winrm.rs。

### P3 — 杀 DPAPI(commands 去解析 + 真用密码处迁 SecretStore,签名不变)
- vestigial 处(env/ini/ini_scanner/batch/log_verify/projects/health_check):删 `resolve_password` + 透传,保留参数 shim。
- **真迁(非 vestigial)**:
  - `commands/ddc_pak.rs` distribute:source-SMB 密码改走 `pak_distribute::resolve_source_smb`(SecretStore),对齐 `commands/pso.rs` 已有做法。
  - `commands/shares.rs:200` inject:读的是 **svc 别名**密码(share secret)→ 改 `SecretStore::get`,**勿当 operator cred 删**。
- `commands/zen`/`commands/discovery` 随 P1/P2 更新;命令签名兼容。
- 不删 DPAPI 实现。

### P4 — CLI 去 operator-cred + deploy 凭据门 + AuthMethod + 凭据子系统
- `cli/credential_args.rs`:删 `--cred-alias` 的 DPAPI resolve 路径;**删 `AuthMethod` enum + `auth_method` 参数**(zen/env/ini/share/machine 全线,仅服务 winrm)。
- core fn:删 `let _=(user,pass)` + `_with_credential` 变体;更新 CLI 调用方;Tauri 层留 shim。
- **deploy 凭据门**(review 坐实):`core/deploy_workflow.rs:324,400` 的 `ok_or_else("creds required")`——SSH leaf 已忽略密码,故去掉这两个门(或确认 SSH 路径不需要),否则 no-op shim 传 None 会运行时崩。
- **凭据子系统**(契约 §5.5):`save_credential`(Tauri)repoint DPAPI→`SecretStore::put`;`from_sql` 容错未知 kind + 保留 `CredentialKind::Winrm`;`listCredentials` 读 SQLite 元数据。
- **新建 `secret` CLI 域(args.rs `Domain::Secret` + `SecretAction`)**:`secret set <alias>`(`--value`/stdin)、`secret get <alias>`、`secret list`、`secret delete <alias>`,直管 `core::secrets::SecretStore`(put/get/delete + 列表源用 SQLite 别名元数据或新增 `SecretStore::list`)。`cred` CLI 域退役(其 DPAPI `save` 无意义);保留只读兼容或删,二选一实现时定。terminal/agent 由此能管密钥。
- 不删 DPAPI 实现。

### P5 — 拆除(删,门禁:前置完成 + grep 清零 + §5.6 UI smoke)
- **5a 删 WinRM 核心(门禁 P1+P2)**:`core/winrm.rs`、`cli/domain_winrm.rs`、`invoke-remote.ps1`、`test-winrm.ps1`、`core/bootstrap.rs`、`core/preflight.rs`、`enable-winrm.ps1`、`bootstrap-winrm-remote.ps1`、`preflight-path-b.ps1`;清 `lib.rs`/`mod.rs`/`run.rs`/`args.rs` winrm 注册。
- **5a 退役远程推送纳管(不删 Vue 命令)**:
  - `commands/bootstrap.rs` 两命令 **repoint**(契约 §5.4):`bootstrap_winrm`→graceful「远程推送退役,用 `UECM-Bootstrap.cmd`」结果;`get_winrm_bootstrap_script`→返回 SSH 纳管脚本/说明;保留 `WinrmBootstrapResult` 兼容形状。
  - `cli/domain_machine.rs::authorize`/`deep_scan` + `MachineAction::Authorize`:退役或 repoint 成 SSH 纳管提示(不再调已删的 bootstrap/preflight)。
- **5a 补全 `ssh` CLI 域(替代退役的 `winrm` 域 + `machine authorize`)**:`ssh package-bootstrap [--out <dir>]`(打包含当前 keystore 公钥的 SSH USB 纳管包 = `UECM-Bootstrap.cmd`+`enable-ssh.ps1`+`uecm.pub`+`PsExec64.exe`,取代裸 PS `package-winrm-bootstrap.ps1` + `winrm bootstrap-script`)、`ssh authorize <host>`(向已开 SSH 的节点重推当前公钥)。`ssh probe` 已在 P1 落地。**先补 `ssh` 域命令、再删 `winrm` 域**(同 phase 内,build 绿,CLI 覆盖不断档)。
- `UECM-Bootstrap.cmd` 转纯 SSH;`package-winrm-bootstrap.ps1` 转 SSH-only;顺手更新 health remediation 文案(`health_check.rs:89-106`)+ health-probes remediation,去掉 `winrm bootstrap` 指向。
- **5b 删 DPAPI(门禁 P3+P4)**:`core/credentials.rs` store/resolve/list_aliases、`cred-set/cred-delete/cred-list/dpapi.ps1`;确认 `save_credential` 已 repoint、`list_credentials`/`from_sql` 仍工作(legacy winrm 行不报错)。
- 每删一项先 `grep` 零活引用。收尾:全量 build + lanPC E2E(health + zen 全链 + share/distribute)+ §5.6 受影响 Tauri 命令 smoke。

## 7. 顺序与依赖
**P0 → P1 → P2 → P3 → P4 → P5a → P5b**。P0 解锁所有真机验证。删除门禁:5a 需 P1+P2;5b 需 P3+P4;两者均需 §5.6 UI 审计。

## 8. 实现时要钉死的开放项
1. `ssh::probe` loopback 旁路实现 + `deep_scan` 提示改写(P1)。
2. zen 12 脚本逐个 param→stdin 映射 + 长任务 keepalive(P2);3 孤儿删/留决策(P2)。
3. deploy `WriteBackendGraph`/`SetPsoCvars` 去门 vs 保留 creds 流(P4)——确认 SSH 路径真不需要 username。
4. `bootstrap_winrm`/`get_winrm_bootstrap_script` repoint 后的返回类型归属(留 `WinrmBootstrapResult` 形状 or core/ssh 新类型)(P5a)。
5. 必填 `String` cred 参数的 shim 处理:接受-忽略 vs 仍需 username(逐命令,P3)。
6. `network.winrm_open`/`output.HostProbe.winrm_open`/`probe_keys.tcp_5985`:本迁移**不动**(machine scan 仍用 5985 探测;Vue 渲染依赖),仅在 §9 标为后续/B 项,避免 §11 grep 误删。

## 9. 范围内 / 范围外
**范围内**:`core/*`、`cli/*`、`commands/*.rs`(Rust transport + Tauri 实现,**API 向后兼容**)、`ps-scripts/*`、纳管脚本、`core/secrets.rs`。
**范围外(子项目 B)**:`src/**`(Vue);含清理失效 cred-alias 控件/选择器、`kind==='winrm'` 过滤、`winrm_ok`/`winrm_open` 字段重命名、最终删 Tauri shim 参数。本迁移留好向后兼容供 B 排期。
**范围外(后续/非本次)**:`machine scan` 的 5985 端口探测(`network.rs`/`output.rs` winrm_open——诊断用,SSH 世界仍可作连通参考)、health probe taxonomy(`probe_keys.rs` tcp_5985/L2-L3,health 执行已 SSH,仅文案在 P5 顺手更)、`domain_system` 本地 echo、节点主动上报/nDisplay。

## 10. 风险
- **P0 是新的硬前置**:不把账号创建 + 节点 prep 折进 `enable-ssh.ps1` 并重 onboard lanPC,后续全部 phase 的真机验证都做不了,且删 WinRM 后纳管出来的节点不可用。
- **P2 体量被低估过**:12 脚本逐个 param→stdin 重写 + 真机验证 + 长任务 keepalive,是长 pole;3 孤儿别白迁。
- **vestigial 误判风险**:ddc_pak/deploy/shares-inject 已坐实真用密码;P3/P4 逐处再核「这个密码 SSH 侧到底用不用」,prefer 真测一遍。
- **UI 静默崩**:靠 §5 兼容契约(请求+响应+repoint+listCredentials 走 SQLite)消解;残余靠 §5.6 审计 + smoke 兜底。
- **lanPC E2E**:P0 重 onboard 前不通;期间真机验证走 `ssh lanpc` 手动喂 stdin(A4 用过的姿势)。
- **PS5.1 真机坑**:5 条配方坑 + A4 暴露的 PsExec stdout/file-redirect 类,每个改脚本 phase 必真机抽验。

## 11. 验证策略
- 每 phase:`cargo test --lib`(975 基线)+ `cargo check --all-targets` + 改脚本真机抽验 + codex review。
- 改 Tauri 命令/响应的 phase:确认请求**和响应**向后兼容(§5.3);Vue-facing 字段名冻结。
- P5 前:§5.6 Vue-caller 审计 + 受影响 Tauri 命令 smoke(Tauri dev 跑关键流 或 直接 invoke 断言不报错)。
- P5 后:全量 build + lanPC E2E。最终 `grep -rn 'winrm\|resolve_password\|store_password\|invoke-remote'` 仅余:测试/注释、§9 明列的后续项(winrm_open/tcp_5985)、Vue-facing 冻结名——**不得**误删这些。
