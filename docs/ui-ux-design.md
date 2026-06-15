# UECM UI/UX 设计文档

> **目的**：定义 UECM 桌面端的核心功能边界，以及「操作台 / 向导优先」形态下的 UX 交互逻辑，作为重做 UI 的设计基准。
> **依据**：`_walkthrough/REPORT.md`（走读验证报告）+ `src-tauri/src/cli/args.rs`（CLI 命令树，18 域逐域核对）。
> **状态**：设计稿；集群规模等假设见 §8，落地前需确认。

---

## 1. 产品定位

UECM（UE Cache Manager）是一个 **operator 单机操控 UE 渲染集群缓存** 的远程运维工具。operator 在一台机器上，通过 SSH / WinRM（**pull 模式**，被控端不装常驻 agent），把一群 UE 渲染节点的 DDC（Derived Data Cache）缓存「配好、灌满、监控好」，目标是让渲染时缓存命中、不重复编译 shader。

**UI 定位：操作台 / 向导优先（task / wizard-first）。**

不是常驻监控大盘——这与 UECM「operator 触发的离散配置 / 短任务、不引入常驻 agent」的 pull 哲学一致。operator 平时不挂着它，有任务时才打开；UI 的主线是一条动作链：

```
选对象 → 预览（dry-run）→ 确认 → 看进度流 → 结果回读验证
```

---

## 2. 核心功能总结

按 operator 的运维价值链（而非 CLI 域字母序）归并为 **5 大块**：

### A. 集群资产纳管（Inventory）
| 能力 | 关键 CLI |
|---|---|
| 机器纳管 | `machine scan`（探活，不写库）→ `add`（入库）→ `refresh`/`deep-scan`（探 UE 安装、GPU、last-seen）→ `detail`/`rename`/`set-ue-user`/`delete` |
| GPU 一致性 | `gpu matrix`（全集群 GPU 矩阵，PSO 分发的前置约束） |
| 项目身份 | `project discover`（远程扫 `.uproject`）/ `list` / `locations`（跨机同一项目对齐） |
| 凭据 | `cred save/list/delete`（底层 `secret`，AES-GCM SecretStore） |
| 新机引导 | `ssh package-bootstrap`（生成引导包，现场双击开 WinRM / 建账号） |

### B. 缓存基础设施配置（Provisioning）—— 三种后端
| 后端 | 关键 CLI |
|---|---|
| 本地 DDC | `local-cache create` |
| SMB 共享 DDC | `share create`（Mode A 开放 / Mode B 专用账号）/ `inject-system-cred` |
| ZenServer 共享 DDC（独立服务器，主推） | `zen register → detect-binary → apply-config → urlacl add → service install/start → probe → enable`（10 步链路） |
| 路由 / 配置底座 | `env set`/`set-region-host`/`clean-env`（环境变量按机 / 区覆盖）、`ini set/remove/backend-graph`、`ini gc-pause`/`gc-resume`（共享 DDC 垃圾回收开关） |

### C. 缓存内容的生产与分发（Cache Pipeline）
| 流水线 | 关键 CLI |
|---|---|
| DDC pak | `ddc generate`（跑 UE 编译 shader 灌缓存，**可达 30+ 分钟长任务**）→ `verify` → `distribute`（Robocopy 到目标机） |
| PSO 缓存 | `pso verify`（CVar 检查）→ `collect`（跑 UE `-game` 收集，**长任务 NDJSON 流**）→ `list` → `distribute`（带 GPU 不匹配 preflight） |

### D. 诊断与健康（Observability）
| 能力 | 关键 CLI |
|---|---|
| INI 扫描修复 | `ini scan` → `findings`/`get-finding` → `apply`（一键自动修）/`skip`（每条 finding 带 recommendation） |
| 集群健康 | `health run`（L1 端口 / L2 / L3 三层，每条 critical 带 `remediation`）→ `results` / `consistency-check` / `file-stats`（本地 vs 共享 DDC 失衡）/ `analyze-advisories`（S001–S005 症状建议） |
| Zen 监控 | `zen status` / `probe` / `cache-stats` |
| 日志验证 | `log verify-startup`（nullrhi 跑 UE，解析它实际用了哪层缓存） |

### E. 编排（Orchestration）
| 能力 | 关键 CLI |
|---|---|
| 一键部署 | `deploy ddc --plan <json>`（把整条 DDC 部署流程编排成 plan，支持 `--dry-run`） |

---

## 3. 设计前提：7 条由功能本质决定的硬约束

这 7 条不是审美问题，是功能本质——它们直接决定按钮长什么样、交互怎么走。后面所有交互设计都是对它们的回应。

| # | 约束 | 来源 | 对 UX 的要求 |
|---|---|---|---|
| 1 | 操作是**远程 + 异步 + 可能长时间运行** | `ddc generate` 实测 34 分钟；`pso collect` NDJSON 流式 | 必须有**任务队列 + 实时进度 / 日志流**，长任务不阻塞界面 |
| 2 | `--dry-run` 是一等公民 | 几乎每个写操作都支持 | **「先预览 steps 再执行」是默认交互**，不是高级选项 |
| 3 | destructive 操作强制确认 | 缺 `--yes` 一律 exit 2 拒绝 | 统一的**二次确认模式**，且展示「将影响哪些机器」 |
| 4 | 操作有**严格前置依赖顺序** | bootstrap→add→refresh→set-ue-user；Zen 服务端 10 步 | 这类天然适合**向导 + 步骤校验**，而非平铺散按钮 |
| 5 | 强结构化输出契约 | text / json / ndjson 三态 | UI 直接消费结构化结果做表格 / 徽章，不解析人类文本 |
| 6 | **权限分层 + 通道差异** | WinRM 被 UAC 过滤的操作要走提权 SSH | UI 要显示**用了哪条通道、为什么失败、怎么补** |
| 7 | 「问题 → 修复」闭环已内建 | health 的 `remediation`、ini findings 的 recommendation | 最大杠杆是把它们做成**一键修复按钮** |

---

## 4. 信息架构

**双层左导航 + 常驻任务抽屉。**

```
┌──────────────┬────────────────────────────────────────┬──────────────┐
│ ▸ 任务中心    │                                          │  任务抽屉      │
│   Playbooks  │          主工作区                          │  Task Drawer │
│  ─ 纳管新机   │   (列表 / 详情 / 向导步骤)                 │              │
│  ─ 搭共享缓存 │                                          │ ⟳ ddc gen #3 │
│  ─ 接入工作站 │                                          │   ▓▓▓▓░ 62%  │
│  ─ 生产&分发  │                                          │   [看日志流]  │
│  ─ 巡检&修复  │                                          │              │
│  ─ 一键部署   │                                          │ ✓ ini apply  │
│              │                                          │ ✓ zen probe  │
│ ▸ 资产域      │                                          │ ✗ share crt  │
│   Resources  │                                          │   [看错误]    │
│  ─ Machines  │                                          │              │
│  ─ Creds     │                                          │ (NDJSON 实时) │
│  ─ Shares    │                                          │              │
│  ─ Zen       │                                          │              │
│  ─ INI/Config│                                          │              │
│  ─ Health    │                                          │              │
│  ─ Artifacts │                                          │              │
└──────────────┴────────────────────────────────────────┴──────────────┘
```

### ① 任务中心（Playbooks）—— 向导优先的主入口
把 REPORT「操作顺序」里那些**有严格前置依赖**的编排（约束 #4）做成命名向导：

| Playbook | 编排的命令链 |
|---|---|
| **纳管新机** | `ssh package-bootstrap` →（现场双击）→ `machine add` → `refresh` → `set-ue-user` |
| **搭共享缓存** | 分支：SMB `share create` / ZenServer 独立服务器（`register`→`detect-binary`→`apply-config`→`urlacl add`→`service install`→`start`→`probe`→`enable`） |
| **接入工作站** | `machine set-ue-user` → `zen enable`（客户端 2 步） |
| **生产 & 分发** | DDC pak（`generate`→`verify`→`distribute`）/ PSO（`verify`→`collect`→`distribute`） |
| **巡检 & 修复** | `zen probe` → `zen cache-stats` → `health run` → 一键修；或 `ini scan` → `apply` |
| **一键部署** | `deploy ddc --plan` |

### ② 资产域（Resources）—— 给「我知道要改哪台 / 哪项」的直达入口
和 CLI 18 域压缩成 7 项：**Machines**（含 GPU 矩阵、Projects）、**Credentials**、**Shares**、**Zen endpoints**、**INI / Config**、**Health**、**Artifacts**（DDC pak / PSO 列表）。

### ③ 任务抽屉（Task Drawer）—— 常驻右栏
所有异步长任务的进度条 + 可展开实时日志（NDJSON）都汇到这里。可同时跑多个、可回看历史、失败的留痕。这是约束 #1 的核心答案。

---

## 5. 全局交互模式（4 个，贯穿所有页面）

这是整个 UI 的灵魂。每个页面的具体交互都是这 4 个模式的实例化。

### 5.1 预览–确认–执行 三段式（约束 #2 / #3）
任何写操作的统一流程：

```
[主按钮: 预览]
   → 跑 <command> --dry-run，弹出「预览面板」
   → 面板列出：将执行的 steps、影响的机器清单
   → destructive 额外标红：「⚠ 将影响 N 台机器」+ 逐台列出
[确认面板内按钮: 确认执行]
   → 真跑（去掉 --dry-run / 带 --yes），任务进抽屉
```

- 主操作按钮文案一律是 **「预览」** 而不是「执行」——让 dry-run 成为肌肉记忆。
- 「确认执行」只在预览面板内出现，避免误触。
- 安全操作（纯读 / `scan` / `list` / `status`）跳过预览，直接出结果。

### 5.2 任务抽屉 + 实时流（约束 #1 / #5）
- 点「确认执行」→ 任务卡片进抽屉，界面立即可用。
- 卡片状态：`排队 / 运行(进度条) / 成功 / 失败`，对应状态色徽章。
- 长任务（`ddc generate`、`pso collect`）卡片可展开 → 内嵌 **NDJSON 实时日志流**（等宽字体、自动滚动、可暂停、可搜索）。
- 失败卡片保留，点「看错误」展开 stderr + 退出码 + 命中的权限通道（见 5.x）。

### 5.3 统一机器选择器（贯穿）
几乎每个操作都要回答「在哪些机器上跑」。做成一个统一多选组件：
- 多选 + 全选；按角色筛选（如 `shared_upstream` / 工作站 / 其它）；按标签筛选。
- 选完的机器集合直接喂给三段式的「影响范围」展示。
- 单机操作（如 `machine detail`）退化为单选。

### 5.4 问题 → 一键修复（约束 #7）
- `health` 每条 critical、`ini` 每条 finding，**行内直接挂「应用修复」按钮**（底层 `apply` / `remediation`）。
- 点「应用修复」同样走 5.1 三段式（先预览 recommendation 会改什么）。
- 旁边配「跳过」（`ini skip`）和「查看详情」。

---

## 6. 核心页面 / 向导的交互逻辑

### 6.1 落地页（操作台模式下的首页）
不是大盘，是**「上次集群快照 + 快捷入口」**：
- 顶部：集群一句话状态条（N 台在线 / M 台离线 / 上次 health run 时间 + 红黄绿）。**这是缓存的上次结果，不是实时轮询**——旁边一个「刷新巡检」按钮（跑 `zen probe`→`cache-stats`→`health run`，进抽屉）。
- 中部：6 个 Playbook 大卡片入口。
- 底部：最近任务（抽屉历史摘要）。

### 6.2 Machines（资产域代表页）
- **列表**：每行 = 机器名 / IP / 角色 / GPU 摘要 / UE 版本 / last-seen / 健康徽章。顶部 `[扫描子网]`（`machine scan`，结果是「发现 X 台未纳管」的待入库列表）`[手动添加]`。
- **行操作**：`刷新`(refresh) / `详情` / `重命名` / `删除`(destructive，走三段式)。
- **批量**：勾选多台 → 顶部出现 `批量刷新` / `批量设置 UE 用户` 等，全部走机器选择器 + 三段式。
- **详情抽屉**：UE 安装列表、GPU（已过滤虚拟适配器）、最近健康、该机相关的 share / zen endpoint / project 反向链接。

### 6.3 Playbook 向导（以「搭共享缓存 → ZenServer」为例）
向导是约束 #4 的核心答案。形态：**左侧步骤条 + 右侧当前步表单 + 底部「上一步 / 预览本步 / 下一步」**。

```
步骤条（带状态）            当前步
✓ 1 选服务器机器       ┌─────────────────────────────┐
✓ 2 选凭据             │ Step 4: 配置 ZenServer        │
● 3 注册 endpoint      │  endpoint: render-zen-01      │
○ 4 apply-config       │  data-dir:  D:\ZenData        │
○ 5 urlacl             │  [预览本步配置]               │
○ 6 安装服务           │   → 显示将写入的 zen.lua +     │
○ 7 启动 + probe       │     SHA256 校验               │
○ 8 enable 工作站      └─────────────────────────────┘
```

关键交互规则：
- **每步独立预览 + 执行**：不是填完一长表最后一把梭，而是每步可单独 dry-run、确认、看结果——因为每步都是一次远程写，且后一步依赖前一步成功。
- **步骤门控**：前一步未成功，后一步禁用（灰显 + tooltip 说明缺什么）。
- **失败可重试**：某步失败，步骤条标红，可单独重跑该步，不必从头。
- **结果回读**：写配置的步骤（如 `enable` 写 `[StorageServers] Shared`）执行后自动回读（`ini read`）并展示「expected vs actual」对比——这是 REPORT 强调的「写入后读回确证」。

「纳管新机」「接入工作站」等其它 Playbook 同构，只是步骤不同；其中「现场双击 bootstrap」步显式标注为**唯一需要人到现场的手动步**，UI 给出文件路径 + 等待「我已在目标机运行完」的确认。

### 6.4 Health → 修复闭环
- **运行**：顶部 `[运行巡检]`（提示「建议先 `zen probe`，否则 `zen_reachable` 可能因 probe 过期误报 critical」——把 REPORT 的 F-043 坑写进 UI 提示）。
- **结果**：按 L1/L2/L3 分组的检查项列表，每行 = 检查名 / 状态徽章 / message。
- **修复**：critical / warning 行**行内挂「应用修复」**（底层 remediation），点了走三段式预览。Zen 模式下被降级为 `na` 的项（DESIGN-1）显示为灰色「不适用」并附原因，不报红。
- **下钻**：`file-stats`（本地 vs 共享 DDC 失衡）、`consistency-check`（跨机不一致）、`analyze-advisories`（S001–S005）作为子标签。

### 6.5 INI 扫描修复
- **扫描**：选机器 → `[扫描 INI]`（`ini scan`）→ 进抽屉 → 完成后落到 findings 列表。
- **findings 列表**：每行 = 规则号(如 R015) / 严重度 / 机器 / 文件 / 现状摘要 / `[应用修复]` `[跳过]` `[详情]`。
- **详情**：展示 finding 的 recommendation 会把哪个 INI key / tuple 字段改成什么；`apply` 前后自动 re-scan 显示 warning 计数变化（REPORT 里 BUG-3 复验就是这个体验）。
- **附**：`ini apply` 会自动建 `.bak.<timestamp>` 备份——UI 在确认面板里明示「将创建备份」。

### 6.6 DDC / PSO 流水线
- **DDC pak**：三步水平流 `生成 → 校验 → 分发`。
  - `生成`(generate)：选项目 + backend(`auto`/`legacy`/`zen`) → 预览 → 执行（**长任务**，进抽屉，实时 shader 编译进度日志）。`auto` 模式 Zen 可达时会 `skipped`，UI 明示「Zen 接管，已智能跳过」而非报错。
  - `分发`(distribute)：选目标机（机器选择器）→ 预览 Robocopy 计划 → 执行。
- **PSO**：`校验 CVar → 收集 → 分发`。
  - `收集`(collect)：**长任务 NDJSON**，进抽屉看实时 PSO 创建日志。
  - `分发`(distribute)：**带 GPU 不匹配 preflight**——选目标机后，UI 先比对源 / 目标 GPU（`gpu matrix`），不匹配的标红警告并要求二次确认。

### 6.7 任务抽屉（细节）
- 卡片标题 = `<域> <动作> #<序号>`（如 `ddc generate #3`）。
- 进度：确定进度用进度条，不确定用脉冲；时间戳 + 已耗时。
- 展开：实时日志（NDJSON 解析成行）/ 最终 summary（结构化 JSON 渲染成 KV）/ 错误（stderr + exit code + 通道）。
- 历史：抽屉顶部切「进行中 / 历史」，历史可复制命令、重跑、导出日志。

---

## 7. 通用交互规范

### 7.1 destructive 确认（约束 #3）
代表性 destructive 操作：`machine delete`、`zen unregister`、`cred delete`、`zen service stop`、`ini gc-pause/gc-resume`、`zen enable/disable/clean-env/set-region-host` 等。统一规则：
- 主按钮红色 / 危险色（`destructive` token）。
- 确认面板必须列出**逐台影响清单**，destructive 且影响多机时要求输入确认（如勾选「我确认对 N 台执行」）。
- 对应 CLI 的 `--yes`；UI 永不静默执行 destructive。

### 7.2 dry-run 预览（约束 #2）
- 凡 CLI 支持 `--dry-run` 的写操作，UI 主路径都先预览。
- 预览面板用结构化方式渲染 steps（而非贴原始文本）。

### 7.3 权限通道展示（约束 #6）
- 操作卡片 / 结果里显示走的是 **WinRM** 还是 **提权 SSH**。
- 当某操作因 UAC 被 WinRM 过滤而失败（如写 Machine scope 环境变量、停服务），错误态直接提示「该操作需提权 SSH 通道」+ 一键切换重试入口，并链到「权限限制」说明。

### 7.4 状态语义 / 空态 / 错误态
- 健康 / 状态一律用语义状态色（healthy / warning / critical / info / offline / unknown / na），**不自造颜色**。
- 空态：未纳管机器时，Machines 页引导去「纳管新机」Playbook。
- 错误态：永远给「下一步怎么办」（重试 / 切通道 / 看日志 / 查文档）。

---

## 8. 假设与待定

| 项 | 当前假设 | 若不成立的影响 |
|---|---|---|
| **集群规模** | home lab ~ 小型渲染农场（**几台到几十台**） | 若上百台节点，机器选择器要改成「先按组 / 标签筛选再操作」的重型设计，列表要虚拟滚动 + 服务端分页，落地页要从「快照」升级为可分组的总览 |
| **平台限制** | 核心命令 **Windows-only**（除 `machine scan`/`list`） | GUI 若在非 Windows 运行，除扫描 / 列表外的操作按钮应**灰显 + 提示「核心运维需在 Windows operator 机执行」**，避免点了才报 Windows-only |
| **operator 人数** | 单 operator / 低并发 | 多 operator 协作需加操作审计、并发锁（避免两人同时改同一机器配置） |
| **实时性** | 状态是**上次巡检的缓存快照**，靠手动「刷新巡检」更新 | 若要实时，需引入轮询 / 推送，与 pull 哲学有张力，需另行决策 |

---

## 附录：CLI 18 域 → UI 落点映射

| CLI 域 | UI 落点 |
|---|---|
| `machine` / `gpu` / `project` | 资产域 · Machines（列表 + 详情 + GPU 矩阵 + 项目反链） |
| `cred` / `secret` | 资产域 · Credentials |
| `ssh` | Playbook · 纳管新机（bootstrap / probe） |
| `share` | 资产域 · Shares + Playbook · 搭共享缓存(SMB 分支) |
| `zen` | 资产域 · Zen endpoints + Playbook · 搭共享缓存(Zen 分支) / 接入工作站 |
| `env` / `ini` | 资产域 · INI/Config + 各 Playbook 的配置步 |
| `local-cache` | Playbook · 搭共享缓存（本地分支） |
| `ddc` / `pso` | Playbook · 生产 & 分发 + 资产域 · Artifacts |
| `health` / `log` | 资产域 · Health + Playbook · 巡检 & 修复 |
| `deploy` | Playbook · 一键部署 |
| `system` | 设置 / 关于（version / db-path / ps-dir / exit-codes） |
| `manifest` | 开发者 / 调试入口（命令契约） |
