//! Tauri commands for network scan + per-machine refresh.

use crate::core::{discovery, network, winrm};
use crate::data::{
    machine_gpus, machine_ue_installs, machines as data_machines, Db, GpuInfo, Machine, UeInstall,
};
use crate::error::{UecmError, UecmResult};
use serde::Serialize;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct ScanResult {
    pub probed: Vec<network::ProbedHost>,
}

#[tauri::command]
pub async fn scan_network(cidr: String) -> UecmResult<ScanResult> {
    let probed = network::scan_cidr(&cidr, network::DEFAULT_TIMEOUT_MS).await?;
    Ok(ScanResult { probed })
}

/// Adds a discovered IP as a Machine row (or no-op if already present).
/// hostname defaults to the IP — caller can rename later.
#[tauri::command]
pub fn add_discovered_machine(
    db: State<'_, Db>,
    ip: String,
    hostname: Option<String>,
) -> UecmResult<i64> {
    let display_name = hostname.unwrap_or_else(|| ip.clone());
    // If a machine with this IP already exists, return its id; else insert.
    let existing = data_machines::list_all(&db)?
        .into_iter()
        .find(|m| m.ip == ip);
    if let Some(m) = existing {
        return m
            .id
            .ok_or_else(|| UecmError::OperationFailed("machine missing id".to_string()));
    }
    let machine = Machine::new(&display_name, &ip);
    data_machines::insert(&db, &machine)
}

#[derive(Debug, Serialize)]
pub struct RefreshResult {
    pub machine_id: i64,
    pub winrm_ok: bool,
    pub ue_installs: Vec<UeInstall>,
    pub gpus: Vec<GpuInfo>,
    pub error: Option<String>,
}

/// Probes WinRM connectivity to a known machine, then re-queries UE + GPU
/// info if reachable, persisting results into the data layer.
#[tauri::command]
pub fn refresh_machine(db: State<'_, Db>, machine_id: i64) -> UecmResult<RefreshResult> {
    let machine = data_machines::list_all(&db)?
        .into_iter()
        .find(|m| m.id == Some(machine_id))
        .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", machine_id)))?;

    let probe = match winrm::probe(&machine.ip) {
        Ok(p) if p.ok => Some(p),
        Ok(_) => None,
        Err(e) => {
            return Ok(RefreshResult {
                machine_id,
                winrm_ok: false,
                ue_installs: vec![],
                gpus: vec![],
                error: Some(format!("probe failed: {}", e)),
            });
        }
    };

    if probe.is_none() {
        return Ok(RefreshResult {
            machine_id,
            winrm_ok: false,
            ue_installs: vec![],
            gpus: vec![],
            error: Some("WinRM unreachable".to_string()),
        });
    }

    let detected_ue = match discovery::detect_ue_versions(&machine.ip) {
        Ok(v) => v,
        Err(e) => {
            return Ok(RefreshResult {
                machine_id,
                winrm_ok: true,
                ue_installs: vec![],
                gpus: vec![],
                error: Some(format!("UE detection failed: {}", e)),
            });
        }
    };

    let detected_gpus = match discovery::detect_gpus(&machine.ip) {
        Ok(v) => v,
        Err(e) => {
            return Ok(RefreshResult {
                machine_id,
                winrm_ok: true,
                ue_installs: vec![],
                gpus: vec![],
                error: Some(format!("GPU detection failed: {}", e)),
            });
        }
    };

    // Persist UE installs (upsert per version)
    for d in &detected_ue {
        machine_ue_installs::upsert(
            &db,
            &UeInstall {
                id: None,
                machine_id,
                version: d.version.clone(),
                install_path: d.install_path.clone(),
                is_primary: false,
            },
        )?;
    }

    // Replace GPU set wholesale (GPUs change as a unit on hardware swap)
    let gpu_records: Vec<GpuInfo> = detected_gpus
        .iter()
        .map(|g| GpuInfo {
            id: None,
            machine_id,
            gpu_model: g.gpu_model.clone(),
            driver_version: g.driver_version.clone(),
            vendor: g.vendor.clone(),
            vram_mb: g.vram_mb,
        })
        .collect();
    machine_gpus::replace_for_machine(&db, machine_id, &gpu_records)?;

    Ok(RefreshResult {
        machine_id,
        winrm_ok: true,
        ue_installs: machine_ue_installs::list_for_machine(&db, machine_id)?,
        gpus: machine_gpus::list_for_machine(&db, machine_id)?,
        error: None,
    })
}
