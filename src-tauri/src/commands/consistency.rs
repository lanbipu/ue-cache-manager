use crate::core::consistency_check::{self, HostSnapshot, Inconsistency};
use crate::core::credentials as core_credentials;
use crate::data::{credentials as data_credentials, Db};
use crate::error::UecmError;
use tauri::State;

#[tauri::command]
pub async fn run_consistency_check(
    db: State<'_, Db>,
    hosts: Vec<String>,
    credential_alias: Option<String>,
) -> Result<(Vec<HostSnapshot>, Vec<Inconsistency>), String> {
    let creds: Option<(String, String)> = match credential_alias.as_deref() {
        Some(a) if !a.is_empty() => {
            let cred = data_credentials::find_by_alias(&db, a)
                .map_err(|e: UecmError| e.to_string())?
                .ok_or_else(|| format!("credential '{}' not found", a))?;
            let password = core_credentials::resolve_password(a)
                .map_err(|e: UecmError| e.to_string())?;
            Some((cred.username, password))
        }
        _ => None,
    };
    tokio::task::spawn_blocking(move || -> Result<(Vec<HostSnapshot>, Vec<Inconsistency>), String> {
        let mut snaps = Vec::new();
        for h in &hosts {
            snaps.push(
                consistency_check::snapshot(
                    h,
                    creds.as_ref().map(|(u, p)| (u.as_str(), p.as_str())),
                )
                .map_err(|e| e.to_string())?,
            );
        }
        let inc = consistency_check::compare(&snaps);
        Ok((snaps, inc))
    })
    .await
    .map_err(|e| format!("task join: {}", e))?
}
