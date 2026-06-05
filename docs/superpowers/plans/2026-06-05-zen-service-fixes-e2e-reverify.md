# Zen service-install fixes — lanPC E2E 复验 prompt (bugs 1–4)

> 给 **lanPC 本机的 Claude Code** 执行。前一轮 E2E（`docs/research/2026-06-05-zen-service-install-e2e-findings.md`）发现 F2 service 链被 4 个 bug 挡死；mac 端已修好并 commit（`7ee134c`），本 prompt 让你在真机重跑、确认那几个之前 FAIL/不可达的断言，确认后才 push。
>
> **机器 / 路径**：machine 13 = lanPC = 192.168.10.20（本机）。CLI = `C:\Tools\UECM\uecm-cli.exe`；源码仓 = `E:\AIWorkspace\vp\ue-cache-manager`；**权威 ps 目录 = `C:\Tools\UECM\ps-scripts`（不是 ProgramData）**。够节点命令在本机（WSL interop / 原生）跑**不 hang**（前一轮已证），交互式跑即可。

## 修了什么（4 个 bug，对应预期）

| Bug | 修法 | 复验断言 |
|---|---|---|
| **4** installed_service 用了用户私有 install-copy exe（LocalService 无 ACL，start 即退） | exe 解析 **intree 优先**（`pick_service_zen_exe`） | 服务 binary 落在 `D:\Program Files\Epic Games\UE_5.8\…\Win64\zenserver.exe`（带 `Users:RX`），**start 后真的常驻**、probe 8558 reachable |
| **2** 幂等比较拿 zen.exe 比 zenserver.exe，恒判 drift | `Normalize-ZenExe` 同目录归一 | 配置全对的重装 → `already_installed:true`（不再误报 different ZenExePath） |
| **1** 已存在服务只 no-op/refuse，无 repair | 仅 port/http drift → 原地 patch ImagePath → `repaired:true` | 造「缺 --port」坏态后重装 → `repaired:true` + ImagePath 补回 port/http |
| **3** built-in 账户 `password= ''` 被 splat 吞 → sc 1639 | built-in 省掉 password token | `zen service install --service-user LocalSystem` → ok（不再 1639） |

## 0. 取修复 + build + 部署

```powershell
$repo = "E:\AIWorkspace\vp\ue-cache-manager"
# 0a. FF 进 lanPC main（base 46ac186，lanPC 已在该 commit，干净 FF +2）
git -C $repo fetch "$env:USERPROFILE\zen-service-fixes.bundle" main
git -C $repo merge --ff-only FETCH_HEAD
git -C $repo log --oneline -3          # 应见 7ee134c fix(zen): repair installed_service deploy chain
# 0b. build CLI（共享 target 缓存，约 2–3min；别用 pnpm tauri build，这次只要 CLI）
cd "$repo\src-tauri"; cargo build --release --bin uecm-cli
# 0c. 部署 exe + ps 到运行位置（权威 ps 目录 = C:\Tools\UECM\ps-scripts）
Copy-Item "$repo\src-tauri\target\release\uecm-cli.exe" C:\Tools\UECM\uecm-cli.exe -Force
Copy-Item "$repo\ps-scripts\*.ps1" C:\Tools\UECM\ps-scripts\ -Force
# 0d. 冒烟
C:\Tools\UECM\uecm-cli.exe zen service install --help | Select-String service-user
```

## 1. 前置 + 清理上一轮残留

- [ ] `uecm-cli machine refresh 13` → authenticated，记录 ue_versions / gpus。
- [ ] `uecm-cli zen detect-binary --machine 13` → `install_record_written:true` + `intree_records_written:5`（F5，照旧）。
- [ ] 清干净 service 态（上一轮 endpoint 1 应仍在、service 已 uninstall；若 `sc query ZenServer` 还在，先 `uecm-cli zen service uninstall --endpoint-id 1 --yes`）。endpoint 1 = port 8558 / `F:\Epic\DDC\Zen` / asio / shared_upstream；不在就 `uecm-cli zen register --machine 13 --role shared_upstream --declared-port 8558 --scheme http --data-dir F:\Epic\DDC\Zen --httpserverclass asio` 重建并记 `<EP>`。
- [ ] `uecm-cli zen urlacl add --endpoint-id <EP> --principal "NT AUTHORITY\LOCAL SERVICE" --yes`（注意 principal 是 **LOCAL SERVICE**，前一轮 doc 写的 `NT SERVICE\ZenServer` 是笔误——zen 默认账户是 LocalService）。

## 2. Bug 4 + 断言 ①④ — 全新装 intree + start 常驻

- [ ] `uecm-cli zen service install --endpoint-id <EP> --yes` → ok。
- [ ] `sc qc ZenServer` → **BINARY_PATH_NAME token0 = `D:\Program Files\Epic Games\UE_5.8\Engine\Binaries\Win64\zenserver.exe`**（intree，**不是** `C:\Users\…\AppData\Local\…`），且含 `--port 8558 --http asio`。
- [ ] `uecm-cli zen service start --endpoint-id <EP>` → ok。**关键（Bug 4）**：`sc query ZenServer` → **RUNNING 且保持**（不再 STOPPED exit 0）。
- [ ] `uecm-cli zen probe --machine 13` → `effective_port:8558, reachable:true`。
- [ ] stop→start 再来一次 → 仍 8558（断言④端口稳定）。

## 3. Bug 2 + Bug 1（断言 ②）— drift 修复 + 幂等

- [ ] 幂等：再跑 `uecm-cli zen service install --endpoint-id <EP> --yes` → **`already_installed:true`**（Bug 2：不再误判 different ZenExePath）。
- [ ] 造坏态（保持同一 exe，仅去掉 port/http）：`sc config ZenServer binPath= "\"D:\Program Files\Epic Games\UE_5.8\Engine\Binaries\Win64\zenserver.exe\" --data-dir F:\Epic\DDC\Zen"`，确认 `sc qc` ImagePath 不含 `--port`。
- [ ] `uecm-cli zen service install --endpoint-id <EP> --yes` → **`repaired:true`**（Bug 1）+ ImagePath 补回 `--port 8558 --http asio`。提示含「需 stop+start 生效」。

## 4. Bug 3 — built-in 账户

- [ ] `uecm-cli zen service uninstall --endpoint-id <EP> --yes` → ok。
- [ ] `uecm-cli zen service install --endpoint-id <EP> --service-user LocalSystem --yes` → **ok（不再 1639/rollback）**；`sc qc ZenServer` StartName = `LocalSystem`。
- [ ] 复位默认账户给下一步：uninstall → `zen service install --endpoint-id <EP> --yes`（默认 LocalService）→ start → RUNNING。

## 5. 断言 ③ — sponsor-down 守卫（之前不可达，现在应可达）

- [ ] 服务（默认 LocalService）正在跑、占 8558。`uecm-cli zen sponsor-down --endpoint-id <EP> --dry-run` → **`refused:true, is_installed_service:true`**，listener_pid = 服务 PID，提示「该用 zen service stop」。
- [ ] 正例（7b）：`zen service stop` 后用 UE Editor 拉起 editor sponsor 占 8558 → `sponsor-down --dry-run` → `would_stop:true, is_installed_service:false` → `--yes` 关停 ok。

## 6. 收尾

- [ ] `uecm-cli zen cache-stats --endpoint-id <EP>` → providers 含 `z$`。
- [ ] `uecm-cli zen status --machine 13` → `lifecycle_mode:installed_service, ok:true`。
- [ ] `uecm-cli health run --machine-ids 13` → 多数 healthy。

## 完成判据 + push

- 关键四项：**①** intree ImagePath + start RUNNING、**②** `repaired:true` + 幂等 `already_installed`、**③** 服务在跑时 sponsor-down `refused`、**④** 端口稳定 8558；外加 Bug 3 `--service-user LocalSystem` ok。
- 把「期望 vs 实际」追加进 `docs/research/2026-06-05-zen-service-install-e2e-findings.md`（或新建一份 reverify findings）。
- **全过** → 这批修复（mac commit `7ee134c`）+ 之前的 backflow（`debda37`/`e3c0622`）一起 push origin main（lanPC git push 走 GCM 无 tty 会失败，用 `git bundle` 搬回 mac push，见 memory `lanpc_git_repo_push`）。
- **任一仍 FAIL** → 记实际输出，回 mac worktree 修，别 push。
