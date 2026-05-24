//! `uecm-cli deploy <action>` handlers.
use crate::cli::args::DeployAction;
use crate::cli::destructive::{self, Outcome};
use crate::cli::EmitSerialize;
use crate::cli::run::Ctx;
use crate::core::deploy_workflow::{self, DeployEvent, DeployPlan, RunOptions};
use crate::error::{UecmError, UecmResult};

pub fn handle(ctx: &mut Ctx<'_>, action: DeployAction) -> UecmResult<()> {
    match action {
        DeployAction::Ddc { plan, stop_on_failure, yes, dry_run, cred } => {
            let body = std::fs::read_to_string(&plan).map_err(|e| {
                UecmError::OperationFailed(format!("read plan {}: {}", plan.display(), e))
            })?;
            let mut p: DeployPlan = serde_json::from_str(&body)
                .map_err(|e| UecmError::InvalidInput(format!("bad plan: {}", e)))?;

            let outcome = destructive::check(yes, dry_run, "deploy.ddc")?;
            let db = ctx.require_db()?.clone();
            cred.preflight(&db)?;
            if outcome == Outcome::DryRun {
                destructive::emit_plan(
                    ctx.emitter.as_mut(),
                    "deploy.ddc",
                    serde_json::json!({
                        "steps": deploy_workflow::plan_steps(&p),
                        "source_machine_id": p.source_machine_id,
                        "target_machine_ids": p.target_machine_ids
                    }),
                );
                return Ok(());
            }
            // SSH key auth: deploy steps take no operator credential. preflight
            // validates flags without reading DPAPI/stdin for a discarded cred.
            cred.preflight(&db)?;
            deploy_workflow::run_plan(
                &db,
                &mut p,
                None,
                RunOptions { stop_on_step_failure: stop_on_failure },
                &mut |e: DeployEvent| {
                    ctx.emitter.emit_result(&e).ok();
                },
            );
            Ok(())
        }
    }
}
