# UECM Plan 7 — Zen Cache Backend Integration

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

## Execution Mode (READ FIRST — overrides default skill behavior)

**Mode: AUTO-CONTINUOUS.** Run all tasks back-to-back without pausing for human approval between them.

**Stop and ask the user ONLY in these cases:**

1. **Plan vs reality conflict** that requires re-design (e.g. M0 fact-finding reveals UE 5.4+ zen INI section/key name differs from this plan's working assumption).
2. **Destructive operation requiring authorization** outside the explicit safeguards listed in §12 Red Lines.
3. **Critical-severity code review finding** with no obvious fix.
4. **lanPC unreachable** when an E2E verification step requires it (M0 实测 / M2 single-machine zen install / M3 cluster enable).
5. **Zen source code behavior conflicts with this plan** — if you read `/Users/bip.lan/AIWorkspace/vp/zen/src/...` and find evidence the plan made an incorrect assumption, surface it before implementing.

**Do NOT stop for:** YAML schema additions, Windows-gated tests skipped on macOS, README polish, dry-run output formatting.

**Final report:** commit list, test count by module, every DONE_WITH_CONCERNS verbatim, M0 verified UE versions, integration test outcome on Windows test host, deferred items.

---

## Revision History

- **v3.1 (2026-05-18)** — Codex second review 修正 5 处：(1) 分清 `zen.exe`（CLI）和 `zenserver.exe`（daemon），baseline / R016 校验目标改 zenserver.exe + 副检 zen.exe；(2) `unverified_policy` 默认改 `refuse`，`verified_versions` 初始为空，M0 实测通过才追加；(3) `EngineAssociation` 加 4 种形态解析规则（version / GUID / empty / unknown），未知形态强制 legacy_pak；(4) 明确 raw_json 与现有 3 个 flat extract 列的关系（baseline 冻结，未来不加新 flat 列）；(5) lockfile 解析改用 `zen status --format json` CLI，不自己实现 Compact Binary parser。
- **v3 (2026-05-18)** — 初版。

## Goal

Plan 7 把 UE 5.4+ 自带的 zen 缓存服务接进 UECM，让 5–20 台 render node 不用再各自维护 SMB 共享 + DDC pak。新工程（UE 5.4+）一键启用 zen，老工程（UE 5.4 以下）照旧走 legacy SMB+Pak 路径。同一集群混合工程版本是默认状态，UECM 按工程 UE 版本 × 机器实际安装组合自动路由。

**完成后**：UECM 一个界面管完整个集群的缓存——zen endpoint 拓扑、健康度、cache 命中率、配置一致性全部可见可控；agent（Claude Code / Codex 等）可直接 spawn `uecm-cli zen ...` 命令全自动跑。

---

## Architecture additions

- `core::zen::*` — 11 个子模块（endpoint / probe / cache_stats / enable / disable / binary / lua_config / lockfile / retention / redaction / rules_loader / verify）
- `core::cache_backend` — 顶级路由器，按工程 + 机器组合决定走 zen 还是 legacy；`ddc generate/distribute` 通过它做选择
- `data::zen_endpoints / zen_probes / zen_cache_stats / project_cache_backend / zen_binary_expected` — 5 张新表 + 2 张老表加字段
- `ps-scripts/zen-*.ps1` — 13 个新 PowerShell sidecar（remote 探测、INI 编辑、URL ACL、service 安装）
- `cli/domain_zen.rs` + `commands/zen.rs` — CLI 与 Tauri API 一一对应，业务逻辑全部在 `core/zen/`
- `docs/research/zen-ini-rules.yaml` — 5.4+ 通用 INI 规则 + verified_versions 列表 + overrides 例外机制

---

## Key design decisions

1. **两个 binary 都用 UE 自带，零部署 / 零升级 / 零替换**：
   - `Engine\Binaries\Win64\zen.exe` — CLI 工具（执行 `service install` / `status` / `up` 等命令）
   - `Engine\Binaries\Win64\zenserver.exe` — **真正跑的 daemon**，`zen service install` 装的是它
   - **Health check / baseline 主要看 zenserver.exe**，zen.exe 只做副检（CLI 完整性）
2. **UECM 责任 = 启用 + 配置 + 监测**，不碰部署
3. **双 backend 并存**，按工程 UE 版本 + 机器实际安装 routing；老 `core::ddc_pak` / `core::pak_distribute` 一行不动
4. **CLI 与 Tauri API 平级实现**，业务逻辑在共享 `core/zen/`，禁止两边各写一套
5. **`/health` / `/stats` 真实 API 形态**：`/health` 只返 `OK!` 文本；详情在 `/health/info`；version 在 `/health/version`；cache stats 在 `/stats/z$` 嵌套 JSON，不在裸 `/stats`
6. **raw_json 优先 + frozen extract baseline**：cache metrics 表存原文 JSON + schema_version 防 zen API drift；现有的 3 个 flat 列（`cache_hit_ratio` / `cache_disk_size_bytes` / `cache_memory_size_bytes`）作为**冻结基线**仅供索引查询，**未来不加新 flat 列**——新指标一律从 raw_json 现取（SQLite JSON1 extension）
7. **port 区分 declared_port / effective_port**：`zen up` 会按 lockfile 选可用端口，probe 必须读 lockfile
8. **规则数据化**：INI Scanner 规则从 `zen-ini-rules.yaml` 读，不 hardcode
9. **未验证 UE 版本默认拒绝**：YAML `unverified_policy: refuse` 兜底，防瞎装炸现场
10. **service install 禁 `--full` flag**：避免拷 binary 进系统目录

---

## 1. Data Model

### 1.1 New tables

```sql
CREATE TABLE zen_endpoints (
    id INTEGER PRIMARY KEY,
    machine_id INTEGER NOT NULL REFERENCES machines(id),
    declared_port INTEGER NOT NULL DEFAULT 8558,
    scheme TEXT NOT NULL DEFAULT 'http',
    role TEXT NOT NULL,                             -- 'local' | 'shared_upstream'
    upstream_endpoint_id INTEGER REFERENCES zen_endpoints(id),
    data_dir TEXT NOT NULL,
    httpserverclass TEXT NOT NULL DEFAULT 'httpsys',
    lifecycle_mode TEXT NOT NULL,                   -- 'editor_owned' | 'installed_service'
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(machine_id, declared_port)
);
CREATE INDEX idx_zen_endpoints_machine ON zen_endpoints(machine_id);

CREATE TABLE zen_probes (
    id INTEGER PRIMARY KEY,
    endpoint_id INTEGER NOT NULL REFERENCES zen_endpoints(id),
    probed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    reachable INTEGER NOT NULL,
    schema_version INTEGER NOT NULL DEFAULT 1,
    effective_port INTEGER,
    pid INTEGER,
    uptime_seconds INTEGER,
    data_root TEXT,
    is_dedicated INTEGER,
    build_version TEXT,
    health_info_json TEXT,
    health_version_json TEXT,
    stats_providers_json TEXT,
    error_message TEXT
);
CREATE INDEX idx_zen_probes_endpoint_time ON zen_probes(endpoint_id, probed_at);

CREATE TABLE zen_cache_stats (
    id INTEGER PRIMARY KEY,
    endpoint_id INTEGER NOT NULL REFERENCES zen_endpoints(id),
    sampled_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    cache_hit_ratio REAL,
    cache_disk_size_bytes INTEGER,
    cache_memory_size_bytes INTEGER,
    provider_path TEXT NOT NULL DEFAULT '/stats/z$',
    raw_json TEXT NOT NULL,
    schema_version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_zen_cache_stats_endpoint_time ON zen_cache_stats(endpoint_id, sampled_at);

CREATE TABLE project_cache_backend (
    project_id INTEGER NOT NULL REFERENCES projects(id),
    machine_id INTEGER NOT NULL REFERENCES machines(id),
    backend TEXT NOT NULL,                          -- 'zen' | 'legacy_pak' | 'auto'
    zen_endpoint_id INTEGER REFERENCES zen_endpoints(id),
    notes TEXT,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (project_id, machine_id)
);

-- baseline 期望值：每个 UE 版本分别记 zen.exe 和 zenserver.exe 两个 hash
CREATE TABLE zen_binary_expected (
    ue_version_major INTEGER NOT NULL,
    ue_version_minor INTEGER NOT NULL,
    binary_kind TEXT NOT NULL,                      -- 'zen_cli' | 'zenserver'
    sha256 TEXT NOT NULL,
    locked_by TEXT,
    first_seen_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (ue_version_major, ue_version_minor, binary_kind)
);
```

### 1.2 Existing tables

```sql
-- zen.exe（CLI）
ALTER TABLE machine_ue_installs ADD COLUMN zen_cli_path TEXT;
ALTER TABLE machine_ue_installs ADD COLUMN zen_cli_version TEXT;
ALTER TABLE machine_ue_installs ADD COLUMN zen_cli_sha256 TEXT;
-- zenserver.exe（真正的 daemon）
ALTER TABLE machine_ue_installs ADD COLUMN zenserver_path TEXT;
ALTER TABLE machine_ue_installs ADD COLUMN zenserver_version TEXT;
ALTER TABLE machine_ue_installs ADD COLUMN zenserver_sha256 TEXT;
-- UE 版本（见下方 §1.3 EngineAssociation 解析）
ALTER TABLE projects ADD COLUMN ue_version_major INTEGER;
ALTER TABLE projects ADD COLUMN ue_version_minor INTEGER;
ALTER TABLE projects ADD COLUMN engine_association_raw TEXT;     -- 原始字符串保留，便于排错
ALTER TABLE projects ADD COLUMN engine_association_kind TEXT;    -- 'version' | 'guid' | 'empty' | 'unknown'
```

### 1.3 EngineAssociation 解析规则（v3 修订）

`.uproject` 的 `EngineAssociation` 字段有 4 种形态，必须分别处理：

| 形态 | 例子 | 处理 |
|---|---|---|
| 标准版本号 | `"5.7"` / `"5.7.0"` | parse 成 `ue_version_major=5, ue_version_minor=7`，kind=`version` |
| GUID（custom build）| `"{8B3F8B3F-...}"` | 查 Windows 注册表 `HKLM\SOFTWARE\Epic Games\Unreal Engine\Builds` 拿到安装路径，再从该 UE 安装的 `Engine\Build\Build.version` 解析版本号；查不到则 kind=`guid`，version 字段 NULL |
| 空字符串 | `""` | kind=`empty`，version 字段 NULL（同目录扫 `Engine\Build\Build.version`） |
| 其他 | 解析失败 | kind=`unknown`，version 字段 NULL |

**Routing fallback 规则**（`core/cache_backend.rs` 实现）：

- version 字段非 NULL → 按 §4.2 决策表正常路由
- version 字段 NULL → **强制 legacy_pak**（保守选择，不在不确定的工程上启用 zen）
- 操作员可以在 UI/CLI 显式覆盖（`project_cache_backend` 表）

**Pre-M1 fix（M0 T0.1）**：现有 `core/project_discovery.rs:83-96` 把 raw EngineAssociation 错填到 `uproject_guid`，纠正映射 → 写到 `engine_association_raw` + `engine_association_kind` + 解析后写 `ue_version_major/minor`。

### 1.4 Retention

- `zen_probes` / `zen_cache_stats`：每 endpoint 保留最近 100 条 + 7 天内全部
- `core/zen/retention.rs` + 启动时调度

---

## 2. CLI Surface

### 2.1 `uecm-cli zen <action>`

```
# 探测
uecm-cli zen status          [--machine ID | --all] [--cred-alias ALIAS]
uecm-cli zen probe           [--machine ID | --all] [--cred-alias ALIAS]
uecm-cli zen cache-stats     [--endpoint-id ID | --all]
uecm-cli zen detect-binary   [--machine ID | --all] --cred-alias ALIAS
                             # 同时扫 zen.exe 和 zenserver.exe，分别写入 machine_ue_installs

# endpoint 管理
uecm-cli zen list-endpoints
uecm-cli zen register        --machine ID --declared-port 8558
                             --scheme http|https
                             --role local|shared_upstream
                             [--upstream-endpoint-id ID]
                             [--data-dir PATH]
                             [--httpserverclass httpsys|asio]
                             [--lifecycle editor_owned|installed_service]
uecm-cli zen unregister      --endpoint-id ID
uecm-cli zen apply-config    --endpoint-id ID [--yes] [--dry-run]
uecm-cli zen lua-preview     --endpoint-id ID

# 工程级启用
uecm-cli zen enable          --project-id ID --machines M1,M2,...
                             --cred-alias ALIAS [--yes] [--dry-run]
uecm-cli zen disable         --project-id ID --machines M1,M2,...
                             --cred-alias ALIAS [--yes] [--dry-run]

# service 生命周期
uecm-cli zen service install   --endpoint-id ID --cred-alias ALIAS [--yes] [--dry-run]
uecm-cli zen service uninstall --endpoint-id ID --cred-alias ALIAS [--yes]
uecm-cli zen service start     --endpoint-id ID --cred-alias ALIAS
uecm-cli zen service stop      --endpoint-id ID --cred-alias ALIAS
uecm-cli zen service status    --endpoint-id ID --cred-alias ALIAS

# URL ACL
uecm-cli zen urlacl add      --endpoint-id ID --user USER --cred-alias ALIAS [--yes]
uecm-cli zen urlacl list     --machine ID --cred-alias ALIAS
uecm-cli zen urlacl remove   --endpoint-id ID --cred-alias ALIAS [--yes]

# 规则验证
uecm-cli zen verify-rules    --ue-version 5.8 --ue-install C:\UE\5.8
                             [--write-verified]

# binary baseline（每个 UE 版本分别 zen.exe / zenserver.exe 两条记录）
uecm-cli zen baseline list   [--ue-version 5.7]
uecm-cli zen baseline lock   --ue-version 5.7 --kind zen_cli|zenserver
                             --sha256 ABCD... [--yes]
uecm-cli zen baseline unlock --ue-version 5.7 --kind zen_cli|zenserver [--yes]
```

所有破坏性命令必须 `--yes` 或 `--dry-run`，沿用现有 `destructive` 模块。

### 2.2 Existing `ddc` domain — add backend flag

```rust
pub enum DdcAction {
    Generate {
        project_id: i64,
        source_machine: i64,
        cred: CredentialArgs,
        #[arg(long, default_value = "auto")]
        backend: BackendChoice,
    },
    // Verify / Distribute 同样加 backend
}

pub enum BackendChoice { Auto, Legacy, Zen }
```

`backend=zen` 路径下 `generate/distribute` 是 no-op：

```json
{"backend":"zen","skipped":true,"reason":"zen handles caching natively"}
```

**退出码 0**，agent 才能区分"明确无需做"和"出错"。

### 2.3 `health` domain — 4 new rows

| 行 | 数据源 |
|---|---|
| `zen_reachable` | 各 endpoint 最近一次 `/health` |
| `zen_version_consistent` | 集群范围 `zen_binary_version` 全等 |
| `zen_binary_intact` | `zenserver_sha256` 跟 `zen_binary_expected (binary_kind='zenserver')` 匹配；zen_cli 副检 |
| `zen_cache_provider_ready` | `/stats` providers list 包含 `z$` |

### 2.4 Exit codes

| 码 | 含义 |
|---|---|
| 0 | 成功 |
| 1 | 部分失败 |
| 2 | 参数错 |
| 3 | 远端不可达 |
| 4 | 凭据错 |
| 5 | zen 配置漂移（hash 不匹配 baseline）|
| 6 | UE 版本未验证（YAML 不含且 policy=refuse）|

---

## 3. Tauri API

`src-tauri/src/commands/zen.rs` 与 CLI 一对一对应。**强制要求**：业务逻辑全部在 `core/zen/`，`commands/zen.rs` 与 `cli/domain_zen.rs` 都只做参数解析 + 调 core。

长任务（enable / disable / apply-config / detect-binary cluster-wide / verify-rules）：

- 返回 `operation_id`（写 `operations` 表）
- 实时进度通过 `batch-progress` Tauri event 推送

---

## 4. Cache Backend Routing

### 4.1 Router

`core/cache_backend.rs`：

```rust
pub enum Backend { Legacy, Zen }

pub fn resolve_for(project_id: i64, machine_id: i64) -> UecmResult<Backend> {
    // 1. 显式配置最高优先
    if let Some(row) = project_cache_backend::find(project_id, machine_id)? {
        if row.backend != "auto" { return Ok(row.backend.into()); }
    }
    // 2. auto: 按工程 + 机器实际 UE 版本组合判断
    let project_ue = projects::ue_version(project_id)?;
    let machine_ues = machine_ue_installs::list(machine_id)?;
    let usable_ue = pick_best_match(project_ue, machine_ues)?;
    
    if usable_ue.major < 5 || (usable_ue.major == 5 && usable_ue.minor < 4) {
        Ok(Backend::Legacy)
    } else if has_reachable_zen_endpoint(machine_id)? {
        Ok(Backend::Zen)
    } else {
        Ok(Backend::Legacy)
    }
}
```

### 4.2 Decision table

| 工程 UE | 机器实际可用 UE | 集群有可达 zen | 默认 |
|---|---|---|---|
| < 5.4 | 任意 | 任意 | legacy_pak（强制）|
| ≥ 5.4 | 5.4+ | 无 | legacy_pak |
| ≥ 5.4 | 5.4+ | 有 | zen |
| 任意 | 机器只装了 < 5.4 | 任意 | legacy_pak |

### 4.3 Pak path preservation

`core/ddc_pak.rs` / `core/pak_distribute.rs` / 现有 PS 脚本一行不动。

---

## 5. INI Scanner — new rules (loaded from YAML, not hardcoded)

| 规则 ID | 检查 | 触发条件 |
|---|---|---|
| R012 | 工程是否启用 Zen Store（按 YAML rule 匹配） | UE ≥ 5.4 且 project backend = zen |
| R013 | Zen upstream URL 指向预期 endpoint | endpoint role = shared_upstream |
| R014 | Legacy DDC `Path=` 和 zen backend 同时存在 | 双配置冲突 |
| R015 | http.sys URL 预留就绪 | endpoint.httpserverclass = httpsys |
| R016 | **zenserver.exe** sha256 跟 `zen_binary_expected (binary_kind='zenserver')` 匹配；副检 zen.exe 跟 `binary_kind='zen_cli'` 匹配 | 防 UE 升级把真正跑的 server binary 替换掉 |
| R017 | zen 进程的 datadir 磁盘容量 | endpoint reachable 且磁盘 < 20% |
| R018 | 集群内 zen 版本一致 | 全集群比较 `zen_binary_version` |

---

## 6. Rule Config — `docs/research/zen-ini-rules.yaml`

### 6.1 Schema

```yaml
zen_ini:
  applies_to: ">=5.4"
  rules:
    enable_zen_backend:
      ini_file: DefaultEngine.ini
      section: "[InstalledDerivedDataBackendGraph]"
      key: Zen
      value_template: "(Type=Zen, Host=\"{host}\", Port={port})"
      backup: true
    set_default_graph:
      ini_file: DefaultEngine.ini
      section: "[InstalledDerivedDataBackendGraph]"
      key: Root
      value_template: "(Type=KeyLength, Length=120, Inner=Zen)"
      backup: true

# 必须以 M0 实测结果为准——下面这个列表是 EMPTY，
# M0 在哪个版本上跑通 verify-rules 就追加哪个版本。
verified_versions: []

# 默认拒绝未验证版本，与 Red Line §12 第 7 条一致
unverified_policy: refuse   # refuse = 拒绝执行 | warn = 提示但允许（不推荐改）

overrides:
  # 未来 UE 5.9 若改了配置形态，加在这里覆盖默认
  # "5.9":
  #   enable_zen_backend:
  #     section: "[NewSectionName]"
```

### 6.2 Semantics

- 命中 `verified_versions` → 直接用规则
- 不命中且 `policy=warn` → 用规则但日志加 WARN
- 不命中且 `policy=refuse` → 拒绝 enable，退出码 6
- `overrides` 里有该版本 → 用 override 规则

> **M0 阶段实测**：具体 section / key / value_template **必须以 M0 阶段在真实 UE 5.6 / 5.7 上的实测结果为准**。本文档给的是基于公开文档的工作假设，未经源码核实。

---

## 7. Agent-driven Usage Path

### 7.1 一次成功路径

```bash
uecm-cli project list --json
uecm-cli machine list --json
uecm-cli zen detect-binary --all --cred-alias render-svc --json
uecm-cli zen register --machine 3 --declared-port 8558 --role local \
   --upstream-endpoint-id 1 --data-dir D:\ZenCache \
   --httpserverclass httpsys --lifecycle installed_service --json
uecm-cli zen apply-config --endpoint-id 12 --yes --json
uecm-cli zen service install --endpoint-id 12 --cred-alias render-svc --yes --json
uecm-cli zen service start --endpoint-id 12 --cred-alias render-svc --json
uecm-cli zen probe --all --cred-alias render-svc --json
uecm-cli zen cache-stats --all --json
uecm-cli zen enable --project-id 5 --machines 1,2,3,4,5 \
   --cred-alias render-svc --yes --json
uecm-cli health run --json
```

### 7.2 Idempotency contract

- `register` 重复 → conflict 不覆盖，返回 `{"changed":false,"existing_id":...}`
- `enable` 已是目标态 → `{"changed":false}` 退出码 0
- `apply-config` lua 内容相同 → 不推、不重启 service
- `service install` 已装且配置相同 → no-op

### 7.3 Dry-run

输出完整 plan：要改的文件 + 要跑的命令 + 影响的机器 + 备份路径，但不真改。

### 7.4 Output contract

- 默认人类可读；`--json` 切 NDJSON
- 每行一个事件：`{"type":"phase",...}` / `{"type":"result",...}` / `{"type":"error",...}`

---

## 8. Implementation Tasks

### M0 — Fact-finding + Pre-M1 fix [3 天]

**目标**：搞清楚 UE 5.4+ 真实 zen 启用 INI 形态 + 修一个现有 bug。M0 不出文档后续阶段全部封盘。

- [ ] T0.1 修 `core/project_discovery.rs:83-96` 把 `EngineAssociation` 写到 `projects.ue_version_major/minor`，不再错填 `uproject_guid`
- [ ] T0.2 在 5.7 测试机上手工创建一个空工程，按公开文档候选写法启用 zen，确认 editor 启动日志 `LogDerivedDataCache: ... Zen ...` 出现且 zen 服务的 `/stats/z$` 命中率 > 0
- [ ] T0.3 在 5.6 测试机上重做 T0.2，确认通用规则有效
- [ ] T0.4 抓 `Engine\Saved\Zen\zen.json` 内容、抓 zen.exe 的 sponsor 启动命令，记录到 `docs/research/zen-launch-mechanism.md`
- [ ] T0.4b **实测确认 `zen status --data-dir <path> --format json` 输出包含 effective_port / pid / executable**（T1.3 依赖这个）。若该 flag 不存在或字段不全，fallback 方案：用 `zen status` 默认输出解析文本（或考虑加 Rust Compact Binary mini-parser，工程量评估写进文档）
- [ ] T0.5 把实测确认的 section / key / value template 写入 `docs/research/zen-ini-rules.yaml`；`verified_versions` **只填实测通过的版本**（如 T0.2/T0.3 跑通 5.6 + 5.7 就只填这两个）；`unverified_policy: refuse`（默认拒绝未实测的版本）
- [ ] T0.6 M0 交付物 review：YAML + launch 机制文档进 repo

### M1 — Schema + Probe + Binary Detect [1 周]

- [ ] T1.1 写 5 张新表 + 2 张 ALTER 的 migration，挂进 `data::schema::migrate()`；每张表加单测
- [ ] T1.2 `data::zen_endpoints` / `data::zen_probes` / `data::zen_cache_stats` / `data::project_cache_backend` / `data::zen_binary_expected` CRUD
- [ ] T1.3 `core::zen::lockfile` 拿 effective_port：**调 `zen status --data-dir <path> --format json` 让 zen 自己解析 lockfile（Compact Binary 格式）**，Rust 只读它输出的 JSON 拿 effective_port / pid / executable。理由：UECM Rust deps 没 CB parser，写一个仅为读 4 个字段不划算。`ps-scripts/zen-read-lockfile.ps1` 是 PS 包装，远端调 `zen status`
- [ ] T1.4 `core::zen::probe` 拉 `/health` + `/health/info` + `/health/version`，写 `zen_probes`
- [ ] T1.5 `core::zen::cache_stats` 拉 `/stats` 拿 providers，挑 `z$` 再拉 `/stats/z$`，写 `zen_cache_stats`
- [ ] T1.6 `core::zen::binary` 远机**同时扫 `zen.exe` 和 `zenserver.exe`**，分别记 path/version/sha256 写到 `machine_ue_installs` 的 zen_cli_* 和 zenserver_* 字段；首次探测自动写 `zen_binary_expected (binary_kind='zen_cli')` 和 `(binary_kind='zenserver')` 两条 baseline
- [ ] T1.7 `core::zen::retention` 启动调度，probes/cache_stats 表 GC
- [ ] T1.8 PS 脚本：`zen-probe-health.ps1` / `zen-probe-cache-stats.ps1` / `zen-detect-binary.ps1` / `zen-read-lockfile.ps1`
- [ ] T1.9 CLI: `zen status` / `zen probe` / `zen cache-stats` / `zen detect-binary` / `zen list-endpoints` / `zen baseline list/lock/unlock`
- [ ] T1.10 Tauri commands 对应包装
- [ ] T1.11 验收：跑 `uecm-cli zen detect-binary --all --json` 在 5.7 机器上**同时拉到 zen.exe 和 zenserver.exe**；`zen probe` 写库；`zen cache-stats` 拉到命中率

### M2 — Endpoint Config + Lua + Service [1.5 周]

- [ ] T2.1 `core::zen::endpoint` CRUD + role 状态机（local / shared_upstream 互转规则）
- [ ] T2.2 `core::zen::lua_config` 生成 lua（真实 key：`server.datadir` / `network.port` / `network.httpserverclass` / `cache.upstream.zen.url`）
- [ ] T2.3 `core::zen::redaction` secret 字段脱敏（应对 `--access-token` / `--password` / `--api-key`）
- [ ] T2.4 PS 脚本：`zen-write-lua-config.ps1` / `zen-urlacl-add.ps1` / `zen-urlacl-list.ps1` / `zen-urlacl-remove.ps1` / `zen-service-install.ps1`（**hard-block `--full` flag**）/ `zen-service-uninstall.ps1` / `zen-service-status.ps1` / `zen-up.ps1` / `zen-down.ps1`
- [ ] T2.5 CLI: `zen register` / `zen unregister` / `zen apply-config` / `zen lua-preview` / `zen service install/uninstall/start/stop/status` / `zen urlacl add/list/remove`
- [ ] T2.6 Tauri commands 对应包装
- [ ] T2.7 dry-run 实现：输出完整改动 plan 不真改
- [ ] T2.8 datadir 安全校验（拒绝 `C:\Windows` / `C:\Program Files` 子路径）
- [ ] T2.9 验收：单机走完 `register → apply-config → urlacl add → service install → service start`，能 `/health/info` 看到运行中

### M3 — Project Enable + Backend Router [1.5 周]

- [ ] T3.1 `core::zen::rules_loader` 读 `zen-ini-rules.yaml`，按 UE 版本匹配规则；处理 verified_versions / unverified_policy / overrides 三层
- [ ] T3.2 `core::zen::enable` 按规则改 `DefaultEngine.ini`（带备份，沿用 `core::ini_apply` 原子写入）
- [ ] T3.3 `core::zen::disable` 反向回滚
- [ ] T3.4 PS 脚本：`zen-enable-project.ps1` / `zen-disable-project.ps1`
- [ ] T3.5 `core::cache_backend` 路由器 + 决策表实现；macOS 单测覆盖所有路径
- [ ] T3.6 `ddc generate/distribute` 加 `--backend` flag；`backend=zen` 走 no-op 路径返回 `{"backend":"zen","skipped":true}`
- [ ] T3.7 CLI: `zen enable` / `zen disable`
- [ ] T3.8 幂等性：`enable` 已是目标态返回 `{"changed":false}` 退出码 0
- [ ] T3.9 验收：5.4+ 工程选 5 台机器一键 enable；editor 启动 cache 走 zen 命中

### M4 — INI Scanner + Health Check + Verify-rules [1 周]

- [ ] T4.1 INI Scanner 加 R012–R018，从 YAML 读规则（不 hardcode）
- [ ] T4.2 Health Check 加 4 行：`zen_reachable` / `zen_version_consistent` / `zen_binary_intact` / `zen_cache_provider_ready`
- [ ] T4.3 R016 binary_intact 接 `zen_binary_expected` 表；不一致告警优先于匹配
- [ ] T4.4 `core::zen::verify` 实现：在 fixture 工程上跑 enable → 起 editor → 抓启动日志确认命中 → 探测命中率 > 0
- [ ] T4.5 CLI: `zen verify-rules --ue-version X --ue-install PATH [--write-verified]`
- [ ] T4.6 PS 脚本：`zen-verify-rules.ps1`
- [ ] T4.7 验收：健康检查识别"集群 zen 版本不一致"、"zen.exe 被 UE 升级覆盖"；`verify-rules` 在 fixture 工程上跑通

### M5 — Hardening + Agent Acceptance [0.5 周]

- [ ] T5.1 retention GC 实测：probes/cache_stats 表 7 天后自动清理
- [ ] T5.2 secret redaction 单测：所有敏感 flag 不落 `operations.log_text`
- [ ] T5.3 `operations` 表启动时扫 `status='running'` 超 1 小时自动标 `interrupted`
- [ ] T5.4 跑 §7.1 那条 11 步 agent 路径全 `--json` ok
- [ ] T5.5 zen schema drift detection：release 时拉真实 zen `/health/info` + `/stats/z$` 对照代码 extract 字段，缺字段 fail build
- [ ] T5.6 写 `docs/zen-integration.md` 给 operator
- [ ] T5.7 最终回归：老 DDC pak + SMB share 全部测试 pass，无回归

---

## 9. Testing Strategy

- `#[cfg(windows)]` 门控 PS sidecar 单测
- `cache_backend.rs` 路由逻辑 + lockfile 解析 + YAML 规则加载必须 macOS 也能跑（纯 Rust）
- 集成测试用一台 Windows test host
- 老 DDC pak + SMB share 全部测试不许动、不许跑挂
- 每次 release 跑 zen schema drift detection

---

## 10. Out of Scope

- 前端 UI（CLI / API 自验，UI 后续 plan 接手）
- zen Compute / Horde / Nomad / Hub 模式
- 自动升级 zen 二进制
- 跨平台（Windows-only）
- UE 5.4 之前版本的 zen 0.2.x 支持

---

## 11. Risks

| 风险 | 对策 |
|---|---|
| UE 升级覆盖 zen.exe | R016 + Health Check `zen_binary_intact`（用 `zen_binary_expected` 表）|
| 集群 zen 版本不齐 | R018 + `zen_version_consistent` |
| http.sys URL 预留未做 | `apply-config` 内联 urlacl + 独立 `zen urlacl list` 排错 |
| operator 同时启 legacy + zen 双配置 | R014 检测 + 强制提示先 disable 一边 |
| 远端 zen 端口被防火墙挡 | M2 enable 流程加 `New-NetFirewallRule` |
| lua datadir 指到系统盘 | `apply-config` 校验 datadir 不在 `C:\Windows` / `C:\Program Files` 下 |
| zen schema drift（API 变了）| raw_json 优先 + schema_version + release 时跑 drift detection |
| port 实际 ≠ declared | probe 必读 lockfile 拿 effective_port |
| service install --full 误用 | PS 脚本 hard-block `--full`；CLI 不暴露这个 flag |
| 拍摄现场断网 upstream zen 不可达 | 本机 zen 仍可用；Health Check 用降级状态而非 CRITICAL |
| UECM crash 时未完成 enable | `operations.status='running'` 超 1 小时自动标 `interrupted`，UI 提示重跑 |
| 多 editor 同启 zen 抢锁 | zen 本身 sponsor 机制处理 |
| secret 泄露 | §12 红线 + `zen/redaction.rs` |
| UE 新主版本（5.8 / 5.9）配置真改了 | YAML `overrides` 加 entry；`unverified_policy=refuse` 兜底 |

---

## 12. Red Lines

1. **禁 `zen service install --full`**——会拷 binary 进系统目录，违背"用 UE 自带 zen"
2. **现有 3 个 flat extract 列冻结**——`zen_cache_stats.cache_hit_ratio` / `cache_disk_size_bytes` / `cache_memory_size_bytes` 是 baseline indexed columns；**未来不加新 flat metric 列**，新指标走 `raw_json` + SQLite JSON1 extension 现取
3. **禁 port 硬编码**——所有 probe / register 必须接受显式 port 或读 lockfile
4. **禁绕过 DPAPI/cmdkey**——zen 远程操作必须走现有 `core/credentials.rs` + cmdkey 注入
5. **禁 secret 入日志**——zen `--access-token` / `--password` / `--api-key` 必须经 `zen/redaction.rs` 处理才进 operations.log_text
6. **禁动 `Engine\Binaries\Win64\zen.exe` 和 `zenserver.exe`**——两个 binary 都只读，发现 hash 不一致只告警不修
7. **禁在 YAML 没标 verified 的 UE 版本上 enable**（policy=refuse 时）

---

## 13. Continuous Maintenance — UE 新主版本怎么办

operator 在 UE 新主版本发布后 1 周内做的事：

```bash
# 1. 装新版 UE
# 2. 在该机器上跑 verify
uecm-cli zen verify-rules --ue-version 5.8 --ue-install C:\UE\5.8

# 2a. 通过 → 自动加进 verified_versions
uecm-cli zen verify-rules --ue-version 5.8 --ue-install C:\UE\5.8 --write-verified

# 2b. 失败 → 看日志哪条规则失效，手工补 overrides[5.8] entry，再重跑 verify
```

成本估算：
- 通过：**半天**
- 需要补 override：**1-2 天**
- 完全失效（罕见）：触发 mini-M0，**3-5 天**

---

## 14. References

- Zen source: `/Users/bip.lan/AIWorkspace/vp/zen/src/`
  - `/health` 实现：`zenserver/diag/diagsvcs.cpp:34-40` 只返 `OK!`
  - `/health/info`：`zenserver/diag/diagsvcs.cpp:42-100`
  - `/health/version`：`zenserver/diag/diagsvcs.cpp:125-138`
  - `/stats` providers：`zenhttp/monitoring/httpstats.cpp:114-133`
  - `/stats/z$` cache：`zenserver/storage/cache/httpstructuredcache.cpp:1877-1957`
  - Lua config keys：`zenserver/config/config.cpp:129-181`
  - `zen service` CLI：`zen/cmds/service_cmd.cpp:154-228`
  - `zen up` sponsor：`zen/cmds/up_cmd.cpp:123-189`
- UECM existing patterns:
  - CLI structure: `src-tauri/src/cli/`
  - PS sidecar: `core/winrm.rs` + `ps-scripts/invoke-remote.ps1`
  - Credential injection: `core/credentials.rs` + cmdkey
  - Destructive guard: `cli/destructive.rs`
  - Operations log: `data/operations.rs`
  - INI atomic write: `core/ini_apply.rs`
  - Existing DDC pak (do NOT modify): `core/ddc_pak.rs` / `core/pak_distribute.rs`

---

## 15. One-line Summary

把 zen 当成 UECM 管的一种新 cache 后端。**zen 二进制零部署**、**配置 YAML 化**（一份 5.4+ 通用规则 + verified 版本列表）、**未验证版本默认拒绝**、**老 SMB+Pak 路径完全保留**、**CLI 全暴露**（agent 直接 spawn 可用）。**总 5.5 周单人交付**，UE 新版本维护成本压到半天。
