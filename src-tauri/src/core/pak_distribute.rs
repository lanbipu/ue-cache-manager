//! Robocopy fan-out planning and per-target execution for DDC Pak files.

use crate::core::powershell;
use crate::data::{
    machines as data_machines,
    project_locations::{self, ProjectLocation},
    Db,
};
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
pub struct DistributeOutcome {
    pub target_machine_id: i64,
    pub ok: bool,
    pub exit_code: i32,
    pub bytes_copied: i64,
    pub stdout_tail: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DistributePlanItem {
    pub target_machine_id: i64,
    pub target_host: String,
    pub source_unc: String,
    pub target_local: String,
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
    source_machine_id: i64,
    source_host: &str,
    source_location: &ProjectLocation,
    target_machine_ids: &[i64],
    project_id: i64,
    named_share_unc: Option<&str>,
    credential_user: Option<String>,
    credential_pass: Option<String>,
    source_smb_user: Option<String>,
    source_smb_pass: Option<String>,
) -> UecmResult<Vec<DistributePlanItem>> {
    if target_machine_ids.is_empty() {
        return Err(UecmError::InvalidInput("no target machines".into()));
    }

    let source_unc = if let Some(unc) = named_share_unc {
        format!("{}\\DerivedDataCache", unc.trim_end_matches('\\'))
    } else {
        admin_share_ddc_unc(source_host, &source_location.abs_path)?
    };

    let mut out = Vec::new();
    for target_id in target_machine_ids {
        if *target_id == source_machine_id {
            continue;
        }
        let location = project_locations::get_for_project_machine(db, project_id, *target_id)?
            .ok_or_else(|| {
                UecmError::InvalidInput(format!(
                    "project {} has no location on machine {}",
                    project_id, target_id
                ))
            })?;
        let target = data_machines::find_by_id(db, *target_id)?.ok_or_else(|| {
            UecmError::InvalidInput(format!("target machine {} not found", target_id))
        })?;
        out.push(DistributePlanItem {
            target_machine_id: *target_id,
            target_host: target.ip,
            source_unc: source_unc.clone(),
            target_local: format!("{}\\DerivedDataCache", location.abs_path.trim_end_matches('\\')),
            credential_user: credential_user.clone(),
            credential_pass: credential_pass.clone(),
            source_smb_user: source_smb_user.clone(),
            source_smb_pass: source_smb_pass.clone(),
        });
    }
    Ok(out)
}

fn admin_share_ddc_unc(source_host: &str, abs_path: &str) -> UecmResult<String> {
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
    let rest = &normalized[2..];
    Ok(format!(
        "\\\\{}\\{}$\\{}\\DerivedDataCache",
        source_host,
        drive,
        rest.trim_start_matches('\\')
    ))
}

fn build_distribute_args(item: &DistributePlanItem, preflight: bool) -> Vec<String> {
    let mut args = vec![
        "-HostName".to_string(),
        item.target_host.clone(),
        "-SourceUnc".into(),
        item.source_unc.clone(),
        "-TargetLocal".into(),
        item.target_local.clone(),
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

pub async fn preflight_one(item: &DistributePlanItem) -> UecmResult<()> {
    let args = build_distribute_args(item, true);
    let args_ref: Vec<&str> = args.iter().map(String::as_str).collect();
    let result: DistributeRaw =
        powershell::run_json(&powershell::script_path("distribute-pak-file.ps1"), &args_ref)?;
    if !result.ok {
        return Err(UecmError::OperationFailed(
            result
                .message
                .unwrap_or_else(|| format!("preflight failed: {}", result.stdout_tail)),
        ));
    }
    Ok(())
}

pub async fn run_one(item: DistributePlanItem) -> UecmResult<DistributeOutcome> {
    let args = build_distribute_args(&item, false);
    let args_ref: Vec<&str> = args.iter().map(String::as_str).collect();
    let result: DistributeRaw =
        powershell::run_json(&powershell::script_path("distribute-pak-file.ps1"), &args_ref)?;
    Ok(DistributeOutcome {
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
    use crate::data::{machines, open_in_memory, projects, schema, Machine, Project};

    fn setup() -> (Db, i64, i64, i64) {
        let db = open_in_memory().unwrap();
        {
            let mut conn = db.lock().unwrap();
            schema::migrate(&mut conn).unwrap();
        }
        let source = machines::insert(&db, &Machine::new("SOURCE", "1.1.1.1")).unwrap();
        let target = machines::insert(&db, &Machine::new("TARGET", "2.2.2.2")).unwrap();
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
        (db, source, target, project_id)
    }

    fn source_loc(project_id: i64, source: i64) -> ProjectLocation {
        ProjectLocation {
            id: Some(0),
            project_id,
            machine_id: source,
            abs_path: "D:\\X".into(),
            uproject_path: "D:\\X\\X.uproject".into(),
            discovery_status: crate::data::DiscoveryStatus::Auto,
            discovered_at: None,
        }
    }

    #[test]
    fn plan_rejects_empty_targets() {
        let (db, source, _, project_id) = setup();
        let result = plan(
            &db,
            source,
            "1.1.1.1",
            &source_loc(project_id, source),
            &[],
            project_id,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(matches!(result, Err(UecmError::InvalidInput(_))));
    }

    #[test]
    fn plan_skips_source_in_targets() {
        let (db, source, target, project_id) = setup();
        project_locations::upsert(
            &db,
            &ProjectLocation {
                id: None,
                project_id,
                machine_id: target,
                abs_path: "E:\\Y".into(),
                uproject_path: "E:\\Y\\X.uproject".into(),
                discovery_status: crate::data::DiscoveryStatus::Auto,
                discovered_at: None,
            },
        )
        .unwrap();
        let items = plan(
            &db,
            source,
            "1.1.1.1",
            &source_loc(project_id, source),
            &[source, target],
            project_id,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source_unc, "\\\\1.1.1.1\\D$\\X\\DerivedDataCache");
        assert_eq!(items[0].target_local, "E:\\Y\\DerivedDataCache");
    }

    #[test]
    fn plan_uses_named_share_when_provided() {
        let (db, source, target, project_id) = setup();
        project_locations::upsert(
            &db,
            &ProjectLocation {
                id: None,
                project_id,
                machine_id: target,
                abs_path: "E:\\Y".into(),
                uproject_path: "E:\\Y\\X.uproject".into(),
                discovery_status: crate::data::DiscoveryStatus::Auto,
                discovered_at: None,
            },
        )
        .unwrap();
        let items = plan(
            &db,
            source,
            "1.1.1.1",
            &source_loc(project_id, source),
            &[target],
            project_id,
            Some("\\\\HOST\\DDC"),
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(items[0].source_unc, "\\\\HOST\\DDC\\DerivedDataCache");
    }
}
