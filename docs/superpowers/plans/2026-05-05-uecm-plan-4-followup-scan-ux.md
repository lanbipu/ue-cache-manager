# Plan 4 Follow-up — INI Scanner & Health Check UX

**Status:** Open. To be picked up before or alongside Plan 5.

发现于 2026-05-05 lanPC E2E：scan 调用串行 14 次 PS sidecar，每次启动 PowerShell 1.5s + WinRM session 1.5s + read。即使全部成功也要 30 秒+。一旦 cred 失败，每次 30s timeout × 14 = 7 分钟黑屏。用户感知"卡死"。

健康度检查同样问题（11 项 × 1 次 PS round-trip，但 derived 项还要再读 ConsoleVariables.ini 等）。

## 三件待办（优先级从高到低）

### 1. 进度可见性（小工作量，~半天）

**改动点：**
- `core::ini_scanner::scan_machine` 新增可选 `progress_tx: Option<mpsc::Sender<ScanProgress>>` 参数。每次 `read_file` 完成后发一个事件 `{ machine_id, file_index, file_total, file_path, ok }`。
- `commands::ini_scanner::scan_inis` 把 mpsc 消息 emit 到 Tauri 事件 `ini-scan-progress`。
- 前端 `useDiagnosticsStore.runScan` 订阅事件，写到 store 的 `progress` 字段。
- `IniScanWizard.vue` 显示进度条 + 当前 file path。
- 同样改 `core::health_check::run_health_check` + Health wizard。

**收益：** 从"黑屏 30 秒"变成"看着 14 个文件逐个推进"。失败的话立刻能看出哪个 file 卡。

### 2. Cancel 按钮（小工作量，~半天）

**改动点：**
- 新增 `tokio::sync::CancellationToken` 作为 scan/health 的 cancel signal。
- `commands::cancel_scan(scan_run_id)` Tauri command 触发 token cancel。
- `core::ini_scanner::scan_machine` 每次 `read_file` 前 check token，命中就 break 循环，把已收集的部分 finding 持久化，scan_run row finished_at 标记 cancelled。
- Wizard 加 Cancel 按钮 + busy 状态切换。

**收益：** 用户卡住时不必等 7 分钟，立刻能取消。

### 3. 并行 + WinRM session 复用（中等工作量，~1.5 天）

**改动点：**
- 把 `read_file` 提取为一个 `async fn`，scanner 用 `futures::stream::iter(...).buffer_unordered(N)` 并行跑（N=4 起步，避免 lanPC WinRM 会话上限）。
- 把每个 file 一个独立 PS 进程改为：单个 PSSession + 多 ScriptBlock。需要一个新的 `ps-scripts/read-ini-batch.ps1`，接受 `-FilePaths` 数组，返回 `[{ path, found, sections }]`。
- `core::ini_scanner` 把 N 个 file path 一次性传给 batch script。
- 同样改 health probes（已经是单次 round-trip 8 probes，可以扩展同 session 跑 derived 检查）。

**收益：** 14 个文件从 14×4s=56s 缩到 1×6s≈6s（PS 启动一次 1.5s + WinRM session 1.5s + 14 个 file Get-Content 串行 ~3s）。

## 约束

- 不能引入 PowerShell 7 依赖（现场客户端可能只有 5.1）。
- 不能改 `read-ini-file.ps1` 已有的输出格式（保持兼容老 caller，新建 `read-ini-batch.ps1`）。
- 并行度 cap 必须可配置（lanPC WinRM 默认 25 sessions，但客户机可能更低）。

## 测试计划

- 单元：mock progress_tx，断言事件次数 = file count。
- 单元：mock cancel token，断言 break 后已 commit 的 finding 数。
- E2E（lanPC）：scan 跑完看进度条更新；中途 cancel 看部分结果保留。

## 验收标准

- 14 文件 INI scan ≤ 10 秒（成功路径）。
- 中途取消 ≤ 1 秒。
- 失败 file 立刻在进度条上标红，不会拖到全部 timeout。

## 4. Loopback 路径 admin elevation（中等工作量，~1 天）

发现于 2026-05-06 Codex review 后续清理（commit `9c999b8` 留的 P2 flag）。

`core::health_probes::run` 命中 loopback 时直接走 `-Local` 模式，把 caller 提供的 admin credential explicit drop 掉。原因是同进程内安全切换 token 不可靠，但带来的副作用是：UECM 以普通 user token 启动时，即使 wizard 选了 admin cred，依赖 `PsExec -s`/SYSTEM 视角的 probe（`Probe-CredSystem`、`Probe-SystemWrite`）仍跑在 medium IL 下，结果就是 lanPC 普通模式跑出来的 `cred_system` / `system_write` 永远偏向 warning/critical。

**改动思路：**
- `health-probes.ps1` `-Local` 分支接受 `-Username`/`-Password`，存在时用 `Start-Process -Credential -Verb RunAs` 启动一个 elevated PowerShell child，让它跑 scriptblock 并把 JSON 写到临时文件，父进程读回后清理。
- `core::health_probes::run` 在 loopback + cred 都有的情况下，把 cred 透传给 ps-script，不再 drop。
- `core::ini_scanner` 的 loopback bypass 同问题：`std::fs::read_to_string` 用当前 token，admin-only path（如 `C:\ProgramData\Epic\UnrealEngineLauncher`）读不到。同样用 elevated child process 的话需要 ini_scanner 也走 ps-script `-Local` 模式而不是 std::fs，这个改造更大。

**约束：**
- `Start-Process -Verb RunAs` 会触发 UAC 弹窗。要么 wizard 提示用户接受，要么仅在已经是 admin token 时才走这条路。
- elevated child 写临时文件的位置要选 user 都能读的地方（`$env:TEMP` 通常 OK）。
- 取消逻辑要能 kill child process，不能挂在 `Start-Process` 上。

**收益：** loopback + admin cred 真的能产生 admin probe 结果，UECM 普通模式下也能正确 verify SYSTEM-level credential / NTFS ACL。当前是无法绕开的盲区。
