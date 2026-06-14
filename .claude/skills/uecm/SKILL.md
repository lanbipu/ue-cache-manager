---
name: uecm
description: >-
  驱动 uecm-cli 执行 UE 渲染集群的全部缓存运维流程——机器纳管（scan/add/refresh/bootstrap）、
  两级 DDC 缓存配置（本地 + SMB 共享）、ZenServer 共享 DDC 独立服务器部署、DDC Pak 生成/分发、
  PSO 缓存收集/分发、INI 扫描与自动修复、集群 health 诊断、log 验证、一键 deploy。
  只要任务涉及 UECM、uecm-cli、UE 渲染节点/集群运维、DDC（Derived Data Cache）缓存、
  ZenServer / Zen 共享缓存 / shared DDC、渲染机纳管或批量配置、PSO precaching、
  BaseEngine.ini / DDC backend graph 修复、集群健康检查——即使用户没点名 uecm-cli，
  也务必触发本 skill。它封装了走读实测得来的正确操作顺序、前置依赖和踩坑规避，
  比直接拼命令可靠得多。不适用于纯 UE Editor 内操作（那用 ue-mcp-operate）或与缓存运维无关的任务。
---

# UECM — UE 渲染集群缓存运维

`uecm-cli` 是一个从 operator 机通过 SSH/PowerShell 远程管理多台 Windows 渲染节点的 CLI，
覆盖 UE 的两级 DDC 缓存（本地磁盘 + SMB 共享）和 ZenServer 共享 DDC。本 skill 让你能按
**实测验证过的正确流程**驱动它，而不是凭命令印象拼参数。

**核心价值**：命令清单 `manifest`/`--help` 自己能查到；本 skill 真正封装的是那些"踩过坑才知道"
的东西——正确的操作顺序、前置依赖、安全门、提权通道、Windows-only 与路径格式限制、
ZenServer 必须独立部署的架构约束、Zen 模式下 health 的误报。**动手前先读对应流程 reference，别跳。**

---

## 1. 执行环境（每次开工先确认）

- **二进制**：`/mnt/c/Tools/UECM/uecm-cli.exe`（lanPC C 盘）。当前会话在 lanPC 的 WSL2 里，
  可经 **WSL interop 直接调用 `.exe`**，无需 SSH 到别的机器。约定一个变量：
  ```bash
  BIN=/mnt/c/Tools/UECM/uecm-cli.exe
  ```
- **Windows-only**：除 `machine scan` / `machine list` 外，**所有命令只能在 Windows 上跑**。
  在 lanPC WSL 里经 interop 调 `.exe` 算 Windows 执行，没问题；纯 Linux/mac 会直接报 Windows-only。
- **AI 调用约定**：解析输出时加 `--output json`（单 JSON 对象）；长任务流式输出用默认或 `--output ndjson`
  （每行一个对象）。一律加 `--no-input`（拒绝交互提示，适合自动化）。
- **开工自检**（确认 bridge 与 DB 可用）：
  ```bash
  "$BIN" system version
  "$BIN" system db-path        # 确认操作的是哪个 DB
  "$BIN" system echo ping      # round-trip 验证 PowerShell bridge（msg 是位置参数，不是 --message）
  ```
- **DB 选择**：默认操作生产 DB（不传 `--db-path`）。要在隔离库里演练加全局 `--db-path <Windows路径>`。

---

## 2. 黄金安全规则（写操作前必守）

uecm-cli 把"读"和"写"分得很清楚（`manifest` 里每个 operation 有 `side_effects.writes`）。

1. **读操作直接跑**，不必确认：`*list` / `*detail` / `*status` / `*results` / `*findings` /
   `machine scan` / `zen probe`(只读探测) / `health run`(诊断) / `*--dry-run` 等。

2. **写操作走安全门三段式**——这是本 skill 的执行模式，不可省：
   1. 先带 `--dry-run` 跑一次，把预览（将改什么、写哪台机、哪个文件）**展示给用户**；
   2. 用户**确认**后；
   3. 再带 `--yes` 真正执行。
   绝不在未展示预览、未确认的情况下直接 `--yes` 写生产配置。

3. **destructive 命令缺 `--yes` 必被拒**（exit 2，提示 `pass --yes to confirm or --dry-run`）。
   这是设计如此的安全闸，不是 bug，别反复硬试。真实 id 不带 `--yes` 也不会误删。

4. **凭据用别名传递**：几乎所有远程命令都接受 `--cred-alias <ALIAS>`。先 `cred save --alias <别名>
   --user uecm-svc --pass-stdin` 存一次，之后所有远程命令带 `--cred-alias <别名>`。
   渲染节点统一运维凭据见 CLAUDE.md（`uecm-svc` / `UecmRender@2026`）。

5. **路径一律 Windows 格式**：`E:\Unreal Projects\X`、`\\LANPC\DDC-Shared`、
   `D:\Program Files\Epic Games\UE_5.8\...`。**不接受 WSL/Unix 路径**（`/tmp/...` 会报"找不到文件"）。
   要给 CLI 写临时文件（如 deploy plan），写到 Windows 可见路径（如 `C:\Temp\` 或仓库 `E:\...`）。

6. **部分操作要提权 SSH 通道**：写 Machine-scope 环境变量、停/卸 Windows 服务等，WinRM 受限 token
   （UAC 过滤）做不了。多数已被封装进对应命令（`zen clean-env` / `zen service stop` 等内部走提权），
   但"重装已存在的 zen 服务"等场景仍需手动先停服。细节见 `references/troubleshooting.md`。

---

## 3. 标准操作顺序（前置依赖不能跳）

这些顺序是踩坑总结，跳步会报错或留下错配置。详细版在各 reference，这里给骨架：

**机器纳管**：`ssh package-bootstrap`（生成包，需人工传到新机双击）→ `machine add --ip` →
`machine refresh <id>` → `machine set-ue-user`。  *(scan 只探测不写库；add 才入库)*

**两级 DDC 缓存配置**：`local-cache create`（先建目录）→ `env set UE-LocalDataCachePath`（再写变量）；
`share create`（建 SMB 共享）→ `env set UE-SharedDataCachePath`（写共享 UNC）。
**两个环境变量都要设，顺序是先建目录/共享再写变量。**

**ZenServer 部署（必须独立服务器，不与工作站共机）**：服务端 `zen register` →
`zen detect-binary`（apply-config 的前置）→ `zen apply-config` → `zen urlacl add` →
`zen service install` → `zen service start` → `zen probe`；客户端 `machine set-ue-user`（zen enable 前置）→ `zen enable`。

**health 诊断**：先 `zen probe` + `zen cache-stats` 刷新探针 → 再 `health run`（否则 zen_reachable 会因数据过期误报 critical）。

**INI 修复**：`ini scan` → `ini findings <run_id>` → 区分 finding 类型（`set` 可 `ini apply` 自动修，
`manual` 需人工告知值）→ `ini apply <finding_id>` 或 `ini skip`。

> 这里只列顺序。每条流程的完整命令、参数、判断逻辑见下表的 reference 文件——**执行前务必打开对应文件**。

---

## 4. 八大流程导航（执行前读对应 reference）

| 用户意图 / 场景 | 流程 | 读这个文件 |
|---|---|---|
| 纳管新渲染机、扫描网段、刷新机器信息、配本地+共享 DDC 缓存 | 机器纳管 + 两级缓存 | `references/01-onboarding-and-cache.md` |
| 部署/启停 ZenServer 共享 DDC、给工作站 enable Zen、区域路由、迁服务 | ZenServer 全套 | `references/02-zenserver.md` |
| 生成/分发 DDC Pak、收集/分发 PSO 缓存（启 UE 的长任务） | 缓存产物作业 | `references/03-cache-jobs.md` |
| 扫 INI 配置问题并自动修、跑集群 health 诊断、log 验证 | 诊断与修复 | `references/04-ini-health-log.md` |
| 一条命令按 plan 跑完整 DDC 部署（provision→share→pak→pso→verify） | 一键 deploy | `references/05-deploy.md` |
| 任意域全部命令的准确 flag 速查（91 命令 / 17 域） | 命令参考 | `references/command-reference.md` |
| 报错了、exit code 含义、提权通道、Windows-only/路径坑、Zen 模式误报 | 排错与限制 | `references/troubleshooting.md` |

**自然语言意图 → 流程映射**（帮你快速路由）：
- "帮我把这台新机器加进来 / 纳管 192.168.x.x" → 01
- "给这台机配本地 DDC 缓存 / 设共享缓存服务器" → 01
- "部署 Zen 共享缓存 / 装 ZenServer / 让工作站用上共享 Zen" → 02
- "生成 DDC 包发给其他节点 / 收集 PSO" → 03
- "检查集群配置对不对 / 哪台机配置有问题 / 一键修 ini" → 04
- "按计划把整套 DDC 缓存铺好" → 05

---

## 5. 命令域速查（17 域）

`system`(自检/schema) · `machine`(纳管/扫描/refresh/deep-scan/authorize) · `ssh`(probe/bootstrap包) ·
`cred`+`secret`(凭据/密钥) · `env`(远程环境变量) · `ini`(扫描/修复/backend-graph/gc) ·
`share`(SMB 共享) · `project`(项目发现/定位) · `health`(L1/L2/L3 诊断/file-stats/advisories) ·
`gpu`(一致性矩阵) · `ddc`(Pak 生成/验证/分发) · `pso`(收集/分发) · `log`(verify-startup) ·
`local-cache`(本地缓存目录) · `deploy`(一键 ddc) · `zen`(ZenServer 全套)。

准确 flag 一律查 `references/command-reference.md`，或临场 `"$BIN" <域> <子命令> --help` 核对
（CLAUDE.md 要求：引用命令必须核对真实 flag，binary 可能比源码新/旧，以重新 build 出的 help 为准）。

---

## 6. 工作循环（推荐方式）

对每个运维请求：
1. **定位流程** → 读对应 reference（第 4 节表）。
2. **查状态** → 先跑读操作（list/detail/status/scan）摸清当前集群状态，别盲改。
3. **确认前置** → 对照 reference 的操作顺序，缺前置先补（如 zen enable 前要 set-ue-user）。
4. **预览** → 写操作先 `--dry-run`，把预览给用户看。
5. **执行** → 确认后带 `--yes`。出错先看 exit code（`references/troubleshooting.md`）和 JSON `error.message`。
6. **复验** → 改完跑回对应的读/诊断命令验证（如改完 ini 重 scan、配完 zen 跑 health）。
