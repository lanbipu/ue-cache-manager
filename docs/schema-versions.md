# uecm-cli Schema / Contract Versions

## 输出 envelope schema_version

- 当前：`1.0`（常量 `src-tauri/src/cli/envelope.rs::SCHEMA_VERSION`）。
- 语义化 `MAJOR.MINOR`：breaking 改动（删字段 / 改类型 / 改含义）→ MAJOR+1；新增可选字段 → MINOR+1。
- 出现在每个 `--output json` 成功 / 错误 envelope 顶层，以及每条 ndjson 事件（spec §4.4）。

## Contract Manifest contract_version

- 当前：`1.0`（`src-tauri/src/cli/manifest.rs::manifest_json`）。
- 与 envelope `schema_version` 对齐演进（spec §4.4）。
- 权威快照：`docs/contract-manifest.json`（由 `uecm-cli manifest --output json | jq '.data'` 生成，当前 89 个 operation，每个带 `input_schema` / `output_schema` / `error_schema`）。

## CLI self-description spec_version

- `uecm-cli system schema` 输出的 `spec_version: 1`，标识命令树 dump 格式版本（与上面两者独立，仅描述 schema 自描述结构）。

## 内部 DB schema 版本

- SQLite 迁移版本与上述输出契约**无关**，由 `src-tauri/src/data/schema.rs` 的 migration 序列管理，不对外承诺稳定性。

## 变更流程

改动输出字段时：
1. 评估是否 breaking；按规则 bump `SCHEMA_VERSION` 与 `contract_version`。
2. 重新生成 `docs/contract-manifest.json`（`uecm-cli manifest --output json | jq '.data' > docs/contract-manifest.json`）。
3. 在 `CHANGELOG.md` 记录 `contract_version` 变化。
