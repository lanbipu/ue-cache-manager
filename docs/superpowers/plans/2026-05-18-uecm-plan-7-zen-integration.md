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

- **v4 (2026-05-18)** — M0 lanPC 真机 fact-finding 后大改 7 大方向（详见 `docs/research/zen-launch-mechanism.md`）：
  (1) **zen 启用机制错向**：UE 5.4+ local zen 自动启，不需要工程 INI；plan §6 yaml 从 `enable_zen_backend` 重写为 `enable_zen_shared` —— 真正要配的是 `ZenShared` upstream，不是 `Zen` backend；
  (2) **binary 实际跑在 `%LOCALAPPDATA%\UnrealEngine\Common\Zen\Install\`**，所有 UE 版本共享同一份 install，按"最大见过版本"决策；§1.1 `zen_binary_expected` PK 改为 `(zen_build_version, binary_kind)`，§1.2 加 install_* 字段，R016 / R018 / T1.6 全部按 install 路径校验，InTree 副本仅作 reference；
  (3) **zen HTTP API 全是 Compact Binary 不是 JSON**：除 `/health` 纯文本 `OK!` 和 `/health/version` 纯文本外，`/health/info` / `/stats` / `/stats/z$` 都返回 CB；§1.1 raw_json → raw_cb BLOB，schema_version 改为 CB schema 兼容版本；
  (4) **`zen.exe status --format json` 选项不存在**，原 v3.1 决策破产；改用 **Rust CB mini-parser** 直接解 `.lock` lockfile + HTTP API 响应（user 已确认 2026-05-18）；
  (5) **httpserverclass 默认实测是 `asio` 不是 `httpsys`**，§1.1 `zen_endpoints.httpserverclass DEFAULT` 改为 `'asio'`；
  (6) **集群"共享 cache"真实 key 是 `ZenShared`**（不是 plan v3 假设的"在 InstalledDerivedDataBackendGraph 加 Zen entry"）；M3 enable/disable 全部按 ZenShared 配置改；
  (7) Red Line #6 措辞修订：禁动 InTree 和 install 两个位置的 binary，UECM 仅监测 hash 漂移不修复。
- **v3.1 (2026-05-18)** — Codex second review 修正 5 处：(1) 分清 `zen.exe`（CLI）和 `zenserver.exe`（daemon），baseline / R016 校验目标改 zenserver.exe + 副检 zen.exe；(2) `unverified_policy` 默认改 `refuse`，`verified_versions` 初始为空，M0 实测通过才追加；(3) `EngineAssociation` 加 4 种形态解析规则（version / GUID / empty / unknown），未知形态强制 legacy_pak；(4) 明确 raw_json 与现有 3 个 flat extract 列的关系（baseline 冻结，未来不加新 flat 列）；(5) lockfile 解析改用 `zen status --format json` CLI，不自己实现 Compact Binary parser。**v4 把这条 lockfile 决策反转了 —— 该 flag 实测不存在。**
- **v3 (2026-05-18)** — 初版。

## Goal

Plan 7 把 UE 5.4+ 自带的 zen 缓存服务接进 UECM，让 5–20 台 render node 不用再各自维护 SMB 共享 + DDC pak。新工程（UE 5.4+）一键启用 zen，老工程（UE 5.4 以下）照旧走 legacy SMB+Pak 路径。同一集群混合工程版本是默认状态，UECM 按工程 UE 版本 × 机器实际安装组合自动路由。

**完成后**：UECM 一个界面管完整个集群的缓存——zen endpoint 拓扑、健康度、cache 命中率、配置一致性全部可见可控；agent（Claude Code / Codex 等）可直接 spawn `uecm-cli zen ...` 命令全自动跑。

---

## Architecture additions

- `core::zen::*` — 12 个子模块（endpoint / probe / cache_stats / enable / disable / binary / lua_config / lockfile / retention / redaction / rules_loader / verify / **cb_parser**（v4 新增））
- `core::cache_backend` — 顶级路由器，按工程 + 机器组合决定走 zen 还是 legacy；`ddc generate/distribute` 通过它做选择
- `data::zen_endpoints / zen_probes / zen_cache_stats / project_cache_backend / zen_binary_expected / zen_binary_intree / machine_zen_install` — 7 张新表 + 1 张老表加字段（v4: 拆 install vs InTree）
- `ps-scripts/zen-*.ps1` — 13 个新 PowerShell sidecar（remote 探测、INI 编辑、URL ACL、service 安装）。v4 区分两类回传策略：
  - **小元数据回传**（path / version / sha256 字符串）：`zen-detect-binary.ps1` 远端用 `Get-FileHash` + 读 `zen.version` 文件，只回 JSON 字符串，避免几十 MB binary 拉回 Rust
  - **CB binary 回传**（base64 编码）：`zen-read-lockfile.ps1` / `zen-probe-health.ps1` / `zen-probe-cache-stats.ps1` 拉 `.lock` 文件 / `/health/info` / `/stats/z$` 等 CB 流，base64 回传，Rust 侧 mini-parser 解码
- `cli/domain_zen.rs` + `commands/zen.rs` — CLI 与 Tauri API 一一对应，业务逻辑全部在 `core/zen/`
- `docs/research/zen-ini-rules.yaml` — v4: ZenShared upstream + legacy DDC 清理规则 + verified_versions 列表 + overrides 例外机制
- `docs/research/zen-launch-mechanism.md` — v4 M0 fact-finding 交付物，记录 UE 5.4+ zen 真实运行机制

---

## Key design decisions

1. **zen binary 实际跑在 `%LOCALAPPDATA%\UnrealEngine\Common\Zen\Install\`，所有 UE 版本共享一份 install**（v4 实测修订）：
   - `Engine\Binaries\Win64\zen.exe` / `zenserver.exe` 是 **InTree 源版本**（每个 UE 版本各一份），UE editor 启动时按 "install 版本 >= InTree 就用 install" 决策。结果：集群里只要任一台开过 UE 5.8，那台的 install 路径就升到 5.8.x，之后即使跑 UE 5.7 工程也用 5.8.x zenserver。
   - **Health check / baseline 校验对象 = install 路径下的 zenserver.exe**（按 zen build version 索引，跟 UE major/minor 解耦）；InTree 副本另设 reference 表，hash 漂移仅作信息不告警。
   - 集群"zen 版本一致" = 所有机器的 install 路径 zen build version 一致；同步靠 rsync install 目录或让每台机器各开一次最新 UE 版本。
2. **UECM 责任 = 启用 + 配置 + 监测**，不碰部署
3. **双 backend 并存**，按工程 UE 版本 + 机器实际安装 routing；老 `core::ddc_pak` / `core::pak_distribute` 一行不动
4. **CLI 与 Tauri API 平级实现**，业务逻辑在共享 `core/zen/`，禁止两边各写一套
5. **HTTP API 真实形态：`/health` / `/health/version` 是纯文本，其余都是 Compact Binary**（v4 实测修订）：
   - `/health` → 纯文本 `OK!`
   - `/health/version` → 纯文本如 `5.8.10`
   - `/health/info` → **CB 二进制**，含 `DataRoot` / `BuildVersion` / `Pid` / `EffectivePort` / `IsDedicated` / `Hostname` / `System` 等
   - `/stats` → **CB 二进制**，含 `providers` 数组（`dashboard` / `http` / `prj` / `sessions` / `ws` / `z$`）
   - `/stats/z$` → **CB 二进制**，含 `requests` 直方图 + `cache` 指标（hit_ratio / hits / misses / writes / size disk/memory / cid* / size 分布）
   - **zen 没有 `?format=json` 或类似开关**。`zen.exe info` 只接 `-u`、`zen.exe status` 没有 `--format` flag。
6. **raw_cb BLOB + Rust CB mini-parser**（v4 取代 v3.1 raw_json 决策）：cache metrics / probes 表存 zen 返回的原始 CB 二进制 + `schema_version` 标记 CB schema 兼容版本；现有 3 个 flat 列（`cache_hit_ratio` / `cache_disk_size_bytes` / `cache_memory_size_bytes`）作为**冻结基线**仅供索引查询，未来不加新 flat 列——新指标由 mini-parser 现取 CB blob。
7. **lockfile 解析必须用 Rust CB mini-parser 直接读 `<data-dir>\.lock`**（v4 推翻 v3.1）：实测 `zen.exe status --format json` flag 不存在，`zen.exe status --data-dir <path>` 也返回 `No Zen state found`（CLI 自己读不出 lockfile）。zen CB 格式 spec 在 UE 源码 `Engine/Source/Runtime/Core/Public/Serialization/CompactBinary.h`；mini-parser read-only 实现工程量 ~2-3 天。port 区分 declared_port / effective_port 仍然有效：probe 必读 lockfile 拿 effective_port / pid / executable。
8. **规则数据化**：ZenShared upstream + legacy DDC 清理规则从 `zen-ini-rules.yaml` 读，不 hardcode
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
    httpserverclass TEXT NOT NULL DEFAULT 'asio',   -- v4: 实测默认 asio（UE 5.7 sponsor 启动）
    lifecycle_mode TEXT NOT NULL,                   -- 'editor_owned' | 'installed_service'
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(machine_id, declared_port)
);
CREATE INDEX idx_zen_endpoints_machine ON zen_endpoints(machine_id);

-- v4: probes 表存 CB BLOB（zen HTTP API 全是 Compact Binary），mini-parser 解码后写 flat 列
CREATE TABLE zen_probes (
    id INTEGER PRIMARY KEY,
    endpoint_id INTEGER NOT NULL REFERENCES zen_endpoints(id),
    probed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    reachable INTEGER NOT NULL,
    schema_version INTEGER NOT NULL DEFAULT 1,      -- CB schema 兼容版本（mini-parser 不识别字段时 bump）
    effective_port INTEGER,                         -- from /health/info CB → RuntimeConfig.EffectivePort
    pid INTEGER,                                    -- from /health/info CB → Pid
    uptime_seconds INTEGER,                         -- from /health/info CB → System.uptime_seconds
    data_root TEXT,                                 -- from /health/info CB → DataRoot
    is_dedicated INTEGER,                           -- from /health/info CB → IsDedicated
    build_version TEXT,                             -- from /health/version 纯文本（或 /health/info CB → BuildVersion）
    health_info_cb BLOB,                            -- v4: /health/info 原始 CB 二进制流
    health_version_text TEXT,                       -- v4: /health/version 纯文本（短字符串如 "5.8.10"）
    stats_providers_cb BLOB,                        -- v4: /stats 原始 CB（含 providers 数组）
    error_message TEXT
);
CREATE INDEX idx_zen_probes_endpoint_time ON zen_probes(endpoint_id, probed_at);

-- v4: cache_stats 表 raw 改为 CB BLOB
CREATE TABLE zen_cache_stats (
    id INTEGER PRIMARY KEY,
    endpoint_id INTEGER NOT NULL REFERENCES zen_endpoints(id),
    sampled_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    cache_hit_ratio REAL,                           -- 冻结基线，从 raw_cb CB → cache.hit_ratio
    cache_disk_size_bytes INTEGER,                  -- 冻结基线，从 raw_cb CB → cache.size.disk
    cache_memory_size_bytes INTEGER,                -- 冻结基线，从 raw_cb CB → cache.size.memory
    provider_path TEXT NOT NULL DEFAULT '/stats/z$',
    raw_cb BLOB NOT NULL,                           -- v4: /stats/z$ 原始 CB 二进制流
    schema_version INTEGER NOT NULL DEFAULT 1       -- CB schema 兼容版本
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

-- v4: baseline = install 路径副本的 sha256，按 zen build version 索引（跟 UE major/minor 解耦）
-- 实测：所有 UE 版本共享 %LOCALAPPDATA%\UnrealEngine\Common\Zen\Install\，按"最大见过版本"决策
CREATE TABLE zen_binary_expected (
    zen_build_version TEXT NOT NULL,                -- 如 "5.8.10-202605071938-windows-x64-release-fbacdecd"
    binary_kind TEXT NOT NULL,                      -- 'zen_cli' | 'zenserver'
    sha256 TEXT NOT NULL,
    locked_by TEXT,
    first_seen_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (zen_build_version, binary_kind)
);

-- v4 新增：InTree 源版本 reference 表（每个 UE 版本各一份，仅作信息不告警）
-- UECM 不指望 InTree 跟 install 一致；hash 漂移仅日志，不进 R016 / R018
CREATE TABLE zen_binary_intree (
    ue_version_major INTEGER NOT NULL,
    ue_version_minor INTEGER NOT NULL,
    binary_kind TEXT NOT NULL,                      -- 'zen_cli' | 'zenserver'
    build_version TEXT,                             -- InTree zen.version 文件内容
    sha256 TEXT,
    last_seen_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (ue_version_major, ue_version_minor, binary_kind)
);
```

### 1.2 Existing tables

```sql
-- v4 实测修订：每台机器只有一份共享 install zenserver/zen.exe（跨 UE 版本），
-- 所以 install_* 列存到 machines 表（machine 级），不存 machine_ue_installs（每 UE 版本一行）。
-- machine_ue_installs 表上的 zen_cli_intree_* / zenserver_intree_* 是 InTree 源版本 reference（每 UE 版本各一份）。

-- 新表 machine_zen_install: 每台机器一行（machine 级 install 路径 + 副本元数据）
CREATE TABLE machine_zen_install (
    machine_id INTEGER PRIMARY KEY REFERENCES machines(id),
    install_dir TEXT,                               -- 如 'C:\Users\<user>\AppData\Local\UnrealEngine\Common\Zen\Install'
    zen_cli_path TEXT,                              -- install 路径下 zen.exe
    zen_cli_build_version TEXT,                     -- 来自同目录 zen.version 文件
    zen_cli_sha256 TEXT,
    zenserver_path TEXT,                            -- install 路径下 zenserver.exe（真正跑的 daemon）
    zenserver_build_version TEXT,
    zenserver_sha256 TEXT,
    last_detected_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- machine_ue_installs 上仅记 InTree 源版本（仅作 reference，不参与 R016 / R018）
ALTER TABLE machine_ue_installs ADD COLUMN zen_cli_intree_path TEXT;
ALTER TABLE machine_ue_installs ADD COLUMN zen_cli_intree_version TEXT;
ALTER TABLE machine_ue_installs ADD COLUMN zen_cli_intree_sha256 TEXT;
ALTER TABLE machine_ue_installs ADD COLUMN zenserver_intree_path TEXT;
ALTER TABLE machine_ue_installs ADD COLUMN zenserver_intree_version TEXT;
ALTER TABLE machine_ue_installs ADD COLUMN zenserver_intree_sha256 TEXT;
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

v4 修订：UE 5.4+ local zen 自动启不需 INI 改动；INI Scanner 真正要检的是 **ZenShared upstream 配置 + legacy DDC 路径清理**。

| 规则 ID | 检查 | 触发条件 |
|---|---|---|
| R012 | 工程 / user-level 是否配置了 ZenShared upstream（按 YAML rule 匹配，section/key 实测后填） | UE ≥ 5.4 且 project backend = zen 且 zen_endpoint role = local 时配 ZenShared 指 cluster master |
| R013 | ZenShared.Host 指向预期 cluster master endpoint | endpoint role = shared_upstream 已注册 |
| R014 | 配置 ZenShared 后 legacy Shared SMB 路径 (`UE-SharedDataCachePath` env / `[Shared]` ini section) 是否仍激活 | 双配置冲突 = SMB+Zen 同时启 |
| R015 | http.sys URL ACL 预留就绪 | endpoint.httpserverclass = httpsys（注：实测默认 asio，httpsys 是 opt-in） |
| R016 | **install 路径** `%LOCALAPPDATA%\UnrealEngine\Common\Zen\Install\zenserver.exe` 的 sha256 跟 `zen_binary_expected (zen_build_version=X, binary_kind='zenserver')` 匹配；副检 install zen.exe 跟 `binary_kind='zen_cli'` 匹配。InTree 副本 hash 漂移仅日志不告警。 | 防止 zen install 被外部工具篡改 |
| R017 | zen 进程的 datadir 磁盘容量 | endpoint reachable 且磁盘 < 20% |
| R018 | 集群内每台机器 install zen build version 一致 | 全集群比较 `machine_zen_install.zenserver_build_version`（跟 UE major/minor 解耦） |

---

## 6. Rule Config — `docs/research/zen-ini-rules.yaml`

### 6.1 Schema（v4 重写：方向从「启用 zen」改为「配置 ZenShared upstream + 清理 legacy DDC」）

```yaml
zen_ini:
  applies_to: ">=5.4"
  # v4 实测结论：UE 5.4+ local zen 自动启，工程 INI 不需要也不应该配 "启 zen"。
  # plan §6 真正要管的是两件事：
  #   (1) 配置 ZenShared 指向 cluster master endpoint（让 render node 走集群共享 cache）
  #   (2) 关闭 legacy SMB+Pak 配置（避免双路径冲突）
  rules:
    enable_zen_shared:
      ini_file: DefaultEngine.ini
      # ⚠ 实际 section/key/value_template 待 M0 ZenShared 定向 fact-find 后填入。
      # 候选位置见 docs/research/zen-launch-mechanism.md §6.3。
      # 候选 A (沿用 InstalledDerivedDataBackendGraph 风格):
      section: "[InstalledDerivedDataBackendGraph]"
      key: ZenShared
      value_template: "(Type=Zen, Host=\"{host}\", Port={port}, Namespace=\"ue.ddc\")"
      backup: true
      # 候选 B (UE 5.4+ 新 hierarchy):
      # section: "[/Script/Engine.DerivedDataCacheSettings]"
      # key: ZenShared.Host
      # value_template: "{host}"
      # 两个候选都要 M0 实测确认。
    disable_legacy_smb_shared:
      ini_file: DefaultEngine.ini
      section: "[InstalledDerivedDataBackendGraph]"
      key: Shared
      # remove 操作：删整行，或注释掉
      action: remove
      backup: true
      # env var UE-SharedDataCachePath 也需清理（user-level，PS 脚本处理）
    disable_legacy_pak:
      ini_file: DefaultEngine.ini
      section: "[InstalledDerivedDataBackendGraph]"
      key: "Pak,CompressedPak"
      action: remove
      backup: true

# 必须以 M0 实测结果为准——下面这个列表是 EMPTY，
# M0 在哪个版本上跑通 verify-rules 就追加哪个版本（"verified" 在 v4 改为「ZenShared 配置在该 UE 版本的 INI 形态已实测」）。
verified_versions: []

# 默认拒绝未验证版本，与 Red Line §12 第 7 条一致
unverified_policy: refuse   # refuse = 拒绝执行 | warn = 提示但允许（不推荐改）

overrides:
  # 未来 UE 5.9 若改了 ZenShared 配置形态，加在这里覆盖默认
  # "5.9":
  #   enable_zen_shared:
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

**目标**：搞清楚 UE 5.4+ 真实 zen 运行机制 + zen 配置启用形态 + 修一个现有 bug。M0 不出文档后续阶段全部封盘。

**v4 修订**：T0.2 / T0.4 / T0.4b 已通过 lanPC 真机探测一次性完成，结论写在 `docs/research/zen-launch-mechanism.md`。**结论颠覆 plan 原假设**（详见 v4 revision history）。剩下要做：T0.3 在 5.6 上再确认一次；T0.5 改为 ZenShared 定向 fact-find + 写入 yaml。

- [x] T0.1 修 `core/project_discovery.rs:83-96` 把 `EngineAssociation` 写到 `projects.ue_version_major/minor`，不再错填 `uproject_guid`（**v4: DONE 2026-05-18**，commit `b224af9`）
- [x] T0.2 在 5.7 测试机上 zen 启用验证（**v4: DONE 2026-05-18，结论是 UE 5.4+ local zen 自动启不需要工程 INI**，活样本 `e:\RenderStream Projects\test_0311` 已确认 `LogDerivedDataCache: ZenLocal ... status: OK!` 出现，/stats/z$ 命中率 > 0）
- [ ] T0.3 在 5.6 测试机上重做：确认 UE 5.6 sponsor 启动 zen 的机制和 5.7 一致（install 路径决策、HTTP API CB 格式、ZenShared 配置形态）。如果一致则归并到通用规则
- [x] T0.4 抓 zen sponsor 启动命令 + install 路径 binary 元数据（**v4: DONE 2026-05-18**，详见 `docs/research/zen-launch-mechanism.md` §2 + §3 + 附录 A）
- [x] T0.4b 实测 zen status / lockfile：**`--format json` flag 不存在，`zen status --data-dir` 也返回 No Zen state found**（v4: DONE 2026-05-18，详见 §5）。**改用 Rust CB mini-parser** 直接读 `<data-dir>\.lock` 二进制（user 确认，工程量 2-3 天，落到 M1 T1.3）
- [ ] T0.5 **ZenShared 配置定向 fact-find**：人为在 lanPC 上配 test 工程 ZenShared 指向自身 → 重启 editor → 抓 log 确认 INI section/key/value 真实形态 → 写入 `docs/research/zen-ini-rules.yaml`（v4 schema 见 §6.1）。`verified_versions` 只填实测通过的版本；`unverified_policy: refuse`
- [ ] T0.6 M0 交付物 review：`zen-launch-mechanism.md` + `zen-ini-rules.yaml` 进 repo；plan v4 修订核对一致；T0.3 / T0.5 结论纳入

### M1 — Schema + Probe + Binary Detect + CB Parser [1.5 周（v4 增加 mini-parser 工程量）]

- [ ] T1.1 写 6 张新表（`zen_endpoints` / `zen_probes` / `zen_cache_stats` / `project_cache_backend` / `zen_binary_expected` / `zen_binary_intree` / `machine_zen_install`）+ ALTER `machine_ue_installs` 的 migration，挂进 `data::schema::migrate()`；每张表加单测
- [ ] T1.2 `data::zen_endpoints` / `data::zen_probes` / `data::zen_cache_stats` / `data::project_cache_backend` / `data::zen_binary_expected` / `data::zen_binary_intree` / `data::machine_zen_install` CRUD
- [ ] **T1.2b（新增）**`core::zen::cb_parser` Rust Compact Binary mini-parser（read-only）：实现 root object / field iter / typed reader（null / int / uint / float / string / array / object）。fixture 来源（base64 / SHA256 / 字段断言全部在 `docs/research/zen-launch-mechanism.md` 附录 C）：
  - `health_info.cb` (1160 bytes) — 覆盖 nested object / array / float / uint / string / bool 全 type，**主 fixture**
  - `stats.cb` (50 bytes) — 覆盖 string array 字段
  - `stats_z.cb` (481 bytes) — 覆盖 frozen extract 3 列 (`hit_ratio` float64 / `size.disk` uint64 / `size.memory` uint) + nested histograms
  - lockfile fixture **pending**（zen 独占锁，需 Win32 BackupRead；T1.3 实现时在 Windows test host 停一次 zen 抓副本，写到 `lockfile.cb`）
  - 单测断言每条 fixture 按附录 C 字段断言表逐字段验证；解码到未知 field type 必须返回 error 而不是 panic；fuzz 单测（cargo-fuzz）覆盖随机字节流
- [ ] T1.3 `core::zen::lockfile` 用 `cb_parser` 直接读 `<data-dir>\.lock` 拿 effective_port / pid / executable（v4 取代 v3.1 `zen status --format json` 路径，该 flag 实测不存在）。`ps-scripts/zen-read-lockfile.ps1` 仅做 SMB read + base64 回传二进制，不做解析
- [ ] T1.4 `core::zen::probe` 拉 `/health`（纯文本）+ `/health/version`（纯文本）+ `/health/info`（CB BLOB），写 `zen_probes`；用 `cb_parser` 解码后 extract effective_port / pid / data_root / is_dedicated / uptime_seconds 写 flat 列
- [ ] T1.5 `core::zen::cache_stats` 拉 `/stats`（CB）解出 providers → 挑 `z$` → 拉 `/stats/z$`（CB），写 `zen_cache_stats.raw_cb`；用 `cb_parser` 解码后 extract cache_hit_ratio / cache_disk_size_bytes / cache_memory_size_bytes 写冻结基线 flat 列
- [ ] T1.6 `core::zen::binary` 远机扫**两个位置**：(a) `%LOCALAPPDATA%\UnrealEngine\Common\Zen\Install\` 的 `zen.exe` / `zenserver.exe` + `zen.version` → 写 `machine_zen_install` 表；(b) 每个 `Engine\Binaries\Win64\` InTree 的 `zen.exe` / `zenserver.exe` + `zen.version` → 写 `machine_ue_installs.zen_cli_intree_*` / `zenserver_intree_*` 字段。首次探测自动写 `zen_binary_expected (zen_build_version=X, binary_kind=...)` baseline + `zen_binary_intree` reference
- [ ] T1.7 `core::zen::retention` 启动调度，probes/cache_stats 表 GC
- [ ] T1.8 PS 脚本（v4 区分两类回传策略）：
  - **小元数据脚本**：`zen-detect-binary.ps1` 远端用 `Get-FileHash` + 读 `zen.version` 文件，回 JSON 字符串（path / build_version / sha256 各 64 字符），单次 detect 元数据量 < 1 KB
  - **CB binary 脚本**：`zen-probe-health.ps1`（拉 /health 文本 + /health/info CB BLOB + /health/version 文本）/ `zen-probe-cache-stats.ps1`（拉 /stats CB + /stats/z$ CB）/ `zen-read-lockfile.ps1`（`[System.IO.File]::Open` 用 `FileShare.ReadWrite` 绕开 zen 进程独占锁，读 `.lock` 二进制）。CB / lockfile 用 base64 编码回传，Rust 侧 mini-parser 解码
- [ ] T1.9 CLI: `zen status` / `zen probe` / `zen cache-stats` / `zen detect-binary` / `zen list-endpoints` / `zen baseline list/lock/unlock`
- [ ] T1.10 Tauri commands 对应包装
- [ ] T1.11 验收：跑 `uecm-cli zen detect-binary --all --json` 在 5.7 机器上**同时拉到 install 路径 + InTree 两套 binary 元数据**；`zen probe` 写库（raw_cb BLOB + extracted flat 列）；`zen cache-stats` 拉到命中率；CB parser 单测覆盖 fixture 样本

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

**v4 修订**：`zen enable/disable` 的真实操作不是"在 INI 里启 zen"（5.4+ local zen 自动启），而是**配置 ZenShared upstream 指向 cluster master endpoint + 关闭 legacy SMB+Pak 路径**。

- [ ] T3.1 `core::zen::rules_loader` 读 `zen-ini-rules.yaml`（v4 schema：`enable_zen_shared` + `disable_legacy_smb_shared` + `disable_legacy_pak`），按 UE 版本匹配规则；处理 verified_versions / unverified_policy / overrides 三层
- [ ] T3.2 `core::zen::enable` 按规则改 `DefaultEngine.ini`：(a) 配 ZenShared.Host = cluster master endpoint；(b) 移除 legacy `Shared` SMB 配置（DefaultEngine.ini + user-level env var `UE-SharedDataCachePath`）；(c) 移除 legacy `Pak/CompressedPak` 配置。沿用 `core::ini_apply` 原子写入 + 备份
- [ ] T3.3 `core::zen::disable` 反向回滚：恢复 SMB+Pak 配置（从备份），清掉 ZenShared
- [ ] T3.4 PS 脚本：`zen-enable-project.ps1` / `zen-disable-project.ps1`（v4 同时处理 user-level env var 清理）
- [ ] T3.5 `core::cache_backend` 路由器 + 决策表实现；macOS 单测覆盖所有路径
- [ ] T3.6 `ddc generate/distribute` 加 `--backend` flag；`backend=zen` 走 no-op 路径返回 `{"backend":"zen","skipped":true}`
- [ ] T3.7 CLI: `zen enable` / `zen disable`
- [ ] T3.8 幂等性：`enable` 已是目标态返回 `{"changed":false}` 退出码 0
- [ ] T3.9 验收：5.4+ 工程选 5 台机器一键 enable；editor 启动 log 显示 `ZenShared: Using ZenServer HTTP service at <host>:<port> status: OK!`（替代当前 `ZenShared: Disabled because Host is set to 'None'`）；cluster master 的 /stats/z$ 命中率上涨

### M4 — INI Scanner + Health Check + Verify-rules [1 周]

- [ ] T4.1 INI Scanner 加 R012–R018，从 YAML 读规则（不 hardcode）。v4: R012-R014 改成 ZenShared 视角，R016/R018 接 install 路径表
- [ ] T4.2 Health Check 加 4 行：`zen_reachable` / `zen_version_consistent` / `zen_binary_intact` / `zen_cache_provider_ready`。v4: `zen_version_consistent` 比对 `machine_zen_install.zenserver_build_version`（不是 UE major/minor）
- [ ] T4.3 R016 binary_intact 接 `zen_binary_expected (zen_build_version, binary_kind)` 表；比对 install 路径 sha256；不一致告警优先于匹配。InTree 漂移仅日志不告警
- [ ] T4.4 `core::zen::verify` 实现：在 fixture 工程上跑 enable → 起 editor → 抓启动日志确认 `ZenShared: Using ZenServer HTTP service at <host>:<port> status: OK!` 出现 → 探测 cluster master /stats/z$ 命中率 > 0
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
| install 路径 binary 被外部工具替换 | R016 + Health Check `zen_binary_intact`（用 `zen_binary_expected (zen_build_version, binary_kind)` 表，对 install 路径 sha256）|
| 集群 install 路径 zen 版本不齐 | R018 + `zen_version_consistent`（v4: 比对 `machine_zen_install.zenserver_build_version`，不是 UE major/minor）|
| UE 升级导致 install 路径升版本（"最大见过版本"机制）| 信息性事件：detect 到 install version 升了自动写新 baseline + 通知集群其他机器 sync；UECM 不阻止 |
| InTree zen.exe 跟 install 版本不一致 | 正常状态（UE 升 InTree 但 install 已经更新），仅日志不告警；R016 不检查 InTree |
| http.sys URL 预留未做 | `apply-config` 内联 urlacl + 独立 `zen urlacl list` 排错（注：实测默认 asio，httpsys 是 opt-in）|
| operator 同时启 legacy SMB + ZenShared 双配置 | R014 检测 + 强制提示先 disable 一边 |
| 远端 zen 端口被防火墙挡 | M2 enable 流程加 `New-NetFirewallRule` |
| lua datadir 指到系统盘 | `apply-config` 校验 datadir 不在 `C:\Windows` / `C:\Program Files` 下 |
| zen CB schema drift（API 字段变了）| raw_cb BLOB 保留 + `schema_version` 标记 CB 兼容版本 + release 时跑 drift detection（mini-parser 遇未知字段告警）|
| port 实际 ≠ declared | probe 必读 lockfile 拿 effective_port（v4: Rust CB mini-parser 直接解 `.lock`）|
| service install --full 误用 | PS 脚本 hard-block `--full`；CLI 不暴露这个 flag |
| 拍摄现场断网 upstream zen 不可达 | 本机 zen 仍可用；Health Check 用降级状态而非 CRITICAL |
| UECM crash 时未完成 enable | `operations.status='running'` 超 1 小时自动标 `interrupted`，UI 提示重跑 |
| 多 editor 同启 zen 抢锁 | zen 本身 sponsor 机制处理 |
| secret 泄露 | §12 红线 + `zen/redaction.rs` |
| UE 新主版本 ZenShared 配置形态变了 | YAML `overrides` 加 entry；`unverified_policy=refuse` 兜底 |
| zen 升级后 CB 格式 incompat | mini-parser fixture 加新版本样本；schema_version bump；旧版 raw_cb 保留可读 |

---

## 12. Red Lines

1. **禁 `zen service install --full`**——会拷 binary 进系统目录，违背"用 UE 自带 zen"
2. **现有 3 个 flat extract 列冻结**——`zen_cache_stats.cache_hit_ratio` / `cache_disk_size_bytes` / `cache_memory_size_bytes` 是 baseline indexed columns；**未来不加新 flat metric 列**，新指标走 `raw_cb` BLOB + mini-parser 现取
3. **禁 port 硬编码**——所有 probe / register 必须接受显式 port 或读 lockfile（v4: 用 Rust CB mini-parser 解 `.lock`）
4. **禁绕过 DPAPI/cmdkey**——zen 远程操作必须走现有 `core/credentials.rs` + cmdkey 注入
5. **禁 secret 入日志**——zen `--access-token` / `--password` / `--api-key` 必须经 `zen/redaction.rs` 处理才进 operations.log_text
6. **禁动 install 路径和 InTree 路径下的任何 zen binary**——v4: 两个位置都只读，UECM 仅监测 hash 漂移不修复。install 路径（`%LOCALAPPDATA%\UnrealEngine\Common\Zen\Install\`）漂移触发 R016；InTree 路径（`Engine\Binaries\Win64\`）漂移仅日志
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

把 zen 当成 UECM 管的一种新 cache 后端。**zen 二进制零部署**（5.4+ UE editor 自带 sponsor 启动）、**实际工作 = 配置 ZenShared upstream + 清理 legacy SMB/Pak**（v4 实测修订）、**配置 YAML 化**（一份 5.4+ 通用规则 + verified 版本列表）、**未验证版本默认拒绝**、**老 SMB+Pak 路径完全保留**、**Rust CB mini-parser** 直接解 zen CB API + lockfile（v4）、**CLI 全暴露**（agent 直接 spawn 可用）。**总 6 周单人交付**（v4 因 CB parser 多半周），UE 新版本维护成本压到半天。
