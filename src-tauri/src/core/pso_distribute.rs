//! Robocopy fan-out for collected PSO cache files.

use crate::core::powershell;
use crate::data::{machines, project_locations, Db, PsoCacheFile};
use crate::error::{UecmError, UecmResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct DistributeRaw {
    ok: bool,
    #[serde(default)]
    exit_code: String,
    #[serde(default)]
    bytes_copied: String,
    #[serde(default)]
    stdout_tail: String,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PsoDistributeOutcome {
    pub target_machine_id: i64,
    pub ok: bool,
    pub exit_code: i32,
    pub bytes_copied: i64,
    pub stdout_tail: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PsoDistributePlanItem {
    pub target_machine_id: i64,
    pub target_host: String,
    pub source_unc: String,
    pub target_local: String,
    pub file_name: String,
    pub credential_user: Option<String>,
    #[serde(skip_serializing)]
    pub credential_pass: Option<String>,
    pub source_smb_user: Option<String>,
    #[serde(skip_serializing)]
    pub source_smb_pass: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub fn plan(
    db: &Db,
    source_host: &str,
    file: &PsoCacheFile,
    target_machine_ids: &[i64],
    named_share_unc: Option<&str>,
    credential_user: Option<String>,
    credential_pass: Option<String>,
    source_smb_user: Option<String>,
    source_smb_pass: Option<String>,
) -> UecmResult<Vec<PsoDistributePlanItem>> {
    if target_machine_ids.is_empty() {
        return Err(UecmError::InvalidInput("no target machines".into()));
    }

    let source_location =
        project_locations::get_for_project_machine(db, file.project_id, file.source_machine_id)?
            .ok_or_else(|| UecmError::InvalidInput("source project location missing".into()))?;
    let source_unc = if let Some(unc) = named_share_unc {
        format!("{}\\Saved\\CollectedPSOs", unc.trim_end_matches('\\'))
    } else {
        admin_share_pso_unc(source_host, &source_location.abs_path)?
    };

    let mut items = Vec::new();
    for target_id in target_machine_ids {
        if *target_id == file.source_machine_id {
            continue;
        }
        let target_location = project_locations::get_for_project_machine(db, file.project_id, *target_id)?
            .ok_or_else(|| {
                UecmError::InvalidInput(format!(
                    "project {} has no location on target {}",
                    file.project_id, target_id
                ))
            })?;
        let target = machines::find_by_id(db, *target_id)?
            .ok_or_else(|| UecmError::InvalidInput(format!("target machine {} not found", target_id)))?;
        items.push(PsoDistributePlanItem {
            target_machine_id: *target_id,
            target_host: target.ip,
            source_unc: source_unc.clone(),
            target_local: format!(
                "{}\\Saved\\CollectedPSOs",
                target_location.abs_path.trim_end_matches('\\')
            ),
            file_name: file.file_name.clone(),
            credential_user: credential_user.clone(),
            credential_pass: credential_pass.clone(),
            source_smb_user: source_smb_user.clone(),
            source_smb_pass: source_smb_pass.clone(),
        });
    }
    Ok(items)
}

fn admin_share_pso_unc(source_host: &str, abs_path: &str) -> UecmResult<String> {
    let normalized = abs_path.replace('/', "\\");
    let mut chars = normalized.chars();
    let drive = chars.next().ok_or_else(|| {
        UecmError::InvalidInput(format!("abs_path not drive-rooted: {}", abs_path))
    })?;
    if chars.next() != Some(':') {
        return Err(UecmError::InvalidInput(format!(
            "abs_path not a drive-rooted Windows path: {}",
            abs_path
        )));
    }
    Ok(format!(
        "\\\\{}\\{}$\\{}\\Saved\\CollectedPSOs",
        source_host,
        drive,
        normalized[2..].trim_start_matches('\\')
    ))
}

fn build_distribute_args(item: &PsoDistributePlanItem, preflight: bool) -> Vec<String> {
    let mut args = vec![
        "-HostName".to_string(),
        item.target_host.clone(),
        "-SourceUnc".into(),
        item.source_unc.clone(),
        "-TargetLocal".into(),
        item.target_local.clone(),
        "-FileName".into(),
        item.file_name.clone(),
    ];
    if let (Some(user), Some(pass)) = (item.credential_user.as_deref(), item.credential_pass.as_deref()) {
        args.push("-Username".into());
        args.push(user.into());
        args.push("-Password".into());
        args.push(pass.into());
    }
    if let (Some(user), Some(pass)) = (item.source_smb_user.as_deref(), item.source_smb_pass.as_deref()) {
        args.push("-SourceSmbUser".into());
        args.push(user.into());
        args.push("-SourceSmbPass".into());
        args.push(pass.into());
    }
    if preflight {
        args.push("-PreflightOnly".into());
    }
    args
}

pub async fn preflight_one(item: &PsoDistributePlanItem) -> UecmResult<()> {
    let args = build_distribute_args(item, true);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let result: DistributeRaw =
        powershell::run_json(&powershell::script_path("distribute-pso-cache.ps1"), &refs)?;
    if !result.ok {
        return Err(UecmError::OperationFailed(
            result.message.unwrap_or_else(|| result.stdout_tail.clone()),
        ));
    }
    Ok(())
}

pub async fn run_one(item: PsoDistributePlanItem) -> UecmResult<PsoDistributeOutcome> {
    let args = build_distribute_args(&item, false);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let result: DistributeRaw =
        powershell::run_json(&powershell::script_path("distribute-pso-cache.ps1"), &refs)?;
    Ok(PsoDistributeOutcome {
        target_machine_id: item.target_machine_id,
        ok: result.ok,
        exit_code: result.exit_code.parse().unwrap_or(-1),
        bytes_copied: result.bytes_copied.parse().unwrap_or_default(),
        stdout_tail: result.stdout_tail,
        message: result.message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{
        machines, open_in_memory, project_locations, projects, pso_cache_files, schema,
        DiscoveryStatus, Machine, Project, ProjectLocation, PsoCacheFile,
    };

    fn setup() -> (Db, PsoCacheFile, i64, i64) {
        let db = open_in_memory().unwrap();
        {
            let mut conn = db.lock().unwrap();
            schema::migrate(&mut conn).unwrap();
        }
        let source_id = machines::insert(&db, &Machine::new("SOURCE", "1.1.1.1")).unwrap();
        let target_id = machines::insert(&db, &Machine::new("TARGET", "2.2.2.2")).unwrap();
        let project_id = projects::upsert(
            &db,
            &Project {
                id: None,
                uproject_name: "X.uproject".into(),
                uproject_stem_lower: "x".into(),
                uproject_guid: None,
                display_name: None,
                first_seen_at: None,
                last_seen_at: None,
            },
        )
        .unwrap();
        for (machine_id, abs_path, uproject_path) in [
            (source_id, "D:\\Src\\X", "D:\\Src\\X\\X.uproject"),
            (target_id, "E:\\Tgt\\X", "E:\\Tgt\\X\\X.uproject"),
        ] {
            project_locations::upsert(
                &db,
                &ProjectLocation {
                    id: None,
                    project_id,
                    machine_id,
                    abs_path: abs_path.into(),
                    uproject_path: uproject_path.into(),
                    discovery_status: DiscoveryStatus::Auto,
                    discovered_at: None,
                },
            )
            .unwrap();
        }
        let file_id = pso_cache_files::upsert(
            &db,
            &PsoCacheFile {
                id: None,
                project_id,
                source_machine_id: source_id,
                file_path: "D:\\Src\\X\\Saved\\CollectedPSOs\\X.upipelinecache".into(),
                file_name: "X.upipelinecache".into(),
                size_bytes: 123,
                gpu_signature: "nvidia:RTX 4090:551.86".into(),
                ue_version: None,
                collected_at: None,
            },
        )
        .unwrap();
        let file = pso_cache_files::get(&db, file_id).unwrap().unwrap();
        (db, file, source_id, target_id)
    }

    #[test]
    fn plan_skips_source_and_builds_admin_unc() {
        let (db, file, source_id, target_id) = setup();
        let items = plan(
            &db,
            "1.1.1.1",
            &file,
            &[source_id, target_id],
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].target_machine_id, target_id);
        assert_eq!(items[0].source_unc, "\\\\1.1.1.1\\D$\\Src\\X\\Saved\\CollectedPSOs");
        assert_eq!(items[0].target_local, "E:\\Tgt\\X\\Saved\\CollectedPSOs");
        assert_eq!(items[0].file_name, "X.upipelinecache");
    }

    #[test]
    fn plan_uses_named_share_when_provided() {
        let (db, file, _, target_id) = setup();
        let items = plan(
            &db,
            "1.1.1.1",
            &file,
            &[target_id],
            Some("\\\\HOST\\PSO"),
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(items[0].source_unc, "\\\\HOST\\PSO\\Saved\\CollectedPSOs");
    }

    #[test]
    fn plan_rejects_empty_targets() {
        let (db, file, _, _) = setup();
        assert!(matches!(
            plan(&db, "1.1.1.1", &file, &[], None, None, None, None, None),
            Err(UecmError::InvalidInput(_))
        ));
    }
}
