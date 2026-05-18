//! Orchestrate per-machine probes + cluster-level aggregators (GPU, INI consistency).

use crate::data::machine_gpus::GpuInfo;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckOutcome {
    pub status: String,
    pub message: String,
    pub sample: String,
    #[serde(default)]
    pub remediation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GpuConsistencyReport {
    pub outcomes: HashMap<i64, CheckOutcome>,
}

pub fn aggregate_gpu_consistency(gpus: &[GpuInfo]) -> GpuConsistencyReport {
    let mut by_machine: HashMap<i64, &GpuInfo> = HashMap::new();
    for g in gpus { by_machine.insert(g.machine_id, g); }

    let mut combo_counts: HashMap<(String, String), i64> = HashMap::new();
    for g in by_machine.values() {
        *combo_counts
            .entry((g.gpu_model.clone(), g.driver_version.clone()))
            .or_insert(0) += 1;
    }
    let model_counts: HashMap<String, i64> = {
        let mut m = HashMap::new();
        for g in by_machine.values() { *m.entry(g.gpu_model.clone()).or_insert(0) += 1; }
        m
    };

    let mut outcomes = HashMap::new();
    for (mid, g) in &by_machine {
        let same_combo = combo_counts.get(&(g.gpu_model.clone(), g.driver_version.clone())).copied().unwrap_or(0);
        let same_model = model_counts.get(&g.gpu_model).copied().unwrap_or(0);
        let total = by_machine.len() as i64;
        let status = if total == 1 || same_combo == total {
            "healthy"
        } else if same_model == total {
            "warning"
        } else {
            "critical"
        };
        outcomes.insert(*mid, CheckOutcome {
            status: status.into(),
            message: format!(
                "{} {} ({} of {} machines have same combo)",
                g.gpu_model, g.driver_version, same_combo, total
            ),
            sample: format!("{} / {}", g.gpu_model, g.driver_version),
            remediation: if status == "critical" {
                "Standardize GPU + driver across cluster, or split into compatible subgroups before PSO distribute"
                    .into()
            } else if status == "warning" {
                "Make every node run the same NVIDIA driver: audit with `nvidia-smi --query`, then push a matched installer cluster-wide"
                    .into()
            } else {
                String::new()
            },
        });
    }
    GpuConsistencyReport { outcomes }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::machine_gpus::{GpuInfo, GpuVendor};

    fn gpu(mid: i64, model: &str, drv: &str) -> GpuInfo {
        GpuInfo {
            id: None,
            machine_id: mid,
            gpu_model: model.to_string(),
            driver_version: drv.to_string(),
            vendor: GpuVendor::Nvidia,
            vram_mb: Some(10240),
        }
    }

    #[test]
    fn all_machines_with_same_gpu_are_healthy() {
        let gpus = vec![gpu(1, "RTX 3080", "545.92"), gpu(2, "RTX 3080", "545.92")];
        let report = aggregate_gpu_consistency(&gpus);
        assert_eq!(report.outcomes.get(&1).unwrap().status, "healthy");
        assert_eq!(report.outcomes.get(&2).unwrap().status, "healthy");
    }

    #[test]
    fn one_machine_with_different_driver_is_warning() {
        let gpus = vec![gpu(1, "RTX 3080", "545.92"), gpu(2, "RTX 3080", "537.00")];
        let report = aggregate_gpu_consistency(&gpus);
        assert_eq!(report.outcomes.get(&2).unwrap().status, "warning");
    }

    #[test]
    fn one_machine_with_different_model_is_critical() {
        let gpus = vec![gpu(1, "RTX 3080", "545.92"), gpu(2, "RTX 3080", "545.92"), gpu(3, "RTX 4090", "545.92")];
        let report = aggregate_gpu_consistency(&gpus);
        assert_eq!(report.outcomes.get(&3).unwrap().status, "critical");
    }

    #[test]
    fn machine_with_no_gpu_data_is_unknown() {
        let report = aggregate_gpu_consistency(&[]);
        assert!(report.outcomes.is_empty());
    }

    #[test]
    fn check_outcome_serializes_remediation_field() {
        let outcome = CheckOutcome {
            status: "critical".into(),
            message: "LanmanServer stopped".into(),
            sample: "Stopped".into(),
            remediation: "Start the service: Start-Service LanmanServer".into(),
        };
        let json = serde_json::to_string(&outcome).unwrap();
        assert!(json.contains("\"remediation\":\"Start the service: Start-Service LanmanServer\""));
    }

    #[test]
    fn check_outcome_deserializes_missing_remediation_as_empty() {
        let json = r#"{"status":"healthy","message":"","sample":""}"#;
        let outcome: CheckOutcome = serde_json::from_str(json).unwrap();
        assert_eq!(outcome.remediation, "");
    }
}
