# CLI 扫描展示增强 + Project 深扫 — 设计 Spec

- **日期**: 2026-05-24
- **状态**: 设计已批准（brainstorming 阶段），待实现计划（writing-plans）
- **关联**: CLI 全覆盖目标；feature03（ini scan）/ project 域
- **范围**: 仅 uecm-cli（CLI）。UI 版 `scan_inis` 已支持 project_roots，不在本 spec 改动范围。

---

## 1. 背景与动机

2026-05-24 在 lanPC 实测发现 uecm-cli 三处展示/能力缺口：

1. **引擎信息展示精简**：`machine refresh` 探测到完整 UE 安装（`version` + `install_path` + `is_primary`）并写 `machine_ue_installs`，但 human 模式 `✓ done` 行只回 `ue_versions:N` 计数。详细信息要另跑 `machine detail`，且默认是 pretty JSON。

2. **ini scan 只报诊断结论、不报配置实况**：扫描器是 finding-oriented —— 只持久化命中规则的 finding（`ini_findings` 表），不保存被扫文件的配置内容。operator 看不到"这个文件实际配了什么 DDC / PSO / Zen"，尤其当 0 finding 时（引擎默认 ini 没有非默认配置）什么都看不到。

3. **CLI 缺 project 深扫**：`cli::domain_ini::scan_cluster` 硬编码 `project_roots: &[]`，`ini scan --machine-ids` 只扫引擎默认 `BaseEngine.ini` + user 配置，扫不到项目级 `DefaultEngine.ini` / `ConsoleVariables.ini`（DDC/PSO/Zen 配置最集中处）。`project discover` / `project list` / `project locations` CLI 已存在，但发现的项目无法被深扫 —— 发现与扫描没有打通。

---

## 2. 横切决策

- human 模式（默认，不加 `--json`）统一**对齐表格**输出，遵循项目既有的"handler 接管 human 渲染"模式（参考 `HostProbe` 的 `winrm=✓ smb=✗`、`Finding` 的 `[severity] rule_id ...` 自定义渲染）。
- `--json`（NDJSON）**保持完整结构化、不变**，自动化消费兼容。
- 所有新增展示同时覆盖 human（表格）+ `--json`（结构化）两条路。

---

## 3. 改动 1 — 引擎信息展示

### 3.1 `machine refresh <id>`
探测完成、`Completed` 事件之前，human 模式向 **stdout** 输出 UE installs 对齐表格：

```
VERSION  PRIMARY  INSTALL PATH
5.0               C:\Program Files\Epic Games\UE_5.0
5.4      *        C:\Program Files\Epic Games\UE_5.4
5.8               C:\Program Files\Epic Games\UE_5.8
```

`Completed.summary` 保持现有计数字段（`authenticated / gpus / latency_ms / machine_id / ue_versions`）不变 —— 不破坏自动化。

### 3.2 `machine detail <id>`
human 模式接管 `emit_value` 渲染，输出三块对齐表格：
- **Machine**：hostname / ip / last_seen 等
- **UE installs**：version / primary（`*`）/ install_path
- **GPUs**：model / driver_version / vram

`--json` 保持 `{machine, ue_installs, gpus}` 原结构。

### 3.3 数据 / schema
无新数据、无 schema 改动。纯展示层（数据早已在 `machine_ue_installs` / `machine_gpus`）。

---

## 4. 改动 2 — INI 配置快照（DDC / PSO / Zen 实况）

### 4.1 抓取范围（已核对源码）

扫描器读文件时本就已 parse 成 sections/keys，新增一步"提取关注配置"，按 domain：

| domain | 抓取目标 | 来源核对 |
|---|---|---|
| `ddc` | `[DerivedDataBackendGraph]` section 全部 key（Root / Local / Shared / Pak / Cloud 等后端图节点） | `ini_backend_graph` / `BackendGraphAction` 操作的 section |
| `pso` | 名字匹配 `r.PSOPrecaching` / `r.PSOPrecache.*` / `r.ShaderPipelineCache.*` 的 cvar（跨 section） | `ini_diagnostics.rs` R008 `r.PSOPrecaching`、R009 `r.PSOPrecache.Compile`、R010 `r.PSOPrecache.GlobalShaders`、R024 `r.ShaderPipelineCache.Enabled` |
| `zen` | `[InstalledDerivedDataBackendGraph]` section 全部 key（`ZenShared` / `Shared` / Root 等节点） | `ini_diagnostics_zen.rs`（ZenShared / InstalledDerivedDataBackendGraph / legacy Shared） |

**设计原则**：抓"整个关注 section 的所有 key"（DDC/Zen 是 section-based）+ "匹配前缀的所有 cvar"（PSO 是 cvar-based），而非预定义每个 key —— UE 版本演进导致 key 变化时也能如实抓到实际配置。**不抓关注 section 之外的全文配置**（避免信息爆炸，这正是用户反对的"信息量太大"）。

**注**：DDC 诊断规则用的 `DDC_SECTION = "/Script/UnrealEd.DerivedDataCacheSettings"` 与后端图 `[DerivedDataBackendGraph]` 是两个不同 section。`ddc` domain **两个都抓**（均标 `domain='ddc'`）：`[DerivedDataBackendGraph]`（后端图，主来源）+ `[/Script/UnrealEd.DerivedDataCacheSettings]`（DDC settings）。

### 4.2 数据模型（新表 + 迁移）

新增 `ini_config_snapshots`（列风格参照 `ini_findings`）：

```sql
CREATE TABLE IF NOT EXISTS ini_config_snapshots (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    scan_run_id  INTEGER NOT NULL,
    machine_id   INTEGER NOT NULL,
    file_path    TEXT NOT NULL,        -- 目标机上绝对路径
    ue_version   TEXT,                 -- 关联引擎版本(可空)
    domain       TEXT NOT NULL,        -- 'ddc' | 'pso' | 'zen'
    section      TEXT NOT NULL,        -- INI [section]; PSO cvar 记其所在 section
    key_name     TEXT NOT NULL,
    value        TEXT NOT NULL,
    line_number  INTEGER               -- 1-based, 可空
);
CREATE INDEX IF NOT EXISTS idx_ini_config_snapshots_run
    ON ini_config_snapshots(scan_run_id);
```

**追加为新的迁移文件**，不改任何既有 applied 迁移（教训：health migration007 把 uq 索引追加进已 applied 迁移导致永不重跑）。

### 4.3 抓取逻辑
- `core::ini_scanner`：`ScanOutcome` 增 `config_snapshots: Vec<ConfigSnapshot>`；`scan_machine` 对每个成功读取的文件，从已 parse 的 sections 提取上述三类。
- `cli::domain_ini::scan_cluster` + `commands::ini_scanner`（UI 版）：把 snapshot insert 进新表。
- 随 `ini scan` **默认抓取**（增量成本低 —— 文件已 parse，只是多遍历 sections 提取关注 key）。

### 4.4 展示
- **新查询命令** `ini config <scan_run_id> [--domain ddc|pso|zen]`：
  - human：按 `file_path (+ue_version) → domain → key = value` 缩进/对齐展示；
  - `--json`：`ConfigSnapshot` 数组。
- 数据层 `data::ini_config_snapshots`：`insert` / `list_for_run` / `list_for_run_domain`。
- 扫描时是否即时打印 config（vs 仅 `ini scan` 计数、详情走 `ini config`）：默认**不在扫描时刷屏**，详情走查询命令；实现时可再权衡。

### 4.5 新 CLI 命令
```rust
IniAction::Config {
    scan_run_id: i64,                       // positional
    #[arg(long)] domain: Option<String>,    // ddc | pso | zen
}
```

---

## 5. 改动 3 — Project 深扫

### 5.1 命令
`IniAction::Scan` 扩展：

```
ini scan --machine-ids 1,2,3              # 现状: 机器维度(引擎默认 + user 配置)
ini scan --project-id 5                    # 新: 项目维度,自动解析 project_locations
ini scan --project-id 5 --machine-id 11    # 新: 多机副本收窄到指定机
```

- `--project-id` 与 `--machine-ids` **互斥**（二选一，clap `conflicts_with`）。
- `--machine-id`（单数）仅在 `--project-id` 下用于在多机副本中收窄。

### 5.2 数据流
1. `--project-id` → `data::project_locations::list_for_project(id)` → `[(machine_id, abs_path, uproject_path)]`
2. 可选 `--machine-id` 过滤。
3. 空 → `InvalidInput`（提示"该 project 无 location，先跑 `project discover`"）。
4. 按 machine 分组，项目根目录作为 `project_roots`。
5. 复用 `scan_cluster`（重构成接收 `project_paths_per_machine: HashMap<i64, Vec<String>>`，替换当前硬编码 `&[]`）。
6. `enumerate_project_paths` 生成 `DefaultEngine.ini` / `ConsoleVariables.ini` / `WindowsEngine.ini` → 扫描 → finding **＋ 改动 2 的 config 快照**。

### 5.3 改动点
- `args.rs` `IniAction::Scan`：加 `project_id: Option<i64>` / `machine_id: Option<i64>`（clap conflicts_with）。
- `domain_ini.rs` `scan_cluster`：抽出 project_roots 注入点；machine-scoped 与 project-scoped 共用核心扫描循环。
- `scan_runs.scan_type` 仍为 `"ini"`；`summary_json` 增加 `project_id` 标注。

---

## 6. CLI 命令变更汇总

| 命令 | 变更 |
|---|---|
| `machine refresh <id>` | human 加 UE installs 对齐表格 |
| `machine detail <id>` | human 改对齐表格（替代 pretty JSON） |
| `ini scan` | 加 `--project-id` / `--machine-id` |
| `ini config <run_id> [--domain]` | **新增** |

---

## 7. 错误处理
- `--project-id` 不存在 → `InvalidInput`。
- project 无 location → 友好提示先 `project discover`。
- 多机副本逐台扫，单台失败走现有 `ItemCompleted ok=false` 模式，不阻断其余。
- `ini config` 传不存在的 `run_id` → 空结果（不报错，与 `ini findings` 一致）。

---

## 8. 测试策略
- **单元**：config 提取（parsed ini fixture → 期望 snapshot，三 domain 各一）；`project_id → project_roots` 解析（单机 / 多机 / 空 / `--machine-id` 过滤）；human 表格渲染（refresh / detail / config）；新迁移幂等。
- **回归**：现有 968 tests 全绿；`ini scan --machine-ids` 行为不变（project_roots 仍空）。
- **真机（lanPC）**：razer `project discover` → `project list` → `ini scan --project-id <id>` → `ini config <run_id>` 验证 DDC/PSO/Zen 实况落库 + 展示。

---

## 9. 非目标（范围外）
- 不做交互式 project 选择（CLI 非交互；用户 `project list` 看 id 后显式指定）。
- 不抓取关注 section 之外的全文配置（避免信息爆炸）。
- 不改 `--json` 既有结构。
- 不动 UI（UI 版 scan 已支持 project_roots）。

---

## 10. 实现时需从源码核对的精确项
- DDC：确认 `ddc` domain 抓 `[DerivedDataBackendGraph]` 是否足够，要不要并抓 `[/Script/UnrealEd.DerivedDataCacheSettings]`。
- Zen：从 `docs/research/zen-ini-rules.yaml` + `ini_diagnostics_zen.rs` 取 zen 关注 section/key 全集（`ZenShared` / `InstalledDerivedDataBackendGraph` / legacy `Shared`）。
- PSO：cvar 前缀匹配的完整集合（R008/R009/R010/R024 + 同系列 `r.PSOPrecache.*` / `r.ShaderPipelineCache.*`）。
- project：`project_locations` 的 `list_for_project` 是否存在；`abs_path` + `uproject_path` 如何拼成项目根目录（`enumerate_project_paths` 需要的是含 `Config\` 的项目根）。
