//! Contract Manifest (spec §2). Canonical operation_id registry; every operation
//! carries input_schema (from clap), output_schema (per-result type), and a shared
//! error_schema. Built at runtime so output_schema can call schema_for!.

use crate::cli::args::Domain;
use schemars::schema_for;

#[derive(Debug, Clone, Copy)]
pub struct SideEffects {
    pub writes: bool,
    pub external_calls: bool,
    pub idempotent: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Operation {
    pub operation_id: &'static str,
    pub summary: &'static str,
    pub cli_command: &'static str,
    pub side_effects: SideEffects,
    pub exit_codes: &'static [i32],
}

/// 静态操作表（不含 schema；schema 在 manifest_json 运行时拼装）。Task 6 补其余域。
pub fn operations() -> &'static [Operation] {
    const OPS: &[Operation] = &[
        Operation { operation_id: "system.version",    summary: "Print binary + library version",         cli_command: "uecm-cli system version",    side_effects: SideEffects{writes:false,external_calls:false,idempotent:true}, exit_codes: &[0] },
        Operation { operation_id: "system.db_path",    summary: "Print resolved SQLite DB path",           cli_command: "uecm-cli system db-path",    side_effects: SideEffects{writes:false,external_calls:false,idempotent:true}, exit_codes: &[0,3] },
        Operation { operation_id: "system.ps_dir",     summary: "Print resolved ps-scripts dir",           cli_command: "uecm-cli system ps-dir",     side_effects: SideEffects{writes:false,external_calls:false,idempotent:true}, exit_codes: &[0] },
        Operation { operation_id: "system.migrate_db", summary: "Force-run schema migrations",             cli_command: "uecm-cli system migrate-db", side_effects: SideEffects{writes:true, external_calls:false,idempotent:true}, exit_codes: &[0,3] },
        Operation { operation_id: "system.echo",       summary: "Round-trip a message via PowerShell",     cli_command: "uecm-cli system echo",       side_effects: SideEffects{writes:false,external_calls:true, idempotent:true}, exit_codes: &[0,4] },
        Operation { operation_id: "system.schema",     summary: "Dump clap command tree as JSON",          cli_command: "uecm-cli system schema",     side_effects: SideEffects{writes:false,external_calls:false,idempotent:true}, exit_codes: &[0] },
        Operation { operation_id: "system.exit_codes", summary: "Print documented exit-code table",        cli_command: "uecm-cli system exit-codes", side_effects: SideEffects{writes:false,external_calls:false,idempotent:true}, exit_codes: &[0] },
        Operation { operation_id: "machine.list",      summary: "List all known machines",                 cli_command: "uecm-cli machine list",      side_effects: SideEffects{writes:false,external_calls:false,idempotent:true}, exit_codes: &[0,3] },
        Operation { operation_id: "machine.scan",      summary: "Probe a CIDR for live hosts",             cli_command: "uecm-cli machine scan",      side_effects: SideEffects{writes:false,external_calls:true, idempotent:true}, exit_codes: &[0,2] },
        Operation { operation_id: "machine.add",       summary: "Add a machine to inventory",              cli_command: "uecm-cli machine add",       side_effects: SideEffects{writes:true, external_calls:false,idempotent:false}, exit_codes: &[0,2,3] },
        Operation { operation_id: "machine.refresh",   summary: "Refresh a machine (probe + detect)",      cli_command: "uecm-cli machine refresh",   side_effects: SideEffects{writes:true, external_calls:true, idempotent:true}, exit_codes: &[0,2,3,4] },
        Operation { operation_id: "machine.detail",    summary: "Show machine detail",                     cli_command: "uecm-cli machine detail",    side_effects: SideEffects{writes:false,external_calls:false,idempotent:true}, exit_codes: &[0,2] },
        Operation { operation_id: "machine.delete",    summary: "Delete machine(s)",                       cli_command: "uecm-cli machine delete",    side_effects: SideEffects{writes:true, external_calls:false,idempotent:true}, exit_codes: &[0,2] },
        Operation { operation_id: "machine.rename",    summary: "Rename a machine",                        cli_command: "uecm-cli machine rename",    side_effects: SideEffects{writes:true, external_calls:false,idempotent:true}, exit_codes: &[0,2] },
        Operation { operation_id: "machine.deep_scan", summary: "Refresh + INI scan + health per machine", cli_command: "uecm-cli machine deep-scan", side_effects: SideEffects{writes:true, external_calls:true, idempotent:true}, exit_codes: &[0,2,3,4] },
        Operation { operation_id: "machine.authorize", summary: "Authorize machines for remote mgmt",      cli_command: "uecm-cli machine authorize", side_effects: SideEffects{writes:true, external_calls:true, idempotent:true}, exit_codes: &[0,2,4] },
    ];
    OPS
}

pub fn operation_id_for(cmd: &Domain) -> &'static str {
    use crate::cli::args::{MachineAction, SystemAction};
    match cmd {
        Domain::System { action } => match action {
            SystemAction::Version => "system.version",
            SystemAction::DbPath => "system.db_path",
            SystemAction::PsDir => "system.ps_dir",
            SystemAction::MigrateDb => "system.migrate_db",
            SystemAction::Echo { .. } => "system.echo",
            SystemAction::Schema => "system.schema",
            SystemAction::ExitCodes => "system.exit_codes",
        },
        Domain::Machine { action } => match action {
            MachineAction::List => "machine.list",
            MachineAction::Scan { .. } => "machine.scan",
            MachineAction::Add { .. } => "machine.add",
            MachineAction::Refresh { .. } => "machine.refresh",
            MachineAction::Detail { .. } => "machine.detail",
            MachineAction::Delete { .. } => "machine.delete",
            MachineAction::Rename { .. } => "machine.rename",
            MachineAction::DeepScan { .. } => "machine.deep_scan",
            MachineAction::Authorize { .. } => "machine.authorize",
            // 不加 `_ =>`：保持穷尽，新增变体编译器强制来补。
        },
        // Task 6 在此追加其余 16 域。临时兜底（§2.3 / schema 完整性测试会抓到）：
        Domain::Winrm { .. } => "winrm.unmapped",
        Domain::Ssh { .. } => "ssh.unmapped",
        Domain::Cred { .. } => "cred.unmapped",
        Domain::Secret { .. } => "secret.unmapped",
        Domain::Env { .. } => "env.unmapped",
        Domain::Ini { .. } => "ini.unmapped",
        Domain::Share { .. } => "share.unmapped",
        Domain::Project { .. } => "project.unmapped",
        Domain::Health { .. } => "health.unmapped",
        Domain::Gpu { .. } => "gpu.unmapped",
        Domain::Ddc { .. } => "ddc.unmapped",
        Domain::Pso { .. } => "pso.unmapped",
        Domain::Log { .. } => "log.unmapped",
        Domain::LocalCache { .. } => "localcache.unmapped",
        Domain::Deploy { .. } => "deploy.unmapped",
        Domain::Zen { .. } => "zen.unmapped",
    }
}

/// 共享错误 schema（所有 operation 的 error_schema 都是这个）。
pub fn error_schema() -> serde_json::Value {
    serde_json::to_value(schema_for!(crate::cli::envelope::ErrorBody)).unwrap()
}

/// 流式操作（emit_event 序列）的共享输出 schema。
pub fn event_schema() -> serde_json::Value {
    serde_json::to_value(schema_for!(crate::cli::output::Event)).unwrap()
}

/// ad-hoc `serde_json::Value` 输出（无命名类型）用的宽 object schema。
fn dynamic_object_schema() -> serde_json::Value {
    serde_json::json!({ "type": "object", "additionalProperties": true })
}

/// 每个操作的输出（`data`）schema。typed 结果用 schema_for!；流式用 event_schema；
/// ad-hoc json 用 dynamic_object_schema。Task 6 为其余域补 match 臂。
pub fn output_schema_for(operation_id: &str) -> serde_json::Value {
    match operation_id {
        "system.version" => serde_json::to_value(schema_for!(crate::cli::domain_system::VersionInfo)).unwrap(),
        "system.db_path" | "system.ps_dir" => serde_json::to_value(schema_for!(crate::cli::domain_system::PathInfo)).unwrap(),
        "system.migrate_db" | "system.echo" | "system.schema" | "system.exit_codes" => dynamic_object_schema(),
        // emit_event(...) handlers -> event-shaped output. add/delete/rename emit
        // Event::Completed{..} just like scan/refresh/deep_scan/authorize.
        "machine.scan" | "machine.deep_scan" | "machine.refresh" | "machine.authorize"
        | "machine.add" | "machine.delete" | "machine.rename" => event_schema(),
        // emit_result(&T) handlers (machine.list / machine.detail) return ad-hoc json
        // (Task 6 可换成命名类型）：
        s if s.starts_with("machine.") => dynamic_object_schema(),
        // Task 6 之前其余域走兜底（schema 完整性测试会盯着 unmapped 不放）：
        _ => dynamic_object_schema(),
    }
}

/// 从 clap 命令树为某操作派生 input_schema（参数 -> JSON Schema properties）。
pub fn input_schema_for(cli_command: &str) -> serde_json::Value {
    use clap::CommandFactory;
    let parts: Vec<&str> = cli_command.split_whitespace().skip(1).collect(); // drop "uecm-cli"
    let root = crate::cli::args::Cli::command();
    let mut current: &clap::Command = &root;
    for p in &parts {
        match current.find_subcommand(p) {
            Some(sub) => current = sub,
            None => return dynamic_object_schema(),
        }
    }
    let mut props = serde_json::Map::new();
    let mut required = Vec::new();
    for arg in current.get_arguments() {
        let id = arg.get_id().as_str();
        if id == "help" || id == "version" {
            continue;
        }
        let ty = if arg.get_action().takes_values() { "string" } else { "boolean" };
        props.insert(id.to_string(), serde_json::json!({ "type": ty }));
        if arg.is_required_set() {
            required.push(serde_json::json!(id));
        }
    }
    serde_json::json!({
        "type": "object",
        "properties": props,
        "required": required,
        "additionalProperties": false
    })
}

/// 渲染 spec §2.1 完整 manifest 文档。
pub fn manifest_json() -> serde_json::Value {
    let err = error_schema();
    let ops: Vec<serde_json::Value> = operations()
        .iter()
        .map(|op| {
            serde_json::json!({
                "operation_id": op.operation_id,
                "summary": op.summary,
                "input_schema": input_schema_for(op.cli_command),
                "output_schema": output_schema_for(op.operation_id),
                "error_schema": err,
                "side_effects": {
                    "writes": op.side_effects.writes,
                    "external_calls": op.side_effects.external_calls,
                    "idempotent": op.side_effects.idempotent,
                },
                "exit_codes": op.exit_codes,
                "cli": { "command": op.cli_command }
            })
        })
        .collect();
    serde_json::json!({ "contract_version": "1.0", "operations": ops })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_ids_are_unique() {
        let mut ids: Vec<&str> = operations().iter().map(|o| o.operation_id).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, ids.len(), "duplicate operation_id");
    }

    #[test]
    fn manifest_has_three_schemas_per_op() {
        let m = manifest_json();
        for op in m["operations"].as_array().unwrap() {
            let id = op["operation_id"].as_str().unwrap();
            assert!(op["input_schema"].is_object(), "{id} missing input_schema");
            assert!(op["output_schema"].is_object(), "{id} missing output_schema");
            assert!(op["error_schema"].is_object(), "{id} missing error_schema");
        }
    }

    #[test]
    fn output_schema_modes_match_emission() {
        // emit_event(...) handlers carry event-shaped output.
        assert_eq!(output_schema_for("machine.scan"), event_schema());
        // machine.add was reclassified from dynamic to event (it emits
        // Event::Completed{..} from its handler).
        assert_eq!(output_schema_for("machine.add"), event_schema());
        // emit_result(&T) handlers return dynamic object schemas.
        assert_eq!(output_schema_for("machine.list"), super::dynamic_object_schema());
    }
}
