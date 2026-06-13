# UECM CLI 走读验证报告

**走读日期**：2026-06-12 / 2026-06-13  
**CLI 版本**：0.1.0  
**测试机**：lanPC（192.168.10.20，WSL2，Windows 11）  
**隔离 DB**：`_walkthrough/test.db`

---

## 走读覆盖的 8 个核心功能域

| # | 功能域 | 状态 | 关键命令 |
|---|--------|------|----------|
| 1 | 扫描与基础设置 | ✅ 完成 | `machine scan/add/refresh`, `local-cache create`, `env set`, `share create`, `cred save`, `ssh package-bootstrap` |
| 2 | ZenServer 部署 | ✅ 完成 | `zen register/detect-binary/apply-config/urlacl/service install+start/probe/enable` |
| 3 | PSO 缓存 | ⚠️ 部分（collect panic） | `pso verify/list/collect/distribute` |
| 4 | DDC Pack | ⚠️ 部分（legacy generate panic） | `ddc generate/verify/distribute` |
| 5 | INI 扫描修复 | ✅ 完成 | `ini scan/findings/apply/skip` |
| 6 | 集群健康检查 | ✅ 完成 | `health run/results/consistency-check` |
| 7 | Log 验证 | ⚠️ 需真实项目路径 | `log verify-startup`, `health analyze-advisories` |
| 8 | 一键部署 | ✅ dry-run 完成 | `deploy ddc --plan` |

---

## Bug 修复状态（2026-06-13 复验）

### BUG-1：`pso collect` / `ddc generate legacy` Tokio runtime panic（exit 101）✅ 已修复
- **位置**：`src/core/ue_runner.rs:90` → 修复于 `domain_pso.rs` / `domain_ddc.rs`
- **现象**：所有实际启动 UE 进程的命令均 panic "there is no reactor running"
- **修复**：`launch_generation` / `launch_collection` 移入 `rt.block_on()` 闭包内（commit 88a5a91）
- **复验**：`pso collect` 正常输出 NDJSON 流；`ddc generate --backend legacy` 正常启动 UE 并输出 9700+ 行日志

### BUG-2：`ddc verify` 不跟 `ddc generate` 共享 Zen 路由逻辑 ✅ 已修复
- **现象**：`generate --backend auto` Zen 可达时返回 `skipped=true`，但 `verify --backend auto` 仍找 .ddp 文件
- **修复**：`domain_ddc.rs` verify 共享相同路由判断，Zen 可达时直接返回 `skipped:true`（commit 88a5a91）
- **复验**：`ddc verify --backend auto` 返回 `{"skipped":true,"reason":"zen handles caching natively"}`

### BUG-3：`ini apply` R015 只写独立键，不修改 tuple 内联值 ✅ 已修复
- **现象**：修复写入 `Shared.DeleteUnused=true`，但原 tuple 不变，re-scan 持续 warning
- **修复**：`ini_apply.rs` 检测点分 key_name 并通过 `set_backend_field` 就地修改 tuple 字段（commit 88a5a91）
- **复验**：apply 后 tuple 内联包含 `DeleteUnused=true`，re-scan warning 计数从 5 减至 4

### BUG-4（新）：`zen urlacl add` 大小写比对误报冲突 ✅ 已修复
- **现象**：`NT AUTHORITY\LOCAL SERVICE` vs `NT AUTHORITY\LocalService` 被判为不同账号
- **修复**：`zen-urlacl-add.ps1` 新增 SID 比对函数，同 SID 不报冲突（commit 88a5a91）
- **复验**：返回 `ok:true, already_exists:true`

### BUG-5（新）：UECM ZenServer 服务与多版本 UE 冲突 ✅ 已修复
- **现象**：UE 5.7 启动时弹 4 个报错窗口，Zen 缓存不可用
- **根因**：`zen enable` 未在目标机器执行，UE 默认 `AutoLaunch=true` 导致与 UECM 托管的 ZenServer 服务冲突
- **修复 1**（commit 92017be）：服务名改为 `UECMZenServer`，避免 UE 的 `DetermineSystemServiceInfo()` 误识别
- **修复 2**（commit 0e9aeb4）：`domain_zen.rs` 全局模式 UserEngine.ini 路径从 Roaming 改为 Local（与 UE ConfigHierarchy 一致）
- **修复 3**：在 lanPC 执行 `zen enable --global`，写入 `AutoLaunch=false` + `ZenShared` 到 `UserEngine.ini`
- **复验**（2026-06-13）：启动 UE Editor，无弹窗，Zen 缓存正常连接

---

## 设计问题（不影响正确性但影响体验）

### DESIGN-1：health run 的 `env_shared/env_vars` 与 Zen 模式冲突
- Zen enable 要求清除 `UE-SharedDataCachePath`，但 health check 仍期望该变量存在
- 需要 health 规则感知 Zen 模式，Zen 启用后跳过 `env_shared` 检查

### DESIGN-2：`deploy ddc` plan JSON 所有字段必填，即使功能已禁用
- `pso.enabled: false` 仍要 `resolution/max_minutes`
- `verify.run_log_verify: false` 仍要 `editor_exe`
- GUI 生成 plan 文件需完整填充所有字段

### DESIGN-3：`zen env-cleanup` 无独立 CLI 命令
- `zen enable` 完成后提示手动运行 PS sidecar 清除 `UE-SharedDataCachePath`
- 建议补 `zen clean-env` 子命令，内部走提权通道

---

## 操作顺序陷阱（Skill/GUI 必须遵守）

### ZenServer 完整部署顺序
```
1. zen register --machine <id> --role shared_upstream --declared-port 8558 --data-dir <dir>
2. zen detect-binary --machine <id> --cred-alias <alias>        # 必须在 apply-config 之前
3. zen apply-config --endpoint-id <id> --cred-alias <alias> --yes
4. zen urlacl add --endpoint-id <id> --principal "NT AUTHORITY\LocalService" --cred-alias <alias> --yes
5. (如有冲突) 提权 SSH 停服 → zen service uninstall → zen service install
6. zen service start --endpoint-id <id> --cred-alias <alias>
7. zen probe --machine <id>                                      # 验证可达
8. machine set-ue-user --machine <id> --ue-user <Windows用户名> # global 模式必须
9. zen enable --upstream-endpoint-id <id> --global --machines <id> --cred-alias <alias> --yes
10. 提权 SSH 运行 zen-env-cleanup.ps1 清除 UE-SharedDataCachePath
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
- `ddc verify --backend auto` 在 Zen 可达时同样跳过（`skipped=true`，2026-06-13 修复后复验）
- `pso collect` / `ddc generate --backend legacy` 正常启动 UE 进程并流式输出日志（2026-06-13 修复后复验）
- `zen urlacl add` SID 比对：`NT AUTHORITY\LOCAL SERVICE` 与 `NT AUTHORITY\LocalService` 识别为同一账号（2026-06-13 修复后复验）
- `ini apply` 自动创建 `.bak.<timestamp>` 备份
- `ini apply` R015 就地修改 backend-graph tuple 内联字段，re-scan 不再重复触发（2026-06-13 修复后复验）
- `health run` L1/L2/L3 三层检查，每条 critical 有 `remediation` 字段
- `deploy ddc --dry-run` 输出完整 steps 列表，可预览不执行

## 部署注意事项

- **PS 脚本同步**：binary 旁的 `ps-scripts/` 优先于源码 repo（`UECM_PS_DIR` > `<exe-dir>/ps-scripts` > `CARGO_MANIFEST_DIR/../ps-scripts`）；每次更新 binary 必须同步复制 `ps-scripts/*.ps1`

---

## 全部发现索引

见 `FINDINGS.md`（F-001 到 F-035）
