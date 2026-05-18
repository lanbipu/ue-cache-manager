# Zen Cache 启动机制 — lanPC 真机 fact-finding

**M0 T0.4 / T0.4b deliverable，Plan 7 zen integration**

## 采集环境

| 项 | 值 |
|---|---|
| 时间 | 2026-05-18 |
| 机器 | lanPC（Windows 11 25H2，AMD Ryzen 9 7950X3D，NVIDIA RTX 3080，128 GB RAM）|
| UE 安装 | UE_4.27 / 5.1 / 5.2 / 5.3 / 5.4 / 5.5 / 5.6 / 5.7 / 5.8 全装于 `D:\Program Files\Epic Games\` |
| 活跃 zen sponsor 工程 | `E:\RenderStream Projects\test_0311\test_0311.uproject`（EngineAssociation = `"5.7"`）|
| zen 运行版本 | `5.8.10-202605071938-windows-x64-release-fbacdecd`（注意：跑在 UE 5.7 工程上 — 见 §3）|

## 1. 关键事实总览（颠覆 plan 原假设的）

1. **UE 5.4+ 默认就尝试启 local zen，不需要在工程 INI 里配任何东西**。`test_0311` 的 `DefaultEngine.ini` 里完全没有 DDC / Zen / InstalledDerivedDataBackendGraph section，zen 仍然完整启动并被 UE 用作 ZenLocal 后端。
2. **真正跑的 zenserver 在 `%LOCALAPPDATA%\UnrealEngine\Common\Zen\Install\`，不是 `Engine\Binaries\Win64\`**。UE 启动 zen 时按"install 版本 >= InTree 版本就用 install"决策。所有 UE 版本共享同一份 install。
3. **install 路径里的 binary 永远停在「最大见过版本」**：当前 install zenserver = 5.8.10（因为之前打开过 5.8 工程），即使现在跑 5.7 工程也用 5.8.10 zenserver。InTree 5.7.6 不会被回滚。
4. **zen HTTP API 全是 Compact Binary，不是 JSON**。除 `/health`（纯文本 `"OK!"`）和 `/health/version`（纯文本如 `"5.8.10"`）外，`/health/info` / `/stats` / `/stats/z$` 都是 CB 二进制流。**没有 `?format=json` 之类的开关。**
5. **`zen.exe status --format json` 选项不存在**。`zen.exe info` 也只接 `-u/--hosturl`，不接 `-p`。CLI 不暴露 JSON 转换能力。
6. **集群"共享 cache"在 UE 5.4+ 体系里的真实 key 叫 `ZenShared`**，不是 plan §6 假设的"在 InstalledDerivedDataBackendGraph 加 Zen entry"。Plan §6 yaml schema 整个方向错了。

## 2. zen daemon 启动机制（sponsor 模式）

### 2.1 谁触发 zen 启动？

UE 5.4+ 的 `UnrealEditor.exe` 启动时由 `LogZenServiceInstance` 子系统主动检查：

```
LogZenServiceInstance: Found registry key GlobalDataCachePath UE-LocalDataCachePath=F:/Epic/DDC
LogZenServiceInstance: Found local data cache path=F:/Epic/DDC
LogZenServiceInstance: Read zen version cache file from '.../Common/Zen/Install/zen.version', version: '5.8.10-...'
LogZenServiceInstance: Installed service at '.../Common/Zen/Install/zenserver.exe' is up to date
LogZenServiceInstance: No current process using the data dir found, launching a new instance
LogZenServiceInstance: Launching executable '.../Common/Zen/Install/zenserver.exe', working dir '.../Common/Zen/Install', data dir 'F:/Epic/DDC/Zen', args '...'
```

### 2.2 启动命令（活样本，来自 test_0311 sponsor）

```
zenserver.exe
  --port 8558
  --data-dir F:\Epic\DDC\Zen
  --http asio
  --gc-cache-duration-seconds 1209600
  --gc-interval-seconds 21600
  --gc-low-diskspace-threshold 2147483648
  --cache-bucket-limit-overwrites
  --quiet
  --http-forceloopback
  --owner-pid 27304
  --child-id Zen_27304_Startup
```

- `--owner-pid` = `UnrealEditor.exe` PID。Editor 退出 zen 跟着退（sponsor 机制）。
- `--child-id Zen_<owner-pid>_Startup` = 父子关联 token。
- `--http-forceloopback` = 默认只绑 127.0.0.1（v4）+ `[::1]`（v6）。**集群跨机访问需要去掉这个 flag** —— Plan §M2 endpoint 配置的关键点。
- `--http asio` 而不是 `httpsys` —— UE 5.7 默认 asio。Plan §1.1 `zen_endpoints.httpserverclass DEFAULT 'httpsys'` 假设错了，默认应该是 `asio`。
- 父进程：`UnrealEditor.exe` PID 27304 with `e:\RenderStream Projects\test_0311\test_0311.uproject`。

### 2.3 data-dir 父路径来源

`F:/Epic/DDC` 来自 Windows registry key `UE-LocalDataCachePath`（用户级），zen 自己在父目录下建 `Zen\` 子目录跑。这个 registry 在 UECM 控制流里是 **user-level config**，跟工程 INI 解耦。

## 3. install 路径机制（plan §1.1 / R016 / R018 重大修订）

### 3.1 路径

```
%LOCALAPPDATA%\UnrealEngine\Common\Zen\Install\
├── zen.exe                        (12,411,320 bytes, 5.8.10)
├── zenserver.exe                  (14,865,848 bytes, 5.8.10)
├── zen.version                    ("5.8.10-202605071938-windows-x64-release-fbacdecd")
├── zenserver.runcontext           (520 bytes — sponsor 上下文?)
├── AndroidPortForwarder2.dll
├── crashpad_handler.exe
├── OidcToken.exe
├── security-config.json
├── zen_plugins_v1.json
└── zen_plugin_versions.json
```

### 3.2 install 版本选择决策

来自 test_0311 log，UE 5.7 启动时：

```
InTree    'D:/.../UE_5.7/Engine/Binaries/Win64/zenserver.exe' → 5.7.6-...
Installed 'C:/.../Common/Zen/Install/zenserver.exe'           → 5.8.10-...
Installed service ... is up to date  ← 用 5.8.10 跑（不回滚）
```

`ZenServiceInstance` 比较 InTree 和 Installed 版本，取较新者。**这意味着集群里 zen 版本一致性的真相**：

- 集群里只要任意一台跑过 5.8（哪怕一次），那台的 install 路径就被升到 5.8.10；
- 之后该机器跑任何 UE 工程都用 5.8.10 zenserver；
- **集群一致 = 所有机器的 install 路径 zen build version 一致**，跟工程 UE 版本无关；
- 让集群一致的实际做法：手工 sync 各机器的 `Common\Zen\Install\` 内容（rsync / SMB copy），或者让每台机器各开一次最新版 UE。

### 3.3 数据目录布局

`F:\Epic\DDC\Zen\` 内容（活样本）：

| 项 | 类型 | 大小 / 备注 |
|---|---|---|
| `.lock` | 文件 | 163 bytes，运行时 modified —— **lockfile，CB 格式** |
| `state_marker` | 文件 | 63 bytes，运行时 modified |
| `root_manifest` | 文件 | 75 bytes，建表时 modified |
| `auth/` | 目录 | 50 bytes（token 缓存）|
| `cache/` | 目录 | 601 MB —— project store namespace 数据 |
| `cas/` | 目录 | 585 MB —— content-addressed storage 数据 |
| `gc/` | 目录 | 277 MB —— garbage collection 状态 |
| `logs/` | 目录 | 937 KB —— zenserver.log 等 |
| `sessions/` | 目录 | 303 KB —— 活跃会话 |
| `.sentry-native/` | 目录 | crash report 工作目录 |

**lockfile = `.lock` 文件**，163 字节 CB 格式。`zen.exe status --data-dir F:\Epic\DDC\Zen` 居然返回 `No Zen state found` —— CLI 自己读不出来。**确认 Rust mini-parser 是唯一可靠路径解析这个文件。**

## 4. HTTP API 真实形态（plan §1.1 raw_json 列重大修订）

| 端点 | 响应类型 | 实测样本 |
|---|---|---|
| `/health` | 纯文本 | `OK!` |
| `/health/version` | 纯文本 | `5.8.10` |
| `/health/info` | **Compact Binary** | 含字段 `DataRoot` / `AbsLogPath` / `BuildVersion` / `HttpServerClass` / `Port` / `Pid` / `IsDedicated` / `StartTimeMs` / `RuntimeConfig` / `SystemRootDir` / `EffectivePort` / `BasePort` / `CoreLimit` / `MemoryAllocator` / `AsioVersion` / `IsDebug` / `Hostname` / `IpAddresses` / `Platform` / `Arch` / `OS` / `System` (cpu/memory) / `BuildConfig` 等 |
| `/stats` | **Compact Binary** | 含 `providers` 数组，活样本里 providers = `["dashboard", "http", "prj", "sessions", "ws", "z$"]` |
| `/stats/z$` | **Compact Binary** | 含 `requests` (count + rate_mean/14/5/15 + t_avg/min/max/p75/p95/p99/p999) + `cache` (hit_ratio / hits / misses / writes / size{disk,memory} / cidhits / cidmisses / cidwrites + cid1 size dist) + `rpc.records.ops` + `rpc.values.ops` |

### 4.1 CB 格式快速识别

字段名以 ASCII 字符串嵌在 binary 里，可以用 `strings`-style 工具肉眼快速识别字段集。完整 CB 解码（type byte + var-len encoding）规则 Epic 在 UE 源码 `Engine/Source/Runtime/Core/Public/Serialization/CompactBinary.h` 有完整定义。

UECM Rust mini-parser 只需要：
- read root object → field iter
- 对每个 field：read name (length-prefixed UTF-8) + read value by type byte
- 支持 type：null / int / uint / string / object / array / float（够当前所有探测字段）

工程量评估：**~300-500 行 Rust，2-3 天**，包括 fuzz test 和针对 plan §1.1 字段的提取 helper。

### 4.2 字段提取与 plan §1.1 表的对应

| plan §1.1 zen_probes 字段 | 来源 |
|---|---|
| `effective_port` | `/health/info` → `RuntimeConfig.EffectivePort` |
| `pid` | `/health/info` → `Pid` |
| `data_root` | `/health/info` → `DataRoot` |
| `is_dedicated` | `/health/info` → `IsDedicated` |
| `build_version` | `/health/info` → `BuildVersion`（或 `/health/version` 纯文本） |
| `uptime_seconds` | `/health/info` → `System.uptime_seconds` |
| `health_info_json` | **建议改名为 `health_info_cb BLOB`**（CB 二进制存原文，schema_version 标记 CB version 兼容） |
| `health_version_json` | **建议改名为 `health_version_text`**（纯文本短字符串） |
| `stats_providers_json` | `/stats` → `providers` 数组，**改名为 `stats_providers_cb BLOB`** 或干脆 extract 成 `provider_list TEXT`（逗号分隔） |

| plan §1.1 zen_cache_stats 字段 | 来源 |
|---|---|
| `cache_hit_ratio` | `/stats/z$` → `cache.hit_ratio` |
| `cache_disk_size_bytes` | `/stats/z$` → `cache.size.disk` |
| `cache_memory_size_bytes` | `/stats/z$` → `cache.size.memory` |
| `raw_json` | **改名为 `raw_cb BLOB`**，存 `/stats/z$` 原始 CB 流 |

`schema_version` 含义改为「记录 CB schema 兼容版本」，drift detection 仍然有效（zen API 改字段名 → mini-parser 报字段未知 → schema_version bump）。

## 5. zen.exe CLI 真实能力（plan T0.4b 修订）

### 5.1 完整命令列表（来自 5.7.4 InTree `zen.exe --help`）

```
cache store commands:
  builds              Manage builds (list, ls, upload, download, ...)
  cache-details       Details on cache
  cache-gen           Generates cache values into a bucket
  cache-get           Get cache values/records or attachments
  cache-info          Info on cache, namespace or bucket
  cache-stats         Stats on cache
  drop                Drop cache namespace or bucket
  rpc-record-replay   Replays a previously recorded session of rpc requests
  rpc-record-start    Starts recording of cache rpc requests on a host
  rpc-record-stop     Stops recording of cache rpc requests on a host
  workspace           Manage workspaces (create, remove, info)
  workspace-share     Manage workspace shared folders

general commands:
  attach              Add a sponsor process to a running zen service
  down                Bring zen server down
  info                Show high level Zen server information
  jobs                Show/cancel zen background jobs
  logs                Show/control zen logging
  ps                  Enumerate running zen server instances
  serve               Serve files from a directory
  service             Manage zenserver as a service (status, install, uninstall, start, stop)
  status              Show zen status
  top                 Monitor zen server activity
  trace               Control zen realtime tracing
  up                  Bring zen server up
  version             Get zen service version

project store commands:
  oplog-create, oplog-download, oplog-export, oplog-import,
  oplog-mirror, oplog-snapshot, oplog-validate,
  project-create, project-drop, project-info,
  project-op-details, project-stats

storage management commands:
  copy-state          Copy zen server disk state
  flush               Flush storage
  gc                  Garbage collect zen storage
  gc-status           Garbage collect zen storage status check
  gc-stop             Request cancel of running garbage collection
  scrub               Scrub zen storage (verify data integrity)
```

### 5.2 关键 CLI 用法实测

```
$ zen.exe version                                           → "5.7.6"
$ zen.exe service status                                    → "Service 'ZenServer' is not installed"  (sponsor 模式下不算 service)
$ zen.exe ps                                                → "No Zen state found"  (CLI 找不到运行中的 zen)
$ zen.exe status                                            → "No Zen state found"
$ zen.exe status --data-dir F:\Epic\DDC\Zen                 → "No Zen state found"  (即使 data-dir 正确)
$ zen.exe info -p 8558                                      → Error: Option 'p' does not exist
$ zen.exe info -u http://127.0.0.1:8558                    → 未实测，预期 OK
$ zen.exe info --help                                       → 只有 -u/--hosturl，无 -p
$ zen.exe status --format json                              → Error: Option 'format' does not exist
```

**结论**：zen.exe CLI 在 sponsor 模式下基本"看不见"运行中的 daemon。`ps` / `status` / `service status` 都失败。要拿运行态唯一可靠路径：

- **HTTP API**（解 CB 拿 effective_port、pid、build_version 等）
- **lockfile 直接读 .lock 文件**（mini-parser 解 CB）

### 5.3 plan §T0.4b "实测 zen status --format json" 的最终结论

**该 flag 不存在，整个原计划路径破产**。Fallback 方案是 plan T0.4b 第二选项："Rust Compact Binary mini-parser"，已经选为正式方案（user 2026-05-18 决定）。

## 6. zen 启用配置的真实落点（plan §6 重写依据）

### 6.1 local zen 启用 — 不需要任何 INI 改动

UE 5.4+ 启动 editor 时 `ZenServiceInstance` 主动尝试启 zen，触发条件：
- registry `UE-LocalDataCachePath`（用户级，定义 data-dir 父路径）或环境变量等价物存在；
- `%LOCALAPPDATA%\UnrealEngine\Common\Zen\Install\zenserver.exe` 存在（首次打开 5.4+ editor 时自动从 InTree extract）。

**没有任何工程级 INI 开关控制"启 / 不启 local zen"**。要禁用 local zen 必须删 registry / 改环境变量（user-level 操作，跟工程解耦）。

### 6.2 集群共享 zen 的真实 key 是 `ZenShared`

来自 test_0311 log 1013：

```
LogDerivedDataCache: ZenShared: Disabled because Host is set to 'None'
```

`ZenShared` 是 UE 5.4+ 内置的"共享 zen upstream" hierarchy 节点。当前 lanPC 上 `Host=None` → 禁用，所以用 SMB Shared `\\192.168.10.2\Docs\DDC` 兜底。

**要落地 plan 目标"5–20 台 render node 不再各自维护 SMB+Pak"，正确配置是**：

- 集群里挑 1-3 台作 zen master host（local zen + 暴露非 loopback）；
- 每台 render node 配 `ZenShared.Host = <master-zen-host>`；
- 配完 render node 走 ZenShared upstream，本机 local zen 仍然作 L1 cache，但 cache miss 会去 master 拉，集群整体一份；
- 同步禁用 SMB Shared 配置。

### 6.3 `ZenShared` 配置形态（待 fact-find）

**这一波探测没拿到 ZenShared 启用形态**（test_0311 没启用 ZenShared）。需要后续小 fact-find 确认：UE 5.4+ 在 INI 哪个 section、哪个 key、什么 value template 启 ZenShared？

候选位置（按 plan 假设的"InstalledDerivedDataBackendGraph"思路套到 ZenShared）：

```ini
[InstalledDerivedDataBackendGraph]
ZenShared=(Type=Zen, Host="render-host-1", Port=8558, ...)
```

或者按 UE 5.4+ 新 hierarchy：

```ini
[/Script/Engine.DerivedDataCacheSettings]
ZenShared.Host=render-host-1
ZenShared.Port=8558
```

**M0 收尾再补一次定向 fact-find**：人为在 lanPC 上开启 ZenShared 配置（指向自己 / 指向 NAS）、抓 log 看 ZenShared 加载逻辑、确认 INI 写法。

## 7. plan 修订影响清单

逐条对应 plan §x.y，详见下方"plan 改动"部分。

| plan 位置 | 改动 |
|---|---|
| §1 第 4 条 key decision (zen status --format json) | 改为「Rust CB mini-parser 直接解 lockfile + HTTP API」 |
| §1 第 5 条 (HTTP API 形态) | 加注「/health/info / /stats /stats/z$ 返回 CB 不是 JSON」 |
| §1 第 6 条 (raw_json) | 改成 raw_cb BLOB + schema_version 含义 |
| §1.1 zen_probes 表 | health_info_json → health_info_cb BLOB；health_version_json → health_version_text；stats_providers_json → stats_providers_cb BLOB（或 provider_list TEXT 逗号分隔） |
| §1.1 zen_endpoints.httpserverclass DEFAULT 'httpsys' | 改成 DEFAULT 'asio'（UE 5.7 实测默认 asio）|
| §1.1 zen_cache_stats.raw_json | 改成 raw_cb BLOB |
| §1.1 zen_binary_expected 表 | PK 改为 (zen_build_version, binary_kind)，记 install 路径副本 sha256；UE 自带源版本另设 zen_binary_intree 表作 reference |
| §1.2 现有 zen_cli_path / zen_cli_version / zen_cli_sha256 / zenserver_path / ... | 改为 install 路径优先；新增 install_path / install_version / install_sha256 字段，原 InTree 字段保留 |
| §5 R016 binary intact | 比对的是 install 路径 sha256（按 zen_build_version），不是 UE 自带的 |
| §5 R018 集群版本一致 | 比对每台机器 install zen build version，不是 UE major/minor |
| §6 yaml schema | 重写：`enable_zen_shared` + `disable_legacy_smb_shared` + `disable_legacy_pak` 替换 `enable_zen_backend` / `set_default_graph` |
| §6 verified_versions / unverified_policy / overrides | 机制保留，但 verified 含义变成「ZenShared 在该 UE 版本的 INI 形态已实测」 |
| §M0 T0.4b | 改为「实测 .lock 文件 CB 格式 + 实测 /health/info CB 字段集，作为 mini-parser fixture」 |
| §M1 T1.3 lockfile 模块 | 改为「Rust CB mini-parser 解 .lock 文件」 |
| §M1 T1.4 probe | 改为「拉 /health 文本 + /health/version 文本 + /health/info CB 解 + 提取字段」 |
| §M1 T1.5 cache_stats | 改为「拉 /stats CB 解出 providers + 拉 /stats/z$ CB 解 cache 指标」 |
| §M1 T1.6 binary detect | 改为「扫 install 路径 + 扫 InTree 各 UE 版本，双 sha256 记录」 |
| §M3 T3.1-T3.3 zen enable/disable | 改为「ZenShared 配置开启 / 关闭 + legacy SMB+Pak 路径清理」 |
| §M4 T4.1 R012 | 改为「工程是否配置 ZenShared upstream」 |
| §M4 T4.1 R013 | 改为「ZenShared.Host 指向集群预期 master endpoint」 |
| §M4 T4.1 R014 | 改为「ZenShared + SMB Shared 双配置冲突」 |
| §M4 T4.3 R016 binary intact 实现 | 比对 install 路径副本 sha256 |
| §11 风险 "集群 zen 版本不齐" | 对策改为「同步各机器 install 路径版本」 |
| §11 风险 "UE 升级覆盖 zen.exe" | 对策改为「监测 install 路径 sha256 漂移；InTree 漂移仅作信息」 |
| §12 Red Line #6 "禁动 zen.exe / zenserver.exe" | 措辞改为「禁动 InTree 和 install 路径下的 zen binary —— UECM 仅监测不修复 hash 不一致」 |

## 8. T0.5 ZenShared 定向 fact-find — 结果

2026-05-18 在 lanPC sandbox `E:\Sandbox\uecm_t05_zen_shared`（从 UE 5.7 自带 `TP_BlankBP` 复制改名）跑完，结论如下：

### 8.1 候选 A 一击中

在 sandbox 工程的 `DefaultEngine.ini` 末尾追加：

```ini
[InstalledDerivedDataBackendGraph]
ZenShared=(Type=Zen, Host="127.0.0.1", Port=8558, Namespace="ue.ddc")
```

启 UE 5.7.4 editor headless 模式（`-Unattended -NoSplash -NoSound -nullrhi -Quit -log`），日志直接出现成功证据：

```
[10:18:05] LogDerivedDataCache: Display: ZenShared: Using ZenServer HTTP service at 127.0.0.1 with namespace ue.ddc status: OK!.
```

对比 test_0311（无 ZenShared 配置）的日志：

```
LogDerivedDataCache: ZenShared: Disabled because Host is set to 'None'
```

**候选 A 的 section/key/value_template 是 5.7.4 的正确写法**，已落 `docs/research/zen-ini-rules.yaml` 作为 verified rule。候选 B（`[/Script/Engine.DerivedDataCacheSettings]` section）不需要尝试。

### 8.2 意外发现：多 editor 启动 zen 是「抢占」不是「共享」

T0.5 sandbox 启动时同时被监测到：

```
[10:18:03] LogZenServiceInstance: Warning: Found locked valid lock file 'F:/Epic/DDC/Zen/.lock' but can't find registered process (Pid: 31792), will attempt shut down
[10:18:03] LogZenServiceInstance: Warning: Found locked but invalid lock file at 'F:/Epic/DDC/Zen/.lock', attempting shut down of zenserver process with pid 31792
[10:18:03] LogZenServiceInstance: Display: Attempting termination of zenserver process with pid 31792
[10:18:03] LogZenServiceInstance: Display: Successfully shut down zenserver process with pid 31792
[10:18:03] LogZenServiceInstance: Display: Launching executable '...\Common\Zen\Install\zenserver.exe', ... --owner-pid 38024 --child-id Zen_38024_Startup
```

PID 31792 是被 test_0311 editor (PID 27304) sponsor 的 zenserver；sandbox UE (PID 38024) 检测到 lockfile 有效但「无法找到注册的进程」就直接 shutdown 然后重启 zen 以自己为 owner。

**含义**：plan §11 风险表 "多 editor 同启 zen 抢锁 — zen 本身 sponsor 机制处理" 的措辞不准确。**实际行为是抢占（preemption），不是协作**。对集群设计的含义：

- Render node 上每台机器只能有一个 UE editor 拥有 local zen sponsor；
- Cluster master 必须用 `zen service install`（plan §1.1 `lifecycle_mode='installed_service'`）跑 zen，避免被 editor sponsor 抢占；
- 任何 sponsor-mode zen 一旦被新 editor 启动撞上，老 zen 会被 shutdown — 这会让老 editor 此后访问 zen 失败直到它自己再 sponsor 一次（UE 5.4+ 是 lazy 重连，不立即 fatal）。

### 8.3 意外发现：`-Quit` headless flag 在 5.7.4 不立即退出

启动参数包含 `-Unattended -NoSplash -NoSound -nullrhi -Quit -log`，但 editor 在 `LogInit: Engine is initialized` 之后（~20 秒）**继续**加载 asset registry / Python plugins / Pak 等，3 分钟超时仍未退出，被外部 kill 强杀。

**含义**：plan §M4 T4.4 `core::zen::verify` 实现 headless verify 时**不能依赖 `-Quit`**。可选实现：

- **a) commandlet 模式**：`UnrealEditor-Cmd.exe <uproject> -run=Help`（或别的 commandlet）跑完自动退出；
- **b) 流式监控**：启 editor 同时实时 tail stdout，看到 `LogDerivedDataCache: ... ZenShared: Using ZenServer ... status: OK!` 立刻 kill；
- **c) 死等超时**：固定 N 秒后 kill，从 log 里 grep 验证字段（最简单但浪费时间）。

T4.4 推荐 b) 加 a) fallback。

### 8.4 副作用：test_0311 zen sponsor 被替换

T0.5 sandbox 跑完被 kill 后，由 sandbox UE 拉起的 zenserver (owner-pid 38024) 也跟着死（sponsor UE 退出 zen 跟着退）。结果：

- 端口 8558 当前**没人 listen**；
- test_0311 UE editor (PID 27304) 仍在跑但失去了 zen sponsor；
- UE 5.4+ lazy 重连：test_0311 editor 下次需要 cache 时会重新 sponsor 一个新 zen，不需要重启 editor。

若 user 立即在 test_0311 editor 上发起 cook / build / package 之类操作，会触发新 zen sponsor，状态自愈。

## 附录 A：完整启动 log（test_0311 / UE 5.7 / 2026-05-18 14:18:10）

仅 zen 启动相关行（log 行号见原 log）：

```
986: LogDerivedDataCache: Display: Memory: Max Cache Size: -1 MB
987: LogDerivedDataCache: FDerivedDataBackendGraph: Pak pak cache file ... not found
991: LogDerivedDataCache: Display: ../../../Engine/DerivedDataCache/Compressed.ddp: Opened pak cache for reading. (1729 MiB)
994: LogZenServiceInstance: Found registry key GlobalDataCachePath UE-LocalDataCachePath=F:/Epic/DDC
996: LogZenServiceInstance: Found local data cache path=F:/Epic/DDC
997: LogZenServiceInstance: Display: Launching zen utility 'D:/.../UE_5.7/Engine/Binaries/Win64/zen.exe service status'.
999: LogZenServiceInstance: Display: Read zen version cache file from 'C:/.../UnrealEngine/5.7/Saved/Zen/zen.version', version: '5.7.6-...'
1000: LogZenServiceInstance: InTree version at 'D:/.../UE_5.7/Engine/Binaries/Win64/zenserver.exe' is '5.7.6-...'
1001: LogZenServiceInstance: Display: Read zen version cache file from 'C:/.../Common/Zen/Install/zen.version', version: '5.8.10-...'
1002: LogZenServiceInstance: Installed version at 'C:/.../Common/Zen/Install/zenserver.exe' is '5.8.10-...'
1003: LogZenServiceInstance: Display: Installed service at 'C:/.../Common/Zen/Install/zenserver.exe' is up to date
1004: LogZenServiceInstance: No current process using the data dir found, launching a new instance
1005: LogZenServiceInstance: Display: Launching executable 'C:/.../Common/Zen/Install/zenserver.exe', working dir '...', data dir 'F:/Epic/DDC/Zen', args '--port 8558 --data-dir F:\Epic\DDC\Zen --http asio --gc-cache-duration-seconds 1209600 --gc-interval-seconds 21600 --gc-low-diskspace-threshold 2147483648 --cache-bucket-limit-overwrites --quiet --http-forceloopback --owner-pid 27304  --child-id Zen_27304_Startup'
1006: LogZenServiceInstance: Display: Unreal Zen Storage Server HTTP service at http://[::1]:8558 status: OK!.
1007: LogZenServiceInstance: Local ZenServer AutoLaunch initialization completed in 3.095 seconds
1008: LogDerivedDataCache: Display: ZenLocal: Using ZenServer HTTP service at [::1] with namespace ue.ddc status: OK!.
1013: LogDerivedDataCache: ZenShared: Disabled because Host is set to 'None'
1015: LogDerivedDataCache: Shared: Found environment variable UE-SharedDataCachePath=\\192.168.10.2\Docs\DDC
1019: LogDerivedDataCache: Shared: Using data cache path //192.168.10.2/Docs/DDC: Writable
```

## 附录 C：Mini-parser fixture（M0 2026-05-18 真机抓取的 raw CB payload，base64 编码）

T1.2b `core::zen::cb_parser` 的单测和 fuzz 直接用以下 fixture（写到 `src-tauri/src/core/zen/cb_parser/fixtures/` 下，文件名见每条 "fixture name"）。所有样本来自 lanPC zen daemon 5.8.10 在 test_0311 工程下的真实运行态。

### C.1 `/health` —— 纯文本（fixture name: `health_ok.txt`）

```
Content-Type: text/plain
Length: 3 bytes
SHA256: c79a4f748fb9d3b7d944cc1037977d40d3a3e56da9bbd6f6aa81f447b2cb7fc5
Base64: T0sh
Hex:    4f4b21        # = "OK!"
```

### C.2 `/health/version` —— 纯文本（fixture name: `health_version.txt`）

```
Content-Type: text/plain
Length: 6 bytes
SHA256: 8cb3f46b98c55b7d099e6fb22b7d73f69c7dc0bfc4a6e83ba13aa45ffd2cae41
Base64: NS44LjEw
Hex:    352e382e3130        # = "5.8.10"
```

### C.3 `/health/info` —— Compact Binary（fixture name: `health_info.cb`）

```
Content-Type: application/x-ue-cb
Length: 1160 bytes
SHA256: 925cb272d13b9b93d3215d7db6519ee8575af33d293b16e6ec3c71625883dc14
```

Base64（单行）:

```
AoSFhwhEYXRhUm9vdBNcXD9cRjpcRXBpY1xERENcWmVuhwpBYnNMb2dQYXRoJlxcP1xGOlxFcGljXEREQ1xaZW5cbG9nc1x6ZW5zZXJ2ZXIubG9nhwxCdWlsZFZlcnNpb24wNS44LjEwLTIwMjYwNTA3MTkzOC13aW5kb3dzLXg2NC1yZWxlYXNlLWZiYWNkZWNkhw9IdHRwU2VydmVyQ2xhc3MEYXNpb4gEUG9ydKFuiANQaWTAfDCMC0lzRGVkaWNhdGVkiAtTdGFydFRpbWVNc/meObwl7IMNUnVudGltZUNvbmZpZ4Flhw1TeXN0ZW1Sb290RGlyF0M6XFByb2dyYW1EYXRhXEVwaWNcWmVuCkNvbnRlbnREaXIADUVmZmVjdGl2ZVBvcnQEODU1OAhCYXNlUG9ydAQ4NTU4CUNvcmVMaW1pdAEwD01lbW9yeUFsbG9jYXRvcg5taW1hbGxvYyAyLjIuNwtBc2lvVmVyc2lvbgYxLjM4LjAHSXNEZWJ1ZwVmYWxzZQxJc0NsZWFuU3RhcnQFZmFsc2UGSXNUZXN0BWZhbHNlBkRldGFjaAR0cnVlD05vQ29uc29sZU91dHB1dAVmYWxzZQxRdWlldENvbnNvbGUEdHJ1ZQdDaGlsZElkEVplbl8yNzMwNF9TdGFydHVwBUxvZ0lkAApTZW50cnkgRFNOB25vdCBzZXQSU2VudHJ5IEVudmlyb25tZW50AA5TdGF0c2QgRW5hYmxlZAVmYWxzZRJTZWN1cml0eUNvbmZpZ1BhdGgAggtCdWlsZENvbmZpZ4ELjBVaRU5fQUREUkVTU19TQU5JVElaRVKMFFpFTl9USFJFQURfU0FOSVRJWkVSjBRaRU5fTUVNT1JZX1NBTklUSVpFUowSWkVOX0xFQUtfU0FOSVRJWkVSjQ5aRU5fVVNFX1NFTlRSWYwOWkVOX1dJVEhfVEVTVFONEFpFTl9VU0VfTUlNQUxMT0ONEFpFTl9VU0VfUlBNQUxMT0ONEFpFTl9XSVRIX0hUVFBTWVONEVpFTl9XSVRIX01FTVRSQUNLjQ5aRU5fV0lUSF9UUkFDRYwZWkVOX1dJVEhfQ09NUFVURV9TRVJWSUNFU4wOWkVOX1dJVEhfSE9SREWMDlpFTl9XSVRIX05PTUFEhwhIb3N0bmFtZQVMQU5QQ4ULSXBBZGRyZXNzZXMQAQcNMTkyLjE2OC4xMC4yMIcIUGxhdGZvcm0Hd2luZG93c4cEQXJjaAN4NjSHAk9TGFdpbmRvd3MgMTAuMCBCdWlsZCAyNjIwMIMGU3lzdGVtgK6ICWNwdV9jb3VudAEKY29yZV9jb3VudBAIbHBfY291bnQgD3RvdGFsX21lbW9yeV9tYsD8QA9hdmFpbF9tZW1vcnlfbWLAjcgQdG90YWxfdmlydHVhbF9tYuf///8QYXZhaWxfdmlydHVhbF9tYuf/4pcRdG90YWxfcGFnZWZpbGVfbWLCPEARYXZhaWxfcGFnZWZpbGVfbWLBs58OdXB0aW1lX3NlY29uZHPAb70=
```

字段断言（**值已用 T1.2b cb_parser 在真实 bytes 上验证，覆盖 doc 此前的猜测**）：

| 字段路径 | 类型 | 期望值 |
|---|---|---|
| `DataRoot` | string | `\\?\F:\Epic\DDC\Zen` |
| `AbsLogPath` | string | `\\?\F:\Epic\DDC\Zen\logs\zenserver.log` |
| `BuildVersion` | string | `5.8.10-202605071938-windows-x64-release-fbacdecd` |
| `HttpServerClass` | string | `asio` |
| `Port` | uint16 | `8558` |
| `Pid` | uint32 | `31792`（运行时 zen sponsor 的 zenserver PID）|
| `IsDedicated` | bool | `false`（sponsor 模式而非 dedicated service）|
| `StartTimeMs` | uint64 | `~1747613192812` (ms since epoch, dynamic) |
| `RuntimeConfig.EffectivePort` | string | `8558` |
| `RuntimeConfig.BasePort` | string | `8558` |
| `RuntimeConfig.MemoryAllocator` | string | `mimalloc 2.2.7` |
| `RuntimeConfig.ChildId` | string | `Zen_27304_Startup`（27304 是 UE editor parent PID）|
| `Hostname` | string | `LANPC` |
| `IpAddresses[0]` | string/bytes | `192.168.10.20` |
| `Platform` | string | `windows` |
| `Arch` | string | `x64` |
| `OS` | string | `Windows 10.0 Build 26200` |
| `System.cpu_count` | uint | `1` |
| `System.core_count` | uint | `16` |
| `System.lp_count` | uint | `32` |
| `System.uptime_seconds` | uint | `~3000+` (动态值) |

> T1.2b 修正记录：原表此前对 `BuildVersion`（前导 `0`）、`Pid`（27304 = editor parent，非 zen PID）、`IsDedicated`（sponsor 模式实际 `false`）三个值是 doc 编写时的字节序 / 字符串猜测；mini-parser 实际解码以这里为准。

### C.4 `/stats` —— Compact Binary（fixture name: `stats.cb`）

```
Content-Type: application/x-ue-cb
Length: 50 bytes
SHA256: def3e9658d31b866b9b56e023a109654f129b418ab14679cc1fd817b354df2fe
Base64: AzCFCXByb3ZpZGVycyQGBwlkYXNoYm9hcmQEaHR0cANwcmoIc2Vzc2lvbnMCd3MCeiQ=
Hex:    0330850970726f7669646572732406070964617368626f61726404687474700370726a0873657373696f6e73027773027a24
```

字段断言：

| 字段路径 | 类型 | 期望值 |
|---|---|---|
| `providers` | array<string> | `["dashboard", "http", "prj", "sessions", "ws", "z$"]` |

### C.5 `/stats/z$` —— Compact Binary（fixture name: `stats_z.cb`）

```
Content-Type: application/x-ue-cb
Length: 481 bytes
SHA256: 06aa5a6f49953ddd7428697e8fc78cc4717cf6b3b9df7a47bfdac1e11afebfcd
```

Base64（单行）:

```
AoHegghyZXF1ZXN0c4C3iAVjb3VudINZiwlyYXRlX21lYW4/sRvepVM8E4sGcmF0ZV8xMdJAynniMQGLBnJhdGVfNTzUFQfVPXqeiwdyYXRlXzE1PrFufSN0GBmLBXRfYXZnP/3Cj4aCHnaLBXRfbWluPuI+xuUsPyOLBXRfbWF4QEqkuVh7whiLBXRfcDc1P5dCyn/rcqqLBXRfcDk1QCeU9w+WdfiLBXRfcDk5QEPpUaeSgLKLBnRfcDk5OUBJ+/x3tUNDggVjYWNoZYDbiA9iYWRyZXF1ZXN0Y291bnQAggNycGNXiAVjb3VudIMpiANvcHOFhoMHcmVjb3Jkcw+IBWNvdW50gmgDb3BzhMWDBnZhbHVlcw+IBWNvdW50gMEDb3BzgMGDBmNodW5rcw2IBWNvdW50AANvcHMAgwRzaXplFYgEZGlza/Ajm9ljBm1lbW9yecVnMIgEaGl0c4VdiAZtaXNzZXMniAZ3cml0ZXMCiwloaXRfcmF0aW8/7x271Hy8jogHY2lkaGl0c4nHiAljaWRtaXNzZXMAiAljaWR3cml0ZXMAgwNjaWQxgwRzaXplKogEdGlueeBu5XEFc21hbGzwHJ6sUAVsYXJnZeWSyLEFdG90YWzwIqBacg==
```

字段断言（**值已用 T1.2b cb_parser 解码验证**；顶层 fields = `{requests, cache, cid}`，**注意 `rpc.*` 嵌套在 `cache.rpc.*` 下，不是顶层**）：

| 字段路径 | 类型 | 期望值 |
|---|---|---|
| `requests.count` | uint | `89` (0x59) |
| `cache.size.disk` | uint64 | `~9.4 GB` (跨 GB scale，bit-exact 见 parser test fixture) |
| `cache.size.memory` | uint | `~13 MB` |
| `cache.hits` | uint | `93` (0x5d) |
| `cache.misses` | uint | `39` (0x27) |
| `cache.writes` | uint | `2` |
| `cache.hit_ratio` | float64 | `0.9723796033994334` (bit pattern `0x3fef1dbbd47cbc8e`, IEEE 754 — bit-exact 单测断言) |
| `cache.cidhits` | uint | (运行时累计) |
| `cache.rpc.count` | uint | `87` (0x57) — 注意 RPC 计数在 `cache.rpc` 下，不在顶层 |

> T1.2b 修正记录：原表把 `rpc.count` 误标在顶层；实际 zen 在 `cache.rpc` 下嵌套。`cid` 顶层 field 的子结构细节（`cid1.size.tiny` 等）按 parser 真实解码补充，本表只列被 plan §1.1 frozen baseline 直接消费的字段。

### C.6 `.lock` lockfile —— 尚未抓到（M1 T1.3 implementation 时补）

`F:\Epic\DDC\Zen\.lock` 当前被 zen daemon 持完全独占锁（FileShare.None），普通 `[System.IO.File]::Open` 用 ReadWrite/Read 模式都失败：

```
Open: 文件正由另一进程使用，无法访问
```

破解需要 Win32 `BackupRead` API（要 `SeBackupPrivilege`，admin token）或 `volsnap` shadow copy。这是 M1 T1.3 lockfile 模块的 Windows 实现细节，**fixture 留到 T1.3 实现时在 Windows test host 上专门停一次 zen 抓副本**。

Mock 替代：CB parser 单测主路径已经被 C.3 / C.4 / C.5 覆盖；lockfile 单独的字段（effective_port / pid / executable）都是简单的 string / uint，T1.3 单测可以用 C.3 (`/health/info`) 的 `Port` / `Pid` / `RuntimeConfig.EffectivePort` 字段验证 parser 相同逻辑路径。

## 附录 B：zen 5.7.6 build_config 字段（来自 /health/info CB 解码后字段名提取）

```
ZEN_ADDRESS_SANITIZER (false)
ZEN_THREAD_SANITIZER (false)
ZEN_MEMORY_SANITIZER (false)
ZEN_LEAK_SANITIZER (false)
ZEN_USE_SENTRY (true)
ZEN_WITH_TESTS (false)
ZEN_USE_MIMALLOC (true)
ZEN_USE_RPMALLOC (true)
ZEN_WITH_HTTPSYS (true)
ZEN_WITH_MEMTRACK (true)
ZEN_WITH_TRACE (false)
ZEN_WITH_COMPUTE_SERVICES (false)
ZEN_WITH_HORDE (false)
ZEN_WITH_NOMAD (false)
```

`ZEN_WITH_HTTPSYS=true` 表明 httpsys backend 编译进去了（zen 启动时可以 `--http httpsys` 切换），但 sponsor 默认走 `--http asio`。
