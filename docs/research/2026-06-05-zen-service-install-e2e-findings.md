# Zen 部署链 F1–F6 E2E 发现（lanPC machine 13, 2026-06-05）

> 在 lanPC 本机（WSL interop 调 Windows exe）跑 backflow F3–F6 的 E2E。
> backflow commit `debda37`(feat) + `e3c0622`(docs) 已 FF 进 main（base `0bce94e`）。
> 重新 build 的 `C:\Tools\UECM\uecm-cli.exe` SHA256 = `db397006cf5e539f88a2652e75df9e83d7271b2ad4c47aab2398fa40f1250d28`。
> machine 13 = lanPC = 192.168.10.20；endpoint 1：port 8558 / data-dir `F:\Epic\DDC\Zen` / asio / shared_upstream → installed_service。

## 四个关键断言

| 断言 | 期望 | 实际 | 判定 |
|---|---|---|---|
| ① F2 全新装 ImagePath 含 `--port 8558 --http asio` | 是 | ImagePath 正确含 `--port 8558 --http asio` | ✅ PASS |
| ② F2 坏服务重装 `repaired:true` 补回 port | repaired:true | refuse，代码无 repair 路径 | ❌ FAIL（功能缺失） |
| ③ F4 服务在跑时 sponsor-down `refused:true` | refused:true | 服务起不来，无法构造场景 | ⏸️ 不可达 |
| ④ F2 probe `effective_port=8558` 重启稳定 | 8558 | 服务在此环境起不来 | ⏸️ 不可达 |

## 通过项

- A1–A4：bundle FF、Windows cargo build、部署、冒烟（help / ps parse）全过。
- hang 探测：WSL interop 下够节点命令（refresh/detect/register/...经 SSH uecm-svc）**不 hang**。
- F5 `zen detect-binary --machine 13`：`install_record_written:true`（来自 lanPC profile，修复前恒 false）+ `intree_records_written:5`。
- F3 fail-fast：code-read 确认 `core/zen/binary.rs:320 detect_yielded_nothing` + caller `commands/zen.rs:376→391`、`cli/domain_zen.rs:569→578`（消息 `run machine refresh first`）+ 双分支单测（binary.rs:903/920）。
- 断言① F2 全新装：ImagePath port pinning 正确。
- F6 `zen apply-config`（不带 `--dest-path`）：自动推导 `...\Zen\Install\zen.lua` 写入 351 字节。
- F4 dry-run：`would_stop:true, is_installed_service:false, listener_pid`（sponsor 识别正确）。

## Bug 清单（均需回 mac worktree 修）

### Bug 1 — F2 drift-repair 完全缺失（断言②无对应代码）

部署的 `ps-scripts/zen-service-install.ps1` 对「已存在但配置不匹配」的服务**一律 refuse**（`zen-service-install.ps1:380-415`）：`matchesExpected && userMatches` 走 no-op，否则全部走 `ok=false` + "Refusing to re-install without --full"。**没有 `repaired` 字段、没有对已存在服务的 binpath patch 分支**。

原分支 `zen-cli-deploy-fixes` 的 `c55c446 "fix(zen-ps): repair port/http ImagePath drift on existing service (F2, Codex #1)"` **没进 main、也不在 backflow**。backflow（`debda37`）只带 F3/F4/F6；main 的 `0bce94e` 只实现了全新装 port pinning（`zen-service-install.ps1:575+` patch binpath，仅全新装路径）。

**复现**：全新装服务后，`sc config ZenServer binPath= "...zenserver.exe --data-dir F:\Epic\DDC\Zen"`（去掉 --port）→ `uecm-cli zen service install --endpoint-id 1 --yes` → 返回 `error: ... different ZenExePath / DataDir ... Refusing to re-install`，而非 `repaired:true`。

### Bug 2 — drift 检测把 zen.exe 当 zenserver.exe 比较（已存在服务永远误判）

`commands/zen.rs:1068` 把 `install.zen_cli_path`（**zen.exe**）作为 `ZenExePath` 传给脚本（`zen.rs:1144/1154`）。但服务 ImagePath 里 token[0] 是 **zenserver.exe**（zen install 自己设的服务 exe）。脚本 `zen-service-install.ps1:359` 用 `existingExe(zenserver.exe) -eq $expectedExe(zen.exe)` 比较，**永远不等** → reason 永远是 `different ZenExePath`（`zen-service-install.ps1:395`）。

后果：连「配置完全正确、本应 no-op」的幂等重装也判 ExePath drift。Bug 1 的 refuse 报错里 reason 显示 `different ZenExePath / DataDir`，根因就在这里（不是真的路径变了）。

修复方向：比较时应拿 ImagePath 实际的 zenserver.exe 路径，或脚本接收 zenserver.exe 路径而非 zen.exe；或对 exe 名做 zen↔zenserver 归一。

### Bug 3 — `--service-user` 内置账户 sc config 1639（LocalSystem/LocalService/NetworkService 全装不上）

`zen-service-install.ps1:502`：
```powershell
$scArgs = @('config', $ServiceName, 'obj=', $effectiveUser, 'password=', $effectivePassword)
& sc.exe @scArgs
```
built-in 账户 `$effectivePassword = ''`（空字符串，line 501）。PowerShell `& sc.exe @array` 展开数组时**空字符串 token 被丢弃**，sc.exe 看到 `password=` 后无值 → 命令行无效 → `exit 1639 (ERROR_INVALID_COMMAND_LINE)` → 触发回滚。

**复现**：`uecm-cli zen service install --endpoint-id 1 --service-user LocalSystem --yes` → `error: sc.exe config (set service account) failed (exit 1639); service rolled back`。

修复方向：built-in 账户不要传空 password token（sc config obj= 时省略 password=），或对空值用 `'""'` 字面量 / 改用 `Start-Process`/数组拼接保留空参数。

### Bug 4 — 部署架构断裂：F5 用户私有 exe + 默认 LocalService 无权（最根本）

F5（`zen detect-binary` 跨用户发现）把 install-dir binary 记录为 `C:\Users\lanPC\AppData\Local\UnrealEngine\Common\Zen\Install\zenserver.exe`（lanPC 用户私有）。zen 在 Windows 上**硬编码服务账户为 LocalService**（`zen-service-install.ps1:426-432` 注释：zenutil/service.cpp:441-453 `CreateService(..., "NT AUTHORITY\LocalService", NULL)`，忽略 `-u`）。

该 exe 的 ACL（实测）：`uecm-svc:ReadAndExecute / SYSTEM:FullControl / Administrators:FullControl / lanPC:FullControl` —— **无 LocalService**。故服务装得上、start 时进程立即退（exit 0，连日志都没写）。`sc query` 显示 STOPPED / WIN32_EXIT_CODE 0 / SERVICE_EXIT_CODE 0。

之前 8558 上能跑的 PID 12392 是 UE Editor 以 **lanPC 用户身份**拉起的 sponsor（lanPC 有 FullControl），不是服务。

即使修了 Bug 1/2/3，只要 exe 在用户私有目录 + 服务账户非特权，这条链仍断。修复方向（择一）：
- installed_service 默认用 LocalSystem（SYSTEM 对 exe 有 FullControl）——但被 Bug 3 挡住，需先修 Bug 3；
- 或把 zenserver.exe 部署到公共位置（非用户私有 AppData）再装服务；
- 或 install 后给 exe/install-dir 补 LocalService 读+执行 ACL。

## E2E 现场命令记录（关键步骤实际输出）

- `machine refresh 13` → `{"authenticated":true,"gpus":9,"ue_versions":7}`。
- `zen detect-binary --machine 13` → `install_record_written:true, intree_records_written:5`。
- `zen register --machine 13 --declared-port 8558 --data-dir F:\Epic\DDC\Zen --httpserverclass asio --role shared_upstream` → `endpoint_id:1, lifecycle_mode:installed_service`。
- `zen service install --endpoint-id 1 --yes` → ok，`binpath_patched:false`；ImagePath = `...zenserver.exe --data-dir F:\Epic\DDC\Zen --port 8558 --http asio`（断言① PASS）。
- 造坏态 `sc config`（去 --port）→ `zen service install --endpoint-id 1 --yes` → `error: different ZenExePath / DataDir ... Refusing`（Bug 1/2）。
- `zen service start --endpoint-id 1` → `error: Start-Service failed`，8558 无监听，`sc query` STOPPED exit 0（Bug 4）。
- `zen sponsor-down --endpoint-id 1 --dry-run` → `would_stop:true, is_installed_service:false, listener_pid:12392`（F4 dry-run PASS）。
- `zen apply-config --endpoint-id 1 --yes` → 自动推导 `...\Zen\Install\zen.lua`，写 351 字节（F6 PASS）。
- `zen service uninstall --endpoint-id 1 --yes` → ok。
- `zen service install --endpoint-id 1 --service-user LocalSystem --yes` → `error: sc config failed exit 1639; rolled back`（Bug 3）。

## 结论

- 不满足 push 判据（②③④未全过）。backflow 改动**未 push**。
- installed_service 部署链在 lanPC 真实环境（F5 用户私有 exe 路径）走不通，需先修 Bug 1–4。
- urlacl 旁注：E2E doc step4 用 principal `NT SERVICE\ZenServer` 是笔误——zen 默认服务账户是 LocalService，现有 `http://+:8558/` → `NT AUTHORITY\LOCAL SERVICE` 的 reservation 才是对的。
