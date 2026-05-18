//! Tauri command: pull DDC startup log from a host and return a VerifyReport.

use crate::core::credentials as core_credentials;
use crate::core::ue_log_verify::{self, VerifyReport};
use crate::data::{credentials as data_credentials, Db};
use crate::error::UecmError;
use tauri::State;

#[tauri::command]
pub async fn run_log_verify(
    db: State<'_, Db>,
    host: String,
    editor_exe: String,
    project: String,
    timeout: u32,
    credential_alias: Option<String>,
) -> Result<VerifyReport, String> {
    // Resolve credentials while still on the async runtime thread (State<Db> is !Send).
    let creds: Option<(String, String)> = match credential_alias.as_deref() {
        Some(alias) if !alias.is_empty() => {
            let cred = data_credentials::find_by_alias(&db, alias)
                .map_err(|e: UecmError| e.to_string())?
                .ok_or_else(|| format!("credential '{}' not found", alias))?;
            let password = core_credentials::resolve_password(alias)
                .map_err(|e: UecmError| e.to_string())?;
            Some((cred.username, password))
        }
        _ => None,
    };

    tokio::task::spawn_blocking(move || {
        ue_log_verify::run_for_host(
            &host,
            &editor_exe,
            &project,
            timeout,
            creds.as_ref().map(|(u, p)| (u.as_str(), p.as_str())),
        )
        .map_err(|e: UecmError| e.to_string())
    })
    .await
    .map_err(|e| format!("task join: {}", e))?
}
