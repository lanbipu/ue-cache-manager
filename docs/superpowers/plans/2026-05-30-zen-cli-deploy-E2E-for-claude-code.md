# 任务（交给本地 Claude Code 跑）：验证 UECM zen-server 部署链修复 F1–F6（lanPC, machine 13）

> 你（本地 Claude Code）在 lanPC 上**交互式**运行这套 E2E。这些 `uecm-cli` 命令会经 SSH(uecm-svc) 连本机节点 —— 在交互式会话里能正常工作。逐步执行、**每步判定 PASS/FAIL**、关键断言失败就**停下报告**、不要盲目继续。`uecm-cli` 支持 `--output json`，需要断言字段时用它 + `ConvertFrom-Json`。

## 背景

分支 `zen-cli-deploy-fixes` 部署了 6 个修复，已装好：
- 二进制：`C:\Tools\UECM\uecm-cli.exe`（SHA256 `21BAA2E818486630BDDD92E40C1499CBF7413A99A31479E8DCC81FDE6921A899`）
- PS 脚本目录（权威，由 `system ps-dir` 决定）：`C:\Tools\UECM\ps-scripts`（含 `zen-service-install.ps1` / `zen-sponsor-down.ps1` / `zen-detect-binary.ps1`）

被测 6 项：F1 intree fallback、F2 端口持久化进 SCM ImagePath（+ 已存在服务 drift 就地修复）、F3 detect fail-fast、F4 `zen sponsor-down` 身份守卫、F5 detect 跨用户发现、F6 apply-config 自动推 dest-path。

目标机器：machine id **13** = lanPC = 192.168.10.20。数据目录 `F:\Epic\DDC\Zen`。端口 8558。

⚠️ 这套流程会**真实修改本机**（装/起 Windows 服务 `ZenServer`、写 zen.lua、改 ini）—— 这就是预期的「把 Zen 部成 installed_service」结果，不是临时测试。

## 0. 前置自检（只读）

```powershell
$C = 'C:\Tools\UECM\uecm-cli.exe'
& $C --version
(Get-FileHash $C -Algorithm SHA256).Hash            # 期望 21BAA2E8...A899
& $C system ps-dir                                   # 期望 path = C:\Tools\UECM\ps-scripts
```
全对才继续。

## 1. 前置 refresh（F1/F3 数据依赖）

```powershell
& $C machine refresh 13
```
期望：完成、无错误（填充 machine_ue_installs：UE 5.2~5.8）。

## 2. detect-binary —— F5（跨用户发现）

```powershell
& $C zen detect-binary --machine 13 --output json | ConvertFrom-Json
```
**断言（F5）**：`install_record_written = true`（修复前在 lanPC 恒为 false，因为只看 uecm-svc 的 LOCALAPPDATA；F5 后能找到 lanPC 用户 profile 下的 install 目录）。且 `intree_records_written > 0`。
（F3 的 fail-fast 正例需要一台没跑过 refresh 的机器，本机不便造，跳过——F3 已在 mac 单测 + 评审覆盖。）

## 3. 注册 endpoint

```powershell
$reg = & $C zen register --machine 13 --role shared_upstream --declared-port 8558 --scheme http --data-dir F:\Epic\DDC\Zen --httpserverclass asio --output json | ConvertFrom-Json
$EP = (& $C zen list-endpoints --machine 13 --output json | ConvertFrom-Json | Where-Object { $_.declared_port -eq 8558 } | Select-Object -First 1).endpoint_id
"EP = $EP"
```
期望：拿到 endpoint id（register 对 (machine,port) 幂等，已存在则复用）。后续都用 `$EP`。
> 若 `list-endpoints` 的字段名不是 `endpoint_id`/`declared_port`，先 `& $C zen list-endpoints --machine 13 --output json | ConvertFrom-Json | Format-List` 看真实字段名再取。

## 4. URL ACL

```powershell
& $C zen urlacl add --endpoint-id $EP --principal "NT SERVICE\ZenServer" --yes
```
期望：ok（授权 `http://+:8558/`）。

## 5. 装服务 + 【关键断言①】ImagePath 含 --port（F2 全新安装）

先看现状：
```powershell
sc.exe qc ZenServer        # 记下当前是否存在、ImagePath 是否含 --port
```
装：
```powershell
& $C zen service install --endpoint-id $EP --yes --output json | ConvertFrom-Json
sc.exe qc ZenServer
```
**关键断言①（F2）**：`sc qc` 的 `BINARY_PATH_NAME` 含 `--port 8558` 且 `--http asio`。
> 若服务在第 5 步之前就已存在且 ImagePath **没有** `--port`（旧 CLI 装的坏态），那么这次 install 的 JSON 应直接返回 `repaired = true`，这同时也满足了下面的【关键断言②】。

## 5b. 【关键断言②】已存在服务 port/http drift 就地修复（F2，Codex #1 —— 最关键回归点）

若【断言②】尚未在 5 自然满足，则**主动造坏态再修**：
```powershell
# 取当前 exe 路径（ImagePath 第一个带引号 token）
$img = (Get-CimInstance Win32_Service -Filter "Name='ZenServer'").PathName
$exe = ([regex]::Match($img, '^\s*"?([^"]+\.exe)"?')).Groups[1].Value
"exe = $exe"
sc.exe stop ZenServer 2>$null
# 写一个只有 --data-dir、没有 --port 的 ImagePath（模拟旧 CLI 坏态）
sc.exe config ZenServer binPath= "`"$exe`" --data-dir F:\Epic\DDC\Zen"
sc.exe qc ZenServer        # 确认现在 ImagePath 不含 --port
# 重跑 install → 应就地修复
& $C zen service install --endpoint-id $EP --yes --output json | ConvertFrom-Json
sc.exe qc ZenServer
```
**关键断言②（F2/Codex#1）**：install 的 JSON 返回 `repaired = true`（**不是** `already_installed` 静默 no-op），且 `sc qc` 的 ImagePath **重新含** `--port 8558 --http asio`。

## 6. 【关键断言③】sponsor-down 身份守卫拒绝（F4，Codex #2）

确保服务在跑（它占着 8558）：
```powershell
& $C zen service start --endpoint-id $EP
Get-NetTCPConnection -LocalPort 8558 -State Listen | Select-Object OwningProcess
& $C zen sponsor-down --endpoint-id $EP --dry-run --output json | ConvertFrom-Json
```
**关键断言③（F4）**：sponsor-down 返回 `refused = true` 且 `is_installed_service = true`（因为 8558 上跑的是已装的 ZenServer 服务、不是 editor sponsor → 守卫拒绝，提示用 `zen service stop`）。输出应含 `listener_pid`。
> 这验证了「不会误杀已装服务」的安全守卫。fail-closed 分支（path 解析不出 → `path_unresolved:true`）现场难复现，可只读 `C:\Tools\UECM\ps-scripts\zen-sponsor-down.ps1` 的 `$null -eq $listenerPath` 分支确认逻辑在位，不必强测。

## 7. 【关键断言④】服务起在 8558、不 relocate（F2）

```powershell
& $C zen probe --machine 13 --output json | ConvertFrom-Json     # 看 effective_port
# 重启再验端口稳定
& $C zen service stop --endpoint-id $EP --yes
& $C zen service start --endpoint-id $EP
& $C zen probe --machine 13 --output json | ConvertFrom-Json
Get-NetTCPConnection -LocalPort 8558 -State Listen
```
**关键断言④（F2）**：`probe` 的 `effective_port = 8558`（**不是 8658**），且重启后仍 8558。`Get-NetTCPConnection` 显示 8558 有 zenserver 监听。
> 若字段名不是 `effective_port`，`Format-List` 看真实字段。

## 8. F6 —— apply-config 不带 --dest-path 自动推导

```powershell
& $C zen apply-config --endpoint-id $EP --yes --output json | ConvertFrom-Json
```
期望（F6）：ok，自动从 install 目录推出 `...\Zen\Install\zen.lua` 并写入（修复前 `--dest-path` 必填）。
（若该机只有 intree zen.exe、无 install-dir 记录，应报「pass --dest-path explicitly」——这也是正确行为。）

## 9. 收尾确认（只读）

```powershell
& $C zen cache-stats --endpoint-id $EP --output json | ConvertFrom-Json   # providers 含 z$
& $C zen status --machine 13 --output json | ConvertFrom-Json             # lifecycle_mode=installed_service, ok=true
& $C health run --machine-ids 13
```

## 报告格式

逐步给 PASS/FAIL + 实际观测值 vs 期望。最后给一张**四个关键断言**的小结表：

| 断言 | 期望 | 实际 | PASS/FAIL |
|---|---|---|---|
| ① F2 全新安装 ImagePath 含 --port 8558 --http asio | 是 | | |
| ② F2 已存在服务 drift → `repaired:true` + ImagePath 补回 --port | 是 | | |
| ③ F4 服务在跑时 sponsor-down `refused:true, is_installed_service:true` | 是 | | |
| ④ F2 probe `effective_port=8558`（非 8658），重启后稳定 | 是 | | |

任一关键断言 FAIL：保留现场（`sc qc ZenServer`、`zen probe` 的原始输出、相关 JSON），停下报告，不要继续后续步骤。

## 失败返工通道（给发起方）

任一断言不符，发起方在 mac 的 `.worktrees/zen-cli-deploy-fixes/` 改对应 Rust/PS → `git bundle` 重传 lanPC worktree `E:\zcdf-build` → `cargo build --release --bin uecm-cli`（共享 `E:\AIWorkspace\vp\ue-cache-manager\src-tauri\target` 缓存，~2-3min）→ 重新部署 `C:\Tools\UECM\uecm-cli.exe` + `C:\Tools\UECM\ps-scripts\*.ps1` → 重跑本流程对应步骤。
