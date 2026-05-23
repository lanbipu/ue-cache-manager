//! Robocopy fan-out planning and per-target execution for DDC Pak files.

#[cfg(windows)]
use crate::core::powershell; // decode_subprocess_output, used by the loopback robocopy path
use crate::core::ssh::{run_json, NodeScript, SshExecutor};
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
    /// One or more glob patterns.  Each element is a single Robocopy file-pattern
    /// argument (no spaces within an element).  The loopback path (`run_local_robocopy`)
    /// passes only the first pattern; callers that need all patterns must iterate and
    /// call `run_one_with_profile` once per pattern — see `pso_cache_profiles()`.
    pub file_globs: Vec<String>,
    pub ps_script: &'static str,
}

impl DistributeProfile {
    pub fn ddc_pak() -> Self {
        Self {
            source_subdir: "DerivedDataCache".into(),
            file_globs: vec!["*.ddp".into()],
            ps_script: "distribute-pak-file.ps1",
        }
    }

    /// PSO cache covers two extensions.  Use `pso_cache_profiles()` when you need
    /// a separate Robocopy invocation per pattern; this constructor is kept for
    /// code that only needs to inspect the profile shape.
    pub fn pso_cache() -> Self {
        Self {
            source_subdir: "Saved\\CollectedPSOs".into(),
            file_globs: vec!["*.upipelinecache".into(), "*.stablepc.csv".into()],
            ps_script: "distribute-pso-cache.ps1",
        }
    }

    /// Returns one `DistributeProfile` per PSO extension so each gets its own
    /// Robocopy invocation and there is no ambiguity about single-string patterns.
    pub fn pso_cache_profiles() -> Vec<Self> {
        ["*.upipelinecache", "*.stablepc.csv"]
            .iter()
            .map(|glob| Self {
                source_subdir: "Saved\\CollectedPSOs".into(),
                file_globs: vec![(*glob).into()],
                ps_script: "distribute-pso-cache.ps1",
            })
            .collect()
    }

    /// The first (and for DDC pak, only) file-glob pattern.
    pub fn primary_glob(&self) -> &str {
        self.file_globs.first().map(String::as_str).unwrap_or("")
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

/// Source-share SMB access for a distribute run: the managed-share UNC the
/// target should pull from, plus the credential to mount it. Empty (`None`s)
/// means "no managed share / no cred" — fine for an open Mode A source.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceSmb {
    pub named_share_unc: Option<String>,
    pub user: Option<String>,
    pub pass: Option<String>,
}

/// Resolve how the target node reads the source share. Auto-derive (no explicit
/// alias) finds a Managed (Mode B) share on the source host and returns BOTH its
/// `unc_path` AND the SecretStore credential A3-7 wrote — they must stay paired,
/// because the `ddc-svc` account only has rights to that managed share, not the
/// admin `D$` share. An explicit alias selects the matching managed share (so
/// its UNC is used too); if none matches, the cred still resolves but the UNC is
/// left to the caller's admin-share fallback.
///
/// Mode B svc account is `ddc-svc` by convention (see `share create`); the
/// SecretStore holds only the password, so the username is fixed here.
pub fn resolve_source_smb(
    db: &Db,
    source_machine_id: i64,
    explicit_alias: Option<&str>,
) -> UecmResult<SourceSmb> {
    use crate::data::share_configs::{self, ShareMode};
    let share = share_configs::find_by_host(db, source_machine_id)?
        .into_iter()
        .find(|s| {
            s.mode == ShareMode::Managed
                && s.credential_alias.is_some()
                && explicit_alias.map_or(true, |a| s.credential_alias.as_deref() == Some(a))
        });
    let store = crate::core::secrets::SecretStore::from_config()?;
    if let Some(share) = share {
        let alias = share.credential_alias.expect("filtered to Some");
        let pass = store.get(&alias)?;
        return Ok(SourceSmb {
            named_share_unc: pass.as_ref().map(|_| share.unc_path.clone()),
            user: pass.as_ref().map(|_| "ddc-svc".to_string()),
            pass,
        });
    }
    // No managed share matched. An explicit alias still resolves a cred (UNC
    // unknown → admin-share fallback); no alias → nothing (open Mode A source).
    match explicit_alias {
        Some(alias) => {
            let pass = store.get(alias)?;
            Ok(SourceSmb {
                named_share_unc: None,
                user: pass.as_ref().map(|_| "ddc-svc".to_string()),
                pass,
            })
        }
        None => Ok(SourceSmb::default()),
    }
}

/// stdin JSON for the node-pure distribute scripts. The operator→target WinRM
/// cred is gone (SSH key auth); only the target→source SMB cred is forwarded.
fn build_distribute_payload(item: &DistributePlanItem, preflight: bool) -> serde_json::Value {
    let mut obj = serde_json::json!({
        "SourceUnc": item.source_unc,
        "TargetLocal": item.target_local,
        "PreflightOnly": preflight,
    });
    let map = obj.as_object_mut().expect("json object");
    if let Some(file_name) = &item.file_name {
        map.insert("FileName".into(), file_name.clone().into());
    }
    if let (Some(user), Some(pass)) =
        (item.source_smb_user.as_deref(), item.source_smb_pass.as_deref())
    {
        map.insert("SourceSmbUser".into(), user.into());
        map.insert("SourceSmbPass".into(), pass.into());
    }
    obj
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

    let exec = SshExecutor::from_config()?;
    let result: DistributeRaw = run_json(
        &exec,
        &item.target_host,
        &NodeScript {
            name: profile.ps_script,
            args: build_distribute_payload(item, true),
            ssh_user: None,
        },
    )?;
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

    let exec = SshExecutor::from_config()?;
    let result: DistributeRaw = run_json(
        &exec,
        &item.target_host,
        &NodeScript {
            name: profile.ps_script,
            args: build_distribute_payload(&item, false),
            ssh_user: None,
        },
    )?;
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
    _profile: &DistributeProfile,
    item: &DistributePlanItem,
    preflight: bool,
) -> UecmResult<DistributeOutcome> {
    #[cfg(windows)]
    {
        let mut args = vec![
            item.source_unc.as_str(),
            item.target_local.as_str(),
            item.file_name.as_deref().unwrap_or(_profile.primary_glob()),
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
        let stdout = powershell::decode_subprocess_output(&output.stdout);
        let stderr = powershell::decode_subprocess_output(&output.stderr);
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
                ue_version_major: None,
                ue_version_minor: None,
                engine_association_raw: None,
                engine_association_kind: None,
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
    fn resolve_source_smb_none_without_managed_share() {
        use crate::data::share_configs::{insert, ShareConfig, ShareMode};
        let (db, source, _target, _project) = setup();
        // No shares registered → empty (open Mode A path on the node).
        assert_eq!(
            resolve_source_smb(&db, source, None).unwrap(),
            SourceSmb::default()
        );
        // An Open (Mode A) share contributes no managed UNC / cred either.
        insert(
            &db,
            &ShareConfig {
                id: None,
                host_machine_id: source,
                share_name: "DDC".into(),
                unc_path: "\\\\SOURCE\\DDC".into(),
                local_path: "D:\\DDC".into(),
                mode: ShareMode::Open,
                credential_alias: None,
            },
        )
        .unwrap();
        assert_eq!(
            resolve_source_smb(&db, source, None).unwrap(),
            SourceSmb::default()
        );
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

    #[test]
    fn pso_cache_profile_includes_both_pso_extensions() {
        let profile = DistributeProfile::pso_cache();
        assert!(
            profile.file_globs.iter().any(|g| g == "*.upipelinecache"),
            "pso_cache profile must include *.upipelinecache"
        );
        assert!(
            profile.file_globs.iter().any(|g| g == "*.stablepc.csv"),
            "pso_cache profile must include *.stablepc.csv"
        );
        // No single element should contain a space (that would be the old broken shape)
        for g in &profile.file_globs {
            assert!(!g.contains(' '), "glob pattern must not contain a space: {:?}", g);
        }
    }

    #[test]
    fn pso_cache_profiles_returns_one_profile_per_extension() {
        let profiles = DistributeProfile::pso_cache_profiles();
        assert_eq!(profiles.len(), 2);
        let globs: Vec<&str> = profiles.iter().map(|p| p.primary_glob()).collect();
        assert!(globs.contains(&"*.upipelinecache"));
        assert!(globs.contains(&"*.stablepc.csv"));
        // Each profile has exactly one glob (no space-separated multi-pattern)
        for p in &profiles {
            assert_eq!(p.file_globs.len(), 1);
            assert!(!p.file_globs[0].contains(' '));
        }
    }

    #[test]
    fn ddc_pak_profile_has_single_glob() {
        let profile = DistributeProfile::ddc_pak();
        assert_eq!(profile.file_globs.len(), 1);
        assert_eq!(profile.primary_glob(), "*.ddp");
    }
}
