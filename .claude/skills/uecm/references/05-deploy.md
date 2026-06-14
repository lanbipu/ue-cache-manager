# 流程 05 — 一键 deploy ddc

`deploy ddc --plan <PLAN.json>` 按一份 JSON 计划一次跑完整 11 步 DDC 部署，省去逐条手敲。
约定 `BIN=/mnt/c/Tools/UECM/uecm-cli.exe`。

## 11 步工作流（按 plan 的开关裁剪）

固定前 5 步 + 按开关追加：
```
1 ProvisionLocalDir   2 SetLocalEnv   3 CreateSmbShare   4 SetSharedEnv   5 WriteBackendGraph
ddc_pak.enabled       → 6 GenerateDdcPak   7 DistributeDdcPak
pso.enabled           → 8 SetPsoCvars      9 CollectPso      10 DistributePso
verify.run_log_verify → 11 VerifyStartupLogs
```

## plan JSON 结构

模板见 `assets/deploy-plan-template.json`。字段（对应 `core/deploy_workflow.rs::DeployPlan`）：

| 字段 | 类型 | 说明 |
|---|---|---|
| `project_id` | int | 必填 |
| `source_machine_id` | int | 必填，源机器 id |
| `target_machine_ids` | int[] | 必填，分发目标（可空数组 `[]`） |
| `local_cache.path` | string | 必填，本地缓存路径（Windows，JSON 里 `\\`） |
| `local_cache.service_account` | string\|null | 可选 |
| `shared_cache.server_machine_id` | int | 必填 |
| `shared_cache.share_name` | string | 必填 |
| `shared_cache.server_path` | string | 必填，共享在服务器上的本地路径 |
| `shared_cache.mode` | string | 必填，`"a"`(open) / `"b"`(dedicated) |
| `shared_cache.unc_path` | string\|null | 可选，共享的 UNC |
| `ddc_pak.enabled` | bool | 必填 |
| `pso.enabled` | bool | 必填 |
| `pso.resolution` | string | `enabled=true` 时**必填非空**（如 `"1920x1080"`）；禁用时可省/留空 |
| `pso.max_minutes` | int | 同上，禁用时可省 |
| `verify.run_log_verify` | bool | 必填 |
| `verify.editor_exe` | string | `run_log_verify=true` 时**必填非空**；禁用时可省/留空 |
| `verify.timeout_seconds` | int | 同上 |

**字段必填规则**（DESIGN-2）：`pso`/`verify` 的子字段带 `#[serde(default)]`，功能禁用时可省略；
但功能**启用**时缺字段会 `DeployPlan::validate()` 报 `invalid_input`（exit 2），如
`pso.resolution is required when pso.enabled is true`。`local_cache`/`shared_cache`/`ddc_pak`/顶层字段无默认值，**始终必填**。

## 执行

```bash
# 1) 写 plan 到 Windows 可见路径（⚠️ 不能用 WSL /tmp，会"找不到文件"，F-035）
#    可从模板改：cp .claude/skills/uecm/assets/deploy-plan-template.json 后按集群实际改字段
PLAN='C:\Temp\deploy-plan.json'

# 2) 先 dry-run 看完整 steps 列表（不执行），给用户确认
"$BIN" deploy ddc --plan "$PLAN" --cred-alias render-svc --dry-run --output json

# 3) 确认后真正执行；--stop-on-failure 让某步失败即停（否则继续后续步）
"$BIN" deploy ddc --plan "$PLAN" --cred-alias render-svc --stop-on-failure --yes
```

## 注意
- plan 路径与里面所有路径都用 **Windows 格式**；JSON 里反斜杠写 `\\`，UNC 写 `\\\\SERVER\\Share`。
- dry-run 会输出将执行的 step 列表，是给用户预览的最佳点——务必先 dry-run。
- 若集群是 Zen 模式：第 6/7 步（DDC Pak）在 Zen 可达时会 skip（同流程 03 的 backend 路由）；
  共享缓存层用 Zen 时可考虑把 `ddc_pak.enabled` 设 false，避免多余 pak。
- 长任务（CollectPso / VerifyStartupLogs）对重型 VP 项目可能 hang（F-044）——见流程 03/04 的限制说明。
