# Plan 5 + Plan 6 Merge & Fix Execution Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 `codex/plan-5-ddc-pak` 与 `codex/plan-6-pso-cache` 两个分支的所有功能修完 review blocker / major findings 之后，作为两个独立 commit 合入 `main`。

**Architecture:** 不走 `git rebase`，因为 plan-5 当前是"删 plan-4"形态、rebase 冲突量极大。改走"以 main 为基底，把分支文件复制过来 + 手动合并共享文件 + squash 成一个 commit"的策略。`codex/plan-6-pso-cache` 因为历次 merge 已经包含 plan-4 + plan-5 + plan-6 全部代码，可作为参考底板。两阶段：Phase A 出 `codex/plan-5-integrated`（plan-5 内容 + plan-5 fix），Phase B 出 `codex/plan-6-integrated`（plan-5 + plan-6 内容 + plan-6 fix）。

**Tech Stack:** Tauri 2 + React/Vue + Rust (rusqlite, tokio, serde) + PowerShell 7 + pnpm + vitest + cargo。

**4 拍板回顾:**
1. robocopy 默认改 `/E`（不再 `/MIR`），不加开关
2. NTLM loopback self-target 短路按 main `9c999b8` 同样手法（`core/loopback.rs::is_loopback_target` + 调用方加 short-circuit）
3. `pso_distribute` 这次抽 `pak_distribute` 共用框架，PSO 层只覆盖 source subdir + file glob
4. Phase A 末尾一个总 commit、Phase B 末尾一个总 commit；中途 task 只 `git add`，不 commit

**执行修正（2026-05-06 Codex review）:**
- 当前 `origin/main` = `f019d2c`，落后本地 `main` 的 `9c999b8` / `ca9ddcd` / `a5f1814`，所以新 worktree 必须从本地 `main` 创建，不能从 `origin/main` 创建。
- 本地分支约定使用 `codex/*`，所以 `claude/plan-5-integrated` / `claude/plan-6-integrated` 全部改为 `codex/plan-5-integrated` / `codex/plan-6-integrated`。
- Worktree 使用既有 Superpowers convention：`/Users/bip.lan/.config/superpowers/worktrees/ue-cache-manager/codex-plan-5-integrated` 和 `/Users/bip.lan/.config/superpowers/worktrees/ue-cache-manager/codex-plan-6-integrated`。
- 本次用户已明确要求“审查这个plan，有问题请修复，然后执行”，所以 A13/A14 与 B8/B9 不再停下等待口头授权；完成验证后创建两个本地 commit 并 merge 到本地 `main`。Remote `push` 只在用户后续明确要求时执行。

**前置条件:**
- 当前 local main HEAD = `a5f1814 fix(ps+ini-editor): username normalize + case-insensitive section match`
- `codex/plan-5-ddc-pak` 与 `codex/plan-6-pso-cache` 远程已存在
- mac 开发机能跑 `cargo check`、`pnpm build`、`pnpm test`，但跑不了 Windows-only 的 PS 脚本
- Phase C lanPC E2E 由用户在 Windows 端执行，本 plan 只产出 checklist

---

## File Structure

### Phase A 在 `codex/plan-5-integrated` 上要新建的文件

| 路径 | 责任 |
|---|---|
| `src-tauri/src/data/operations.rs` | operations 表 CRUD（job history） |
| `src-tauri/src/data/projects.rs` | projects 表 CRUD |
| `src-tauri/src/data/project_locations.rs` | project_locations 表 CRUD |
| `src-tauri/src/core/project_identity.rs` | uproject 身份匹配（纯函数） |
| `src-tauri/src/core/project_discovery.rs` | discover-uprojects.ps1 包装 |
| `src-tauri/src/core/ue_runner.rs` | UE 进程 runner（local + remote backend） |
| `src-tauri/src/core/ddc_pak.rs` | DDC pak 生成核心 |
| `src-tauri/src/core/pak_distribute.rs` | robocopy 扇出核心（plan-5 原版 + Phase B 会重构） |
| `src-tauri/src/commands/projects.rs` | projects Tauri commands |
| `src-tauri/src/commands/ddc_pak.rs` | DDC pak Tauri commands + `UeJobRegistry` |
| `ps-scripts/discover-uprojects.ps1` | uproject 文件探测 |
| `ps-scripts/start-ue-process.ps1` | UE 启动 sidecar |
| `ps-scripts/tail-ue-log.ps1` | UE 日志 tail（fix M2 long） |
| `ps-scripts/stop-ue-process.ps1` | UE 进程停止（fix B2 `$Pid`） |
| `ps-scripts/generate-ddc-pak.ps1` | DDC pak preflight |
| `ps-scripts/verify-pak-output.ps1` | pak 输出验证 |
| `ps-scripts/distribute-pak-file.ps1` | robocopy 扇出（fix B3 `$args` + M1 `/E`） |
| `src/services/tauri.ts` (modify) | 追加 projects + ddcPak 类型与 invoke 包装 |
| `src/stores/projects.ts` | projects pinia store |
| `src/stores/ddcPak.ts` | DDC pak pinia store |
| `src/components/primitives/UecmProgressBar.vue` | 进度条 |
| `src/components/primitives/UecmTaskCard.vue` | 任务卡 |
| `src/components/primitives/UecmPathInput.vue` | 路径输入 |
| `src/components/primitives/index.ts` (modify) | 追加 3 个 primitive 导出 |
| `src/components/modals/ProjectDiscoveryWizard.vue` | 项目发现 wizard |
| `src/components/modals/ProjectMatchingModal.vue` | 项目匹配 modal |
| `src/components/modals/DdcPakWizard.vue` | DDC pak wizard |
| `src/components/ddcpak/PakJobCard.vue` | pak job 卡 |
| `src/components/ddcpak/DistributeProgressTable.vue` | 分发进度表 |
| `src/views/Projects.vue` (overwrite) | projects 视图重写 |
| `src/views/DDCPak.vue` (overwrite) | DDC pak 视图重写 |
| 11 个 `src/__tests__/*.spec.ts` | 前端单测 |

### Phase A 要修改的共享文件

| 路径 | 修改 |
|---|---|
| `src-tauri/src/data/schema.rs` | 在现有 007 后追加 008/009/010 三条迁移 |
| `src-tauri/src/data/mod.rs` | 追加 `operations` / `projects` / `project_locations` 模块 |
| `src-tauri/src/core/mod.rs` | 追加 6 个新模块声明 |
| `src-tauri/src/commands/mod.rs` | 追加 `projects` / `ddc_pak` 模块声明 |
| `src-tauri/src/lib.rs` | setup block 追加 `app.manage(commands::ddc_pak::UeJobRegistry::default())`、invoke_handler 追加 11 个新命令 |

### Phase A 新增 fix（M3 NTLM loopback self-target）

| 路径 | 修改 |
|---|---|
| `src-tauri/src/core/ue_runner.rs` | `start_remote_process` / `read_tail` / `stop_remote_process` 入口检测 `loopback::is_loopback_target(host)`，短路到 `start_local_process` 等 |
| `src-tauri/src/core/pak_distribute.rs` | `run_one` 入口检测 target host，self-target 时跳过 PSCredential 走本地 robocopy |
| `src-tauri/src/commands/ddc_pak.rs` | `generate_ddc_pak` 入口检测 source machine，is_local 时强制 `UeBackend::Local` |

### Phase B 在 `codex/plan-6-integrated` 上新建/修改的文件

| 路径 | 责任 |
|---|---|
| `src-tauri/src/data/pso_cache_files.rs` | pso_cache_files 表 CRUD |
| `src-tauri/src/data/pso_distributions.rs` | pso_distributions 表 CRUD |
| `src-tauri/src/data/schema.rs` (modify) | 追加 011/012 两条迁移 |
| `src-tauri/src/core/gpu_consistency.rs` | GPU 矩阵聚合（纯函数） |
| `src-tauri/src/core/pso_collect.rs` | PSO 收集（含 PSO log marker 解析） |
| `src-tauri/src/core/pso_distribute.rs` | thin wrapper over `pak_distribute`（M14） |
| `src-tauri/src/core/pak_distribute.rs` (modify) | 抽出可复用 plan / preflight / run 框架，参数化 source subdir + file glob（M14） |
| `src-tauri/src/core/ini_diagnostics.rs` (modify) | 追加 R008/R009/R010 三条 PSO CVar 规则 |
| `src-tauri/src/commands/gpu_consistency.rs` | `get_gpu_consistency_matrix` |
| `src-tauri/src/commands/pso.rs` | start/list/distribute PSO 命令（fix B5 错误传递、B6 时间戳） |
| `src-tauri/src/commands/ini_scanner.rs` (modify) | `verify_pso_precaching` 命令 |
| `src-tauri/src/commands/mod.rs` (modify) | 追加 `gpu_consistency` / `pso` |
| `src-tauri/src/core/mod.rs` (modify) | 追加 `gpu_consistency` / `pso_collect` / `pso_distribute` |
| `src-tauri/src/data/mod.rs` (modify) | 追加 `pso_cache_files` / `pso_distributions` |
| `src-tauri/src/lib.rs` (modify) | invoke_handler 追加 5 个 PSO/GPU 命令 |
| `ps-scripts/list-pso-cache-files.ps1` | PSO 文件枚举 sidecar |
| `ps-scripts/distribute-pso-cache.ps1` | PSO 分发 sidecar |
| `src/services/tauri.ts` (modify) | 追加 pso/gpu 类型与 invoke |
| `src/stores/pso.ts` | PSO store（fix B5 处理失败事件 + cancelled 状态） |
| `src/stores/gpuConsistency.ts` | GPU 矩阵 store |
| `src/stores/healthCheck.ts` (modify) | 派生 PSO/GPU 检查 |
| `src/stores/diagnostics.ts` (modify) | PSO finding 通道 |
| `src/lib/iniRules.ts` (modify) | 追加 R008/R009/R010 metadata（fix B7） |
| `src/components/primitives/{UecmGpuMatrix,UecmHorizontalSplit,UecmKpiTile,UecmScoreTile,UecmStateBlock,UecmCodeBlock,UecmFilterChip,UecmMatrixCell,UecmKV,UecmStat,UecmStatusBadge,UecmStatusDot,UecmIcon,UecmPageHeader,UecmThemeToggle}.vue` | 13 个新 primitive |
| `src/components/modals/{PsoCollectWizard,PsoDistributeWizard,HealthCheckWizard,IniScanWizard}.vue` | 4 个新 wizard（HealthCheckWizard / IniScanWizard 是 plan-4 升级版） |
| `src/components/pso/{PsoFileExplorer,PsoJobCard}.vue` | PSO 视图组件 |
| `src/components/diagnostics/{FindingHierarchy,HealthMatrix}.vue` | 诊断视图组件 |
| `src/views/{Dashboard,PSOCache,HealthCheck,INIScanner}.vue` | 4 个 view 重写 |

---

## Phase A — Plan 5 整合 + Fix

### Task A0: 准备 worktree 与新分支

**Files:**
- Create: 工作目录 `/Users/bip.lan/.config/superpowers/worktrees/ue-cache-manager/codex-plan-5-integrated/`
- Create: 分支 `codex/plan-5-integrated`（从本地 `main`）

- [ ] **Step 1: fetch latest**

```bash
cd /Users/bip.lan/AIWorkspace/vp/ue-cache-manager
git fetch origin
git fetch --all
```

- [ ] **Step 2: 用 worktree 拉新分支（隔离 main 工作目录）**

```bash
git worktree add /Users/bip.lan/.config/superpowers/worktrees/ue-cache-manager/codex-plan-5-integrated -b codex/plan-5-integrated main
```

Expected: 出现新目录 `/Users/bip.lan/.config/superpowers/worktrees/ue-cache-manager/codex-plan-5-integrated`，HEAD 指 `a5f1814`。

- [ ] **Step 3: 切到 worktree 验证状态**

```bash
cd /Users/bip.lan/.config/superpowers/worktrees/ue-cache-manager/codex-plan-5-integrated
git status
git log --oneline -3
```

Expected: clean working tree, HEAD 是 main 的 `a5f1814`。

---

### Task A1: 复制 plan-5 专属新文件（不动共享文件）

**Files:** 一次性 `git checkout codex/plan-5-ddc-pak -- <paths>`

- [ ] **Step 1: 复制 Rust 后端新模块**

```bash
cd /Users/bip.lan/.config/superpowers/worktrees/ue-cache-manager/codex-plan-5-integrated
git checkout codex/plan-5-ddc-pak -- \
  src-tauri/src/data/operations.rs \
  src-tauri/src/data/projects.rs \
  src-tauri/src/data/project_locations.rs \
  src-tauri/src/core/project_identity.rs \
  src-tauri/src/core/project_discovery.rs \
  src-tauri/src/core/ue_runner.rs \
  src-tauri/src/core/ddc_pak.rs \
  src-tauri/src/core/pak_distribute.rs \
  src-tauri/src/commands/projects.rs \
  src-tauri/src/commands/ddc_pak.rs
```

- [ ] **Step 2: 复制 PowerShell 脚本（先复制原版，后面 task 修 bug）**

```bash
git checkout codex/plan-5-ddc-pak -- \
  ps-scripts/discover-uprojects.ps1 \
  ps-scripts/start-ue-process.ps1 \
  ps-scripts/tail-ue-log.ps1 \
  ps-scripts/stop-ue-process.ps1 \
  ps-scripts/generate-ddc-pak.ps1 \
  ps-scripts/verify-pak-output.ps1 \
  ps-scripts/distribute-pak-file.ps1
```

- [ ] **Step 3: 复制前端新模块**

```bash
git checkout codex/plan-5-ddc-pak -- \
  src/stores/projects.ts \
  src/stores/ddcPak.ts \
  src/components/primitives/UecmProgressBar.vue \
  src/components/primitives/UecmTaskCard.vue \
  src/components/primitives/UecmPathInput.vue \
  src/components/modals/ProjectDiscoveryWizard.vue \
  src/components/modals/ProjectMatchingModal.vue \
  src/components/modals/DdcPakWizard.vue \
  src/components/ddcpak/PakJobCard.vue \
  src/components/ddcpak/DistributeProgressTable.vue \
  src/views/Projects.vue \
  src/views/DDCPak.vue
```

- [ ] **Step 4: 复制 11 个前端测试**

```bash
git checkout codex/plan-5-ddc-pak -- \
  src/__tests__/DDCPak-view.spec.ts \
  src/__tests__/DdcPakWizard.spec.ts \
  src/__tests__/DistributeProgressTable.spec.ts \
  src/__tests__/PakJobCard.spec.ts \
  src/__tests__/ProjectDiscoveryWizard.spec.ts \
  src/__tests__/ProjectMatchingModal.spec.ts \
  src/__tests__/Projects-view.spec.ts \
  src/__tests__/UecmPathInput.spec.ts \
  src/__tests__/UecmProgressBar.spec.ts \
  src/__tests__/UecmTaskCard.spec.ts \
  src/__tests__/projects-store.spec.ts \
  src/__tests__/ddc-pak-store.spec.ts
```

- [ ] **Step 5: 验证文件落位**

```bash
git status --short | head -50
```

Expected: 大量 `A` (added) 行覆盖上面所有路径，无 `M` (modified)。

---

### Task A2: 合并 schema.rs（追加 008/009/010）

**Files:** Modify `src-tauri/src/data/schema.rs`

- [ ] **Step 1: 读 plan-5 schema 中 008-010 三条迁移的 SQL**

```bash
git show codex/plan-5-ddc-pak:src-tauri/src/data/schema.rs > /tmp/plan5-schema.rs
grep -n '"008_\|"009_\|"010_' /tmp/plan5-schema.rs
```

- [ ] **Step 2: 在 main 的 `schema.rs` MIGRATIONS 数组里、007 之后追加三条**

打开 `src-tauri/src/data/schema.rs`，找到 `"007_diagnostics_tables"` 那一项的闭合 `),`，在之后插入 plan-5 的 008/009/010 三条（从 `/tmp/plan5-schema.rs` 复制对应 tuple，确保 SQL 内容完整）。

- [ ] **Step 3: 编译验证**

```bash
cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | tail -30
```

Expected: 编译可能因 mod.rs 还没声明而失败，但 schema.rs 本身的语法应该过。如果是 schema.rs 内部语法错（unterminated string、tuple 不匹配）必须修。

---

### Task A3: 合并 data/mod.rs

**Files:** Modify `src-tauri/src/data/mod.rs`

- [ ] **Step 1: 看 plan-5 的 data/mod.rs**

```bash
git show codex/plan-5-ddc-pak:src-tauri/src/data/mod.rs
```

- [ ] **Step 2: 在 main 的 `src-tauri/src/data/mod.rs` 中追加**

```rust
pub mod operations;
pub mod projects;
pub mod project_locations;
```

放在现有 `pub mod` 列表末尾、`schema` 之前的合适位置。

---

### Task A4: 合并 core/mod.rs

**Files:** Modify `src-tauri/src/core/mod.rs`

- [ ] **Step 1: 在 main 的 `src-tauri/src/core/mod.rs` 中追加**

```rust
pub mod project_identity;
pub mod project_discovery;
pub mod ue_runner;
pub mod ddc_pak;
pub mod pak_distribute;
```

放在现有 `pub mod loopback;` 等条目之后。

---

### Task A5: 合并 commands/mod.rs

**Files:** Modify `src-tauri/src/commands/mod.rs`

- [ ] **Step 1: 追加**

```rust
pub mod projects;
pub mod ddc_pak;
```

---

### Task A6: 合并 lib.rs（setup + invoke_handler）

**Files:** Modify `src-tauri/src/lib.rs`

- [ ] **Step 1: setup block 追加 UeJobRegistry**

在现有 `app.manage(db);` 之后插入：

```rust
            app.manage(commands::ddc_pak::UeJobRegistry::default());
```

- [ ] **Step 2: invoke_handler 追加 plan-5 的 11 个命令**

在 `commands::shares::delete_share,` 之后、`commands::system::test_powershell_bridge,` 之前插入：

```rust
            commands::projects::list_projects,
            commands::projects::list_project_locations,
            commands::projects::discover_projects,
            commands::projects::set_project_location,
            commands::projects::delete_project,
            commands::projects::delete_project_location,
            commands::projects::create_project_manual,
            commands::ddc_pak::generate_ddc_pak,
            commands::ddc_pak::cancel_ue_job,
            commands::ddc_pak::verify_pak_output,
            commands::ddc_pak::distribute_ddc_pak,
```

- [ ] **Step 3: 编译验证**

```bash
cd /Users/bip.lan/.config/superpowers/worktrees/ue-cache-manager/codex-plan-5-integrated
cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | tail -50
```

Expected: 全部编译通过，无 unresolved import / undefined symbol。如果有错，回到 A2/A3/A4/A5 检查模块声明。

---

### Task A7: 合并 services/tauri.ts、primitives/index.ts

**Files:** Modify `src/services/tauri.ts`、`src/components/primitives/index.ts`

- [ ] **Step 1: 拿 plan-5 版的 services/tauri.ts**

```bash
git show codex/plan-5-ddc-pak:src/services/tauri.ts > /tmp/plan5-tauri.ts
diff src/services/tauri.ts /tmp/plan5-tauri.ts | head -100
```

- [ ] **Step 2: 把 plan-5 新增的类型与 invoke 包装追加到 main 的 services/tauri.ts**

由于 main 已经有 plan-4 的内容，不能直接覆盖。手动把 plan-5 新增段（projects-related: `ProjectRow`, `ProjectLocationRow`, `discoverProjects`, `setProjectLocation` 等；DDC pak related: `UeJobStartResponse`, `DistributeJobResponse`, `generateDdcPak`, `cancelUeJob`, `distributeDdcPak`, etc.）追加到文件末尾。

- [ ] **Step 3: primitives/index.ts 追加 3 个导出**

```bash
git show codex/plan-5-ddc-pak:src/components/primitives/index.ts
```

把 `UecmProgressBar`、`UecmTaskCard`、`UecmPathInput` 三个 export 追加到 main 的 `src/components/primitives/index.ts`。

---

### Task A8: Fix B2 — `stop-ue-process.ps1` `$Pid` → `$TargetPid`

**Files:** Modify `ps-scripts/stop-ue-process.ps1`

- [ ] **Step 1: 修改 param 块**

把第 3 行：

```powershell
    [Parameter(Mandatory=$true)] [int]$Pid,
```

改成：

```powershell
    [Parameter(Mandatory=$true)] [int]$TargetPid,
```

- [ ] **Step 2: 修改 ArgumentList 引用**

把 ArgumentList 块里：

```powershell
        ArgumentList = @($Pid)
```

改成：

```powershell
        ArgumentList = @($TargetPid)
```

- [ ] **Step 3: 验证 grep 无残留 `$Pid`**

```bash
grep -n '\$Pid' ps-scripts/stop-ue-process.ps1
```

Expected: 输出为空（注意 `$PID` 大写自动变量与 `$Pid` 是同一个，grep 不区分）。如果还有，说明漏改。

- [ ] **Step 4: 调用方同步**

```bash
grep -rn '\.ps1.*-Pid\|"--Pid"\|"Pid"' src-tauri/src/core/ue_runner.rs
```

如果 ue_runner.rs 用 `-Pid` 当参数名调用，改成 `-TargetPid`。

---

### Task A9: Fix B3 + M1 — `distribute-pak-file.ps1` `$args` 重命名 + robocopy `/E`

**Files:** Modify `ps-scripts/distribute-pak-file.ps1`

- [ ] **Step 1: `$args` → `$roboArgs`**

把脚本内 ScriptBlock 中的：

```powershell
            $args = @(
                "$SourceUnc",
                "$TargetLocal",
                "*.ddp",
                '/MIR',
                '/R:3',
                '/W:5',
                '/NP',
                '/NDL',
                '/NJH',
                '/NJS',
                '/BYTES'
            )
```

改成：

```powershell
            $roboArgs = @(
                "$SourceUnc",
                "$TargetLocal",
                "*.ddp",
                '/E',
                '/R:3',
                '/W:5',
                '/NP',
                '/NDL',
                '/NJH',
                '/NJS',
                '/BYTES'
            )
```

- [ ] **Step 2: 同步 Start-Process 引用**

```powershell
            $proc = Start-Process -FilePath 'robocopy.exe' -ArgumentList $roboArgs -PassThru -Wait -NoNewWindow -RedirectStandardOutput $stdoutPath -RedirectStandardError $stderrPath
```

- [ ] **Step 3: 验证**

```bash
grep -n '\$args\|/MIR' ps-scripts/distribute-pak-file.ps1
```

Expected: 输出为空。

---

### Task A10: Fix M2 — `tail-ue-log.ps1` `[int]` → `[long]`

**Files:** Modify `ps-scripts/tail-ue-log.ps1`

- [ ] **Step 1: param 块**

把：

```powershell
    [Parameter(Mandatory=$true)] [int]$LastReadOffset,
    [int]$MaxBytes = 65536,
```

改成：

```powershell
    [Parameter(Mandatory=$true)] [long]$LastReadOffset,
    [int]$MaxBytes = 65536,
```

- [ ] **Step 2: ScriptBlock param**

把：

```powershell
        param($LogPath, $LastReadOffset, $MaxBytes)
```

下面的隐式类型也补一下：

```powershell
        param($LogPath, [long]$LastReadOffset, [int]$MaxBytes)
```

- [ ] **Step 3: Rust 端核对**

```bash
grep -n 'LastReadOffset\|last_read_offset' src-tauri/src/core/ue_runner.rs
```

确认 Rust 用 `i64` 传，参数名是 `-LastReadOffset` 大小写一致。

---

### Task A11: Fix M3 — NTLM loopback self-target 短路（核心新功能）

**Files:** Modify `src-tauri/src/core/ue_runner.rs`、`src-tauri/src/core/pak_distribute.rs`、`src-tauri/src/commands/ddc_pak.rs`

Background: main 的 `core::loopback` 模块已经提供 `is_loopback_target(target: &str) -> bool` 函数（127.x / ::1 / localhost / hostname / NIC IP 全识别）。Plan 5 的 `ue_runner` / `pak_distribute` / `ddc_pak` 都没用，self-target 时会走 WinRM 自连撞 NTLM loopback 拦截。

- [ ] **Step 1: 确认 loopback 模块 API**

```bash
grep -n 'pub fn is_loopback_target' src-tauri/src/core/loopback.rs
```

记下函数签名。

- [ ] **Step 2: `ue_runner.rs` `start_remote_process` 入口短路**

定位 `start_remote_process` 函数（约 257 行附近）。在函数体最开头插入：

```rust
    if crate::core::loopback::is_loopback_target(&host) {
        tracing::debug!(host = %host, "ue_runner: target is local, short-circuit to local backend");
        return start_local_process(...).await;  // 用同样的参数转发
    }
```

注意：`start_local_process` 的参数列表必须与 remote 版本兼容；如果不兼容，封装一个 helper。具体参数对照 plan-5 原版的 `start_local_process` 签名。

- [ ] **Step 3: `read_tail` 同样短路**

定位 `read_tail`，加同样的 self-target 检测，short-circuit 到本地文件读取（`std::fs::File` + `seek` + `read`）。如果 plan-5 没有 local 版本，新写一个 `read_tail_local(log_path, last_offset, max_bytes) -> TailResult`。

- [ ] **Step 4: `stop_remote_process` 同样短路**

self-target 时调 `stop_local_process`（已存在）。

- [ ] **Step 5: `pak_distribute.rs::run_one` 入口短路**

定位 `run_one`（约 200 行附近）。在 PowerShell 调用前判断 `if crate::core::loopback::is_loopback_target(&plan.target_host)`，true 时直接调本地 robocopy（用 `std::process::Command::new("robocopy.exe")`），跳过 PSCredential / Invoke-Command。

- [ ] **Step 6: `commands/ddc_pak.rs::generate_ddc_pak` 入口检测 source**

在函数体最开头，根据 `source_machine_id` 查 machine 表拿到 hostname/IP，判断 `is_loopback_target`，true 时强制设 `backend = UeBackend::Local` 并跳过凭证查询。

- [ ] **Step 7: 单测**

在 `core/ue_runner.rs` 末尾追加：

```rust
#[cfg(test)]
mod loopback_tests {
    use super::*;

    #[test]
    fn local_host_short_circuits() {
        assert!(crate::core::loopback::is_loopback_target("127.0.0.1"));
        assert!(crate::core::loopback::is_loopback_target("localhost"));
    }

    #[test]
    fn remote_host_does_not_short_circuit() {
        assert!(!crate::core::loopback::is_loopback_target("192.168.10.20"));
    }
}
```

- [ ] **Step 8: cargo test 验证**

```bash
cargo test --manifest-path src-tauri/Cargo.toml -- core::ue_runner::loopback_tests 2>&1 | tail -20
```

Expected: 2 个测试全过。

---

### Task A12: Build + Test 验证

**Files:** 无

- [ ] **Step 1: cargo check 全项目**

```bash
cd /Users/bip.lan/.config/superpowers/worktrees/ue-cache-manager/codex-plan-5-integrated
cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | tail -30
```

Expected: `Finished ... in N.NNs`，无错误。warnings 可接受。

- [ ] **Step 2: cargo test 全项目**

```bash
cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -50
```

Expected: 所有测试 PASS。注意 ue_runner 在 mac 上有 `#[cfg(not(windows))]` 测试桩，会跑通。

- [ ] **Step 3: pnpm install + build**

```bash
pnpm install --frozen-lockfile
pnpm build 2>&1 | tail -30
```

Expected: vite build 成功，dist/ 产出。

- [ ] **Step 4: pnpm test**

```bash
pnpm test 2>&1 | tail -50
```

Expected: 所有 spec PASS。如果某个 spec 因为缺 mock 失败，对应修。

- [ ] **Step 5: 如果上面任意一步失败，停下汇报，不要硬上 commit**

---

### Task A13: Squash commit + local branch status

**Files:** 无

- [ ] **Step 1: 看完整 diff**

```bash
git diff main --stat
git diff main | head -100
```

Expected: stat 显示约 49 文件、~5000+ 行（含 fix 增量，比原 plan-5 略多）。

- [ ] **Step 2: stage 全部（注意排除任何 IDE/runtime 文件）**

```bash
git status --short
```

确认列表里只有预期路径。如有 `.DS_Store` / `.idea/` 等 add 进来就 reset。

```bash
git add -A
git status --short
```

- [ ] **Step 3: commit**

```bash
git commit -m "$(cat <<'EOF'
feat(plan-5): integrate DDC pak workflow with NTLM loopback fix

Integrates Plan 5 (DDC Pak) on top of main (post-Plan-4) and fixes
review blockers + key major findings before landing.

Plan 5 capabilities (per Docs/superpowers/plans/2026-05-03-uecm-plan-5-ddc-pak.md):
- Project identity / discovery (uproject scanner + 3-tier matcher)
- UE process runner (local + remote backends, log tail, watchdog)
- DDC pak generation (UE -run=DerivedDataCache pipeline)
- Robocopy fan-out distribution to N target machines
- Schema migrations 008/009/010 (operations, projects, project_locations)
- 11 Tauri commands, 3 wizards, 3 primitives, 11 frontend specs

Review fixes applied:
- B2 stop-ue-process.ps1: $Pid → $TargetPid (PowerShell auto-var collision)
- B3 distribute-pak-file.ps1: $args → $roboArgs (auto-var collision)
- M1 distribute-pak-file.ps1: robocopy /MIR → /E (avoid deleting target-only files)
- M2 tail-ue-log.ps1: [int]$LastReadOffset → [long] (avoid 2GB overflow)
- M3 NTLM loopback self-target short-circuit:
    * core/ue_runner.rs: start/read/stop short-circuit to local backend when host is local
    * core/pak_distribute.rs: bypass WinRM when target is self
    * commands/ddc_pak.rs: force UeBackend::Local when source machine is self

Deferred (tracked for follow-up):
- M4 admin_share_ddc_unc duplicate suffix normalization
- M5/M6 ue_runner PID parse / read_tail failure cap
- M7 operations source/target machine bookkeeping
- M8 robocopy stdout_tail propagation on failure
- M9 verify_pak_output sync → async
- M10 ProjectMatchingModal partial-state guard
- M11 generate-ddc-pak.ps1 rename
- T22 lanPC E2E (Phase C of merge plan)

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

- [ ] **Step 4: 记录 local branch status（不 push）**

```bash
git status --short --branch
git log --oneline -1
```

- [ ] **Step 5: 继续本地 merge**

本轮用户已授权执行，不在这里停下；继续 A14 做本地 merge。

---

### Task A14: Merge 到 main（本轮已授权）

**Files:** 无

- [ ] **Step 1: 确认本轮授权**

用户本轮已要求执行完整 plan；继续合入本地 `main`。

- [ ] **Step 2: 切回主 worktree 的 main**

```bash
cd /Users/bip.lan/AIWorkspace/vp/ue-cache-manager
git checkout main
git status --short
```

- [ ] **Step 3: fast-forward merge（保留独立 feature commit，不创建 merge commit）**

```bash
git merge --ff-only codex/plan-5-integrated
```

- [ ] **Step 4: 记录 main status（不 push）**

```bash
git status --short --branch
```

- [ ] **Step 5: 清理 worktree**

```bash
git worktree remove /Users/bip.lan/.config/superpowers/worktrees/ue-cache-manager/codex-plan-5-integrated
```

---

## Phase B — Plan 6 整合 + Fix（基于已 merge 完 plan-5 的 main）

### Task B0: 准备 worktree 与新分支

**Files:**
- Create: 工作目录 `/Users/bip.lan/.config/superpowers/worktrees/ue-cache-manager/codex-plan-6-integrated/`
- Create: 分支 `codex/plan-6-integrated`（从已合 plan-5 的本地 `main`）

- [ ] **Step 1: fetch & worktree**

```bash
cd /Users/bip.lan/AIWorkspace/vp/ue-cache-manager
git fetch origin
git worktree add /Users/bip.lan/.config/superpowers/worktrees/ue-cache-manager/codex-plan-6-integrated -b codex/plan-6-integrated main
cd /Users/bip.lan/.config/superpowers/worktrees/ue-cache-manager/codex-plan-6-integrated
git log --oneline -3
```

Expected: HEAD 是 Phase A commit。

---

### Task B1: 复制 plan-6 专属新文件

- [ ] **Step 1: 后端新模块**

```bash
git checkout codex/plan-6-pso-cache -- \
  src-tauri/src/data/pso_cache_files.rs \
  src-tauri/src/data/pso_distributions.rs \
  src-tauri/src/core/gpu_consistency.rs \
  src-tauri/src/core/pso_collect.rs \
  src-tauri/src/core/pso_distribute.rs \
  src-tauri/src/commands/gpu_consistency.rs \
  src-tauri/src/commands/pso.rs
```

- [ ] **Step 2: PowerShell 脚本**

```bash
git checkout codex/plan-6-pso-cache -- \
  ps-scripts/list-pso-cache-files.ps1 \
  ps-scripts/distribute-pso-cache.ps1
```

- [ ] **Step 3: 前端新模块**

```bash
git checkout codex/plan-6-pso-cache -- \
  src/stores/pso.ts \
  src/stores/gpuConsistency.ts \
  src/components/primitives/UecmGpuMatrix.vue \
  src/components/primitives/UecmHorizontalSplit.vue \
  src/components/primitives/UecmKpiTile.vue \
  src/components/primitives/UecmScoreTile.vue \
  src/components/primitives/UecmStateBlock.vue \
  src/components/primitives/UecmCodeBlock.vue \
  src/components/primitives/UecmFilterChip.vue \
  src/components/primitives/UecmMatrixCell.vue \
  src/components/primitives/UecmKV.vue \
  src/components/primitives/UecmStat.vue \
  src/components/primitives/UecmStatusBadge.vue \
  src/components/primitives/UecmStatusDot.vue \
  src/components/primitives/UecmIcon.vue \
  src/components/primitives/UecmPageHeader.vue \
  src/components/primitives/UecmThemeToggle.vue \
  src/components/modals/PsoCollectWizard.vue \
  src/components/modals/PsoDistributeWizard.vue \
  src/components/pso/PsoFileExplorer.vue \
  src/components/pso/PsoJobCard.vue \
  src/components/diagnostics/FindingHierarchy.vue \
  src/components/diagnostics/HealthMatrix.vue
```

- [ ] **Step 4: 修改后的 view + wizard（plan-6 升级版）**

```bash
git checkout codex/plan-6-pso-cache -- \
  src/components/modals/HealthCheckWizard.vue \
  src/components/modals/IniScanWizard.vue \
  src/views/Dashboard.vue \
  src/views/PSOCache.vue \
  src/views/HealthCheck.vue \
  src/views/INIScanner.vue
```

注意：HealthCheckWizard / IniScanWizard / HealthCheck.vue / INIScanner.vue 在 main 上已存在（plan-4）；这里 overwrite 成 plan-6 升级版。
保留 commit message 里说明这是 plan-6 升级版本。

- [ ] **Step 5: 验证**

```bash
git status --short | head -50
```

Expected: A 标记新文件 + M 标记 view/wizard overwrite。

---

### Task B2: schema 011/012 + 共享文件合并

**Files:** Modify `src-tauri/src/data/schema.rs`、`data/mod.rs`、`core/mod.rs`、`commands/mod.rs`、`lib.rs`、`primitives/index.ts`、`services/tauri.ts`、`stores/healthCheck.ts`、`stores/diagnostics.ts`、`lib/healthChecks.ts`、`stores/cluster.ts`、`views/DDCPak.vue`

- [ ] **Step 1: schema.rs 追加 011/012**

```bash
git show codex/plan-6-pso-cache:src-tauri/src/data/schema.rs > /tmp/plan6-schema.rs
grep -n '"011_\|"012_' /tmp/plan6-schema.rs
```

把对应两条 tuple 追加到当前 `schema.rs` 末尾（010 后）。

- [ ] **Step 2: data/mod.rs 追加**

```rust
pub mod pso_cache_files;
pub mod pso_distributions;
```

- [ ] **Step 3: core/mod.rs 追加**

```rust
pub mod gpu_consistency;
pub mod pso_collect;
pub mod pso_distribute;
```

- [ ] **Step 4: commands/mod.rs 追加**

```rust
pub mod gpu_consistency;
pub mod pso;
```

- [ ] **Step 5: lib.rs invoke_handler 追加**

在 `commands::ddc_pak::distribute_ddc_pak,` 之后插入：

```rust
            commands::pso::start_pso_collection,
            commands::pso::list_pso_cache_files,
            commands::pso::distribute_pso_cache,
            commands::gpu_consistency::get_gpu_consistency_matrix,
            commands::ini_scanner::verify_pso_precaching,
```

- [ ] **Step 6: ini_scanner 命令追加 verify_pso_precaching**

从 plan-6 拿到 `verify_pso_precaching` 的实现追加到 `commands/ini_scanner.rs`：

```bash
git show codex/plan-6-pso-cache:src-tauri/src/commands/ini_scanner.rs > /tmp/plan6-ini-scanner.rs
diff src-tauri/src/commands/ini_scanner.rs /tmp/plan6-ini-scanner.rs | head -100
```

把 plan-6 新增的 `verify_pso_precaching` 函数（含 `#[tauri::command]` 注解）追加到当前文件末尾。注意保持 use 导入完整。

- [ ] **Step 7: ini_diagnostics 追加 R008/R009/R010**

```bash
git show codex/plan-6-pso-cache:src-tauri/src/core/ini_diagnostics.rs > /tmp/plan6-ini-diag.rs
diff src-tauri/src/core/ini_diagnostics.rs /tmp/plan6-ini-diag.rs | head -150
```

把新增的 3 条规则（`pso_cvar_rule` 系列）和 5 个相关单测追加到当前 `ini_diagnostics.rs`。

- [ ] **Step 8: primitives/index.ts 追加 13 个导出**

```bash
git show codex/plan-6-pso-cache:src/components/primitives/index.ts
```

整体替换为 plan-6 版本（包含 plan-5 的 3 个 + plan-6 的 13 个）。

- [ ] **Step 9: services/tauri.ts 追加 pso/gpu 类型与 invoke**

```bash
git show codex/plan-6-pso-cache:src/services/tauri.ts > /tmp/plan6-tauri.ts
diff src/services/tauri.ts /tmp/plan6-tauri.ts | head -200
```

把 plan-6 新增段（`PsoCacheFile`, `GpuMatrix`, `startPsoCollection`, `listPsoCacheFiles`, `distributePsoCache`, `getGpuConsistencyMatrix`, `verifyPsoPrecaching` 等）追加到末尾。

- [ ] **Step 10: stores/healthCheck.ts overwrite + diagnostics.ts overwrite + cluster.ts diff**

healthCheck 与 diagnostics 改动较大，直接 overwrite：

```bash
git checkout codex/plan-6-pso-cache -- \
  src/stores/healthCheck.ts \
  src/stores/diagnostics.ts \
  src/lib/healthChecks.ts \
  src/lib/iniRules.ts \
  src/stores/cluster.ts \
  src/views/DDCPak.vue
```

注意 `views/DDCPak.vue` 在 plan-5 已经写过；plan-6 只 +5 行小调整，overwrite 安全。

- [ ] **Step 11: 编译验证**

```bash
cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | tail -30
```

Expected: 通过。如果 ini_scanner.rs 因 verify_pso_precaching 用了不存在的 helper 失败，回 Step 6 检查。

---

### Task B3: Fix B7 — `iniRules.ts` 加 R008/R009/R010 metadata

**Files:** Modify `src/lib/iniRules.ts`

> 注意：B2 Step 10 已经 overwrite 了 `iniRules.ts`，这里检查是否包含 R008/R009/R010；如果 plan-6 分支本身就缺，按下面补。

- [ ] **Step 1: 检查**

```bash
grep -n 'R008\|R009\|R010' src/lib/iniRules.ts
```

如果 0 条命中，说明 plan-6 分支前端 metadata 没补。继续 Step 2-3。

- [ ] **Step 2: 补三条**

打开 `src/lib/iniRules.ts`，找到 `INI_RULES` map 定义。在末尾追加：

```typescript
  R008: {
    title: 'PSO precaching enabled',
    tone: 'critical',
    description: 'r.PSOPrecaching must be 1 to enable PSO precaching pipeline.',
  },
  R009: {
    title: 'PSO cache load enabled',
    tone: 'critical',
    description: 'r.ShaderPipelineCache.Enabled must be 1 for PSO cache to load at runtime.',
  },
  R010: {
    title: 'PSO log verbosity',
    tone: 'warning',
    description: 'LogShaderPipelineCache=Display recommended for PSO collection diagnostics.',
  },
```

具体 title / tone / description 文案按 `core/ini_diagnostics.rs` 里 R008/R009/R010 的 message 内容对照写。

- [ ] **Step 3: 验证 type check**

```bash
pnpm exec tsc --noEmit 2>&1 | tail -20
```

Expected: 0 errors。

---

### Task B4: Fix B5 — collect 错误传递

**Files:** Modify `src-tauri/src/commands/pso.rs`、`src/stores/pso.ts`

- [ ] **Step 1: 定位问题代码**

```bash
grep -n 'pso-collect-finalized\|enumerate_remote\|finalize_persist' src-tauri/src/commands/pso.rs
```

找到 collect 后台任务里 `if let Ok(files) = pso_collect::enumerate_remote(...)` 那一段（plan-6 review 报告 172-194 行附近）。

- [ ] **Step 2: 改写错误分支**

把：

```rust
if let Ok(files) = pso_collect::enumerate_remote(&app_handle, &source_machine, &project_location, &credential).await {
    if let Ok(()) = pso_collect::finalize_persist(&db, project_id, source_machine_id, &files).await {
        let _ = app_handle.emit("pso-collect-finalized", FinalizedPayload {
            job_id: job_id.clone(),
            files_collected: files.len() as u64,
            error_message: None,
        });
    }
}
```

改成：

```rust
match pso_collect::enumerate_remote(&app_handle, &source_machine, &project_location, &credential).await {
    Ok(files) => {
        match pso_collect::finalize_persist(&db, project_id, source_machine_id, &files).await {
            Ok(()) => {
                let _ = app_handle.emit("pso-collect-finalized", FinalizedPayload {
                    job_id: job_id.clone(),
                    files_collected: Some(files.len() as u64),
                    error_message: None,
                });
            }
            Err(err) => {
                tracing::error!(?err, "pso finalize_persist failed");
                let _ = app_handle.emit("pso-collect-finalized", FinalizedPayload {
                    job_id: job_id.clone(),
                    files_collected: None,
                    error_message: Some(format!("persist failed: {err}")),
                });
            }
        }
    }
    Err(err) => {
        tracing::error!(?err, "pso enumerate_remote failed");
        let _ = app_handle.emit("pso-collect-finalized", FinalizedPayload {
            job_id: job_id.clone(),
            files_collected: None,
            error_message: Some(format!("enumerate failed: {err}")),
        });
    }
}
```

- [ ] **Step 3: 修改 `FinalizedPayload` struct**

定位 struct（同一文件靠前）。把：

```rust
struct FinalizedPayload {
    job_id: String,
    files_collected: u64,
    error_message: Option<String>,
}
```

改成：

```rust
struct FinalizedPayload {
    job_id: String,
    files_collected: Option<u64>,
    error_message: Option<String>,
}
```

- [ ] **Step 4: 前端 store 同步**

打开 `src/stores/pso.ts`，定位 `onFinalized` 回调（找 `pso-collect-finalized`）：

```typescript
listen('pso-collect-finalized', (event) => {
    const payload = event.payload as { job_id: string; files_collected: number; error_message: string | null };
    const job = jobs.value.find(j => j.job_id === payload.job_id);
    if (job) {
        job.status = 'completed';
        job.files_collected = payload.files_collected;
    }
});
```

改成：

```typescript
listen('pso-collect-finalized', (event) => {
    const payload = event.payload as { job_id: string; files_collected: number | null; error_message: string | null };
    const job = jobs.value.find(j => j.job_id === payload.job_id);
    if (!job) return;
    if (payload.error_message) {
        job.status = 'failed';
        job.error = payload.error_message;
    } else {
        job.status = 'completed';
        job.files_collected = payload.files_collected ?? 0;
    }
});
```

- [ ] **Step 5: store interface 加 `error` 字段（如果没有）**

确认 `PsoCollectJob` interface 有 `error?: string`。没有就补。

- [ ] **Step 6: cargo check + tsc**

```bash
cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | tail -10
pnpm exec tsc --noEmit 2>&1 | tail -10
```

Expected: 通过。

---

### Task B5: Fix B6 — `distributed_at` 时间戳格式

**Files:** Modify `src-tauri/src/commands/pso.rs`

- [ ] **Step 1: 定位**

```bash
grep -n 'distributed_at\|now_millis' src-tauri/src/commands/pso.rs
```

找到 `now_millis().to_string()` 那一行（约 411-418）。

- [ ] **Step 2: 替换**

把：

```rust
distributed_at: Some(now_millis().to_string()),
```

改成：

```rust
distributed_at: Some(chrono::Utc::now().to_rfc3339()),
```

- [ ] **Step 3: 确认 `chrono` 已在 Cargo.toml**

```bash
grep -n 'chrono' src-tauri/Cargo.toml
```

如果没有，追加：

```toml
chrono = { version = "0.4", features = ["serde"] }
```

- [ ] **Step 4: 检查同文件是否还有别的 `now_millis` 写时间字段**

```bash
grep -n 'now_millis' src-tauri/src/commands/pso.rs
```

如果用作 `job_id` 而不是时间字段，保留；如果用作时间字段，全改 `to_rfc3339`。

- [ ] **Step 5: cargo check**

```bash
cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | tail -10
```

---

### Task B6: Fix M14 — 抽 `pak_distribute` 共用框架，`pso_distribute` 改 thin wrapper

**Files:** Modify `src-tauri/src/core/pak_distribute.rs`、`src-tauri/src/core/pso_distribute.rs`

> 这是这次最大的 refactor。目标：把 `pak_distribute` 里 `plan` / `build_distribute_args` / `preflight_one` / `run_one` 的通用流程抽出来，参数化 `source_subdir`（`DerivedDataCache` vs `Saved/CollectedPSOs`）和 `file_glob`（`*.ddp` vs `*.upipelinecache`）。`pso_distribute` 改成 50 行内的 wrapper。

- [ ] **Step 1: 在 `pak_distribute.rs` 顶部新增 `DistributeProfile` struct**

```rust
#[derive(Clone, Debug)]
pub struct DistributeProfile {
    pub source_subdir: String,        // e.g. "DerivedDataCache" or "Saved/CollectedPSOs"
    pub file_glob: String,            // e.g. "*.ddp" or "*.upipelinecache"
    pub ps_script: &'static str,      // "distribute-pak-file.ps1" or "distribute-pso-cache.ps1"
}

impl DistributeProfile {
    pub fn ddc_pak() -> Self {
        Self {
            source_subdir: "DerivedDataCache".into(),
            file_glob: "*.ddp".into(),
            ps_script: "distribute-pak-file.ps1",
        }
    }

    pub fn pso_cache() -> Self {
        Self {
            source_subdir: "Saved/CollectedPSOs".into(),
            file_glob: "*.upipelinecache".into(),
            ps_script: "distribute-pso-cache.ps1",
        }
    }
}
```

- [ ] **Step 2: `plan` / `preflight_one` / `run_one` 函数签名改为接受 `&DistributeProfile`**

打开 `pak_distribute.rs`，把 `pub fn plan(...)` 改成 `pub fn plan(profile: &DistributeProfile, ...)`。`run_one` 同理。函数体里所有出现 `"DerivedDataCache"` / `"*.ddp"` / `"distribute-pak-file.ps1"` 字面量都换成 `profile.source_subdir` / `profile.file_glob` / `profile.ps_script`。

- [ ] **Step 3: 调用方 `commands/ddc_pak.rs::distribute_ddc_pak` 同步**

把所有 `pak_distribute::plan(...)` 调用改成 `pak_distribute::plan(&DistributeProfile::ddc_pak(), ...)`。

- [ ] **Step 4: 重写 `core/pso_distribute.rs`**

整体替换为：

```rust
//! PSO cache distribution. Thin wrapper over core::pak_distribute parameterized
//! with the PSO profile (Saved/CollectedPSOs subdir + *.upipelinecache glob +
//! distribute-pso-cache.ps1 sidecar).

use crate::core::pak_distribute::{self, DistributeProfile, DistributePlanItem, DistributeJobResponse};
use crate::error::UecmResult;
use rusqlite::Connection;
use tauri::AppHandle;

pub fn plan(
    conn: &Connection,
    project_id: i64,
    source_machine_id: i64,
    target_machine_ids: &[i64],
    file_ids: Option<&[i64]>,
) -> UecmResult<Vec<DistributePlanItem>> {
    pak_distribute::plan(
        &DistributeProfile::pso_cache(),
        conn,
        project_id,
        source_machine_id,
        target_machine_ids,
        file_ids,
    )
}

pub async fn run(
    app: &AppHandle,
    plan_items: Vec<DistributePlanItem>,
    job_id: String,
) -> UecmResult<DistributeJobResponse> {
    pak_distribute::run(
        &DistributeProfile::pso_cache(),
        app,
        plan_items,
        job_id,
    ).await
}
```

如果原 `pso_distribute` 有 PSO-specific 的字段（比如 `gpu_signature` 校验），保留为额外 wrapper 函数（`plan_gpu_filtered` 等）。

- [ ] **Step 5: `commands/pso.rs::distribute_pso_cache` 同步调用**

把 `pso_distribute::plan(...)` / `pso_distribute::run(...)` 调用保持不变（因为新 wrapper 签名兼容）。但要确认 emit 的事件名（M15 后续 fix，本 task 暂不动）。

- [ ] **Step 6: 删除 `pso_distribute.rs` 里重复的 `build_distribute_args` / `preflight_one` / `admin_share_pso_unc`**

确保只剩 wrapper 函数。文件应当 < 80 行。

- [ ] **Step 7: cargo check + cargo test**

```bash
cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | tail -20
cargo test --manifest-path src-tauri/Cargo.toml -- pak_distribute 2>&1 | tail -30
cargo test --manifest-path src-tauri/Cargo.toml -- pso_distribute 2>&1 | tail -30
```

Expected: 全过。如果原 plan-5 / plan-6 测试假定 pso_distribute 内部函数存在，对应迁到 pak_distribute 测试或删除（因为现在是 wrapper）。

---

### Task B7: Build + Test 验证

- [ ] **Step 1: cargo check + test**

```bash
cd /Users/bip.lan/.config/superpowers/worktrees/ue-cache-manager/codex-plan-6-integrated
cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | tail -20
cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -50
```

Expected: 全过。

- [ ] **Step 2: pnpm install + build**

```bash
pnpm install --frozen-lockfile
pnpm build 2>&1 | tail -30
```

Expected: vite build 成功。

- [ ] **Step 3: pnpm test**

```bash
pnpm test 2>&1 | tail -50
```

Expected: 全过。

- [ ] **Step 4: 任何一步失败，停下汇报**

---

### Task B8: Squash commit + local branch status

- [ ] **Step 1: 看 diff**

```bash
git diff main --stat
```

Expected: ~89 文件 / ~22000+ 行（包含 plan-6 新文件 + 几处 fix 增量；不含 lockfile）。

- [ ] **Step 2: stage + commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
feat(plan-6): integrate PSO cache + GPU consistency on top of plan-5

Integrates Plan 6 (PSO Cache + GPU Consistency Matrix + Dashboard rewrite)
on top of the just-merged Plan 5, with review blockers and the pso_distribute
refactor applied before landing.

Plan 6 capabilities (per Docs/superpowers/plans/2026-05-03-uecm-plan-6-pso-cache.md):
- PSO collection (UE -game args + watchdog + Saved/CollectedPSOs enumeration)
- PSO cache distribution (robocopy fan-out, GPU-signature filtering)
- GPU consistency matrix (per-machine signature aggregation, baseline selection)
- INI rules R008/R009/R010 (PSO precaching CVar checks)
- Dashboard rewrite + HealthCheck rewire to PSO/GPU sources
- 5 new Tauri commands, 2 wizards, 13 new primitives
- Schema migrations 011/012 (pso_cache_files, pso_distributions)

Review fixes applied:
- B5 commands/pso.rs: collect errors now emit pso-collect-finalized with
    error_message instead of being silently swallowed; frontend pso store
    surfaces failed status with error text
- B6 commands/pso.rs: distributed_at uses chrono::Utc::now().to_rfc3339()
    for parity with other ISO timestamp columns (was raw millis string)
- B7 lib/iniRules.ts: added R008/R009/R010 metadata so FindingHierarchy
    can render badge title/tone for PSO CVar findings
- M14 core/pak_distribute.rs + core/pso_distribute.rs: extracted DistributeProfile
    parameterizing source_subdir (DerivedDataCache vs Saved/CollectedPSOs) and
    file_glob (*.ddp vs *.upipelinecache); pso_distribute is now a 50-line
    wrapper, no duplicated plan/preflight/run logic

Deferred (tracked for follow-up):
- M12 GPU signature comparison normalization (trim + ascii case-insensitive)
- M13 watchdog coverage during start_remote_process hangs
- M15 distinct event name pso-distribute-progress (vs shared pak-distribute-progress)
- M16 admin_share_pso_unc UNC path support
- M17 distribute-pso-cache.ps1 /MT + /Z parity with pak script
- M18 list_pso_cache_files gpu_signature/source filter
- M19 PSOCache.vue auto-select when single project
- M20 PSO log marker parsing in pso_collect (LogShaderPipelineCache "Wrote N PSOs")
- T24 lanPC E2E (Phase C of merge plan)

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

- [ ] **Step 3: 记录 local branch status（不 push）**

```bash
git status --short --branch
git log --oneline -1
```

- [ ] **Step 4: 记录 branch status 后继续 merge**

---

### Task B9: Merge 到 main（本轮已授权）

- [ ] **Step 1: 确认本轮授权**

用户本轮已要求执行完整 plan；继续合入本地 `main`。

- [ ] **Step 2: merge**

```bash
cd /Users/bip.lan/AIWorkspace/vp/ue-cache-manager
git checkout main
git status --short
git merge --ff-only codex/plan-6-integrated
git status --short --branch
```

- [ ] **Step 3: 清理 worktree**

```bash
git worktree remove /Users/bip.lan/.config/superpowers/worktrees/ue-cache-manager/codex-plan-6-integrated
```

---

## Phase C — lanPC 真机 E2E（用户负责，本 plan 提供 checklist）

### Task C0: 部署最新构建到 lanPC

> 由用户在 mac 端打 release build，scp 推送到 lanPC，按 CLAUDE.md memory `deploy_tauri_build.md` 的注意事项操作。

- [ ] **Step 1: mac 端打 release build**

```bash
cd /Users/bip.lan/AIWorkspace/vp/ue-cache-manager
pnpm install --frozen-lockfile
pnpm tauri build --no-bundle
```

> 注意（来自 memory `deploy_tauri_build.md`）：必须 `pnpm tauri build --no-bundle`，不要直接 `cargo build --release`。

- [ ] **Step 2: 用 tar 推送（设 `COPYFILE_DISABLE=1`）**

> 注意（来自 memory `deploy_tar_appledouble.md`）：`COPYFILE_DISABLE=1` 必须设，否则 `._*` 文件污染 Tauri capabilities loader。

```bash
COPYFILE_DISABLE=1 tar -czf /tmp/uecm-build.tar.gz \
  -C src-tauri/target/release uecm.exe \
  -C /Users/bip.lan/AIWorkspace/vp/ue-cache-manager ps-scripts vendor
scp /tmp/uecm-build.tar.gz lanpc@192.168.10.20:C:/Tools/UECM/
```

在 lanPC 上解压后，`C:\Tools\UECM` 必须同时包含：

```text
uecm.exe
ps-scripts\
vendor\
```

只复制 `uecm.exe` 会导致 WinRM probe、UE registry scan、GPU WMI scan、PsExec credential injection 等 sidecar 功能失败。

(具体路径以你 lanPC 部署路径为准)

### Task C1: Plan 5 三组合 wizard 测试

- [ ] **C1.1 — Combo 1: 同 project、同 source、跨 target**
  - Project Discovery wizard: 在 lanPC 上扫 D:\Projects 应能找到至少一个 .uproject
  - 选定一个 project，run DDC Pak Wizard，source 选 lanPC（self-target），target 选 lanbipu-razer
  - Expected: UE 进程启动 → tail 日志正常 → pak 文件生成 → robocopy 推到 razer 成功
  - Critical check: source = self 时**没有** NTLM loopback 报错（M3 fix 验证）

- [ ] **C1.2 — Combo 2: 同 project、跨 source、同 target**
  - source 选 lanbipu-razer，target 选 lanPC
  - Expected: 全程通

- [ ] **C1.3 — Combo 3: 同 project、跨 source、跨 target**
  - source 选 lanbipu-razer，target 选 lanPC + 其他 Windows 机
  - Expected: 多目标并发分发，进度条独立更新

### Task C2: Plan 6 PSO collect + distribute 测试

- [ ] **C2.1 PSO Collect Wizard**
  - 选一个已经能跑的 UE project
  - 选 source = lanPC（self-target，验 M3）
  - 启动 collect → UE 以 `-game` 模式启动 → 等 PSO 生成 → 看到 `Saved/CollectedPSOs/*.upipelinecache`
  - Critical check: 如果 collect 过程报错（如路径不存在），UI **必须**显示错误信息（B5 fix 验证）

- [ ] **C2.2 PSO Distribute Wizard**
  - 选刚 collect 的 PSO 文件
  - target 选 lanbipu-razer（跨机）
  - Expected: robocopy 推 `*.upipelinecache` 成功，`pso_distributions` 表写入 ISO 时间戳（B6 fix 验证）

- [ ] **C2.3 GPU Consistency Matrix**
  - 打开 Health Check，看 #11 GPU consistency cell
  - Expected: 矩阵显示每台机器 GPU signature，baseline cell 高亮

### Task C3: INI Scanner 验 R008/R009/R010

- [ ] **C3.1 — 跑一次 INI scan 在 lanPC**
  - Expected: 如果 lanPC 的 `Engine.ini` 没设 `r.PSOPrecaching=1`，应该看到 R008 finding，且 finding 卡片显示正确的 title / tone（B7 fix 验证）

### Task C4: 烟测后回报

- [ ] **C4.1 — 用户在群里 / commit message / GitHub issue 简报每个 Combo 的结果（pass / fail / 部分通）**

---

## 回退路径

| 阶段失败 | 回退动作 |
|---|---|
| Phase A 任意 task 失败 | `cd /Users/bip.lan/.config/superpowers/worktrees/ue-cache-manager/codex-plan-5-integrated && git reset --hard main`，重新 cherry-pick / 复制；不动 main |
| Phase A merge 后发现严重问题 | `cd /Users/bip.lan/AIWorkspace/vp/ue-cache-manager && git revert <plan-5-commit>`，push 前先复核；分支 `codex/plan-5-integrated` 保留供调查 |
| Phase B 任意 task 失败 | `cd /Users/bip.lan/.config/superpowers/worktrees/ue-cache-manager/codex-plan-6-integrated && git reset --hard main`，重新做 |
| Phase B merge 后发现严重问题 | `git revert <plan-6-commit>`，push 前先复核；如需同时回退 Plan 5，先 revert Plan 6 再 revert Plan 5 |
| Phase C 真机 E2E 失败 | 不 revert（main 已合）；按失败具体内容开 fix PR 跟进 |

---

## Self-Review

**Spec coverage:**
- ✅ B1 plan-5 基线过时 → Phase A 改走"以 main 为基底重组"策略，绕过 rebase 冲突
- ✅ B2 stop-ue-process.ps1 `$Pid` → Task A8
- ✅ B3 distribute-pak-file.ps1 `$args` → Task A9
- ✅ B4 schema MIGRATIONS 002/007 缺失 → Phase A 以 main 为基（main 已有 002/007），Task A2 追加 008-010
- ✅ B5 collect 错误吞 → Task B4
- ✅ B6 distributed_at 时间戳 → Task B5
- ✅ B7 iniRules.ts R008-R010 → Task B3
- ✅ M1 robocopy /MIR → /E → Task A9
- ✅ M2 tail-ue-log.ps1 long → Task A10
- ✅ M3 NTLM loopback self-target → Task A11
- ✅ M14 pso_distribute 抽 wrapper → Task B6
- ⚠️ M4-M11、M12-M13、M15-M20 deferred → 已在 commit message 里列出 follow-up
- ✅ T22/T24 lanPC E2E → Phase C

**Placeholder scan:** 已逐项检查，所有 step 都有具体命令或代码块。Task A11 Step 2-6 的 short-circuit 逻辑给了示意代码加显式参数转发说明，避免"add appropriate handling"占位。

**Type consistency:**
- `DistributeProfile`（B6 Step 1）与 wrapper 调用（B6 Step 4-5）字段名一致
- `FinalizedPayload`（B4 Step 3）`files_collected: Option<u64>` 与前端 `number | null`（B4 Step 4）一致
- `is_loopback_target` 函数名（A11 各 step）与 `9c999b8` commit 中 `loopback.rs` 公开 API 一致（A11 Step 1 验证）
