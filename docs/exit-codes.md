# uecm-cli Exit Codes

权威来源：`uecm-cli system exit-codes --output json`（运行时生成；本文件是其人类可读快照）。
映射实现见 `src-tauri/src/cli/output.rs::exit_code_for` 与 `src-tauri/src/bin/uecm-cli.rs`。

## 进程退出码

| Code | Name | 含义 |
|---|---|---|
| 0 | ok | 成功 |
| 1 | operation_failed | 运行期业务逻辑失败（未分类） |
| 2 | invalid_input | 用户运行期数据非法（未知 id、坏 CIDR 等） |
| 3 | environment_error | 配置 / 数据库 / IO 问题，需用户修复环境（含 SSH 连接、超时、脚本暂存） |
| 4 | powershell_failed | 远端 PowerShell / 节点脚本调用失败 |
| 64 | usage_error | argv 形态错误：缺必填 flag、未知子命令、互斥冲突（sysexits.h EX_USAGE） |

## 错误 envelope 的 `error.code`（`--output json` 下）

| error.code | exit | 来源 |
|---|---|---|
| invalid_input | 2 | handler 校验 |
| operation_failed | 1 | handler 运行期失败 |
| environment_error | 3 | 配置 / 数据库 / IO |
| powershell_failed | 4 | 远端 PowerShell |
| usage_error | 64 | clap argv 解析 |

## 与 AI-Native Spec v3.0 §5 的已接受偏离

Spec §5 把“CLI usage / argument syntax error”归为 exit **2**。本项目**有意**把 **argv 解析层**的用法错误用 **64**（sysexits.h `EX_USAGE`），而把 **handler 层**的运行期数据非法（如未知 machine id）保留为 **2**（`invalid_input`）。

**理由：** 让自动化能区分“命令行拼写/形态写错”（64，改命令行即可）与“命令行合法但运行期数据无效”（2，需改数据/环境）。两类对调用方的修复动作不同。spec §5 把二者都收敛到 2，本项目认为牺牲了这一可区分性。

**实现位置：** `src-tauri/src/bin/uecm-cli.rs`（clap parse 失败 → exit 64，并在 `--output json` / `AI_AGENT=1` 下输出统一 ErrorEnvelope，`error.code = "usage_error"`、`error.exit_code = 64`）。本偏离不计划改回；如需对齐 spec，把该处改为 exit 2 并完全靠 `error.code` 承载区分。

其余退出码（spec §5 的 5/6/7/8/9）当前未细分使用：超时与 SSH 连接归入 3、外部依赖失败归入 4，未单独分配 5（not found）/6（conflict）/9（partial）。后续如需细化在 `exit_code_for` 扩展。
