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

#[derive(Clone, Debug)]
pub struct DistributeProfile {
    pub source_subdir: String,
    pub file_glob: String,
    pub ps_script: &'static str,
}

impl DistributeProfile {
    pub fn ddc_pak() -> Self {
        Self {
            source_subdir: "DerivedDataCache".into(),
            file_glob: "*.ddp".into(),
            ps_script: "distribute-pak-file.ps1",
        }
    }

    pub fn pso_cache() -> Self {
        Self {
            source_subdir: "Saved\\CollectedPSOs".into(),
            file_glob: "*.upipelinecache".into(),
            ps_script: "distribute-pso-cache.ps1",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DistributePlanItem {
    pub target_machine_id: i64,
    pub target_host: String,
    pub source_unc: String,
    pub target_local: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    pub credential_user: Option<String>,
    #[serde(skip_serializing)]
    pub credential_pass: Option<String>,
    pub source_smb_user: Option<String>,
    #[serde(skip_serializing)]
    pub source_smb_pass: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub fn plan(
    profile: &DistributeProfile,
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
        append_source_subdir_once(unc, &profile.source_subdir)
    } else {
        admin_share_unc(source_host, &source_location.abs_path, &profile.source_subdir)?
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
            target_local: append_source_subdir_once(&location.abs_path, &profile.source_subdir),
            file_name: None,
            credential_user: credential_user.clone(),
            credential_pass: credential_pass.clone(),
            source_smb_user: source_smb_user.clone(),
            source_smb_pass: source_smb_pass.clone(),
        });
    }
    Ok(out)
}

fn admin_share_unc(source_host: &str, abs_path: &str, source_subdir: &str) -> UecmResult<String> {
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
    let base_unc = format!("\\\\{}\\{}$\\{}", source_host, drive, rest.trim_start_matches('\\'));
    Ok(append_source_subdir_once(&base_unc, source_subdir))
}

fn append_source_subdir_once(base_path: &str, source_subdir: &str) -> String {
    let base = base_path.trim_end_matches(['\\', '/']);
    let subdir = source_subdir.trim_matches(['\\', '/']).replace('/', "\\");
    if subdir.is_empty() || path_ends_with_segments(base, &subdir) {
        return base.to_string();
    }
    format!("{}\\{}", base, subdir)
}

fn path_ends_with_segments(path: &str, suffix: &str) -> bool {
    let path_segments: Vec<_> = path
        .split(['\\', '/'])
        .filter(|segment| !segment.is_empty())
        .collect();
    let suffix_segments: Vec<_> = suffix
        .split(['\\', '/'])
        .filter(|segment| !segment.is_empty())
        .collect();
    if suffix_segments.is_empty() || suffix_segments.len() > path_segments.len() {
        return false;
    }
    path_segments[path_segments.len() - suffix_segments.len()..]
        .iter()
        .zip(suffix_segments.iter())
        .all(|(left, right)| left.eq_ignore_ascii_case(right))
}

fn build_distribute_args(
    profile: &DistributeProfile,
    item: &DistributePlanItem,
    preflight: bool,
) -> Vec<String> {
    let mut args = vec![
        "-HostName".to_string(),
        item.target_host.clone(),
        "-SourceUnc".into(),
        item.source_unc.clone(),
        "-TargetLocal".into(),
        item.target_local.clone(),
    ];
    if let Some(file_name) = &item.file_name {
        args.push("-FileName".into());
        args.push(file_name.clone());
    }
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
    let profile = DistributeProfile::ddc_pak();
    preflight_one_with_profile(&profile, item).await
}

pub async fn preflight_one_with_profile(
    profile: &DistributeProfile,
    item: &DistributePlanItem,
) -> UecmResult<()> {
    if crate::core::loopback::is_loopback_target(&item.target_host) {
        let result = run_local_robocopy(profile, item, true)?;
        if !result.ok {
            return Err(UecmError::OperationFailed(
                result
                    .message
                    .unwrap_or_else(|| format!("local preflight failed: {}", result.stdout_tail)),
            ));
        }
        return Ok(());
    }

    let args = build_distribute_args(profile, item, true);
    let args_ref: Vec<&str> = args.iter().map(String::as_str).collect();
    let result: DistributeRaw =
        powershell::run_json(&powershell::script_path(profile.ps_script), &args_ref)?;
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
    let profile = DistributeProfile::ddc_pak();
    run_one_with_profile(&profile, item).await
}

pub async fn run_one_with_profile(
    profile: &DistributeProfile,
    item: DistributePlanItem,
) -> UecmResult<DistributeOutcome> {
    if crate::core::loopback::is_loopback_target(&item.target_host) {
        return run_local_robocopy(profile, &item, false);
    }

    let args = build_distribute_args(profile, &item, false);
    let args_ref: Vec<&str> = args.iter().map(String::as_str).collect();
    let result: DistributeRaw =
        powershell::run_json(&powershell::script_path(profile.ps_script), &args_ref)?;
    Ok(DistributeOutcome {
        target_machine_id: item.target_machine_id,
        ok: result.ok,
        exit_code: result.exit_code.parse().unwrap_or(-1),
        bytes_copied: result.bytes_copied.parse().unwrap_or_default(),
        stdout_tail: result.stdout_tail,
        message: result.message,
    })
}

fn run_local_robocopy(
    profile: &DistributeProfile,
    item: &DistributePlanItem,
    preflight: bool,
) -> UecmResult<DistributeOutcome> {
    #[cfg(windows)]
    {
        let mut args = vec![
            item.source_unc.as_str(),
            item.target_local.as_str(),
            item.file_name.as_deref().unwrap_or(profile.file_glob.as_str()),
            "/E",
            "/R:3",
            "/W:5",
            "/NP",
            "/NDL",
            "/NJH",
            "/NJS",
            "/BYTES",
        ];
        if preflight {
            args.push("/L");
        }
        let output = std::process::Command::new("robocopy.exe")
            .args(args)
            .output()
            .map_err(UecmError::Io)?;
        let code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let stdout_tail = stdout
            .lines()
            .rev()
            .take(30)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        let bytes_copied = stdout
            .lines()
            .find_map(|line| {
                let (_, value) = line.split_once("Bytes")?;
                let digits: String = value.chars().filter(|ch| ch.is_ascii_digit()).collect();
                digits.parse::<i64>().ok()
            })
            .unwrap_or_default();

        Ok(DistributeOutcome {
            target_machine_id: item.target_machine_id,
            ok: code < 8,
            exit_code: code,
            bytes_copied,
            stdout_tail,
            message: if code < 8 {
                None
            } else if stderr.trim().is_empty() {
                Some(stdout)
            } else {
                Some(stderr)
            },
        })
    }
    #[cfg(not(windows))]
    {
        let _ = item;
        let _ = preflight;
        Err(UecmError::OperationFailed(
            "local robocopy distribution requires Windows".into(),
        ))
    }
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
            &DistributeProfile::ddc_pak(),
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
            &DistributeProfile::ddc_pak(),
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
            &DistributeProfile::ddc_pak(),
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

    #[test]
    fn plan_does_not_duplicate_named_share_suffix() {
        let (db, source, target, project_id) = setup();
        project_locations::upsert(
            &db,
            &ProjectLocation {
                id: None,
                project_id,
                machine_id: target,
                abs_path: "E:\\Y\\DerivedDataCache".into(),
                uproject_path: "E:\\Y\\X.uproject".into(),
                discovery_status: crate::data::DiscoveryStatus::Auto,
                discovered_at: None,
            },
        )
        .unwrap();
        let items = plan(
            &DistributeProfile::ddc_pak(),
            &db,
            source,
            "1.1.1.1",
            &source_loc(project_id, source),
            &[target],
            project_id,
            Some("\\\\HOST\\DDC\\DerivedDataCache"),
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(items[0].source_unc, "\\\\HOST\\DDC\\DerivedDataCache");
        assert_eq!(items[0].target_local, "E:\\Y\\DerivedDataCache");
    }

    #[test]
    fn pso_profile_does_not_duplicate_nested_suffix() {
        assert_eq!(
            append_source_subdir_once("\\\\HOST\\PSO\\Saved\\CollectedPSOs", "Saved\\CollectedPSOs"),
            "\\\\HOST\\PSO\\Saved\\CollectedPSOs"
        );
    }
}
