# UECM 命令参考（91 命令 / 17 域）

来源：`uecm-cli system schema` + 各命令 `--help`（commit 24c31e7，binary 0.1.0）。
**binary 可能比源码新/旧——临场以 `"$BIN" <域> <子命令> --help` 为准。** 统一约定 `BIN=/mnt/c/Tools/UECM/uecm-cli.exe`。

记号：`<X>` 必填位置参数 · `--flag <X>` 必填选项 · `[--flag <X>]` 可选 · `[--flag]` 布尔开关 ·
**[写]** = 有副作用（走安全门：先 `--dry-run` 预览→确认→`--yes`）。

## 全局参数（所有命令通用）

```
--output <text|json|ndjson>   输出格式；AI 调用用 json（单对象）/ ndjson（流式每行一对象）
--no-input                    拒绝交互提示（AI/CI 必加）
--no-color                    禁用 ANSI 颜色
--quiet / --verbose (-v/-vv)  日志详细度
--db-path <PATH>              覆盖 DB 路径（Windows 路径；也读 env UECM_DB_PATH）
--config <FILE>               从 YAML/JSON 读默认值（文件权限须 ≤0600）
--cred-alias <ALIAS> / --user <USER> / --pass-stdin   远程命令凭据（见各命令）
```

## 退出码（`system exit-codes`）

| Code | 名称 | 含义 |
|---|---|---|
| 0 | ok | 成功 |
| 1 | operation_failed | 业务逻辑运行时失败 |
| 2 | invalid_input | 用户传入数据无效（含缺 `--yes`） |
| 3 | environment_error | 配置 / DB / IO 问题 |
| 4 | powershell_failed | 远程 PowerShell 调用失败 |
| 64 | usage_error | 命令行参数错误 |

---

## system — 自检 / 元信息

```
system version                      打印 binary + 库版本
system db-path                      打印解析出的 SQLite DB 路径
system ps-dir                       打印 ps-scripts 目录（UECM_PS_DIR > exe旁/ps-scripts > repo）
system migrate-db          [写]     强制跑 schema 迁移（新 DB 必跑一次）
system echo <MESSAGE>               经 PowerShell round-trip 回显（验证 bridge；msg 是位置参数）
system schema                       dump 完整 clap 命令树 JSON（含全 flag / exit / error 码）
system exit-codes                   打印退出码表
system completion <SHELL>           生成 shell 补全脚本
```

## machine — 机器纳管

```
machine list                        列出所有已纳管机器（读；可在非 Windows 跑）
machine scan <CIDR> [--timeout-ms <MS>]   探测网段活跃主机（读；只输出不写库；可非 Windows 跑）
machine add --ip <IP> [--hostname <H>]   [写] 录入一台机器，返回 id
machine refresh <ID> [--cred-alias <A>] [--user <U>] [--pass-stdin]   [写] SSH 探测：主机名/UE安装/GPU 写回 DB
machine detail <ID>                 单机完整信息（读）
machine delete [ID] [--machine-ids <M1,M2>] [--all] [--yes] [--dry-run]   [写] 删机器
machine rename <ID> <HOSTNAME>      [写] 改显示名
machine deep-scan [--machine-ids <M1,M2>] [--all] [--cred-alias <A>] [--user <U>] [--pass-stdin]   [写] refresh + ini scan + health 一条龙
machine authorize [--machine-ids <M1,M2>] [--all] [--save-as <ALIAS>] [--cred-alias <A>] [--user <U>] [--pass-stdin]   [写] 授权机器可远程管理（可顺手 --save-as 存凭据别名）
machine set-ue-user --machine <ID> --ue-user <USERNAME>   [写] 设 Windows UE 运行账号（zen enable --global 的前置）
```

## ssh — 远程通道

```
ssh probe <HOST>                    探测主机 SSH 可达性（读）
ssh package-bootstrap --out <DIR> [--local-admin-password <PASS>]   [写] 生成 USB 引导包（需人工传到新机双击；不自动执行）
```

## cred / secret — 凭据与密钥

```
cred list                           列出已存凭据别名（读）
cred save --alias <A> --user <U> [--pass <P>] [--pass-stdin] [--kind <K>]   [写] 存凭据（优先 --pass-stdin，别明文 --pass）
cred delete <ALIAS> [--yes] [--dry-run]   [写]
secret set <ALIAS> [--value <V>]    [写] 存/覆盖一个密钥（省 --value 则从 stdin 读）
secret get <ALIAS>                  打印密钥（读）
secret list                         列出别名（只列 key）（读）
secret delete <ALIAS> [--yes] [--dry-run]   [写]
```
> `cred --kind` default `winrm` 是历史遗留元数据，不影响实际传输（已走 SSH）；可不指定。

## env — 远程环境变量

```
env get --host <H> --name <N> [--cred-alias <A>] [--user <U>] [--pass-stdin]   读单机一个环境变量
env set [--host <H>] [--hosts <H1,H2>] --name <N> --value <V> [--yes] [--dry-run] [--cred-alias <A>] ...   [写] 写远程环境变量（多机用 --hosts）
```
> ⚠️ `env set` 只写注册表键值，**不创建目录**。设 `UE-LocalDataCachePath` 前先 `local-cache create` 建目录。
> ⚠️ 写 Machine-scope 变量、删变量在 WinRM 受限 token 下会被 UAC 拒——见 troubleshooting 提权通道。

## ini — INI 扫描 / 修复 / backend-graph / GC

```
ini read --host <H> --file <F> --section <S> [--cred-alias <A>] ...   读一个 INI section 全部 key
ini set [--host <H>] [--hosts <H1,H2>] --file <F> --section <S> --key <K> --value <V> [--yes] [--dry-run] ...   [写] 写单个 key
ini remove [--host <H>] [--hosts ...] --file <F> --section <S> --key <K> [--yes] [--dry-run] ...   [写] 删单个 key
ini scan [--machine-ids <M1,M2>] [--project-id <P>] [--machine-id <M>] [--cred-alias <A>] ...   [写] 集群 INI 扫描，产出 scan_run_id + findings
ini runs [--limit <N>]              列最近扫描批次（读）
ini findings <SCAN_RUN_ID> [--severity <S>]   列某批次的 findings（读）
ini get-finding <FINDING_ID>        单条 finding 详情（读）
ini apply <FINDING_ID> [--yes] [--dry-run] [--cred-alias <A>] ...   [写] 自动修复一条 finding（仅 set 类可修）
ini skip <FINDING_ID>               [写] 标记 finding 为跳过
ini config <SCAN_RUN_ID> [--domain <D>]   列某批次抓到的 DDC/PSO/Zen 配置快照（读）
ini verify-pso-precaching --project-id <P>   验证 ConsoleVariables.ini 的 PSO precaching CVar（读）
ini backend-graph get --host <H> --file-path <F> [--node <N>] --field <FIELD> [--cred-alias <A>] ...   读 [DerivedDataBackendGraph] tuple 字段
ini backend-graph set [--host <H>] [--hosts ...] --file-path <F> [--node <N>] --field <FIELD> --value <V> [--yes] [--dry-run] ...   [写] 就地改 tuple 字段
ini backend-graph scan --host <H> --file-path <F> [--cred-alias <A>] ...   扫描 tuple 节点（读）
ini gc-pause [--host <H>] [--hosts ...] --project-id <P> [--yes] [--dry-run] ...   [写] 暂停 Shared DDC GC（DeleteUnused=false）
ini gc-resume [--host <H>] [--hosts ...] --project-id <P> [--unused-file-age <AGE>] [--yes] [--dry-run] ...   [写] 恢复 GC（DeleteUnused=true）
```
> finding 分两类：`set`（CLI 知道推荐值，可 `ini apply` 自动修）/ `manual`（需 operator 提供值，如 UNC 路径）。
> ⚠️ `ini apply` summary 的 `backup_path` 字段：tuple 就地改路径下返回的是操作描述而非真实 .bak 路径（F-041），别据它定位备份。

## share — SMB 共享

```
share list                          列本地库里的 share 配置（读）
share create --mode <a|b> --host <H> --share <SHARE> --local-path <LP> [--yes] [--dry-run] [--cred-alias <A>] ...   [写] 建 SMB 共享
share forget <ID> [--yes] [--dry-run]   [写] 仅从本地库忘记（不删实际共享）
share inject-system-cred --client-host <CH> --target-host <TH> [--svc-user <U>] [--yes] [--dry-run] ...   [写] 在客户端注入 SYSTEM 上下文凭据
```
> mode a = open（Guest+Everyone）；mode b = dedicated（建 ddc-svc 专用账号）。`--host` 是承载共享的服务器，别漏。

## project — 项目发现 / 定位

```
project list                        列所有项目（读）
project locations <PROJECT_ID>      列项目的所有位置（读）
project discover --machine-id <M> [--roots <R1,R2>] [--cred-alias <A>] ...   [写] 远程发现 .uproject
project create-manual --uproject-name <N> [--display-name <D>]   [写] 手动建项目（不扫描）
project set-location --project-id <P> --machine-id <M> --abs-path <AP> --uproject-path <UP> [--manual-path]   [写] 加/改项目位置
project delete <ID> [--yes] [--dry-run]   [写] 删项目并级联删位置
project delete-location <ID> [--yes] [--dry-run]   [写] 删单条位置
```
> ⚠️ `discover` 会扫入引擎自带 Templates/Samples（已内置过滤引擎子树，仍建议 `--roots` 指精确路径如 `E:\Unreal Projects`）。

## health — 集群诊断

```
health run [--machine-ids <M1,M2>] [--cidr <C>] [--all] [--expected-local-path <LP>] [--expected-shared-path <SP>] [--cred-alias <A>] ...   [写] L1/L2/L3 诊断，每条 critical 带 remediation
health runs [--limit <N>]           列最近 health 批次（读）
health results <SCAN_RUN_ID>        列某批次逐行结果（读）
health consistency-check [--hosts <H1,H2>] [--cred-alias <A>] ...   快照 N 台对比不一致（读）
health scan-command-line --host <H> [--cred-alias <A>] ...   扫快捷方式/bat/服务里的 DDC 路径覆盖（读）
health file-stats --host <H> --local-path <LP> --shared-path <SP> [--cred-alias <A>] ...   本地 vs 共享 DDC 文件数/大小失衡分类（读）
health analyze-advisories --host <H> --editor-exe <EXE> --project <PROJ> --local-path <LP> --shared-path <SP> [--timeout <S>] [--cred-alias <A>] ...   log 验证 + file-stats → 症状建议（读）
```
> ⚠️ `health run` 前先 `zen probe`——zen_reachable 读 DB 最近 probe 记录，过 5 分钟窗口会误报 critical（F-043）。
> ⚠️ Zen 模式下 `env_shared/env_vars` 自动降级为 na（不再误报 critical，DESIGN-1 已修）。

## gpu

```
gpu matrix                          全机 GPU 一致性矩阵（读；F-005 已过滤虚拟显示适配器）
```

## ddc — DDC Pak 生成 / 验证 / 分发

```
ddc generate --project-id <P> --source-machine <M> [--backend <auto|legacy|zen>] [--cred-alias <A>] ...   [写] 启 UE -DDC=CreatePak 生成 .ddp
ddc verify --project-id <P> --source-machine <M> [--backend <auto|legacy|zen>] [--cred-alias <A>] ...   验证 .ddp 是否存在（读）
ddc distribute --project-id <P> --source-machine <M> [--targets <M1,M2>] [--yes] [--dry-run] [--backend <B>] [--source-smb-cred-alias <A>] [--cred-alias <A>] ...   [写] Robocopy 分发到目标机
```
> `--backend` default `auto`：Zen 可达时 generate/verify 都 `skipped:true`（zen 原生缓存，无需 pak）；`legacy` 强制 .ddp 流程；`zen` 是 no-op。`--source-machine` 是机器 id。

## pso — PSO 缓存收集 / 分发

```
pso verify --project-id <P>         验证 PSO precaching CVar（实为提示转向 ini scan，R008-R010 在 ini scan 里跑）（读）
pso collect --project-id <P> --source-machine <M> [--resolution <WxH>] [--windowed] [--max-minutes <N>] [--cred-alias <A>] ...   [写] 启 UE -game 收集 PSO（流式；resolution 默认 1920x1080，max-minutes 默认 10）
pso list --project-id <P>           列已收集的 PSO 缓存文件（读）
pso distribute --project-id <P> --source-machine <M> [--targets <M1,M2>] [--yes] [--dry-run] [--source-smb-cred-alias <A>] [--cred-alias <A>] ...   [写] 分发 PSO 到目标机
```

## log

```
log verify-startup --host <H> --editor-exe <EXE> --project <PROJ> [--timeout <S>] [--cred-alias <A>] ...   启 UE nullrhi 解析 DDC 启动输出（读）
```
> ⚠️ 路径须 Windows 格式且真实存在；重型 VP 项目（nDisplay/ControlRig/Python startup）quit 阶段会 hang→timeout（F-044，UE 项目侧问题）；Zen 模式不输出 legacy DDC 路径行（F-045）。

## local-cache

```
local-cache create [--host <H>] [--hosts <H1,H2>] [--path <P>] [--service-account <SA>] [--yes] [--dry-run] [--cred-alias <A>] ...   [写] 建本地 DDC 目录（path 默认 D:\UE-DDC-Local；create_dir_all 自动建）
```
> ⚠️ 真实命令是 `local-cache create`，**不是** `provision`（旧文档笔误）。

## deploy

```
deploy ddc --plan <PLAN.json> [--stop-on-failure] [--yes] [--dry-run] [--cred-alias <A>] ...   [写] 按 plan JSON 跑完整 11 步 DDC 部署
```
> plan JSON 结构与字段必填规则见 `05-deploy.md` + `assets/deploy-plan-template.json`；`--plan` 路径须 Windows 格式（F-035）。

## zen — ZenServer 全套

```
zen status [--machine <M>] [--all]                只读看每个 endpoint 最新 probe（读）
zen probe [--machine <M>] [--all] [--timeout <S>] [--cred-alias <A>] ...   [写] 立即探测并落库（health run 前置）
zen cache-stats [--endpoint-id <E>] [--all] [--timeout <S>]   [写] 抓 /stats 落一行
zen detect-binary [--machine <M>] [--all] [--cred-alias <A>] ...   [写] 探测 zen.exe 路径落库（apply-config 前置；自动选最高 UE 版本 in-tree）
zen list-endpoints [--machine <M>]                只读列已注册 endpoint（读）
zen baseline list|lock|unlock ...                 baseline 检查与锁定（lock/unlock 为 [写]）
zen register --machine <ID> --role <local|shared_upstream> [--declared-port <P>] [--scheme <S>] [--upstream-endpoint-id <ID>] [--data-dir <PATH>] [--httpserverclass <C>] [--lifecycle <M>]   [写] 注册 endpoint（对 (machine,port) 幂等；port 默认 8558，data-dir 默认 D:\UECM\ZenData）
zen unregister --endpoint-id <ID> [--yes] [--dry-run]   [写]
zen change-role --endpoint-id <ID> --new-role <ROLE> [--upstream-endpoint-id <ID>] [--yes] [--dry-run]   [写] 切 local <-> shared_upstream
zen apply-config --endpoint-id <ID> [--dest-path <P>] [--yes] [--dry-run] [--cred-alias <A>] ...   [写] 渲染 zen.lua 写到目标机（附 SHA256 校验）
zen lua-preview --endpoint-id <ID>                渲染 zen.lua 到 stdout（读）
zen service install --endpoint-id <ID> [--service-user <U>] [--service-pass <P>] [--service-pass-stdin] [--yes] [--dry-run] [--cred-alias <A>] ...   [写] 装 UECMZenServer 服务（工作站会附 advisory 警告）
zen service uninstall|stop --endpoint-id <ID> [--yes] [--dry-run] [--cred-alias <A>] ...   [写]
zen service start|status --endpoint-id <ID> [--cred-alias <A>] ...   start [写] / status 读
zen sponsor-down --endpoint-id <ID> [--yes] [--dry-run] [--cred-alias <A>] ...   优雅关掉占端口的 editor sponsor zenserver
zen urlacl add --endpoint-id <ID> --principal <PRINCIPAL> [--yes] [--dry-run] [--cred-alias <A>] ...   [写] 加 netsh http URL ACL
zen urlacl list --machine <ID> [--port-filter <P>] [--cred-alias <A>] ...   列 URL ACL（读）
zen urlacl remove --endpoint-id <ID> [--yes] [--dry-run] [--cred-alias <A>] ...   [写]
zen enable [--project-id <ID>] [--global] [--machines <M1,M2>] --upstream-endpoint-id <ID> [--namespace <NS>] [--yes] [--dry-run] [--cred-alias <A>] ...   [写] 给项目/全局开 ZenShared upstream（改 INI + 清 legacy env）
zen disable [--project-id <ID>] [--global] [--machines <M1,M2>] [--yes] [--dry-run] [--cred-alias <A>] ...   [写] 移除 ZenShared 条目
zen verify-rules --ue-version <X.Y> --ue-install <PATH> [--write-verified] [--run-editor] [--machine <ID>] [--uproject-path <P>] [--timeout-seconds <S>] [--expected-host <H>] [--expected-port <P>] [--expected-namespace <NS>] [--cred-alias <A>] ...   [写] 解析某 UE 版本的 zen INI 规则集
zen clean-env [--machines <M1,M2>] [--name <NAME>] [--scopes <machine,user>] [--yes] [--dry-run] [--cred-alias <A>] ...   [写] 清 DDC env 变量（内部走提权通道）
zen set-region-host [--machines <M1,M2>] --host <HOST> [--yes] [--dry-run] [--cred-alias <A>] ...   [写] 设按机区域覆盖 UE-ZenSharedDataCacheHost（裸 host:port 自动规范化为 http://host:port）
```
> ⚠️ `zen register --role` 取值是 `local` / `shared_upstream`；`zen enable` 的 `--project-id` 与 `--global` 二选一。
> ⚠️ ZenServer 服务**必须装在独立服务器**（不跑 UE Editor 的机器），与工作站共机必冲突 8558 端口（F-039）。
