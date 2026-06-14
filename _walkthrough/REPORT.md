# UECM CLI 走读验证报告

**走读日期**：2026-06-12 / 2026-06-13 / 2026-06-14  
**CLI 版本**：0.1.0  
**测试机**：lanPC（192.168.10.20，WSL2，Windows 11）  
**隔离 DB**：`_walkthrough/test.db`

---

## 架构决策：ZenServer 独立服务器部署

**决策日期**：2026-06-13  
**依据**：UE 官方文档 *Zenserver as Shared DDC*

ZenServer Shared DDC 部署在**独立的服务器机器**上（不与工作站共机），工作站保持 UE 默认的 `AutoLaunch=true`，由 UE Editor 自管本地 ZenLocal。两层并存互不冲突：

| 层 | 位置 | 管理者 | 端口 |
|---|---|---|---|
| ZenLocal | 工作站 localhost | UE Editor AutoLaunch | 8558 |
| ZenShared | 独立服务器 LAN IP | UECM Windows 服务 | 8558 |

此决策淘汰了之前的"同机部署 + AutoLaunch=false"方案（commit 237b012 回滚了 AutoLaunch 相关代码）。

---

## 走读覆盖的 8 个核心功能域

| # | 功能域 | 状态 | 关键命令 |
|---|--------|------|----------|
| 1 | 扫描与基础设置 | ✅ 完成 | `machine scan/add/refresh`, `local-cache create`, `env set`, `share create`, `cred save`, `ssh package-bootstrap` |
| 2 | ZenServer 部署 | ✅ 完成 | `zen register/detect-binary/apply-config/urlacl/service install+start/probe/enable` |
| 3 | PSO 缓存 | ✅ 完成（panic 已修） | `pso verify/list/collect/distribute` |
| 4 | DDC Pack | ✅ 完成（panic 已修） | `ddc generate/verify/distribute` |
| 5 | INI 扫描修复 | ✅ 完成 | `ini scan/findings/apply/skip` |
| 6 | 集群健康检查 | ✅ 完成 | `health run/results/consistency-check` |
| 7 | Log 验证 | ⚠️ 需真实项目路径 | `log verify-startup`, `health analyze-advisories` |
| 8 | 一键部署 | ✅ dry-run 完成 | `deploy ddc --plan` |

---

## Bug 修复状态

### BUG-1：`pso collect` / `ddc generate legacy` Tokio runtime panic（exit 101）✅ 已修复
- **位置**：`src/core/ue_runner.rs:90` → 修复于 `domain_pso.rs` / `domain_ddc.rs`
- **修复**：`launch_generation` / `launch_collection` 移入 `rt.block_on()` 闭包内（commit 88a5a91）
- **复验**：`pso collect` 正常输出 NDJSON 流；`ddc generate --backend legacy` 正常启动 UE 并输出 9700+ 行日志

### BUG-2：`ddc verify` 不跟 `ddc generate` 共享 Zen 路由逻辑 ✅ 已修复
- **修复**：`domain_ddc.rs` verify 共享相同路由判断，Zen 可达时直接返回 `skipped:true`（commit 88a5a91）
- **复验**：`ddc verify --backend auto` 返回 `{"skipped":true,"reason":"zen handles caching natively"}`

### BUG-3：`ini apply` R015 只写独立键，不修改 tuple 内联值 ✅ 已修复
- **修复**：`ini_apply.rs` 检测点分 key_name 并通过 `set_backend_field` 就地修改 tuple 字段（commit 88a5a91）
- **复验**：apply 后 tuple 内联包含 `DeleteUnused=true`，re-scan warning 计数从 5 减至 4

### BUG-4：`zen urlacl add` 大小写比对误报冲突 ✅ 已修复
- **修复**：`zen-urlacl-add.ps1` 新增 SID 比对函数，同 SID 不报冲突（commit 88a5a91）
- **复验**：返回 `ok:true, already_exists:true`

### BUG-5：UECM ZenServer 服务与 UE Editor 冲突 ✅ 已修复（架构调整）
- **现象**：ZenServer 服务与 UE Editor 部署在同一台机器时，UE 启动弹 4 个错误窗口
- **根因**：UE AutoLaunch 与 UECM 服务争抢同一端口 8558
- **曾尝试**：`AutoLaunch=false`（commit 92017be）→ 禁用了 UE 内建 Zen 管理面板，非正解
- **最终方案**（commit 237b012）：回滚 `AutoLaunch=false`，改为独立服务器部署。UE 官方文档定义的架构：ZenShared 跑在独立服务器上，ZenLocal 由 UE 自管，两层共存无冲突
- **附带保留**：服务名 `UECMZenServer`（避免与 UE 内部服务名碰撞）、UserEngine.ini 路径修正为 `%LocalAppData%`（commit 0e9aeb4）

### BUG-6：`zen service install` sc.exe PowerShell splatting exit 1639 ✅ 已修复
- **根因**：PowerShell splatting 对 `$binpath` 内部引号二次转义
- **修复**：改为 `cmd /c` 字符串方式，绕过 PowerShell 转义层（commit 237b012）
- **复验**：`zen service install` 成功创建 `UECMZenServer` 服务

---

## 待修复清单 —— 已全部修复（2026-06-14）

代码改动均通过 `cargo test --lib`（1055 passed）。所有结论以 UE 源码（`UnrealEngine/Engine/Config/BaseEngine.ini` + `ZenCacheStore.cpp` / `HttpCacheStore.cpp` / `HttpHostBuilder.cpp`）与 Zen 源码核对为准。**真机 E2E（UE 实际连上共享 Zen）需在 lanPC 验证，属后续。**

### 🔴 走读期间新发现的真 bug：Port 未生效（已随 ZEN-1 修复）

UE 的 Zen 缓存解析器（`FZenCacheStoreParams::Parse`）**没有 `Port=` 字段**。旧 value_template `(Type=Zen, Host="render-master", Port=8558, ...)` 里的 `Port=8558` 被静默忽略，`Host="render-master"` 无 scheme/port → `FHttpHostBuilder` 无法解析出可用端点，几乎必然连不上。**端口必须内嵌进 Host URI**（`http://render-master:8558`）。这比单纯的「写法对齐」更根本。

### 高优先级：独立服务器部署技术路径完善

| # | 状态 | 实现 |
|---|---|---|
| ZEN-1 | ✅ | `zen enable` 改写 `[StorageServers] Shared=(Host="http://{host}:{port}", Namespace="{namespace}", EnvHostOverride=UE-ZenSharedDataCacheHost, CommandLineHostOverride=ZenSharedDataCacheHost, DeactivateAt=60)`。依赖 UE 5.4+ 默认就有的 `ZenShared=(Type=Zen, ServerID=Shared)` 节点（BaseEngine.ini）经 `ServerID` 间接引用 —— **不再写 backend graph 节点**（旧的 `[InstalledDerivedDataBackendGraph]` 段在现代 UE 已不是节点容器）。读回链同步更新：`ini_diagnostics_zen.rs`（R013/R014 grammar 改为解析 Host URI）、`ini_config_extract.rs`（新增 StorageServers 捕获）、R026（识别新旧两种形态） |
| ZEN-2 | ✅ | `docs/research/zen-ini-rules.yaml` 的 `enable_zen_shared` 已改 section=`StorageServers`、key=`Shared`、value_template 如上；embedded 副本经 `include_str!` build 时自动同步 |
| ZEN-3 | ✅ | `zen service install` 预检：目标机若有 `ue_runtime_user`（= 工作站信号）则发 advisory 警告（`Event::LogLine` + dry-run/summary 的 `warnings[]`），**不硬失败**。「活跃 UE 进程检测」无现成 probe/列，本期未做（需新 ps-script） |
| ZEN-4 | ✅ | 新增 `zen set-region-host --machines <ids> --host <url>`：规范化为 `http://host:port` 后写 Machine-scope `UE-ZenSharedDataCacheHost`（复用 `setx-machine.ps1`）。配合 ZEN-1 写入的 `EnvHostOverride`，env 覆盖 INI Host 实现按机/区路由。清除走 `zen clean-env --name UE-ZenSharedDataCacheHost` |

### 中优先级：设计问题

| # | 状态 | 实现 |
|---|---|---|
| DESIGN-1 | ✅ | health 感知 Zen 模式：集群级信号 `cluster_has_shared_zen`（存在任一 `shared_upstream` 端点即判定）。激活时把 `env_shared/env_vars` 的 `critical` 降级为 `na`（计入 skipped）并改写 message/remediation。CLI(`domain_health.rs`) + UI(`commands/health_check.rs`) 同改。**集群级而非按机**：独立服务器模型下工作站跑 `zen enable` 不为自己注册端点，按机信号会漏掉误报真正发生的机器 |
| DESIGN-2 | ✅ | `PsoSpec.resolution/max_minutes` + `VerifySpec.editor_exe/timeout_seconds` 加 `#[serde(default)]`；新增 `DeployPlan::validate()`（enabled 但缺字段才报错），CLI + Tauri run 路径反序列化后调用 |
| DESIGN-3 | ✅ | 新增 `zen clean-env --machines <ids> [--name <var>] [--scopes machine,user]`，复用既有 `invoke_env_cleanup` + `zen-env-cleanup.ps1`。**更正 REPORT 原说法**：`zen enable` 早已自动调用清理（非「仅提示手动运行」），本任务只是把该能力暴露为独立命令 |

### 低优先级：增强项

| 来源 | 状态 | 实现 |
|---|---|---|
| F-005 | ✅ | `core/discovery.rs::detect_gpus` 单点过滤（两个写库调用方都经它）：丢弃 model 名命中虚拟适配器 denylist（Microsoft Basic/Remote Display、Parsec、VMware、向日葵/Sunlogin、IddCx、DisplayLink…）的项，并要求 `vendor != Unknown OR vram_mb.is_some()`（厂商未知且无 VRAM 视为虚拟） |
| F-014 | ✅ | `core/project_discovery.rs::run_discovery` 在 persist 前过滤：位于 `machine_ue_installs.install_path` 子树下（精确主信号）或命中引擎子树段（`\Engine\`/`\Templates\`/`\Samples\`/`\FeaturePacks\`，兜底）的 `.uproject` 被排除；跳过数走 `tracing::info!` 记录（不静默截断） |

### 新增 / 变更的 CLI 命令

- `zen clean-env`（DESIGN-3）
- `zen set-region-host`（ZEN-4）
- `zen service install` 现会在工作站上附 advisory 警告（ZEN-3）

---

## 操作顺序（Skill/GUI 必须遵守）

### ZenServer 服务端部署（在独立服务器上执行）
```
1. machine add --ip <server_ip>
2. machine refresh <id>
3. cred save --alias <alias> --user uecm-svc --pass-stdin
4. zen register --machine <id> --role shared_upstream --declared-port 8558 --data-dir <dir>
5. zen detect-binary --machine <id> --cred-alias <alias>
6. zen apply-config --endpoint-id <id> --cred-alias <alias> --yes
7. zen urlacl add --endpoint-id <id> --principal "NT AUTHORITY\LocalService" --cred-alias <alias> --yes
8. zen service install --endpoint-id <id> --cred-alias <alias> --yes
9. zen service start --endpoint-id <id> --cred-alias <alias>
10. zen probe --machine <id>                                      # 验证可达
```

### ZenShared 客户端配置（在工作站上执行）
```
1. machine set-ue-user --machine <workstation_id> --ue-user <Windows用户名>
2. zen enable --upstream-endpoint-id <id> --global --machines <workstation_id> --cred-alias <alias> --yes
```
注意：独立服务器方案下不写 `AutoLaunch=false`，UE 的 ZenLocal 保持正常工作。

`zen enable` 写入的 `[StorageServers] Shared` 含 `EnvHostOverride=UE-ZenSharedDataCacheHost`，故可按机/区覆盖 Host：
```
# 多区域路由（可选）：让某些工作站指向就近的共享 Zen 服务器
zen set-region-host --machines <ids> --host http://<region_server>:8558 --yes
# 还原到 INI 默认（清除区域覆盖）/ 清理遗留 SMB DDC 变量
zen clean-env --machines <ids> --name UE-ZenSharedDataCacheHost --yes
```

### health run 前置刷新
```
zen probe --machine <ids>
zen cache-stats --endpoint-id <id>
health run --machine-ids <ids>
```

### 机器接入顺序（Bootstrap）
```
1. ssh package-bootstrap --out <目录>   # 生成引导包
2. 人工传送到新机器并双击运行           # 唯一需要现场的步骤
3. machine add --ip <IP>
4. machine refresh <id>
5. machine set-ue-user --machine <id> --ue-user <用户名>
```

---

## 权限限制汇总

| 操作 | WinRM（uecm-svc）| 提权 SSH（uecm-svc） |
|------|-----------------|---------------------|
| 读取文件/注册表 | ✅ | ✅ |
| 写入 Machine scope 环境变量 | ❌ UAC 过滤 | ✅ |
| Stop/Uninstall Windows 服务 | ❌ UAC 过滤 | ✅ |
| 创建 SMB 共享 | ✅（需管理员组） | ✅ |
| 写入 Program Files INI | ✅（uecm-svc 在 Admins 组） | ✅ |

提权 SSH 通道：`ssh -i <UecmKeyPath> uecm-svc@<IP>`  
Key 路径：`C:\Users\lanPC\AppData\Roaming\com.lanbipu.uecm\uecm_ed25519`

---

## 已验证的正确行为（Skill/GUI 可信赖）

- `machine scan` 纯探测不写 DB，`machine add` 才写入
- `local-cache create` 用 `create_dir_all`，目录不存在时自动建
- `env set` / `ini apply` 均支持 `--dry-run` 预览
- `zen detect-binary` 自动选最高 UE 版本的 in-tree 二进制
- `zen apply-config` 写 zen.lua 后附 SHA256 完整性校验
- `ddc generate --backend auto` 在 Zen 可达时智能跳过（`skipped=true`）
- `ddc verify --backend auto` 在 Zen 可达时同样跳过（`skipped=true`，BUG-2 修复后复验）
- `pso collect` / `ddc generate --backend legacy` 正常启动 UE 进程并流式输出日志（BUG-1 修复后复验）
- `zen urlacl add` SID 比对正确识别大小写变体（BUG-4 修复后复验）
- `ini apply` 自动创建 `.bak.<timestamp>` 备份
- `ini apply` R015 就地修改 backend-graph tuple 内联字段（BUG-3 修复后复验）
- `health run` L1/L2/L3 三层检查，每条 critical 有 `remediation` 字段
- `deploy ddc --dry-run` 输出完整 steps 列表，可预览不执行

## 部署注意事项

- **PS 脚本同步**：binary 旁的 `ps-scripts/` 优先于源码 repo（`UECM_PS_DIR` > `<exe-dir>/ps-scripts` > `CARGO_MANIFEST_DIR/../ps-scripts`）；每次更新 binary 必须同步复制 `ps-scripts/*.ps1`

---

## 全部发现索引

见 `FINDINGS.md`（F-001 到 F-039）
