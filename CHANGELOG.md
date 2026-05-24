# Changelog

本项目变更记录。标注 `contract_version` 变化（见 `docs/schema-versions.md`）。
格式参考 Keep a Changelog；版本对齐 `src-tauri/Cargo.toml` 的 `version`。

## [Unreleased]

### Added
- CLI 全局 flag：`--output/-o`（text/json/ndjson）、`--no-color`、`--no-input`、`--quiet/-q`、`--verbose/-v`、`--config`、`--input-format`（spec §3.2）。
- `AI_AGENT=1` env 信号默认输出 json（spec §3.4）。
- Contract Manifest：`uecm-cli manifest` 命令 + canonical `operation_id` 体系（spec §2），每个 operation 带 `input_schema` / `output_schema` / `error_schema`（schemars 生成）。快照 `docs/contract-manifest.json`（89 ops）。**contract_version: 1.0**。
- 统一输出 envelope：所有 `--output json` 输出套 `schema_version`/`status`/`operation_id`/`data`/`meta`，`--output ndjson` 流式事件带 `type`/`sequence`/`timestamp`/`request_id`/`final`（spec §4）。
- `uecm-cli system completion <shell>`（spec §10.1）。
- 文档：`docs/exit-codes.md`、`docs/schema-versions.md`。

### Changed
- `--json` 降级为 `--output json` 的别名（向后兼容保留）。
- `--output json` 与 `--output ndjson` 行为分离：json = 单一完整对象（流式命令的事件折叠进 `data.events`），ndjson = 逐行事件流。
- ndjson 事件字段 `kind` 改名为 `type`，并新增 `sequence`/`timestamp`/`request_id`/`final`（spec §4.3）。**breaking**：消费 `kind` 字段的旧脚本需改读 `type`。
- usage parse 错误在 `--output json`/`AI_AGENT=1` 下输出统一 ErrorEnvelope 形状（exit 仍 64）。
- Core `ini_scanner` 的 `eprintln!` 改为 `tracing::warn!`（spec §1.3）。

### Removed
- `--pass` argv flag：密码不再经命令行传递，仅 `--pass-stdin` / `--cred-alias`（spec §9）。

### Notes
- usage error 退出码保留 `64`（sysexits EX_USAGE），相对 spec §5（=2）为已接受偏离，理由见 `docs/exit-codes.md`。
- 已知遗留：`cred save` / `machine bootstrap` / `machine preflight` / `zen install --service-pass` 等域专属命令仍可经 argv 传密码，超出本轮三份计划范围，待后续 §9 加固（有 onboarding 脚本兼容性影响，需评估）。
