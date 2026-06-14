# 流程 03 — DDC Pak + PSO 缓存作业

启 UE 进程的长任务。约定 `BIN=/mnt/c/Tools/UECM/uecm-cli.exe`，远程带 `--cred-alias render-svc`。
这些是流式命令，**用默认或 `--output ndjson`**（每行一对象），不要用 `--output json`（单对象会等到结束才出）。

## 前置：项目要先入库

`ddc`/`pso` 命令都按 `--project-id` + `--source-machine`（机器 id）操作，项目得先发现/录入：

```bash
"$BIN" project discover --machine-id <id> --roots 'E:\Unreal Projects,E:\RenderStream Projects' --cred-alias render-svc
"$BIN" project list --output json          # 拿 project_id
"$BIN" project locations <project_id>      # 确认该 source-machine 上有这个项目的位置
```
- ⚠️ `--roots` 指精确路径，别扫整盘——会扫入引擎自带 Templates/Samples（虽已内置过滤引擎子树，精确 roots 更稳）（F-014）。
- 没有自动发现时用 `project create-manual` + `project set-location` 手动登记。

---

## A. DDC Pak（generate → verify → distribute）

```bash
# 生成（启 UE -DDC=CreatePak）。--backend 默认 auto
"$BIN" ddc generate --project-id <P> --source-machine <M> --cred-alias render-svc
# 验证 .ddp 存在
"$BIN" ddc verify --project-id <P> --source-machine <M> --cred-alias render-svc --output json
# 分发到目标机（Robocopy）
"$BIN" ddc distribute --project-id <P> --source-machine <M> --targets <M1,M2> --cred-alias render-svc --dry-run
"$BIN" ddc distribute --project-id <P> --source-machine <M> --targets <M1,M2> --cred-alias render-svc --yes
```

### backend 路由（关键）
`--backend` 取值 `auto`(默认) / `legacy` / `zen`：
- `auto`：Zen 可达时 **generate 和 verify 都返回 `skipped:true`**（reason: zen handles caching natively）——Zen 原生缓存，不需要 .ddp pak。这是**正确行为**，不是失败（F-027/F-028）。
- `legacy`：强制走 .ddp pak 流程（即使 Zen 在跑）。要在 Zen 集群里也产 pak 时用。
- `zen`：no-op。

所以：**集群启用了 ZenServer（流程 02）就不需要 DDC Pak**；DDC Pak 是传统 SMB 缓存路径或离线分发用。先判断集群是不是 Zen 模式（`zen status`）再决定要不要跑 ddc generate。

---

## B. PSO 缓存（collect → list → distribute）

```bash
# verify 实为提示（PSO CVar 检查 R008-R010 在 ini scan 里跑，不是这里）
"$BIN" pso verify --project-id <P> --output json          # 会提示去跑 ini scan

# 收集（启 UE -game；流式 NDJSON）。resolution 默认 1920x1080，max-minutes 默认 10
"$BIN" pso collect --project-id <P> --source-machine <M> --resolution 1920x1080 --max-minutes 10 --cred-alias render-svc
"$BIN" pso list --project-id <P> --output json            # 看收集到的 PSO 文件
# 分发
"$BIN" pso distribute --project-id <P> --source-machine <M> --targets <M1,M2> --cred-alias render-svc --dry-run
"$BIN" pso distribute --project-id <P> --source-machine <M> --targets <M1,M2> --cred-alias render-svc --yes
```

---

## 已知限制（执行前告知用户，避免误判为 bug）

- **重型 VP 项目 shutdown hang**（F-044）：nDisplay / ControlRig / Python startup 类项目，editor 在停止/quit 阶段会 hang，
  导致 `pso collect` 停止阶段超时、PSO 可能未入库（`pso list` 空），`log verify-startup`/`analyze-advisories` timeout。
  **核心功能实际工作**（DDC 填充、PSO 创建都实测成功），是 UE 项目侧 shutdown 问题，非 CLI 缺陷。增大 timeout 无效（editor 卡在 quit 处不是慢）。
- **commandlet 类干净退出**：`ddc generate --backend legacy`（`-run=DerivedDataCache`）有明确"完成→退出"逻辑，
  能完整跑通干净退出，不受上面的 hang 影响（实测 34min 跑完，编译 4 万+ shader，DDC 556MB→2.4GB）。
- **distribute 需要第二台目标机**：单机环境无法验证 `ddc/pso distribute` 的实际拷贝。
- BUG-1（Tokio panic exit 101）**已修复**——`pso collect`/`ddc generate legacy` 正常启动 UE 不再 panic。
