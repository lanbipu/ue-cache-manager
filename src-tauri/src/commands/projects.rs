//! Project management commands.

use crate::core::project_discovery::{self, DiscoveryResult};
use crate::core::project_identity::stem_lower;
use crate::data::{
    credentials as data_credentials, machines as data_machines, project_locations, projects, Db,
    DiscoveryStatus, Project, ProjectLocation,
};
use crate::error::{UecmError, UecmResult};
use serde::Serialize;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct ProjectSummary {
    pub id: i64,
    pub uproject_name: String,
    pub display_name: Option<String>,
    pub uproject_guid: Option<String>,
    pub location_count: i64,
}

fn resolve_operator_creds(
    db: &Db,
    alias: Option<&str>,
) -> UecmResult<(Option<String>, Option<String>)> {
    let Some(alias) = alias else {
        return Ok((None, None));
    };
    let cred = data_credentials::find_by_alias(db, alias)?
        .ok_or_else(|| UecmError::InvalidInput(format!("credential '{}' not found", alias)))?;
    let pass = crate::core::credentials::resolve_password(&cred.alias)?;
    Ok((Some(cred.username), Some(pass)))
}

#[tauri::command]
pub fn list_projects(db: State<'_, Db>) -> UecmResult<Vec<ProjectSummary>> {
    let mut out = Vec::new();
    for project in projects::list(&db)? {
        let id = project.id.unwrap_or_default();
        let locations = project_locations::list_by_project(&db, id)?;
        out.push(ProjectSummary {
            id,
            uproject_name: project.uproject_name,
            display_name: project.display_name,
            uproject_guid: project.uproject_guid,
            location_count: locations.len() as i64,
        });
    }
    Ok(out)
}

#[tauri::command]
pub fn list_project_locations(
    db: State<'_, Db>,
    project_id: i64,
) -> UecmResult<Vec<ProjectLocation>> {
    project_locations::list_by_project(&db, project_id)
}

#[tauri::command]
pub fn discover_projects(
    db: State<'_, Db>,
    machine_id: i64,
    search_roots: Vec<String>,
    operator_credential_alias: Option<String>,
) -> UecmResult<Vec<DiscoveryResult>> {
    let machine = data_machines::find_by_id(&db, machine_id)?
        .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", machine_id)))?;
    let (user, pass) = resolve_operator_creds(&db, operator_credential_alias.as_deref())?;
    project_discovery::run_discovery(
        &db,
        machine_id,
        &machine.ip,
        &search_roots,
        user.as_deref(),
        pass.as_deref(),
    )
}

#[tauri::command]
pub fn set_project_location(
    db: State<'_, Db>,
    project_id: i64,
    machine_id: i64,
    abs_path: String,
    uproject_path: String,
    manual: bool,
) -> UecmResult<i64> {
    let discovery_status = if manual {
        DiscoveryStatus::ManualPath
    } else {
        DiscoveryStatus::ManualAlias
    };
    project_locations::upsert(
        &db,
        &ProjectLocation {
            id: None,
            project_id,
            machine_id,
            abs_path,
            uproject_path,
            discovery_status,
            discovered_at: None,
        },
    )
}

#[tauri::command]
pub fn delete_project(db: State<'_, Db>, project_id: i64) -> UecmResult<()> {
    projects::delete(&db, project_id)
}

#[tauri::command]
pub fn delete_project_location(db: State<'_, Db>, location_id: i64) -> UecmResult<()> {
    project_locations::delete(&db, location_id)
}

#[tauri::command]
pub fn create_project_manual(
    db: State<'_, Db>,
    uproject_name: String,
    display_name: Option<String>,
) -> UecmResult<i64> {
    projects::upsert(
        &db,
        &Project {
            id: None,
            uproject_stem_lower: stem_lower(&uproject_name),
            uproject_name,
            uproject_guid: None,
            display_name,
            first_seen_at: None,
            last_seen_at: None,
        },
    )
}
