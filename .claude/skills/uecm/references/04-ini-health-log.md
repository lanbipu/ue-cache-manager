# 流程 04 — INI 扫描修复 + health 诊断 + log 验证

约定 `BIN=/mnt/c/Tools/UECM/uecm-cli.exe`，远程带 `--cred-alias render-svc --output json --no-input`。

---

## A. INI 扫描与自动修复

UE 的 DDC/PSO/Zen 配置散落在 BaseEngine.ini / DefaultEngine.ini / ConsoleVariables.ini。
`ini scan` 跑一套规则集（R0xx）找出配置问题，部分能一键修。

### A1. 扫描 → 看 findings

```bash
"$BIN" ini scan --machine-ids <M1,M2> --cred-alias render-svc --output json     # 返回 scan_run_id
"$BIN" ini runs --limit 5                                                        # 列最近批次
"$BIN" ini findings <scan_run_id> --output json                                 # 列该批次所有 finding
"$BIN" ini get-finding <finding_id> --output json                              # 单条详情（含 fix_kind / 推荐值）
"$BIN" ini config <scan_run_id> --output json                                  # 看抓到的 DDC/PSO/Zen 配置快照
```

### A2. finding 分两类，区别对待

- **`set` 类**：CLI 知道推荐值（如 R015 `DeleteUnused=true`），可一键 `ini apply` 自动修。
- **`manual` 类**：CLI 不知道正确值（如 R021 `Shared.Path` 需要 operator 提供的 UNC 路径），必须人工 `ini set`。

判断方式：看 `ini get-finding` 输出里的修复类型字段。**自动修只对 `set` 类做；`manual` 类要把情况告诉用户、问清值再 `ini set`。**

### A3. 修复

```bash
# set 类：一键 apply（自动先建 .bak.<时间戳> 备份）
"$BIN" ini apply <finding_id> --cred-alias render-svc --dry-run
"$BIN" ini apply <finding_id> --cred-alias render-svc --yes

# manual 类：人工填值（例 R021 Shared.Path = 流程 01 share create 得到的 UNC）
"$BIN" ini set --host <ip> --file '...\BaseEngine.ini' --section DerivedDataBackendGraph --key Shared.Path --value '\\LANPC\DDC-Shared' --cred-alias render-svc --yes

# 不打算修的：标记跳过（避免下次扫描重复提醒）
"$BIN" ini skip <finding_id>
```

### A4. 复验

```bash
"$BIN" ini scan --machine-ids <M> --cred-alias render-svc --output json    # 重扫，确认 warning 数下降
```

### 常见 finding 速记
- **R015** `Shared.DeleteUnused` 缺失 → set 类，`ini apply` 就地改 tuple 内联值（F-030 已修，apply 后 tuple 内含 `DeleteUnused=true`）。
- **R021** `Shared.Path` 非 UNC → manual 类，填 `share create` 的 UNC（F-012）。
- **R008-R010** PSO precaching CVar → 在 ini scan 里跑（`pso verify` 只是转向提示，F-026）。也可单独 `ini verify-pso-precaching --project-id <P>`。
- **R013/R014/R026** ZenShared Host URI 形态 → Host 必须带 scheme + 内嵌端口（`http://host:8558`），scheme-less / 游离 `Port=` 标 malformed。

### backend-graph 直接读写（精确操作 tuple）
```bash
"$BIN" ini backend-graph get --host <ip> --file-path '...\BaseEngine.ini' --node Shared --field DeleteUnused --cred-alias render-svc
"$BIN" ini backend-graph set --host <ip> --file-path '...\BaseEngine.ini' --node Shared --field DeleteUnused --value true --cred-alias render-svc --yes
"$BIN" ini backend-graph scan --host <ip> --file-path '...\BaseEngine.ini' --cred-alias render-svc
```

### Shared DDC 垃圾回收开关
```bash
"$BIN" ini gc-pause  --host <ip> --project-id <P> --cred-alias render-svc --yes   # 大批渲染前暂停 GC（DeleteUnused=false）
"$BIN" ini gc-resume --host <ip> --project-id <P> --cred-alias render-svc --yes   # 之后恢复（DeleteUnused=true）
```

---

## B. health 集群诊断

L1（端口）/L2（SSH）/L3（业务配置）三层检查，每条 critical 带 `remediation`（具体修复命令）。

### B1. 跑诊断（前置：先刷 zen 探针）

```bash
"$BIN" zen probe --machine <ids> --cred-alias render-svc          # ⚠️ 必须先刷，否则 zen_reachable 误报 critical
"$BIN" zen cache-stats --endpoint-id <eid>                        # Zen 集群才需要
"$BIN" health run --machine-ids <ids> --cred-alias render-svc --output json
"$BIN" health runs --limit 1                                      # 拿 scan_run_id
"$BIN" health results <scan_run_id> --output json                # 逐行结果 + remediation
```
可选传 `--expected-local-path` / `--expected-shared-path` 让 health 比对期望值。

### B2. 按 remediation 自动修复闭环

health results 每条 critical 都有 `remediation` 字段（可执行命令）。闭环做法：
1. 跑 `health run` → 筛出 critical findings；
2. 逐条读 `remediation`，对照本 skill 的安全门（写操作先 dry-run 给用户看）执行；
3. 修完重跑 `health run` 验证转 healthy。

### B3. health 的误报与降级（别被吓到）
- Zen 模式下 `env_shared/env_vars` 自动降为 `na`（不再因没设 `UE-SharedDataCachePath` 误报 critical，DESIGN-1 已修）。
- `zen_reachable` critical 但服务在跑 → probe 数据过 5 分钟窗口（F-043），重跑 `zen probe`。
- `health file-stats` 对 shared UNC 报 not found → 多半是 SMB 共享环境层没真正生效（F-042），命令本身没错。

### B4. 其他 health 子命令
```bash
"$BIN" health consistency-check --hosts <H1,H2> --cred-alias render-svc --output json   # 多机配置一致性对比
"$BIN" health scan-command-line --host <ip> --cred-alias render-svc                     # 扫快捷方式/bat/服务里的 DDC 路径覆盖
"$BIN" health file-stats --host <ip> --local-path 'D:\UE-DDC-Local' --shared-path '\\LANPC\DDC-Shared' --cred-alias render-svc
"$BIN" gpu matrix --output json                                                          # 全机 GPU 一致性（虚拟适配器已过滤）
```

---

## C. log 验证

```bash
"$BIN" log verify-startup --host <ip> --editor-exe 'D:\Program Files\Epic Games\UE_5.5\Engine\Binaries\Win64\UnrealEditor.exe' --project 'E:\Projects\Foo\Foo.uproject' --cred-alias render-svc --output json
"$BIN" health analyze-advisories --host <ip> --editor-exe '...UnrealEditor.exe' --project '...\Foo.uproject' --local-path 'D:\UE-DDC-Local' --shared-path '\\LANPC\DDC-Shared' --cred-alias render-svc --output json
```
- ⚠️ `--editor-exe` 和 `--project` 必须是**真实存在的 Windows 完整路径**，不能自动推导（可从 `project locations` 辅助提示）（F-033）。
- ⚠️ 重型 VP 项目 quit 阶段 hang → timeout（F-044）；Zen 模式不输出 legacy DDC 路径日志行（F-045），此时改用 `zen probe`/`zen cache-stats` 验证缓存。
