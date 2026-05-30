# Zen CLI Deploy Fixes — lanPC E2E Runbook

> **必须在 lanPC 本机交互式跑**（memory `uecm_node_cmd_hangs_over_ssh`：从 mac `ssh lanpc '<cli> ...'` 跑够节点命令会永久卡死 / 留孤儿进程占 DB）。CLI 在 `C:\Tools\UECM\uecm-cli.exe`；machine 13 = lanPC = 192.168.10.20。
>
> 这批改动（F1–F6）在 mac 上已全部 `cargo test --lib` 通过（1027 pass）+ 三段式 review 通过；PS sidecar 行为（F2/F4/F5）和端到端只能在这里验。逐条勾，记「期望 vs 实际」。

## 0. 构建 + 部署（在 mac 或 lanPC 的源码仓）

- [ ] mac 全量回归：`cd src-tauri && cargo test --lib` → 全绿（基线 ~1027 pass）。
- [ ] release build（**别直接 `cargo build --release`**，memory `deploy_tauri_build`）：`pnpm tauri build --no-bundle`。
- [ ] 把新 `uecm-cli.exe` 复制到 lanPC `C:\Tools\UECM\`；把 `ps-scripts/*.ps1`（含新 `zen-sponsor-down.ps1` + 改过的 `zen-service-install.ps1`、`zen-detect-binary.ps1`）同步到 lanPC `C:\ProgramData\UECM\ps-scripts\`（需管理员，`Start-Process -Verb RunAs`）。
- [ ] 在 lanPC 验证三方 SHA256 对齐（memory `lanpc_deploy_layout`）。
- [ ] 先验 PS 能解析：`pwsh -NoProfile -Command "$null=[System.Management.Automation.Language.Parser]::ParseFile('C:\ProgramData\UECM\ps-scripts\zen-sponsor-down.ps1',[ref]$null,[ref]$null);'ok'"`（对 3 个改过的脚本各跑一次）→ `ok`。

## 1. 前置 — refresh 填表（F1 fallback / F3 依赖）

- [ ] `uecm-cli machine refresh 13` → `machine_ue_installs` 写入 5.2~5.8 行。
- [ ] 记录：UE 版本数 = ___，GPU 数 = ___。

## 2. F3 反例 — 跳过 refresh 时 fail-fast

> 在一台 **没跑过 refresh**（`machine_ue_installs` 为空）但能探到 intree zen 的机器上验；若没有备用机器，可临时清该机的 ue_installs 行后重试，或用一台新纳管节点。

- [ ] `uecm-cli zen detect-binary --machine <空表机器>` → **期望非 0 退出**，该机标记 failed，消息含 `run ... machine refresh ... first`（**不是**静默 ok）。
- [ ] 反向确认：machine 13（已 refresh）`detect-binary` 不触发该错误。

## 3. F5 — detect-binary 跨用户发现 install-dir binary

- [ ] `uecm-cli zen detect-binary --machine 13` → **`install_record_written: true`**（F5 生效，之前恒为 false）。
- [ ] 同时 `intree_records_written > 0`（5.2~5.8 已 refresh）。
- [ ] 记录：install_record_written = ___，intree_records_written = ___。

## 4. 注册 endpoint

- [ ] `uecm-cli zen register --machine 13 --role shared_upstream --declared-port 8558 --scheme http --data-dir F:\Epic\DDC\Zen --httpserverclass asio`
- [ ] 记录 endpoint_id = ___（下文记作 `<EP>`）。role=shared_upstream → lifecycle 自动 installed_service。
- [ ] `uecm-cli zen lua-preview --endpoint-id <EP>` → 配置正确。

## 5. URL ACL

- [ ] `uecm-cli zen urlacl add --endpoint-id <EP> --principal "NT SERVICE\ZenServer" --yes` → `http://+:8558/` 已授权。

## 6. F2 — service install + ImagePath 含 port（全新安装）

- [ ] `uecm-cli zen service install --endpoint-id <EP> --yes` → ok。
- [ ] `sc qc ZenServer` → **BINARY_PATH_NAME 含 `--port 8558 --http asio`**（不只是 `--data-dir`）。响应里 `image_path_port_pinned: true`。

## 6b. F2 真实升级反例 — 已存在「有 data-dir 没 port」的坏服务就地修复（Codex #1）

> 这是 F2 最关键的回归点。

- [ ] 造坏状态：把 ZenServer 服务的 ImagePath 改成只有 `--data-dir`（去掉 `--port`/`--http`），或直接拿一个旧 CLI 装出来的服务。确认 `sc qc ZenServer` 的 ImagePath **不含** `--port`。
- [ ] 再跑 `uecm-cli zen service install --endpoint-id <EP> --yes` → **期望响应 `repaired: true`**（不是 `already_installed` 静默 no-op）。
- [ ] `sc qc ZenServer` → ImagePath **已补** `--port 8558 --http asio`。
- [ ] 提示信息含「需 `zen service stop` + `start` 重启生效」。

## 7. F4 守卫反例 — 服务在跑时 sponsor-down 拒绝（Codex #2）

- [ ] 确保 ZenServer 服务 **正在运行** 且占用 8558。
- [ ] `uecm-cli zen sponsor-down --endpoint-id <EP> --dry-run` → **期望 refuse**：`refused: true, is_installed_service: true`，消息提示「该用 `zen service stop`」。输出含 `listener_pid`。
- [ ] （可选 fail-closed 验证）若有办法让监听进程 path 读不出（受保护进程 / 非 admin 上下文），验 `path_unresolved: true` 的 refuse 分支。

## 7b. F4 正例 — sponsor 占端口时优雅关停

- [ ] 制造 sponsor 占用：停掉 ZenServer 服务后，用 UE Editor（或手动）拉起一个 editor-owned zenserver 占 8558。`uecm-cli zen service start --endpoint-id <EP>` 应因端口被占失败。
- [ ] `uecm-cli zen sponsor-down --endpoint-id <EP> --dry-run` → `would_stop: true, is_installed_service: false`，报出 `listener_pid` / `listener_path`（某 UE install 下的 zenserver.exe）。
- [ ] `uecm-cli zen sponsor-down --endpoint-id <EP> --yes` → ok，sponsor 关停。

## 8. F2 — service start 起在正确端口（不 relocate）

- [ ] `uecm-cli zen service start --endpoint-id <EP>` → ok。
- [ ] `uecm-cli zen probe --machine 13` → **`effective_port: 8558`（不是 8658）**，`reachable: true`。
- [ ] 停服再起一次（验 F2 ImagePath 持久、端口稳定）：`zen service stop --endpoint-id <EP> --yes` → `zen service start --endpoint-id <EP>` → 再 probe → 仍 8558。

## 9. F6 + 收尾

- [ ] F6：`uecm-cli zen apply-config --endpoint-id <EP> --yes`（**不带 `--dest-path`**）→ 应自动推导 `…\Zen\Install\zen.lua` 并写成功（之前 `--dest-path` 必填）。intree-only 机器则应报「pass --dest-path explicitly」。
- [ ] `uecm-cli zen cache-stats --endpoint-id <EP>` → providers 含 `z$`（DDC namespace 在线）。
- [ ] `uecm-cli zen status --machine 13` → `lifecycle_mode: installed_service, ok: true`。
- [ ] `uecm-cli health run --machine-ids 13` → 多数 healthy。

## 完成判据

- 全部勾过；关键三项：**6b** `repaired:true` + ImagePath 含 port、**7** 服务在跑时 sponsor-down `refused`、**8** 端口稳定 8558。
- 任一失败：记录实际输出，回 worktree 修对应 PS / Rust，重 build+deploy 再验。
- 全过后 A 组（F1–F4）+ F5/F6 即可合 main。
