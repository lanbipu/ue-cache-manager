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
| **F2** | `--port`/`--http` 持久化进 SCM ImagePath | G2 | `ps-scripts/zen-service-install.ps1` + 两条 Rust service-install 路径 | Rust 传 `Port`/`HttpServerClass`；PS 在 zen 装完 + sc 账户 patch 后，注册表直写 ImagePath 注入 `--port`/`--http` |
| **F3** | detect-binary 写不出 intree 记录时 fail-fast | GAP-ZEN-DETECT-02 | `core/zen/binary.rs` + `cli/domain_zen.rs` detect 路径 | 检测到 intree 候选却因 `machine_ue_installs` 空全部 skip、且 install record 也没写时，返回带指引的 Err（先跑 `machine refresh`） |
| **F4** | sponsor 模式 zenserver 占端口的优雅关停 | ZEN-DEPLOY-2 / G3 | 新 `zen sponsor-down` 命令 + 新 PS 脚本 | 从 endpoint 推 port/machine，用该机最高版本 intree zen.exe 跑 `zen.exe down --port`；destructive 需 `--yes` |

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

### F2 — port/http 持久化进 ImagePath

- **Rust（两条 service-install 路径：`cli/domain_zen.rs` 的 install + `commands/zen.rs` 的 install）**：构建 PS sidecar JSON args 时加 `"Port": ep.declared_port, "HttpServerClass": ep.httpserverclass`。
- **PS `zen-service-install.ps1`**：
  1. 参数段读取 `$Port` / `$HttpServerClass`（缺省空串）。
  2. zen `service install` 完成 + `sc.exe config obj=`（账户 patch）之后，用 `Set-ItemProperty HKLM:\SYSTEM\CurrentControlSet\Services\<ServiceName> ImagePath` 直写：`"<exePart>" --data-dir <normalizedDataDir> --port <Port> --http <HttpServerClass>`。
  3. 保留空格路径的引号（`<exePart>` 带引号），与现有 `--data-dir` 引号处理一致。
- **原因**：zen 自身 `service install` 不持久化 `--port`/`--http`（只写 `--data-dir` 进 SCM PathName），导致服务启动时若 8558 处于 TIME_WAIT 就 relocate 到 base+100（8658）。注册表直写注入端口后，服务稳定起在 8558。

### F3 — detect-binary fail-fast

- **位置**：`core/zen/binary.rs::persist`。当前在 `machine_ue_installs::find` 返回 `None` 时 push 一句 warning 后继续，最终 `Ok(report)`，`intree_records_written` 留 0。
- **改法**：跟踪「检测到 intree 候选（`detection` 含 intree）但因 `machine_ue_installs` 行缺失全部 skip」这一条件。当**该条件成立且 install record 也没写**（即这次 detect 实际产出为空）时，返回带可操作信息的 `UecmError`（提示先 `machine refresh <id>`）。
- **不触发场景**：真正无 UE 安装的空机器（detection 无 intree 候选）仍正常 `Ok`，不报错。
- **`--all` 语义**：保持按机器粒度 —— 单机 fail-fast 不阻断其余机器，整体退出码反映 partial failure（沿用现有 OperationFailed 模式）。
- ⚠️ **测试债**：`binary.rs:709` 现有那条断言 no-row 情况下 `intree_records_written == 0` 的测试要改成断言新的 Err 行为。

### F4 — `zen sponsor-down` 新命令

- **CLI 形状**：`zen sponsor-down --endpoint-id <id> --yes`（destructive，支持 `--dry-run` 预览）。
- **行为**：从 endpoint 行取 `declared_port` + `machine_id` → 用该机 `machine_ue_installs` 最高版本的 intree zen.exe → 跑 `zen.exe down --port <declared_port>`（关掉 editor sponsor 拉起、占用该端口的 zenserver）。
- **新 PS 脚本 `ps-scripts/zen-sponsor-down.ps1`**：参数 `-ZenExePath`、`-Port`；调 `& $ZenExePath down --port $Port`；输出标准 envelope（`ok` / `message` / zen 输出）。幂等：端口上无 sponsor 时返回 ok + "nothing attached"。
- **为何独立、不塞进 `service start`**：自动 kill 别人正在用的 editor zenserver 风险太大；显式命令可审计、可控。runbook 写明：`service start` 报端口占用 → `sponsor-down` → retry。
- **args.rs**：`ZenAction` 加 `SponsorDown { endpoint_id, yes, dry_run, #[command(flatten)] cred }` 变体。
- **注意**：与现有 `zen service stop`（`zen-down.ps1`，按 `-ServiceName` 停 SCM 服务）是两回事 —— 后者停 UECM 装的服务，前者按端口关 editor sponsor 进程。

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
- **F2**：断言传给 PS sidecar 的 JSON args 含 `Port`（= `ep.declared_port`）+ `HttpServerClass`（= `ep.httpserverclass`）。ImagePath 注册表注入本身归 lanPC 验。
- **F3**：`persist()` 在「有 intree 候选 + `machine_ue_installs` 空」时返回 Err（信息含 `machine refresh`）；无 UE 的空机器仍 `Ok`。改 `binary.rs:709` 既有测试。
- **F4**：`sponsor-down` 参数解析；endpoint→port/machine 推导；解析出最高版本 intree zen.exe；组装 `zen.exe down --port <P>`；`--yes`/`--dry-run` destructive 守卫。
- **F6**：dest-path 自动推导逻辑。
- 全量回归：`cargo test --lib` 全绿（基线 ~997–1014 pass）。

### lanPC E2E runbook（逐条勾的 checklist）

前置：节点已 SSH 纳管（machine 13 = lanPC 已就绪）。逐条记录"期望 vs 实际"：

1. `machine refresh 13` → `machine_ue_installs` 有 5.2~5.8 行。
2. **F3 反例**：在空表机器上跑 `zen detect-binary` → 期望**非 0 退出 + 提示 machine refresh**（不是静默成功）。
3. `zen detect-binary --machine 13` → intree 记录已写。
4. `zen register --machine 13 --role shared_upstream --declared-port 8558 --data-dir F:\Epic\DDC\Zen`。
5. `zen urlacl add --endpoint-id <id> --principal 'NT SERVICE\ZenServer' --yes`。
6. `zen service install --endpoint-id <id> --yes` → `sc qc ZenServer` 验 ImagePath 含 `--port 8558 --http asio`（**F2**）。
7. 若 `zen service start` 报端口被占 → `zen sponsor-down --endpoint-id <id> --yes`（**F4**）→ 再 start。
8. `zen service start` → zenserver 日志 / `zen probe` 验 `effective_port: 8558`（**不是 8658**）。
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
