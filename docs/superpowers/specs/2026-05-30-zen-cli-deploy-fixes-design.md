# Zen Server CLI 部署链修复 — 设计文档

- **日期**：2026-05-30
- **分支 / worktree**：`zen-cli-deploy-fixes` → `.worktrees/zen-cli-deploy-fixes/`（从 `main` b75b37d 切出）
- **范围**：只做 **CLI** 的 zen-server `installed_service` 部署链。GUI、onboarding 单列后续 spec。
- **来源**：2026-05-29 lanPC 用 CLI 把 Zen 部成 Windows 服务的实战 + 2026-05-30 的 17-agent 审计（见 memory `zen_deploy_flow_gaps`）。

---

## 1. 背景与问题

2026-05-29 那次部署"成功"，靠的是 Claude Code 中途改了 3 处源码 + 跳出 UECM 用 out-of-band 工具（手动 `zen.exe down`、`sc delete`、`icacls`、UAC）才过的。实测确认：**那 3 个 fix 从未回流到 canonical 仓库**（`git log --all -S` 在 main / 所有分支 / 全历史都查不到），只活在 lanPC 的二进制和 `E:\` 工作副本里。

因此 canonical `main` 上的 CLI zen 部署链**目前仍带全部 bug**：一个纯 CLI 操作员（不借助 Claude Code 改代码）在典型节点上**无法跑出正确的部署结果**。本设计把审计出的所有 CLI 侧有用修复整理落地，并用 mac 单测 + lanPC E2E 双层验证，让这条链脱离 Claude Code 也能用。

### 完整 CLI deploy 命令链（修复后的目标流程）

```
machine refresh <id>                                  # 填 machine_ue_installs（intree zen.exe 来源）
zen detect-binary --machine <id>                      # 写 install/intree binary 记录；F3 保证缺前置时 fail-fast
zen register --machine <id> --role shared_upstream \
  --declared-port 8558 --data-dir <abs> [--httpserverclass asio]
[zen apply-config --endpoint-id <id> [--dest-path <abs>] --yes]   # F6 后 --dest-path 可选
zen urlacl add --endpoint-id <id> --principal 'NT SERVICE\ZenServer' --yes
zen service install --endpoint-id <id> [--service-user .. --service-pass-stdin] --yes
# 若 start 报端口被占（editor sponsor 占用）：
zen sponsor-down --endpoint-id <id> --yes             # F4 新命令
zen service start --endpoint-id <id>                  # F2 后服务真起在 8558，不再 relocate 到 8658
zen service status / zen probe / zen cache-stats / health run
```

---

## 2. 修复清单（分层）

### 🔴 A 组 · 关键路径

| # | 修复 | gap | 改哪里 | 怎么改 |
|---|---|---|---|---|
| **F1** | service install/uninstall 加 intree fallback | ZEN-DEPLOY-1 | `cli/domain_zen.rs:1257,1429` + `commands/zen.rs:1046,1160`（**4 处各加，不抽共享函数**） | install-dir binary 查不到时 `.or_else` 取 `machine_ue_installs` 最高版本的 `zen_cli_intree_path` |
| **F2** | `--port`/`--http` 持久化进 SCM ImagePath（含**已存在服务的就地修复**） | G2 | `ps-scripts/zen-service-install.ps1` + 两条 Rust service-install 路径 | Rust 传 `Port`/`HttpServerClass`；PS 全新安装时注册表直写 ImagePath 注入 `--port`/`--http`；**且对已存在服务做 port/http drift 检测 + 就地 repair**（见 §3 F2，修 Codex review #1） |
| **F3** | detect-binary 写不出 intree 记录时 fail-fast | GAP-ZEN-DETECT-02 | `core/zen/binary.rs` + `cli/domain_zen.rs` detect 路径 | 检测到 intree 候选却因 `machine_ue_installs` 空全部 skip、且 install record 也没写时，返回带指引的 Err（先跑 `machine refresh`） |
| **F4** | sponsor 模式 zenserver 占端口的优雅关停（含**进程身份守卫**） | ZEN-DEPLOY-2 / G3 | 新 `zen sponsor-down` 命令 + 新 PS 脚本 | 从 endpoint 推 port/machine；**先解析监听该端口的 PID 并比对是否为 `ZenServer` SCM 服务进程——是则 refuse**，否则用该机最高版本 intree zen.exe 跑 `zen.exe down --port`；destructive 需 `--yes`，dry-run/apply 输出被停进程身份（修 Codex review #2） |

### 🟡 B 组 · 增强（A 组落地后非阻断）

| # | 修复 | gap | 说明 |
|---|---|---|---|
| **F5** | detect-binary 跨用户发现 install-dir binary | G1 | 枚举所有 user profile（uecm-svc 本地 admin 可遍历），不再只读自己的 `LOCALAPPDATA`；消除 `install_record:false` 误导。有 F1 后非关键路径 |
| **F6** | `zen apply-config` 自动推导 zen.lua dest-path | T2.9 | 从探测到的 install dir 推 `--dest-path`，让它变可选 |

### ⬜ 已记录、不在本批（后续 spec）

- **onboarding**：`ssh authorize <host>`（已可达节点重授权，目前 `args.rs:347` 是 TODO）+ `enable-ssh.ps1` 自提权 gate（`Test-UecmAdministrator` 定义了但从不调用）。GAP-ONBOARD-01。first-contact 纳管受 UAC 物理限制无法纯远程修，仍走 U 盘 bootstrap。
- **GUI**：ZenDeployWizard + 接线 23 个 zen Tauri 命令（前端 0 引用）。GUI-ZEN-001/002/003/005。

---

## 3. 各修复实现细节

### F1 — intree fallback（4 处）

`list_for_machine` 已是 `ORDER BY version DESC`；service install 传的是 `zen_cli_path`（zen.exe CLI，PS 内 `zen.exe service install` 自己找同级 zenserver.exe），fallback 用 `zen_cli_intree_path` 与现有契约对齐。4 处 lookup 各自在 `.and_then(|m| m.zen_cli_path.clone())` 之后接：

```rust
.or_else(|| machine_ue_installs::list_for_machine(&db, ep.machine_id)
    .ok()
    .and_then(|v| v.into_iter().find_map(|i| i.zen_cli_intree_path)))
```

仍找不到才报原 `has no zen.exe (zen_cli) recorded — run detect-binary first` 错。**不抽共享函数**（贴合现有重复风格 + surgical），但 4 处 fallback 表达式保持一字不差，便于将来一致修改。

### F2 — port/http 持久化进 ImagePath（含已存在服务的就地修复）

- **原因**：zen 自身 `service install` 不持久化 `--port`/`--http`（只写 `--data-dir` 进 SCM PathName），导致服务启动时若 8558 处于 TIME_WAIT 就 relocate 到 base+100（8658）。注册表直写注入端口后，服务稳定起在 8558。
- **Rust（两条 service-install 路径：`cli/domain_zen.rs` 的 install + `commands/zen.rs` 的 install）**：构建 PS sidecar JSON args 时加 `"Port": ep.declared_port, "HttpServerClass": ep.httpserverclass`。
- **PS `zen-service-install.ps1`**：
  1. 参数段读取 `$Port` / `$HttpServerClass`（缺省空串）。
  2. 抽局部 helper `Patch-ImagePath`：把 ImagePath 直写为 `"<exePart>" --data-dir <normalizedDataDir> --port <Port> --http <HttpServerClass>`（保留空格路径引号，与现有 `--data-dir` 处理一致），写完**回读 verify** 含 `--port`。
  3. **全新安装路径**：zen `service install` + `sc.exe config obj=`（账户 patch）之后调 `Patch-ImagePath`。

- **⚠️ 已存在服务的 drift 修复（修 Codex adversarial review #1）**：
  当前脚本 `zen-service-install.ps1:237-378` 对已存在服务只比对 **exe + data-dir + account**（`$matchesExpected`，347 行），三者匹配即 `ok=true, already_installed, exit 0` **提前返回**，底部 patch 永远到不了。这正是真实升级路径——旧 CLI 装的服务有 `--data-dir` 没 `--port`、exe/data/account 全同 → 修好的 CLI 会**报成功却不改注册表**，服务仍 relocate 到 8658。
  改法：把现有 token-parse 扩展到解析 `--port` / `--http`，把已存在服务判定改成三分支：
  - exe/data/account **且** port/http 都匹配 → 真 no-op（`already_installed=true`）。
  - exe/data/account 匹配 **但** port/http 缺失或不同 → 调 `Patch-ImagePath` **就地修复 + verify**，返回 `ok=true, repaired=true`，message 注明「ImagePath 已补 `--port`/`--http`，需 `zen service stop && start` 重启生效」（ImagePath 只影响下次启动）。
  - exe / data / account 不同 → 维持现有 refuse（指向 `zen-service-uninstall.ps1`）。
  就地 repair 不强制 uninstall：exe/data/account 都没变，只补 runtime flag，是良性原地修。

### F3 — detect-binary fail-fast

- **位置**：`core/zen/binary.rs::persist`。当前在 `machine_ue_installs::find` 返回 `None` 时 push 一句 warning 后继续，最终 `Ok(report)`，`intree_records_written` 留 0。
- **改法**：跟踪「检测到 intree 候选（`detection` 含 intree）但因 `machine_ue_installs` 行缺失全部 skip」这一条件。当**该条件成立且 install record 也没写**（即这次 detect 实际产出为空）时，返回带可操作信息的 `UecmError`（提示先 `machine refresh <id>`）。
- **不触发场景**：真正无 UE 安装的空机器（detection 无 intree 候选）仍正常 `Ok`，不报错。
- **`--all` 语义**：保持按机器粒度 —— 单机 fail-fast 不阻断其余机器，整体退出码反映 partial failure（沿用现有 OperationFailed 模式）。
- ⚠️ **测试债**：`binary.rs:709` 现有那条断言 no-row 情况下 `intree_records_written == 0` 的测试要改成断言新的 Err 行为。

### F4 — `zen sponsor-down` 新命令（含进程身份守卫）

- **CLI 形状**：`zen sponsor-down --endpoint-id <id> --yes`（destructive，支持 `--dry-run` 预览）。
- **行为**：从 endpoint 行取 `declared_port` + `machine_id` → 用该机 `machine_ue_installs` 最高版本的 intree zen.exe → （通过身份守卫后）跑 `zen.exe down --port <declared_port>`（关掉 editor sponsor 拉起、占用该端口的 zenserver）。
- **新 PS 脚本 `ps-scripts/zen-sponsor-down.ps1`**：参数 `-ZenExePath`、`-Port`、`-ServiceName`（默认 `ZenServer`）、`-DryRun`。
- **⚠️ 进程身份守卫（修 Codex adversarial review #2）**：`zen.exe down --port` 不区分端口上是 editor sponsor 还是已装的 `ZenServer` 服务/别的 upstream，裸跑会误停生产进程。脚本先做廉价但精确的判定：
  1. `Get-NetTCPConnection -LocalPort $Port -State Listen` 取 `OwningProcess` PID；无监听 → 返回 `ok=true, nothing_attached=true`（幂等）。
  2. 取 `ZenServer` SCM 服务的 `ProcessId`（`Get-CimInstance Win32_Service -Filter "Name='$ServiceName'"`，仅 Running 时有值）。**若监听 PID == 该服务 PID → refuse**（`ok=false`，message：「端口 $Port 由已安装的 `$ServiceName` 服务提供，不是 editor sponsor；请用 `zen service stop`」）。
  3. 否则解析该 PID 的进程 path 用于报告（理应是某 UE install 下的 `zenserver.exe`）；非 zenserver 进程 → refuse 并报出实际进程，避免误杀无关程序。
  4. 通过守卫后才 `& $ZenExePath down --port $Port`。
- **dry-run / apply 输出都带身份**：`listener_pid` / `listener_path` / `is_installed_service`（bool），让操作员在动手前看清要停的是谁。
- **为何独立、不塞进 `service start`**：自动 kill 别人正在用的 editor zenserver 风险太大；显式命令 + 身份守卫可审计、可控。runbook 写明：`service start` 报端口占用 → `sponsor-down`（守卫确认非服务后）→ retry。
- **args.rs**：`ZenAction` 加 `SponsorDown { endpoint_id, yes, dry_run, #[command(flatten)] cred }` 变体。
- **注意**：与现有 `zen service stop`（`zen-down.ps1`，按 `-ServiceName` 停 SCM 服务）是两回事 —— 后者停 UECM 装的服务，前者按端口关 editor sponsor 进程；守卫第 2 步正是为了把这两者区分开。

### F5 — detect-binary 跨用户发现（B 组）

- **位置**：`ps-scripts/zen-detect-binary.ps1:98`，当前 `$installDir = Join-Path $env:LOCALAPPDATA 'UnrealEngine\Common\Zen\Install'` —— SSH 以 uecm-svc 登录时只看自己 profile，对装 zen 的 UE 交互用户盲。
- **改法**：枚举 `C:\Users\*\AppData\Local\UnrealEngine\Common\Zen\Install\` 下的 zenserver/zen.exe（uecm-svc 是本地 admin，可遍历其他 profile）；取到的 install-dir binary 正常写 `machine_zen_install`。消除恒为 false 的 `install_record_written`。
- 有 F1 后这不在关键部署路径上，故归 B 组。

### F6 — apply-config 自动推导 dest-path（B 组）

- **位置**：`zen apply-config` 当前 `--dest-path` 必填（`args.rs:1106`，T2.9 pending）。
- **改法**：当 `--dest-path` 省略时，从该 endpoint 机器探测到的 zen install dir 推导 `...\Zen\Install\zen.lua`；显式传入则以传入为准。

---

## 4. 验证计划（双层）

### Rust 单测（mac：`cd src-tauri && cargo test --lib`）

- **F1**：机器无 `machine_zen_install` 行、`machine_ue_installs` 有 5.2 / 5.8 两行带 intree path → 断言 resolver 选 **5.8** 的 `zen_cli_intree_path`；两者皆无 → 断言仍报 `has no zen.exe`。覆盖 install + uninstall 两类入口。
- **F2**：断言传给 PS sidecar 的 JSON args 含 `Port`（= `ep.declared_port`）+ `HttpServerClass`（= `ep.httpserverclass`）。ImagePath 注册表注入 + 已存在服务 drift 修复本身是 PS 行为，归 lanPC 验（见 runbook 6/6b）；PS 层若可在 mac 用 pwsh 跑纯函数（token-parse + 三分支判定）则补 Pester/单元断言，否则纯 lanPC。
- **F3**：`persist()` 在「有 intree 候选 + `machine_ue_installs` 空」时返回 Err（信息含 `machine refresh`）；无 UE 的空机器仍 `Ok`。改 `binary.rs:709` 既有测试。
- **F4**：`sponsor-down` 参数解析；endpoint→port/machine 推导；解析出最高版本 intree zen.exe；组装 `zen.exe down --port <P>`；`--yes`/`--dry-run` destructive 守卫。**身份守卫判定**（监听 PID == 服务 PID → refuse；非 zenserver → refuse；standalone sponsor → proceed）尽量抽成可在 mac 单测的纯决策函数（喂入 listener PID / service PID / process path），SCM/NetTCP 取数留 lanPC。
- **F6**：dest-path 自动推导逻辑。
- 全量回归：`cargo test --lib` 全绿（基线 ~997–1014 pass）。

### lanPC E2E runbook（逐条勾的 checklist）

前置：节点已 SSH 纳管（machine 13 = lanPC 已就绪）。逐条记录"期望 vs 实际"：

1. `machine refresh 13` → `machine_ue_installs` 有 5.2~5.8 行。
2. **F3 反例**：在空表机器上跑 `zen detect-binary` → 期望**非 0 退出 + 提示 machine refresh**（不是静默成功）。
3. `zen detect-binary --machine 13` → intree 记录已写。
4. `zen register --machine 13 --role shared_upstream --declared-port 8558 --data-dir F:\Epic\DDC\Zen`。
5. `zen urlacl add --endpoint-id <id> --principal 'NT SERVICE\ZenServer' --yes`。
6. `zen service install --endpoint-id <id> --yes` → `sc qc ZenServer` 验 ImagePath 含 `--port 8558 --http asio`（**F2** 全新安装路径）。
6b. **F2 真实升级反例**（关键）：先用一个只有 `--data-dir` 没 `--port` 的旧 ImagePath 造出"坏服务"（或直接拿 lanPC 现存旧服务），再跑 `zen service install --endpoint-id <id> --yes` → 期望返回 `repaired=true` 且 `sc qc` 显示 ImagePath **已补** `--port 8558 --http asio`（而不是 `already_installed` no-op 静默放过）。
7. **F4 守卫反例**：在 `ZenServer` 服务正跑在 8558 时跑 `zen sponsor-down --endpoint-id <id> --dry-run` → 期望 **refuse**（提示这是已安装服务、该用 `zen service stop`），且输出含 `listener_pid` / `is_installed_service=true`。
7b. 若 `zen service start` 报端口被占（sponsor 占用、服务未起）→ `zen sponsor-down --endpoint-id <id> --dry-run` 看清身份（`is_installed_service=false`）→ `--yes` 关停 → 再 start。
8. `zen service start` → zenserver 日志 / `zen probe` 验 `effective_port: 8558`（**不是 8658**）；停服后重启再验端口稳定（F2 重启生效）。
9. `zen cache-stats` / `health run --machine-ids 13` 收尾。

---

## 5. 落地顺序

每个修复 TDD（先写失败测试再实现），独立 commit：

1. **F1 + F2** — 两个 session 验证过的关键修复，在仓库里**干净重写 + 补测试**（不从 lanPC git-bundle 搬，更干净且有回归保护），恢复已知可用部署。
2. **F3** — detect-binary fail-fast（含改 `binary.rs:709` 测试）。
3. **F4** — `zen sponsor-down` 新命令 + 新 PS 脚本。
4. **F5 + F6** — B 组增强。

A 组（F1–F4）跑通 lanPC E2E 后即可合 main；B 组可后续追加。

## 6. 非目标 / 假设

- 不碰 GUI（Vue 前端 / Tauri command 仅 F1 顺带改 `commands/zen.rs` 的 2 处 lookup，不新增/接线任何 GUI 功能）。
- 不碰 onboarding 域（假设目标节点已 SSH 纳管好）。
- 不做 unrelated 重构（不抽共享 helper，不动相邻无关代码）。
- PS sidecar 改动遵守仓库既有约定（envelope 输出格式、`Out-String`、空格路径引号处理）。

## 7. 修订记录

### rev1 — Codex adversarial review（2026-05-30）

两条 high finding，均经读源码核实成立后采纳：

- **#1（采纳，扩大 F2）**：原 F2 只在「全新安装」末尾 patch ImagePath，但 `zen-service-install.ps1:237-378` 对已存在服务只比 exe+data+account、匹配即 no-op exit 0 提前返回——旧 CLI 装的"有 data-dir 没 port"坏服务在真实升级路径上会被静默放过，修了等于没修。→ F2 扩展为对已存在服务做 port/http drift 检测 + 就地 repair（exe/data/account 同、仅 port/http 缺失/不同时原地补 ImagePath 而非强制 uninstall）。补 runbook 步骤 6b。
- **#2（采纳，裁剪版，加固 F4）**：原 F4 裸跑 `zen.exe down --port` 不分辨端口上是 sponsor 还是已装的 `ZenServer` 服务/别的 upstream，可能误停生产进程。→ F4 加进程身份守卫：解析监听 PID，命中 `ZenServer` 服务 PID 则 refuse（该用 `zen service stop`），非 zenserver 进程也 refuse，并在 dry-run/apply 输出被停进程身份。**裁剪**：只做「listener PID vs 服务 PID」+ 进程 path 报告这类廉价高价值判定，不做 Codex 建议的完整 owner/lockfile 取证（YAGNI）。补 runbook 步骤 7（守卫反例）。
