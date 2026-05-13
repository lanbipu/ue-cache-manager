# UE Cache Manager (UECM) — 设计文档

**日期**：2026-05-01
**状态**：设计阶段（待实施计划）
**作者**：lanbipu + Claude (brainstorming)
**项目代号**：UE Cache Manager（UECM）

---

## 1. 摘要

UE Cache Manager（UECM）是一款 Windows 桌面工具，用于统一管理 Unreal Engine 项目在多机集群上的缓存配置——核心覆盖 **DDC（Derived Data Cache）** 和 **PSO Cache** 两大类。核心场景是 VP/XR 渲染集群：单台操作机管理局域网内所有 Render Node、开发机、共享 NAS/Host，零安装、一键配置、健康可视。

工具不是 UE 插件，独立 Tauri 桌面应用，体积约 8 MB，绿色运行。技术栈：Rust 后端 + React/Vue + Tailwind 前端 + PowerShell sidecar。

**最终目标**：让 VP 集群任何一台机器开 UE 工程都能"零编译启动 + 零卡顿运行"。

---

## 2. 问题背景

VP/XR 项目里，UE 工程在多台机器上启动时会反复编译 Shader、生成派生数据，每台新机器冷启动需 30 分钟到 2 小时。即使解决了启动慢问题，运行时切换场景仍可能因 GPU 管线状态首次创建造成 100ms+ 卡顿——这在 LED 屏直播 / 实拍场景下是致命的。

UE 5 的"零卡顿"完整方案是三层组合：

1. **DDC 共享**：消除 Shader 重编译（解决启动慢）
2. **PSO Precaching**（UE 5.1+ 自动）：加载关卡时主动预创建 PSO（解决换场景卡）
3. **PSO Cache 文件**：兜底 Precaching 可能漏掉的边缘组合（解决最后边缘卡顿）

落地这三层时存在以下痛点：

- **DDC 配置分散**：路径可在四个地方设（命令行 / 环境变量 / 项目 ini / Editor Prefs UI），优先级复杂，配错就静默失效
- **DDC 静默失效**：`EditorPerProjectUserSettings.ini` 等用户级覆盖会压过环境变量，问题难发现
- **SMB 认证陷阱**：Windows Service 账户读不到普通用户存的凭据，RenderStream 服务启动 UE 时无法访问共享盘
- **PSO Cache 采集需手工跑 `-game` 模式遍历**：耗时长、易遗漏场景、跨机分发需手动
- **PSO 与 GPU/驱动强绑定**：集群里某台机器换显卡或驱动 → PSO 失效但用户不知道
- **多机部署烦琐**：每台机器手动 setx、改 ini、配凭据、传 PSO 文件，新机器接入流程低效
- **缺乏可视性**：没有面板能看到"集群整体健康度"

UECM 的目标是把上述操作集中、自动化、可视化。

---

## 3. 目标与非目标

### 目标（in scope）

- 单点部署：UECM 只装在一台操作机上，其他机器零安装
- 局域网机器发现 + UE 版本探测 + GPU/驱动信息采集
- 单机 / 集群级 DDC 路径配置（环境变量 + 项目 ini）
- SMB 共享 DDC 一键创建（Mode A 开放共享 / Mode B 统一账户 + SYSTEM 凭据注入）
- 全局 INI 扫描 + 冲突诊断 + 一键修复（含 `EditorPerProjectUserSettings.ini`）
- DDC Pak 操作：本机生成、远程生成、远程分发，三种组合
- **PSO Cache 文件操作：采集触发、分发、GPU/驱动一致性检查**（新）
- **PSO Precaching CVar 验证**（新，轻量）
- 配置生效验证（远程读 UE 启动日志）
- 集群健康检查（DDC + PSO 综合检查矩阵）
- 操作历史 + 自动备份 + 一键回滚

### 非目标（out of scope）

- 不做 UE Editor 插件版本（独立桌面应用即足够）
- **不做 FShaderCache**——UE 4 时代老技术，已被 UE 5 的 PSO Precaching 实质替代，加了基本是冗余配置
- 不做 ZenServer 部署（仅支持 Filesystem 共享 DDC，未来可扩展）
- 不做 Mac / Linux 版本（VP 集群是纯 Windows 场景）
- 不做用户/权限/审计的多用户协作（单操作员模型）

---

## 4. 架构

### 部署拓扑

```
┌─────────────────────────────────────────────────────────┐
│           OPERATOR MACHINE  (UECM 仅装这一台)            │
│                                                          │
│  ┌─────────────────────────────────────────────────┐   │
│  │              UECM.exe (Tauri App, ~8MB)          │   │
│  │  ┌──────────────────────────────────────────┐   │   │
│  │  │   前端 UI (Vue/React + Tailwind)          │   │   │
│  │  └──────────────────────────────────────────┘   │   │
│  │  ┌──────────────────────────────────────────┐   │   │
│  │  │   后端 (Rust) - 主业务逻辑               │   │   │
│  │  └──────────────────────────────────────────┘   │   │
│  │  ┌──────────────────────────────────────────┐   │   │
│  │  │   PowerShell Sidecar 脚本（兜底层）       │   │   │
│  │  └──────────────────────────────────────────┘   │   │
│  └─────────────────────────────────────────────────┘   │
│                                                          │
│  本地存储：Windows Credential Manager + SQLite          │
└─────────────────────────────────────────────────────────┘
                          │
              WinRM 5985  │  SMB 445
                          ▼
┌─────────────────────────────────────────────────────────┐
│              LAN MANAGED MACHINES (零安装)               │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐  ...  │
│  │ ⭐ HOST    │  │ Render Node│  │ Dev/Editor │       │
│  │ + 共享盘   │  │ + GPU info │  │ + GPU info │       │
│  │ + ddc-svc  │  │            │  │            │       │
│  └────────────┘  └────────────┘  └────────────┘        │
└─────────────────────────────────────────────────────────┘
```

### 关键架构决策

| 决策点 | 选择 | 理由 |
|---|---|---|
| 部署模型 | 单点（操作机 1 台） | 远端不可能完全"零接触"，但单点部署 + 远端 WinRM 已是最接近"零安装"的方案 |
| 通讯协议 | WinRM (5985) + SMB (445) | Windows 原生、能力够、远端只需一次 `Enable-PSRemoting` |
| 后端语言 | Rust | Tauri 原生栈、性能、内存安全；Windows API 调用通过 `windows-rs` |
| 前端栈 | Web (React/Vue + Tailwind + shadcn/ui) | 配合 Figma 设计稿、UI 上限高、迭代速度快 |
| 系统调用兜底 | PowerShell sidecar 脚本 | Windows 复杂操作（PsExec、cmdkey SYSTEM、net share）经实战验证最稳 |
| 本地存储 | SQLite + Windows Credential Manager | 关系数据用 SQLite，敏感凭据用系统级安全存储 |
| 远端 Agent | 不需要 | 完全 agentless，纯 WinRM 调度 |
| PSO Cache 采集 | 远程触发 UE `-game` 模式 + 日志监控 | UE 没有 commandlet，必须真启动 game 模式 |

---

## 5. 功能模块

### 模块组织（按依赖分层）

```
A. Discovery & Inventory  (基础层)
   ├─ A1. 网络扫描 (ICMP + mDNS + WinRM/SMB 端口探测)
   ├─ A2. UE 版本探测 (远程读注册表 + 扫安装目录)
   └─ A3. GPU/驱动信息采集 (新)

B. Sharing Setup  (底座层)
   └─ B1. SMB 共享一键创建向导 (Mode A / Mode B)

C. Configuration Management  (DDC 主战场)
   ├─ C1. 环境变量配置 (远程 setx /M)
   ├─ C2. 项目 ini 写入 (DefaultEngine.ini 编辑 + 备份)
   └─ C3. 集群批量应用 (配置模板 + 并发推送)

D. DDC Pak Operations  (出差/离线场景)
   ├─ D1. 生成引擎 (本机 + 远程 + 进度监控)
   ├─ D2. 分发引擎 (项目身份匹配 + Robocopy + 路径映射缓存)
   └─ D3. 一站式向导 (三种组合)

F. PSO Cache Operations  (零卡顿，新)
   ├─ F1. PSO Precaching CVar 验证 (检查 r.PSOPrecaching 等是否启用)
   ├─ F2. PSO Cache 文件采集 (远程触发 -game 模式 + 进度监控)
   ├─ F3. PSO Cache 文件分发 (类似 D2，目标 Saved/CollectedPSOs/)
   └─ F4. GPU/驱动一致性检查 (跨机比对，差异预警)

E. Diagnostics & Health  (差异化)
   ├─ E1. 配置生效验证 (远程读 UE 日志)
   ├─ E2. INI 扫描器 + 冲突诊断 ★
   └─ E3. 集群健康检查 (DDC + PSO 综合 11 项检查矩阵)

横切支撑：凭据管理 / 操作历史 / 任务调度 / 配置模板
```

### 关键模块详解

**B1. SMB 共享一键创建向导**

支持两种模式：

- **Mode A（开放共享）**：Host 启用 Guest + Everyone:Full 权限。客户端零认证。仅限封闭剧组现场使用。
- **Mode B（统一账户）**：Host 创建 `ddc-svc` 账户（24 位随机密码），共享盘只授权该账户。客户端通过 `cmdkey` 注入用户级凭据，并通过 `psexec -s cmdkey` 注入 SYSTEM 级凭据（关键：解决 RenderStream Service 访问问题）。客户端不创建本地账户，所有用户继续用日常账号登录。

向导流程：选模式 → 选 Host → 配置共享 → 选客户端 → 预览（PowerShell 命令明文） → 执行（每机进度条）。

**E2. INI 扫描器**

扫描以下文件并诊断：

- 项目级：`Config/DefaultEngine.ini`、`Config/Windows/WindowsEngine.ini`、`Config/ConsoleVariables.ini`
- 用户级：`%LocalAppData%\UnrealEngine\<Ver>\Saved\Config\WindowsEditor\EditorPerProjectUserSettings.ini`（隐形杀手）
- 引擎级：`<Engine>/Engine/Config/BaseEngine.ini`（边缘扫描）

诊断规则识别：

- 🔴 **高危**：硬编码 `Path=` 无 `EnvPathOverride`、用户级覆盖存在、路径指向已下线设备
- 🟡 **警告**：使用映射盘符（Service 不可达）、已弃用 CVar、ini 与环境变量不一致
- 🟢 **健康**：使用 `EnvPathOverride=UE-SharedDataCachePath` 且环境变量已设

修复方式：跳过 / 自定义改 / 打开文件手改 / 应用建议（含自动备份）。

**D. DDC Pak 三种实用组合**

D1（生成）和 D2（分发）解耦设计，可任意组合出三种实用模式：

- **组合 1：本机生成（仅生成不分发）** — 你在自己机器上做完工程 → 生成 .ddp → 用 U 盘拷走
- **组合 2 ★：远程生成 + 远程分发** — 选定开发机 Render01 上的工程 → 在 Render01 上生成 .ddp → 自动同步到 Render02-08（核心场景，不经过操作机中转）
- **组合 3：本机生成 + 远程分发** — 操作机同时是 UE 开发机时 → 本机生成 → 推到所有渲染节点

UE 命令行：
```
UnrealEditor.exe Project.uproject -run=DerivedDataCache -fill -DDC=CreatePak
```
产物路径：`<ProjectDir>/DerivedDataCache/Compressed.ddp`（或 `DDC.ddp`，UE 版本相关）

**D2. 跨机器项目身份匹配**

不同机器同一项目路径不同（`D:\Work\EXLY` vs `E:\Projects\EXLY`），三级匹配策略：

- 第一级（自动）：按 `.uproject` 文件名匹配
- 第二级（半自动）：UI 列出所有发现的 `.uproject`，用户手动指定关联
- 第三级（手动）：用户填路径映射表

映射结果写入 SQLite `project_locations` 表，下次分发自动复用。

**F. PSO Cache 操作（新模块）**

UE 5 的"零卡顿"实际由两件事共同实现：

1. **PSO Precaching**（UE 5.1+ 引擎自动）—— 加载关卡时引擎扫描场景材质并自动预创建 PSO，**不需要预先采集**
2. **PSO Cache 文件**（手工采集）—— 兜底 Precaching 可能漏掉的边缘组合（例如低概率特效、动态生成的材质）

工具的角色：

**F1. PSO Precaching CVar 验证**
- 检查每台机器的 `r.PSOPrecaching=1`、`r.PSOPrecache.Compile=1` 等关键 CVar 是否启用
- UI 显示状态，未启用则一键设置（写入项目 ini 或环境变量）
- 工程量：~半天，并入健康检查矩阵

**F2. PSO Cache 文件采集**
- 选定一台开发机（GPU + 驱动跟集群一致）
- 选定项目 + 选定要遍历的关卡列表
- 远程通过 WinRM 启动 UE：
  ```
  UnrealEditor.exe Project.uproject -game -r.ShaderPipelineCache.Enabled=1 -windowed -resx=1920 -resy=1080
  ```
- 监控 UE 日志确认采集进行中
- 引导用户手动遍历场景（或脚本驱动 UE 自动切相机）
- 采集结果存放：`<ProjectDir>/Saved/CollectedPSOs/`

**F3. PSO Cache 文件分发**
- 复用 D2 的项目身份匹配机制
- 目标路径：每台机器的 `<ProjectDir>/Saved/CollectedPSOs/`
- Robocopy /MIR 多机并发

**F4. GPU/驱动一致性检查**
- 远程查询每台机器的 GPU 型号 + 驱动版本（通过 `dxdiag` 或 WMI `Win32_VideoController`）
- 集群矩阵视图：列出每台机器的 GPU + 驱动
- 不一致的标红警告："这些机器 GPU/驱动不一致，PSO Cache 文件不可共用"
- 给出建议：升级驱动到统一版本，或为不同 GPU 型号分别采集 PSO 文件

---

## 6. UI 设计

UI 设计的功能性需求（信息架构、每个视图的内容与操作、跨视图用户流、通用 UI 模式）已独立成 UI Brief 文档：

**`docs/superpowers/specs/2026-05-01-uecm-ui-brief.md`**

UI Brief 仅描述"功能性"，**不涉及视觉风格**（配色、字体、间距、主题、风格参考等）。视觉设计由设计师在 Claude Design / Figma 中独立产出，本 spec 不做约束。

### 关键交互原则（实施时必须遵守，与具体视觉无关）

- **预览即契约**：所有变更操作执行前必看预览页，PowerShell 命令明文，无黑盒
- **Dry Run 模式**：所有写操作支持假执行（仅产生预览不真改）
- **自动备份**：每次写操作前对相关注册表/ini 做快照，可一键回滚
- **每台机器独立进度**：批量操作时一台失败不阻塞其他，可针对失败项重试
- **失败结构化展示**：错误信息 = 诊断 + 影响 + 建议修复

### 一级视图清单（共 7 个）

Dashboard / Machines / Projects / DDC Pak / PSO Cache / INI Scanner / Health Check。每个视图的详细 brief 见 UI Brief 文档。

---

## 7. 技术栈

### 前端

- **框架**：Vue 3（团队熟悉度高）或 React 18（生态广），二选一
- **样式**：Tailwind CSS + CSS Variables（与 Figma Design Token 对接）
- **组件库**：shadcn-ui（React）或 Element Plus（Vue），按选定框架定
- **表格**：TanStack Table（机器列表、INI 结果、健康检查矩阵、GPU 矩阵）
- **状态管理**：Pinia（Vue）或 Zustand（React）
- **路由**：Vue Router 或 React Router

### 后端

- **运行时**：Tauri 2.x
- **语言**：Rust 1.75+
- **关键 crate**：
  - `windows` / `windows-rs`：Windows API 调用
  - `winreg`：注册表读写
  - `wmi`：WMI 查询（GPU 信息也走这个）
  - `rusqlite`：SQLite 驱动
  - `rust-ini`：UE ini 解析（可能要 fork 适配 UE 怪癖）
  - `tokio`：异步任务调度
  - `reqwest` / `quick-xml`：WinRM SOAP 客户端（或自实现）
  - `mdns-sd` + 自实现 ICMP：网络发现

### PowerShell Sidecar

打包独立 .ps1 脚本到 Tauri resources，包含：

- `enable-winrm.ps1`：远端启用 WinRM
- `setup-share-mode-a.ps1` / `setup-share-mode-b.ps1`：共享创建
- `inject-system-credential.ps1`：通过 PsExec 注入 SYSTEM 凭据
- `setx-machine.ps1`：远程设系统级环境变量
- `verify-config.ps1`：读 UE 日志验证配置
- `query-gpu-driver.ps1`：查 GPU 型号 + 驱动版本（新）
- `start-ue-pso-collect.ps1`：启动 UE `-game` 模式做 PSO 采集（新）

Rust 通过 `Command::new("powershell.exe")` 调用，传参 + 解析 stdout JSON。

### 工具与分发

- 构建：Tauri CLI
- 安装：单 .exe 绿色版（依赖 WebView2，Win10/11 99% 已预装）
- 自动更新：Tauri Updater（可选 v2 加）

---

## 8. 数据模型

### SQLite 表

| 表 | 关键字段 | 用途 |
|---|---|---|
| `machines` | id, hostname, ip, last_seen_at, role, winrm_status, status | 局域网机器持久化清单 |
| `machine_ue_installs` | machine_id, version, install_path, is_primary | 每机器装的 UE 版本 |
| `machine_gpus` | machine_id, gpu_model, driver_version, vendor, vram_mb（**新**） | 每机器 GPU 信息（PSO 一致性检查用） |
| `projects` | id, uproject_name, uproject_guid, last_seen_at | 项目逻辑身份（跨机统一） |
| `project_locations` | project_id, machine_id, abs_path | 项目在每台机器的实际路径 |
| `config_templates` | id, name, role_filter, env_vars (JSON), ini_overrides (JSON), is_default | 配置模板（Render Node 套餐等） |
| `share_configs` | id, host_machine_id, share_name, unc_path, mode, credential_alias | 已建共享 DDC 实例 |
| `operations` | id, started_at, action_type, target_machines (JSON), status, snapshot_blob, log_text | 操作历史 + 回滚快照 |
| `ini_findings` | id, scan_run_id, machine_id, project_id, file_path, severity, snippet, recommendation, fixed_at | INI 扫描结果 |
| `scan_runs` | id, scan_type, started_at, finished_at, summary_json | 扫描会话元数据 |
| `health_check_runs` | id, started_at, machine_results (JSON), summary | 健康检查历史 |
| `pso_cache_files`（**新**） | id, project_id, source_machine_id, file_path, generated_at, gpu_signature | PSO 采集产物记录 |
| `pso_distributions`（**新**） | id, pso_cache_file_id, target_machine_id, distributed_at, status | PSO 分发记录 |

### Windows Credential Manager

存敏感数据，SQLite 仅存别名引用：

- `UECM:winrm:<hostname>` → 远端 WinRM 登录用户名 + 密码
- `UECM:share:<host>:ddc-svc` → ddc-svc 账户密码（24 位强随机）

---

## 9. 错误处理

### 错误分类与处理

| 类别 | 例子 | 策略 |
|---|---|---|
| 网络瞬时 | WinRM 超时、SMB 临时不可达 | 指数退避重试 3 次（1s/3s/9s） |
| 认证失败 | WinRM 密码错、HOST\ddc-svc 拒绝 | 立即停止该机器，UI 弹"重新输入凭据" |
| 权限不足 | 工具非管理员、NTFS 拒写 | 立即停止整个操作，提示提权 |
| 资源冲突 | ini 被 UE 占用、共享盘锁 | 等 5s 重试一次，仍冲突跳过并报告 |
| 环境缺失 | 远端无 WinRM、UE 版本对不上 | 标"不适用"，详细日志 |
| UE 命令行失败 | DDC Pak 生成时 UE 崩溃、PSO 采集时 UE 退出 | 抓 UE 日志最后 200 行，UI 展示 |
| GPU/驱动不一致 | 集群里某台机器 GPU 不同（PSO 操作） | 高亮警告，引导用户决定是否继续/分别采集 |
| 用户中止 | 推配置中途暂停 | 已完成保留，进行中等当前命令完成后停 |
| 数据完整性 | INI 解析非法格式 | 跳过该段，标"无法解析" |

### 通用约束

- **写前快照**：相关注册表项、ini、共享配置自动备份到 `operations.snapshot_blob`
- **半成功明确**：N 台 7 成 1 败，UI 显示 "7/8 成功，1 失败可重试"
- **错误结构化**：`{ code, severity, machine, action, message, remediation, raw_output }`
- **全留痕**：所有 PowerShell 调用 stdin/stdout/stderr 入 `operations.log_text`
- **超时分级**：发现 5s、WinRM 命令默认 30s、DDC Pak 生成 60min（可配）、PSO 采集 120min（可配）

---

## 10. 测试策略

### 四层测试

**L1. 单元测试（CI 友好）**
- INI 解析器（覆盖 UE ini 各种格式）
- 诊断引擎（finding 生成正确性）
- 项目身份匹配
- GPU 签名匹配（PSO 一致性判断）
- 凭据序列化、配置模板渲染、操作快照生成

**L2. 集成测试（本地 Windows）**
- WinRM mock server（本地起 SOAP 服务）
- PowerShell 脚本测试（真 PowerShell + 注册表/ini 断言）
- SMB mock（本地假共享）
- SQLite 数据层（in-memory 事务测试）

**L3. E2E 测试（VM 矩阵）**
- 场景 A：全 5.4.4 + Mode B 共享 + DDC 配置
- 场景 B：5.3 + 5.4 混合 + INI 扫描修复
- 场景 C：DDC Pak 远程生成 + 分发，验证 .ddp 完整性
- 场景 D：故意制造冲突 → 健康检查检出
- 场景 E：PSO Cache 采集 + 分发 + 一致性检查（要真 GPU，VM 用 GPU passthrough 或物理机）

**L4. 现场冒烟（手动 checklist）**
- 新现场首次部署完整流程
- RenderStream Service 重启验证 SYSTEM 凭据可用
- 改资产 → DDC 重算 → 共享盘有新数据
- .ddp 拷新机器 → 零编译启动
- PSO 采集后切场景 → 用 UE Stat GPU 命令对比首帧耗时

### 测试难点

- **PsExec / SYSTEM 上下文测试** 需真实 Windows 主机，CI 容器跑不动 → 本地 + 现场跑
- **PSO 采集测试** 需真实 GPU + 真实 UE 工程，无法 mock → 本地 + 现场跑
- **Windows 版本差异**：至少覆盖 Win10 21H2 / Win11 22H2 / Win Server 2022
- **UE 版本差异**：至少覆盖 5.3 + 5.4 各一套测试
- **GPU 多样性**：至少覆盖 NVIDIA RTX 系列两个代次（30/40 系），AMD 卡为 v2

---

## 11. 工程量与分阶段

总预估：**10-14 周**（一人开发）

### 阶段 1：基础底座（2 周）
- Tauri 脚手架 + Vue/React 前端骨架
- SQLite schema + 数据访问层
- WinRM 客户端封装
- PowerShell sidecar 调度框架
- 基础 UI 布局（活动栏 + 二级面板 + 多 Tab）

### 阶段 2：发现与配置（2 周）
- 模块 A：网络扫描 + UE 探测 + GPU 信息采集
- 模块 C1/C2：环境变量配置 + 项目 ini 写入（单机）
- 凭据管理 + Windows Credential Manager 集成

### 阶段 3：共享建立（2 周）
- 模块 B1：Mode A + Mode B 一键向导
- SYSTEM 凭据注入（关键技术点）
- 配置生效验证

### 阶段 4：差异化能力（2 周）
- 模块 E2：INI 扫描器（含 EditorPerProjectUserSettings.ini）
- 模块 E3：健康检查 11 项（含 PSO 相关）
- 一键修复 + 自动备份回滚

### 阶段 5：DDC Pak（1.5 周）
- 模块 D：生成 + 分发 + 项目身份匹配

### 阶段 6：PSO Cache（2 周，新）
- 模块 F1：PSO Precaching CVar 验证（半天，并入健康检查）
- 模块 F2：PSO Cache 文件远程采集（启动 -game 模式 + 进度监控）
- 模块 F3：PSO Cache 文件分发（复用 D2 的项目匹配 + Robocopy）
- 模块 F4：GPU/驱动一致性检查矩阵 UI

### 阶段 7：打磨与冒烟（1.5 周）
- 完整 UI 美化（Figma → 代码）
- E2E 测试矩阵
- 现场冒烟测试 + 文档

---

## 12. 风险与开放问题

### 风险

| 风险 | 影响 | 缓解 |
|---|---|---|
| Rust + WinRM 生态不成熟 | 进度延迟 | 复杂操作回退 PowerShell sidecar |
| WebView2 缺失（极老机器） | 启动失败 | Tauri Bundle 模式打包（成本 +140MB） |
| UE 版本差异难全覆盖 | 部分用户翻车 | 优先支持 5.4 + 5.3，5.5+ 后续扩展 |
| RenderStream 不同版本行为差异 | Service 凭据可能不通用 | 测试矩阵覆盖主流版本 |
| 多账户机器 cmdkey 注入复杂 | UX 退化 | 先做"当前用户 + SYSTEM"覆盖 95%，边缘场景手动 |
| PSO 采集自动化难（需手动遍历场景） | UX 退化 | v1 引导用户手动遍历，v2 探索"自动相机巡游"脚本 |
| GPU 驱动版本碎片化 | PSO Cache 复用率低 | 健康检查强提醒"统一驱动版本" |
| `-game` 模式启动 UE 时间长 | 操作员等待 | 后台异步执行 + 系统托盘通知 |

### 开放问题（待实施时决策）

- 前端框架最终选 Vue 还是 React？（看团队熟悉度）
- 是否在 v1 集成 ZenServer 模式？（暂不集成，标为 v2）
- DDC Pak / PSO 采集时，UE 命令行的进度回调粒度？（待 PoC 验证）
- 健康检查的"定时自动扫描"频率上限？（避免压垮局域网，建议 ≥5 分钟）
- 操作历史 SQLite 大小如何控制？（建议保留最近 90 天 + 1000 条上限）
- PSO 采集是否支持"多机分布式采集"（不同机器跑不同关卡）？（v2 探索）

---

## 附：关键技术参考

- UE DDC 官方文档：UE 5.7 Derived Data Cache
- UE PSO Caching 官方文档：UE 5.x PSO Caching
- UE PSO Precaching 官方文档：UE 5.1+ Auto Precaching
- Windows Credential Manager API：`CredRead` / `CredWrite`
- PsExec SYSTEM 凭据注入：Sysinternals PsExec
- WinRM SOAP 协议：Microsoft WS-Management
- Tauri 2.x 文档：tauri.app
- 内部参考：`/RenderStream Shader DDC PSO FShaderCache SOP`（Notion）

---

## 附：实施计划进度

整体实施拆为 6 份 plan（对应 spec 章节 11 的 7 个阶段，其中阶段 6+7 合并）。每份 plan 完成后在此勾选并填入文件名。

- [x] **Plan 1：基础底座** — `docs/superpowers/plans/2026-05-01-uecm-plan-1-foundation.md`
  - Tauri 脚手架 + Vue 3 前端骨架 + SQLite + PowerShell sidecar 框架 + 7 视图 stub
  - 工期：~2 周
  - 状态：✅ 已执行完成（2026-05-01），24 commits on main
- [x] **Plan 2：发现与配置** — `docs/superpowers/plans/2026-05-01-uecm-plan-2-discovery-and-config.md`
  - 网络扫描 + UE/GPU 探测 + 凭据管理 + 单机环境变量配置 + 单机 ini 编辑 + WinRM 客户端封装
  - 工期：~2 周
  - 状态：✅ 已执行完成（2026-05-02 lanPC E2E 7/7 通过），20 commits + 3 个 fix commits
- [x] **Plan 3：提权 + 共享 + 集群批量** — `docs/superpowers/plans/2026-05-02-uecm-plan-3-elevation-and-shares.md`
  - 提权与显式凭据 + SMB 共享一键创建（Mode A + Mode B）+ SYSTEM 凭据注入 + 集群批量配置推送 + Plan 2 E2E 收口（hostname rename / last_seen_at / GPU VRAM DXGI fallback）
  - 工期：~2 周
  - 状态：✅ Plan 文件已写好，待执行
- [x] **Plan 4：诊断模块** — `docs/superpowers/plans/2026-05-03-uecm-plan-4-diagnostics.md`
  - INI 扫描器 + 冲突诊断 + 集群健康检查矩阵（11 项）
  - 工期：~2 周
  - 状态：✅ 已执行完成（2026-05-05，分支 `feature/plan-4-diagnostics`，20 commits `c1b5c57..5df4607`，待合并 main）
- [ ] **Plan 5：DDC Pak**
  - 生成 + 分发 + 项目身份匹配（三种实用组合）
  - 工期：~1.5 周
- [ ] **Plan 6：PSO Cache + 打磨**
  - PSO Precaching 验证 + PSO Cache 文件采集分发 + GPU 一致性 + Figma 视觉应用 + E2E 测试 + 现场冒烟
  - 工期：~3.5 周

**注**：Plan 1 把原本 spec 阶段 1 计划的"WinRM 客户端封装"挪到 Plan 2，理由：WinRM client 实现量约 4-5 天，挤进 Plan 1 会让 Plan 1 过重，且 Plan 1 阶段并不真正使用 WinRM。Plan 2 才真正在网络发现里用 WinRM，落地更合理。
