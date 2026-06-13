# UECM CLI 走读问题记录

走读日期：2026-06-12
CLI 版本：0.1.0
目的：为后续制作 Skill / GUI 提供准确的接口行为依据

---

## 问题清单

### F-001 · `system check` 子命令不存在
**类型**：DOC（文档错误）
**发现步骤**：Step 3
**现象**：调用 `uecm-cli system check` 报 `unrecognized subcommand 'check'`（exit 64）
**实际**：`system` 域没有 `check` 子命令，实际子命令为：
`version` / `db-path` / `ps-dir` / `migrate-db` / `echo` / `schema` / `exit-codes` / `completion`
**影响**：文档或 Skill 里不能引用 `system check`；自检功能需改用 `system echo`（验证 PowerShell bridge）+ `system db-path`（验证 DB 路径）组合替代

---

### F-002 · `system echo` 参数是位置参数，不是 `--message` flag
**类型**：DOC（文档/直觉错误）
**发现步骤**：Step 3b
**现象**：`system echo --message "hello"` 报 `unexpected argument '--message'`（exit 64）
**实际**：正确用法是 `system echo "hello"`，消息是位置参数
**影响**：Skill / 脚本调用时直接传字符串，不加 `--message`

---

### F-003 · `machine scan` 不写入 DB，只探测端口
**类型**：BUG / DOC（行为与直觉不符）
**发现步骤**：Step 4 → Step 5
**现象**：`machine scan 192.168.10.0/24` 返回 4 台活跃主机，但随后 `machine list` 返回空数组
**实际**：`machine scan` 只做 TCP 端口探测（5985/445），结果只在当次输出里返回，不持久化到 DB
**正确流程**：`scan` 发现候选 → `machine add --ip <IP>` 手动把目标机器录入 DB
**影响**：GUI 上"扫描网段"和"加入集群"应该是两个独立操作（扫描结果展示在列表，operator 勾选后再 add）

---

### F-004 · `machine add` 的 IP 是 `--ip` flag，不是位置参数
**类型**：DOC（直觉错误）
**发现步骤**：Step 5b
**现象**：`machine add 192.168.10.20` 报 `unexpected argument '192.168.10.20'`（exit 64）
**实际**：正确用法是 `machine add --ip 192.168.10.20`
**影响**：Skill / 脚本里调用 add 必须带 `--ip` flag

---

---

### F-005 · `machine detail` GPU 列表包含大量虚拟显示适配器
**类型**：ENHANCE（GUI 展示需过滤）
**发现步骤**：Step 7
**现象**：lanPC 报告 9 个 GPU，实际真实物理 GPU 只有 2 个（RTX 3080 + AMD 核显），其余 7 个是 Parsec / 向日葵 / GameViewer / MS IDD 等远控软件的虚拟显示适配器
**特征**：真实 GPU 有 `vram_mb` 值且 `vendor` 为 `nvidia`/`amd`；虚拟适配器 `vendor=unknown`、`vram_mb=null`
**影响**：GUI 的 GPU 列表页应按 `vendor != unknown` 或 `vram_mb != null` 过滤，只显示真实 GPU；或标注「虚拟」与「物理」分组

---

### F-006 · `machine refresh` 不填充 Zen 路径，zen 字段全为 null
**类型**：DOC（行为说明）
**发现步骤**：Step 7
**现象**：`machine detail` 中所有 UE 安装的 `zen_cli_intree_path` / `zenserver_intree_path` 等字段均为 null
**实际**：`machine refresh` 只检测 UE 安装路径和 GPU，Zen daemon 相关路径需要单独跑 `zen scan` 才会填充
**影响**：GUI 上 Zen 状态不能从 machine detail 直接读，需要等 `zen scan` 之后再查询

---

---

### F-007 · `cred save` 的 `--kind` 默认值是 `winrm`，但 UECM 已迁移到 SSH
**类型**：DOC（历史遗留字段）
**发现步骤**：Step 9
**现象**：`cred list` 显示 `kind=winrm`，但 UECM 的传输层已从 WinRM 迁移到 SSH（ssh 域替代了退役的 winrm 域）
**实际**：`--kind` flag 目前可能仅作为元数据标记，不影响实际传输行为；传输层由 SSH 接管
**影响**：GUI 上凭据详情里的 `kind` 字段可以不显示或显示为「通用」；Skill 里保存凭据不需要指定 `--kind`

---

---

### F-008 · 破坏性写操作统一要求 `--yes` 确认，支持 `--dry-run` 预览
**类型**：DOC（重要安全行为，GUI 设计需考虑）
**发现步骤**：Step 11b
**现象**：`env set` 不带 `--yes` 直接报 exit 2：`env.set is destructive; pass --yes to confirm or --dry-run to preview`
**实际**：写操作三段式流程：`--dry-run`（预览，不执行）→ 确认内容 → `--yes`（实际写入）
**影响**：GUI 上所有写操作应实现二步确认（预览 → 确认执行）；Skill 里调用写操作必须带 `--yes`，且建议先跑 `--dry-run` 给用户看预览

---

---

### F-009 · `env set UE-LocalDataCachePath` 应在 `local-cache provision` 之后执行
**类型**：DOC（流程顺序错误）
**发现步骤**：Step 11b → 用户反馈
**现象**：`env set` 把 `UE-LocalDataCachePath=D:\UE-DDC-Local` 写入注册表，但 `D:\UE-DDC-Local` 目录实际不存在
**实际**：`env set` 只写注册表键值，不创建目录；目录需要 `local-cache provision` 单独建
**正确顺序**：
```
1. local-cache provision --host <IP> --path "D:\UE-DDC-Local"  # 先建目录
2. env set --host <IP> --name UE-LocalDataCachePath --value "D:\UE-DDC-Local" --yes  # 再写变量
```
**影响**：GUI 的 DDC 配置向导应把 provision 和 env set 合并为一个操作步骤，确保顺序正确；Skill 里必须保证这个先后顺序

---

---

### F-010 · 本地缓存 + 共享缓存是核心高频操作，应作为 Skill/UI 的一级入口
**类型**：ENHANCE（产品设计建议）
**发现步骤**：Step 11 + Step 12
**背景**：UE DDC 两级缓存体系——本地缓存（本机磁盘）+ 共享缓存（SMB 网络目录）构成完整缓存链路
**读取流程**：UE 先查本地 → 未命中则查共享 → 共享命中后回填本地 → 两者都未命中则重新编译并同时写入两级
**写入流程**：编译完成同时写入本地 + 共享，其他机器之后可直接从共享读取，免重编译

**对应 CLI 操作：**
- 设置本地缓存目录：`env set --name UE-LocalDataCachePath --value <路径>`（写注册表）
- 创建共享缓存目录：`share create --mode a --share <名称> --local-path <路径>`（建 SMB Share）
- 注意顺序：先 `local-cache provision`（建目录）再 `env set`（写变量）

**产品建议**：
- 用户自然语言指令示例："帮我在这台机器上配置本地 DDC 缓存" → 触发 `local-cache provision` + `env set`
- 用户自然语言指令示例："把这台机器设为共享缓存服务器" → 触发 `share create`
- 这两个操作是 DDC 配置的核心入口，GUI 上应放在显眼位置（不能埋在深层菜单里）

---

---

### F-011 · ini scan finding 分两类：manual（需人工）和 set（可自动修复）
**类型**：DOC（重要行为，GUI/Skill 设计必知）
**发现步骤**：Step 13b
**现象**：11 个 finding 中，R021（Shared.Path 非 UNC）是 `manual`，R015（DeleteUnused 缺失）是 `set`
**区别**：
- `manual`：CLI 不知道正确的值是什么（比如 UNC 路径需要 operator 告知），只能提醒人工修改
- `set`：CLI 知道推荐值（如 `DeleteUnused=true`），可以通过 `ini apply <id>` 直接写入
**影响**：
- GUI 上 findings 列表应视觉区分 manual（需人工处理，标橙/红）和 set（可一键修复，标蓝）
- Skill 里可以自动 apply 所有 `set` 类型的 finding；`manual` 类型需要告知用户手动处理

---

### F-012 · R021 需人工填写正确的 UNC 路径（即刚才 share create 的 UNC 路径）
**类型**：DOC（操作流程关联）
**发现步骤**：Step 13b
**关联**：R021 的修复值就是 Step 12 中 `share create` 得到的 `\\LANPC\DDC-Shared`
**正确修复方式**：
```bash
ini set --host 192.168.10.20 --file "...\BaseEngine.ini" \
  --section DerivedDataBackendGraph --key Shared.Path \
  --value "\\\\LANPC\\DDC-Shared" --yes
```
**影响**：GUI/Skill 应在用户完成 `share create` 后，自动将得到的 UNC 路径填入 R021 的修复建议，减少人工操作

---

---

### F-013 · `ini apply` 修改 INI 文件前自动创建 `.bak` 备份
**类型**：DOC（重要安全行为）
**发现步骤**：Step 14
**现象**：apply 成功后返回 `backup_path: ...BaseEngine.ini.bak.1781320936129`
**实际**：每次 apply 都会先把原文件备份为 `<原文件名>.bak.<时间戳>`，再写入新值
**影响**：GUI 上可以提示用户"已自动备份原文件"并显示备份路径；如果 apply 出错，用户可以手动从 .bak 恢复

---

---

### F-014 · `project discover` 会扫入 UE 引擎自带的 Templates/Samples，需过滤
**类型**：ENHANCE（GUI/Skill 设计建议）
**发现步骤**：Step 15b
**现象**：扫描 `E:\` 发现 142 个项目，其中约 100 个是 UE 引擎自带的 `Templates/` 和 `Samples/` 目录下的内置项目，不是真实业务项目
**实际**：`discover` 按文件名 `*.uproject` 递归扫描，不区分「引擎内置」和「用户项目」
**影响**：
- GUI 上项目列表应支持过滤（按路径前缀、按是否含 `Templates/Samples/Programs` 关键词）
- Skill 里执行 `ddc generate` / `pso collect` 前应让用户确认目标项目，而不是对所有 142 个项目操作
- 建议 `--roots` 指定更精确的路径（如 `E:\Unreal Projects,E:\RenderStream Projects`）避免扫入引擎目录

---

---

### F-015 · DDC 配置需要设置两个环境变量，不只一个
**类型**：DOC（流程遗漏）
**发现步骤**：Step 16（health run critical: env_shared）
**现象**：health check 报 `env_shared=critical`，`UE-SharedDataCachePath` 未设置
**实际**：DDC 完整配置需要两个系统环境变量：
- `UE-LocalDataCachePath` — 本地缓存路径（Step 11 已设置）
- `UE-SharedDataCachePath` — 共享缓存 UNC 路径（**本次遗漏，未设置**）
**正确命令**：
```bash
env set --host 192.168.10.20 --name UE-SharedDataCachePath --value "\\\\LANPC\\DDC-Shared" --yes
```
**影响**：GUI/Skill 的 DDC 配置步骤应同时设置两个变量；`share create` 完成后应自动提示设置 `UE-SharedDataCachePath`

---

### F-016 · health run 是集群配置状态的核心诊断入口，应作为 Skill/UI 一级功能
**类型**：ENHANCE（产品设计建议）
**发现步骤**：Step 16
**背景**：health run 三层检查（端口/SSH/业务）覆盖了集群所有关键配置项，并且每个 critical 都附带 `remediation` 字段（具体的修复命令）
**影响**：
- GUI 上"集群健康"页面直接从 health results 取数据，按 critical/warning/healthy 分色展示
- Skill 里可以：跑 health run → 筛出 critical findings → 逐条对照 remediation 字段自动执行修复 → 再跑一次 health run 验证

---

---

### F-017 · `ssh package-bootstrap` 只生成引导包，不自动执行，需人工传送到新机器运行
**类型**：DOC（流程说明，GUI/Skill 必须告知用户）
**发现步骤**：功能 1(b)
**现象**：`package-bootstrap` 成功返回文件列表，但新机器上什么都没有发生
**实际**：新机器在 Bootstrap 之前 SSH/WinRM 未开，UECM 无法远程连接，必须人工介入
**完整接入流程**：
```
1. ssh package-bootstrap --out <目录>   # operator 机器上生成包
2. 把包传到新机器（U 盘 / 文件共享）    # 人工操作，唯一需要现场的步骤
3. 新机器上双击 UECM-Bootstrap.cmd      # 开通 SSH + 建 uecm-svc 账号
4. machine add --ip <新机器IP>           # 录入 DB
5. machine refresh <id>                  # SSH 探测 + 收集 UE/GPU 信息
```
**影响**：GUI 上机器接入向导必须在第 2 步明确提示"请将引导包传送到目标机器并手动执行"，不能让用户以为点了按钮就自动完成

---

---

### F-018 · Bootstrap 返回 `changes` 列表，明确区分"新执行"与"已存在跳过"
**类型**：DOC（GUI 设计参考）
**发现步骤**：功能 1(b) Bootstrap 实际执行
**现象**：重复运行 Bootstrap，`changes` 数组里每一项都说明了实际发生了什么（新建/重置/跳过）
**影响**：GUI 上机器接入完成后，可以把 `changes` 列表展示给用户，让他们清楚地知道哪些配置被修改了、哪些已经是正确状态

---

### F-019 · `zen service install` 遇到已存在服务时需先停服再卸载，WinRM 因 UAC 过滤无法直接执行
**类型**：WORKFLOW（操作顺序陷阱）
**发现步骤**：功能 2 — zen service install
**现象**：ZenServer 服务已存在（UE 自动安装）时，`zen service install` 报 "Service 'ZenServer' is already installed ... Refusing to re-install without --full"（exit 4）；先执行 `zen service uninstall` 也报失败（exit 4），提示 "zen service uninstall failed (exit 1)"
**根因**：
1. WinRM 会话即使使用 Administrators 组账号（uecm-svc），UAC 也会过滤 token，导致 SCM（服务控制管理器）写操作被拒绝
2. `zen.exe service uninstall` 在 WinRM 受限 token 下 exit 1
**解法**：
```
# 1. 先通过提权 SSH 通道（Windows ssh.exe + uecm_ed25519 key）停止服务
ssh -i "$UecmKeyPath" uecm-svc@<IP> "powershell Stop-Service ZenServer -Force"
# 2. 再让 UECM 卸载（此时 zen.exe 可以无冲突地注销已停止的服务注册）
uecm-cli zen service uninstall --endpoint-id <ID> --cred-alias <ALIAS> --yes
# 3. 重新安装
uecm-cli zen service install --endpoint-id <ID> --cred-alias <ALIAS> --yes
```
**影响**：GUI 的"重装 Zen 服务"流程必须先停服、再卸载、再安装；不能直接调 UECM uninstall 而不先停服

---

### F-020 · `zen urlacl add` 大小写比对 bug ✅ 已修复（commit 88a5a91）
**类型**：BUG（UECM 实现问题）
**发现步骤**：功能 2 — zen urlacl add
**现象**：`http://+:8558/` 已存在 URL 保留但归属 `NT AUTHORITY\LOCAL SERVICE`（全大写），UECM 期望 `NT AUTHORITY\LocalService`（混合大小写）→ 报 "owned by ... not ..." 错误
**实际**：两者是同一 Windows 账号，功能完全等价，保留实际有效
**影响**：GUI 的 urlacl 状态展示不要依赖字符串精确匹配来判断"是否已正确设置"，应改为检查 SDDL 里的 `LS` SID
**修复**：`ps-scripts/zen-urlacl-add.ps1` 新增 `Test-SamePrincipal` 函数，先 `-ieq` 字符串比对，再通过 `NTAccount.Translate(SecurityIdentifier)` SID 比对；同一 SID 不报冲突（S-1-5-19 = LocalService = LOCAL SERVICE）
**复验结果**（2026-06-13）：`zen urlacl add --principal "NT AUTHORITY\LocalService"` 返回 `ok:true, already_exists:true`，不再误报冲突
**注意**：PS 脚本需同步到 `C:\Tools\UECM\ps-scripts\`（binary 旁的 ps-scripts/ 优先于源码 repo）

---

### F-021 · `zen enable --global` 前必须先设置 `ue_runtime_user`
**类型**：DOC（前置依赖）
**发现步骤**：功能 2 — zen enable
**现象**：`zen enable --global --machines 1` 报 "machine id=1 has no ue_runtime_user set — run machine set-ue-user first"（exit 2）
**实际**：`--global` 模式需要知道 UE 运行账号来定位 `UserEngine.ini` 路径（`C:\Users\<ue_runtime_user>\AppData\Local\Unreal Engine\Engine\Config\UserEngine.ini`）
**解法**：`machine set-ue-user --machine <ID> --ue-user <Windows用户名>` 先行录入
**影响**：机器接入向导应在 machine refresh 之后引导用户填 ue_runtime_user

---

### F-022 · `zen env-cleanup` 无独立 CLI 命令，Machine scope 需要提权
**类型**：DOC（接口缺失 + 权限陷阱）
**发现步骤**：功能 2 — zen enable 后清 UE-SharedDataCachePath
**现象**：`zen enable` 完成后提示 "1 env var(s) flagged for cleanup; run the zen env-cleanup PS sidecar (T3.4)"，但 `zen env-cleanup` 子命令不存在
**实际**：需要直接调用 PS sidecar（`zen-env-cleanup.ps1`），且 stdin 传 JSON；Machine scope 的 env var 删除需要管理员权限，WinRM 受限 token 下会报 "不允许所请求的注册表访问权"
**解法**：通过提权 SSH 通道执行
```bash
echo '{"Name":"UE-SharedDataCachePath"}' | \
  ssh -i "$UecmKeyPath" uecm-svc@<IP> \
  "powershell -File C:\Tools\UECM\ps-scripts\zen-env-cleanup.ps1"
```
**影响**：UECM 应补一个 `zen clean-env` 子命令，内部走提权通道；暂时 GUI 可以在 zen enable 完成后提示用户手动执行

---

### F-023 · Zen 二进制两种来源：UE Hub 安装（AppData）vs in-tree（UE 安装目录）
**类型**：DOC（部署拓扑说明）
**发现步骤**：功能 2 — zen service uninstall dry-run 对比 service install dry-run
**现象**：
- 卸载时发现现有服务用的是 `C:\Users\lanPC\AppData\Local\UnrealEngine\Common\Zen\Install\zen.exe`（UE Hub 独立部署）
- UECM `service install` dry-run 选用的是 `D:\Program Files\Epic Games\UE_5.8\Engine\Binaries\Win64\zenserver.exe`（in-tree）
**影响**：
- UE Hub 会自动更新 AppData 里的 Zen，可能与 in-tree 版本不同步
- UECM 管理的服务指向 in-tree 二进制，升级需通过 UE 升级而非 Hub
- GUI 上应展示当前服务使用的 zen.exe 路径和版本，供运维人员判断

---

### F-024 · `zen detect-binary` 是 `zen apply-config` 的前置依赖
**类型**：DOC（命令依赖顺序）
**发现步骤**：功能 2 — zen apply-config dry-run
**现象**：未跑 `zen detect-binary` 时，`zen apply-config` 报 "cannot derive --dest-path: machine id=X has no install-dir zen.exe recorded"（exit 2）
**实际**：`detect-binary` 会探测机器上所有 UE 安装位置的 zen.exe / zenserver.exe，选最新版本记入 DB；`apply-config` 从 DB 读这条记录来决定 zen.lua 的写入路径
**完整 ZenServer 部署顺序**（独立服务器方案）：

服务端（在 Zen 服务器上执行）：
```
1. zen register --machine <server_id> --role shared_upstream --declared-port 8558 --data-dir <dir>
2. zen detect-binary --machine <server_id> --cred-alias <alias>
3. zen apply-config --endpoint-id <id> --cred-alias <alias> --yes
4. zen urlacl add --endpoint-id <id> --principal "NT AUTHORITY\LocalService" --cred-alias <alias> --yes
5. zen service install --endpoint-id <id> --cred-alias <alias> --yes
6. zen service start --endpoint-id <id> --cred-alias <alias>
7. zen probe --machine <server_id>
```

客户端（在各工作站上执行）：
```
8. machine set-ue-user --machine <workstation_id> --ue-user <Windows用户名>
9. zen enable --upstream-endpoint-id <id> --global --machines <workstation_id> --cred-alias <alias> --yes
```
注意：独立服务器方案下不需要 `AutoLaunch=false`，UE 的 ZenLocal 保持正常工作。

---

### F-025 · `pso collect` Tokio runtime panic（exit 101，功能完全不可用）✅ 已修复（commit 88a5a91）
**类型**：BUG（严重）
**发现步骤**：功能 3 — pso collect
**现象**：
```
thread 'main' panicked at src\core\ue_runner.rs:90:5:
there is no reactor running, must be called from the context of a Tokio 1.x runtime
exit code: 101
```
**根因**：`ue_runner.rs:90` 处代码需要 Tokio 异步运行时（reactor），但当前 CLI 的该调用路径没有初始化 Tokio runtime
**影响**：PSO 缓存收集整个工作流（collect → list → distribute）完全无法使用
**修复**：`src-tauri/src/cli/domain_pso.rs` — 将 `launch_collection` 调用移入 `rt.block_on()` 闭包内，确保 spawn 在活跃 executor 上执行
**复验结果**（2026-06-13）：`pso collect` 成功启动 UE 进程并输出 NDJSON 日志流（type:spawned → type:log_line），不再 panic

---

### F-026 · `pso verify` 不独立运行检查，而是转向 `ini scan`
**类型**：DOC（接口设计说明）
**发现步骤**：功能 3 — pso verify
**现象**：`pso verify --project-id 1` 返回 `"note":"use ini scan --machine-ids <ids> with project paths; PSO CVar check (R008-R010) runs as part of the full ini scan"`
**实际**：PSO CVars 检查（R008-R010）是 `ini scan` 规则集的一部分，不是 `pso verify` 独立运行的
**影响**：GUI 里 PSO 配置验证功能应引导用户跑 `ini scan`，而不是 `pso verify`；`pso verify` 目前相当于一个占位提示命令

---

### F-027 · `ddc generate --backend auto` 在 Zen 启用时自动路由跳过，行为正确
**类型**：DOC（设计备忘，GUI 参考）
**发现步骤**：功能 4 — ddc generate
**现象**：Zen 可达时 `ddc generate --backend auto` 返回 `skipped:true, reason:"zen handles caching natively"`，routing 说明了判断逻辑
**路由判断字段**：
- `zen_reachable`：Zen 探针是否可达
- `machine_best_ue`：机器上最高 UE 版本
- `project_ue`：项目绑定的 UE 版本
**影响**：GUI 上"生成 DDC Pack"按钮，在 Zen 启用后应变成灰色/跳过，并展示路由原因文字

---

### F-028 · `ddc verify` 不跟随 Zen 路由逻辑，Zen 启用后仍然查找 .ddp 文件 ✅ 已修复（commit 88a5a91）
**类型**：BUG（verify/generate 不一致）
**发现步骤**：功能 4 — ddc verify
**现象**：`ddc verify --backend auto` 在 Zen 启用、没有 .ddp 文件时报 "operation failed: .ddp not found after generation"（exit 1）；而同样 `--backend auto` 的 `ddc generate` 会 `skipped=true`
**影响**：DDC 工作流的 generate → verify → distribute 三步中，generate 和 verify 的 Zen 感知逻辑不对称；`verify` 应和 `generate` 一样在 Zen 路由时返回 `skipped=true`
**修复**：`src-tauri/src/cli/domain_ddc.rs` — verify 现在共享相同的 Zen 路由判断逻辑；Zen 可达时直接返回 `skipped:true`，仅将"文件不存在"的具体错误转为结构化 found=false 结果（其他操作性失败仍上报 exit 1）
**复验结果**（2026-06-13）：`ddc verify --backend auto` 在 Zen 可达时返回 `{"skipped":true,"reason":"zen handles caching natively","backend":"zen"}`，不再 exit 1

---

### F-029 · `ddc generate --backend legacy` / `pso collect` 共享同一 Tokio panic 代码路径 ✅ 已修复（commit 88a5a91，与 F-025 同次修复）
**类型**：BUG（与 F-025 同根因）
**发现步骤**：功能 4 — ddc generate legacy
**根因**：`src/core/ue_runner.rs:90` 是所有"实际启动 UE 进程"命令的共同入口，缺少 Tokio runtime 初始化
**受影响命令**：`pso collect`、`ddc generate --backend legacy/auto-but-not-zen`
**修复**：`src-tauri/src/cli/domain_ddc.rs` — `launch_generation` 调用移入 `rt.block_on()` 内（同 domain_pso.rs 的处理方式）
**复验结果**（2026-06-13）：`ddc generate --backend legacy` 成功启动 UE 进程并输出 9700+ 行日志，未 panic

---

### F-030 · `ini apply` R015 只写独立键，不修改 tuple 内联值，导致重复触发 ✅ 已修复（commit 88a5a91）
**类型**：BUG（修复不完整）
**发现步骤**：功能 5 — ini apply R015 后重扫
**现象**：`ini apply` R015（`Shared.DeleteUnused`）成功返回 `applied: true`，写入了 `Shared.DeleteUnused=true` 独立键；但重跑 `ini scan` 仍触发 R015
**根因**：BaseEngine.ini 里同时存在两种格式：
- `Shared.DeleteUnused=true`（我们写入的独立键，line 2756）
- `Shared=(Type=FileSystem, DeleteUnused=false, ...)`（原始 tuple，line 2855）
扫描器读 tuple 内联值，没有认可独立键的覆盖
**影响**：R015 修复形成"永久告警"——每次扫都 warning，但实际 UE 行为未确认（独立键 vs tuple 的优先级）；apply 应改为就地修改 tuple 里的 `DeleteUnused` 值
**修复**：`src-tauri/src/core/ini_apply.rs` — 检测 `key_name` 是否为 `NodeName.FieldName` 点分格式，若是则通过 `set_backend_field` 就地修改 backend-graph 节点内的 tuple 字段值（以 section 名为守卫，避免影响 `r.PSOPrecaching` 等 CVar 点分键）
**复验结果**（2026-06-13）：`ini apply 34`（UE 5.7 R015）后，UE_5.7/Engine/Config/BaseEngine.ini line 2745 中 `Shared=(...)` tuple 直接包含 `DeleteUnused=true`；re-scan warning 从 5 降至 4，不再重复触发

---

### F-031 · Zen 启用后 `health run` 的 `env_shared/env_vars` 持续 critical（设计冲突）
**类型**：DESIGN（接口设计问题）
**发现步骤**：功能 6 — health run
**现象**：`zen enable` 清除 `UE-SharedDataCachePath` 后，`health run` 仍报 `env_shared: critical`，remediation 建议设置该变量——与 Zen 流程相反
**根因**：health check L3 的 `env_shared` 规则是为传统 SMB DDC 设计的，不感知 Zen 模式
**影响**：GUI 上"集群健康"页面在 Zen 模式下会一直显示 critical，混淆用户；需在 health run 里加 Zen 模式判断，Zen 启用时 `env_shared` 应自动跳过或显示 healthy

---

### F-032 · `health run` 的 `zen_reachable` 依赖 probe 数据时效，需先刷新
**类型**：DOC（操作顺序）
**发现步骤**：功能 6 — health run 第一次 vs 第二次
**现象**：第一次 health run 显示 `zen_reachable: critical`；运行 `zen probe` + `zen cache-stats` 后重跑才变 healthy
**根因**：health run 读 DB 里的最新 probe 记录，不实时探测；probe 数据可能过时
**正确操作顺序**：health run 前先 `zen probe --machine <ids>` + `zen cache-stats --endpoint-id <id>` 刷新探针数据

---

### F-033 · `log verify-startup` / `health analyze-advisories` 需要操作员提供项目路径
**类型**：DOC（前置要求）
**发现步骤**：功能 7 — log verify-startup
**现象**：两个命令都需要 `--editor-exe` 和 `--project` 的完整 Windows 路径；路径错误报 "project not found"（exit 1）
**影响**：GUI 上"Log 验证"功能必须先让用户选择/输入 UE 编辑器路径和项目文件路径，不能自动推导；可以从 DB 里的 project_locations 表辅助提示

---

### F-034 · `deploy ddc` plan JSON schema 要求所有字段全填，即使功能已禁用
**类型**：UX（用户体验问题）
**发现步骤**：功能 8 — deploy ddc dry-run
**现象**：`pso.enabled: false` 时仍然要求填 `resolution` 和 `max_minutes`；`verify.run_log_verify: false` 时仍要求填 `editor_exe` 和 `timeout_seconds`；`shared_cache: null` 报错，必须填 SharedCacheSpec 结构
**影响**：CLI 做计划文件较繁琐；GUI 生成 plan JSON 时必须填充所有字段，不能按功能开关裁剪

---

### F-035 · `deploy ddc --plan` 路径必须是 Windows 格式，不接受 WSL/Unix 路径

**类型**：DOC（平台限制）
**发现步骤**：功能 8 — deploy ddc
**现象**：`--plan /tmp/xxx.json` 报 "系统找不到指定的文件"（exit 1）；改用 `E:\...` 格式路径正常
**影响**：GUI 内部生成 plan 文件必须写到 Windows 可见路径，不能用 WSL temp 目录

---

---

### F-037 · `zen service install` 新 sc.exe 方式在 PowerShell 下 exit 1639 ✅ 已修复（commit 237b012）
**类型**：BUG（服务安装失败）
**发现步骤**：ZenServer 重新部署复验
**现象**：`zen service install` 报 "sc create failed (exit 1639)"（`ERROR_INVALID_COMMAND_LINE`）
**根因**：commit 92017be 将 `zen.exe service install` 改为 `sc.exe create` 直接调用，但使用 PowerShell splatting 传参 `& sc.exe @scArgs` 时，`$binpath` 字符串中已有内部引号，PowerShell 对其进行二次转义，sc.exe 无法识别该格式
**修复**：`ps-scripts/zen-service-install.ps1` — 将 sc.exe 调用改为 `cmd /c` 字符串方式，绕过 PowerShell 参数转义层
**复验结果**（2026-06-13）：`zen service install` 成功，`UECMZenServer` 服务创建并启动（Running, Automatic）

---

### F-038 · `zen enable --global` 写 UserEngine.ini 路径错误（Roaming → Local）✅ 已修复（commit 0e9aeb4）
**类型**：BUG（AutoLaunch=false 写入但 UE 未读到）
**发现步骤**：分析 `AutoLaunch=false` 修复后仍弹窗的根因
**现象**：`zen enable --global` 完成并显示 "AutoLaunch=false 写入成功"，但 UE 打开时仍弹出 "Failed to auto launch, failed to shut down currently running service using port 8558"
**根因**：`domain_zen.rs` 硬编码的 UserEngine.ini 路径为 `AppData\Roaming\Unreal Engine\Engine\Config\UserEngine.ini`，但 UE 读取配置用的是 `AppData\Local\Unreal Engine\Engine\Config\UserEngine.ini`。
UE 源码确认链路：`ConfigHierarchy.h` Layer "UserSettingsDir" 路径模板 = `{USERSETTINGS}Unreal Engine/Engine/Config/User{TYPE}.ini`；`{USERSETTINGS}` = `FPlatformProcess::UserSettingsDir()` = `FOLDERID_LocalAppData` = `%LocalAppData%`（而非 `%AppData%`/Roaming）。写入的 Roaming 路径不在 UE GEngineIni 合并链中，`AutoLaunch=false` 永远不生效。
**修复**：`src-tauri/src/cli/domain_zen.rs` 两处 UserEngine.ini 路径从 `AppData\Roaming` 改为 `AppData\Local`（commit 0e9aeb4）
**复验步骤**：
1. 替换 `C:\Tools\UECM\uecm-cli.exe` 为新 build
2. 重新运行 `zen enable --global --upstream-endpoint-id <id> --machines <id> --cred-alias <alias> --yes`
3. 确认写入路径为 `C:\Users\lanPC\AppData\Local\Unreal Engine\Engine\Config\UserEngine.ini`
4. 启动 UE，确认不再弹 "Failed to auto launch" 弹窗

---

### F-036 · UECM 管理的 ZenServer 系统服务与 UE Editor 冲突 ✅ 已修复（架构调整）
**类型**：BUG（同机部署时 UE Editor 与 UECM 服务端口冲突）
**发现步骤**：走读后分析 + UE 5.7 启动截图
**现象**：UECM 在工作站上安装 ZenServer 服务后，UE 启动时弹出 4 个错误弹窗（权限不足、卸载失败、端口冲突），Zen 缓存不可用
**根因**：UE AutoLaunch 与 UECM 服务共机部署，争抢端口 8558。UE 尝试接管已有的 zenserver 进程，但没有管理员权限操作系统服务
**曾尝试**（已回滚）：
- commit 92017be：写入 `[Zen] AutoLaunch=false` 禁用 UE 自管理 → 虽然消除弹窗，但禁用了 UE 内建 Zen Server Status 面板
**最终方案**（commit 237b012）：
- 回滚 `AutoLaunch=false` 逻辑
- 改为 UE 官方推荐的**独立服务器部署**：ZenShared 跑在专用服务器上，工作站保持 AutoLaunch 默认行为，两层无端口冲突
- 保留服务名 `UECMZenServer`（避免与 UE 内部 `ZenServer` 碰撞）
**复验**（2026-06-13）：独立服务器方案下，UE Editor 启动无弹窗，ZenLocal + ZenShared 均 `status: OK!`

---

### F-039 · 架构决策：ZenServer 必须独立服务器部署，不可与工作站共机
**类型**：ARCH（架构约束，2026-06-13 确认）
**发现步骤**：F-036 修复过程中对照 UE 官方文档 *Zenserver as Shared DDC*
**背景**：UE DDC 分三层——ZenLocal（工作站本地）/ ZenShared（LAN 共享服务器）/ Cloud（公网）。UE Editor 的 AutoLaunch 机制自动管理 ZenLocal，监听 localhost:8558。如果 UECM 的 Shared DDC 服务也装在同一台机器的 8558 端口，两者必然冲突。
**约束**：
- UECM `zen service install` 目标机器必须是**不运行 UE Editor 的独立服务器**
- 工作站只执行 `zen enable`（客户端配置），不安装 zen 服务
- 同一台机器不能同时承担 ZenLocal 和 ZenShared 角色
**影响**：
- CLI/GUI 的 `zen service install` 流程应校验目标机器不是工作站（或至少 warn）
- CLAUDE.md 和 Skill 的部署指引必须明确这一约束
- 之前的 `AutoLaunch=false` workaround 已从代码中移除（commit 237b012）

---

## 接口行为备忘

### machine 域正确流程
```
1. machine scan <CIDR>          # 探测网段，只输出不写 DB
2. machine add --ip <IP>        # 把目标机器录入 DB，返回 id
3. machine refresh <id>         # SSH 连入，收集主机名/UE安装/GPU，写回 DB
4. machine list                 # 确认 DB 状态
5. machine detail <id>          # 查看单机完整信息
```

### system 域可用子命令
```
system version     # 打印版本
system db-path     # 打印当前 DB 路径
system ps-dir      # 打印 ps-scripts 目录
system migrate-db  # 初始化/迁移 DB 表结构（新 DB 必跑）
system echo <msg>  # 验证 PowerShell bridge 可用（msg 是位置参数）
system exit-codes  # 打印所有 exit code 含义
system schema      # 输出完整命令树 JSON（供 AI / 自动化使用）
```

### exit code 速查
| Code | 名称 | 含义 |
|------|------|------|
| 0 | ok | 成功 |
| 1 | operation_failed | 业务逻辑运行时失败 |
| 2 | invalid_input | 用户传入数据无效 |
| 3 | environment_error | 配置 / DB / IO 问题 |
| 4 | powershell_failed | 远程 PowerShell 调用失败 |
| 64 | usage_error | 命令行参数错误 |
