//! Project discovery through the `discover-uprojects.ps1` sidecar.

use crate::core::ssh::{run_json, NodeScript, SshExecutor};
use crate::core::project_identity::{parse_engine_association, stem_lower, DiscoveredUproject};
use crate::data::{
    project_locations::{self, DiscoveryStatus, ProjectLocation},
    projects::{self, Project},
    Db,
};
use crate::error::{UecmError, UecmResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
struct DiscoveryItemRaw {
    uproject_filename: String,
    uproject_path: String,
    abs_path: String,
    engine_association: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DiscoveryScriptResult {
    ok: bool,
    items: Vec<DiscoveryItemRaw>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveryResult {
    pub project_id: i64,
    pub location_id: i64,
    pub uproject_filename: String,
    pub abs_path: String,
}

pub fn run_discovery(
    db: &Db,
    machine_id: i64,
    host: &str,
    search_roots: &[String],
    operator_user: Option<&str>,
    operator_pass: Option<&str>,
) -> UecmResult<Vec<DiscoveryResult>> {
    // SSH key auth: per-call WinRM creds no longer used (params kept until A5 cleanup).
    let _ = (operator_user, operator_pass);
    if search_roots.is_empty() {
        return Err(UecmError::InvalidInput("search_roots is empty".into()));
    }

    let exec = SshExecutor::from_config()?;
    let result: DiscoveryScriptResult = run_json(
        &exec,
        host,
        &NodeScript {
            name: "discover-uprojects.ps1",
            args: serde_json::json!({ "Roots": search_roots, "MaxDepth": 6 }),
            ssh_user: None,
        },
    )?;

    if !result.ok {
        return Err(UecmError::OperationFailed(
            result.message.unwrap_or_else(|| "project discovery failed".into()),
        ));
    }

    persist_discovered(db, machine_id, result.items)
}

fn persist_discovered(
    db: &Db,
    machine_id: i64,
    items: Vec<DiscoveryItemRaw>,
) -> UecmResult<Vec<DiscoveryResult>> {
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let stem = stem_lower(&item.uproject_filename);
        let parsed = parse_engine_association(item.engine_association.as_deref());
        let project_id = projects::upsert(
            db,
            &Project {
                id: None,
                uproject_name: item.uproject_filename.clone(),
                uproject_stem_lower: stem,
                // EngineAssociation is NOT a project GUID — historical bug.
                // Leave uproject_guid alone here; it stays None for new rows
                // discovered via this path.
                uproject_guid: None,
                display_name: None,
                first_seen_at: None,
                last_seen_at: None,
                ue_version_major: parsed.ue_version_major,
                ue_version_minor: parsed.ue_version_minor,
                engine_association_raw: parsed.raw,
                engine_association_kind: Some(parsed.kind.to_string()),
            },
        )?;
        let location_id = project_locations::upsert(
            db,
            &ProjectLocation {
                id: None,
                project_id,
                machine_id,
                abs_path: item.abs_path.clone(),
                uproject_path: item.uproject_path.clone(),
                discovery_status: DiscoveryStatus::Auto,
                discovered_at: None,
            },
        )?;
        out.push(DiscoveryResult {
            project_id,
            location_id,
            uproject_filename: item.uproject_filename,
            abs_path: item.abs_path,
        });
    }
    Ok(out)
}

pub fn to_discovered(machine_id: i64, items: &[DiscoveryResult]) -> Vec<DiscoveredUproject> {
    items
        .iter()
        .map(|item| DiscoveredUproject {
            machine_id,
            abs_path: item.abs_path.clone(),
            uproject_path: format!("{}\\{}", item.abs_path, item.uproject_filename),
            uproject_filename: item.uproject_filename.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{machines, open_in_memory, schema, Machine};

    fn setup() -> (Db, i64) {
        let db = open_in_memory().unwrap();
        {
            let mut conn = db.lock().unwrap();
            schema::migrate(&mut conn).unwrap();
        }
        let machine_id = machines::insert(&db, &Machine::new("RENDER-01", "1.1.1.1")).unwrap();
        (db, machine_id)
    }

    // (run_discovery's remote path now goes over SSH and is validated on a real
    // node; the old "Windows-only PowerShell error" test is obsolete and would
    // also generate a key in the real config dir via from_config().)

    #[test]
    fn empty_search_roots_returns_invalid_input() {
        let (db, machine_id) = setup();
        let result = run_discovery(&db, machine_id, "RENDER-01", &[], None, None);
        assert!(matches!(result, Err(UecmError::InvalidInput(_))));
    }

    #[test]
    fn to_discovered_preserves_identity_fields() {
        let items = vec![DiscoveryResult {
            project_id: 1,
            location_id: 2,
            uproject_filename: "Demo.uproject".into(),
            abs_path: "D:\\Demo".into(),
        }];
        let out = to_discovered(7, &items);
        assert_eq!(out[0].machine_id, 7);
        assert_eq!(out[0].uproject_path, "D:\\Demo\\Demo.uproject");
    }
}
