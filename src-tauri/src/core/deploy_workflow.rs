//! Orchestrates the 11-step DDC deployment workflow. Takes &Db from caller;
//! never opens DB itself.

use crate::data::Db;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployPlan {
    pub project_id: i64,
    pub source_machine_id: i64,
    pub target_machine_ids: Vec<i64>,
    pub local_cache: LocalCacheSpec,
    pub shared_cache: SharedCacheSpec,
    pub ddc_pak: PakSpec,
    pub pso: PsoSpec,
    pub verify: VerifySpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalCacheSpec {
    pub path: String,
    pub service_account: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedCacheSpec {
    pub server_machine_id: i64,
    pub share_name: String,
    pub server_path: String,
    pub mode: String,
    pub unc_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PakSpec { pub enabled: bool }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsoSpec {
    pub enabled: bool,
    pub resolution: String,
    pub max_minutes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifySpec {
    pub run_log_verify: bool,
    pub editor_exe: String,
    pub timeout_seconds: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DeployStep {
    ProvisionLocalDir,
    SetLocalEnv,
    CreateSmbShare,
    SetSharedEnv,
    WriteBackendGraph,
    GenerateDdcPak,
    DistributeDdcPak,
    SetPsoCvars,
    CollectPso,
    DistributePso,
    VerifyStartupLogs,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeployEvent {
    StepStarted { step: DeployStep, hosts: Vec<String> },
    StepHostOk { step: DeployStep, host: String, message: Option<String> },
    StepHostError { step: DeployStep, host: String, error: String },
    StepCompleted { step: DeployStep, ok_count: u32, fail_count: u32 },
    PlanCompleted { ok: bool, summary: String },
}

pub fn plan_steps(plan: &DeployPlan) -> Vec<DeployStep> {
    use DeployStep::*;
    let mut s = vec![
        ProvisionLocalDir,
        SetLocalEnv,
        CreateSmbShare,
        SetSharedEnv,
        WriteBackendGraph,
    ];
    if plan.ddc_pak.enabled {
        s.push(GenerateDdcPak);
        s.push(DistributeDdcPak);
    }
    if plan.pso.enabled {
        s.push(SetPsoCvars);
        s.push(CollectPso);
        s.push(DistributePso);
    }
    if plan.verify.run_log_verify {
        s.push(VerifyStartupLogs);
    }
    s
}

#[derive(Debug, Clone)]
pub struct RunOptions {
    pub stop_on_step_failure: bool,
}

// Re-export for downstream tasks (M5.2 fills these in). Keeping the
// module declaration here so M5.3+ can `use crate::core::deploy_workflow::run_step`.
// The Db import is currently unused but reserved for run_step's signature.
#[allow(dead_code)]
fn _ensure_db_import_used(_db: &Db) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn baseline_plan() -> DeployPlan {
        DeployPlan {
            project_id: 1,
            source_machine_id: 100,
            target_machine_ids: vec![200, 201],
            local_cache: LocalCacheSpec { path: "D:\\UE-DDC-Local".into(), service_account: None },
            shared_cache: SharedCacheSpec {
                server_machine_id: 300,
                share_name: "DDC".into(),
                server_path: "D:\\DDC".into(),
                mode: "b".into(),
                unc_path: None,
            },
            ddc_pak: PakSpec { enabled: true },
            pso: PsoSpec { enabled: true, resolution: "1920x1080".into(), max_minutes: 10 },
            verify: VerifySpec {
                run_log_verify: true,
                editor_exe: "C:\\UE\\UnrealEditor.exe".into(),
                timeout_seconds: 180,
            },
        }
    }

    #[test]
    fn full_plan_has_11_steps() {
        assert_eq!(plan_steps(&baseline_plan()).len(), 11);
    }

    #[test]
    fn minimal_plan_skips_optional_phases() {
        let mut p = baseline_plan();
        p.ddc_pak.enabled = false;
        p.pso.enabled = false;
        p.verify.run_log_verify = false;
        let steps = plan_steps(&p);
        assert_eq!(steps.len(), 5);
        assert!(steps.contains(&DeployStep::WriteBackendGraph));
        assert!(!steps.contains(&DeployStep::GenerateDdcPak));
    }

    #[test]
    fn pak_only_plan_has_7_steps() {
        let mut p = baseline_plan();
        p.pso.enabled = false;
        p.verify.run_log_verify = false;
        assert_eq!(plan_steps(&p).len(), 7);
    }

    #[test]
    fn deploy_step_serializes_snake_case() {
        let s = DeployStep::WriteBackendGraph;
        let j = serde_json::to_string(&s).unwrap();
        assert_eq!(j, r#""write_backend_graph""#);
    }
}
