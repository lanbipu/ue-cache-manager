//! Tauri commands for cluster batch ops. Each command resolves credentials
//! once, then fans out to N machines via core::batch::run_batch, forwarding
//! progress events to the frontend via the `batch-progress` Tauri event.

use crate::core::{batch, credentials as core_creds, env_vars, ini_editor};
use crate::data::{credentials as data_creds, machines as data_machines, Db};
use crate::error::{UecmError, UecmResult};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

const BATCH_EVENT_NAME: &str = "batch-progress";

struct ResolvedCred {
    username: String,
    password: String,
}

fn resolve(db: &Db, alias: &str) -> UecmResult<ResolvedCred> {
    let cred = data_creds::find_by_alias(db, alias)?.ok_or_else(|| {
        UecmError::InvalidInput(format!("credential alias '{}' not found", alias))
    })?;
    let password = core_creds::resolve_password(alias)?;
    Ok(ResolvedCred {
        username: cred.username,
        password,
    })
}

fn ip_for(db: &Db, machine_id: i64) -> UecmResult<String> {
    Ok(data_machines::find_by_id(db, machine_id)?
        .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", machine_id)))?
        .ip)
}

#[tauri::command]
pub async fn batch_set_env_var(
    db: State<'_, Db>,
    app: AppHandle,
    machine_ids: Vec<i64>,
    name: String,
    value: String,
    credential_alias: String,
) -> UecmResult<()> {
    let cred = Arc::new(resolve(&db, &credential_alias)?);
    let ips: Vec<(i64, String)> = machine_ids
        .iter()
        .map(|id| ip_for(&db, *id).map(|ip| (*id, ip)))
        .collect::<UecmResult<Vec<_>>>()?;
    let ip_lookup: std::collections::HashMap<i64, String> = ips.into_iter().collect();
    let name = Arc::new(name);
    let value = Arc::new(value);

    let mut rx = batch::run_batch(machine_ids, batch::DEFAULT_MAX_CONCURRENCY, {
        let cred = cred.clone();
        let name = name.clone();
        let value = value.clone();
        let ip_lookup = ip_lookup.clone();
        move |machine_id| {
            let cred = cred.clone();
            let name = name.clone();
            let value = value.clone();
            let host = ip_lookup.get(&machine_id).cloned();
            async move {
                let host = host.ok_or_else(|| {
                    UecmError::InvalidInput(format!("machine {} not in lookup", machine_id))
                })?;
                tokio::task::spawn_blocking(move || {
                    env_vars::set_with_credential(
                        &host,
                        &name,
                        &value,
                        &cred.username,
                        &cred.password,
                    )
                })
                .await
                .map_err(|e| UecmError::OperationFailed(format!("join error: {}", e)))?
            }
        }
    })
    .await;

    while let Some(ev) = rx.recv().await {
        let _ = app.emit(BATCH_EVENT_NAME, ev);
    }
    Ok(())
}

#[tauri::command]
pub async fn batch_set_ini_key(
    db: State<'_, Db>,
    app: AppHandle,
    machine_ids: Vec<i64>,
    file_path: String,
    section: String,
    name: String,
    value: String,
    credential_alias: String,
) -> UecmResult<()> {
    let cred = Arc::new(resolve(&db, &credential_alias)?);
    let ips: Vec<(i64, String)> = machine_ids
        .iter()
        .map(|id| ip_for(&db, *id).map(|ip| (*id, ip)))
        .collect::<UecmResult<Vec<_>>>()?;
    let ip_lookup: std::collections::HashMap<i64, String> = ips.into_iter().collect();
    let file_path = Arc::new(file_path);
    let section = Arc::new(section);
    let name = Arc::new(name);
    let value = Arc::new(value);

    let mut rx = batch::run_batch(machine_ids, batch::DEFAULT_MAX_CONCURRENCY, {
        let cred = cred.clone();
        let file_path = file_path.clone();
        let section = section.clone();
        let name = name.clone();
        let value = value.clone();
        let ip_lookup = ip_lookup.clone();
        move |machine_id| {
            let cred = cred.clone();
            let file_path = file_path.clone();
            let section = section.clone();
            let name = name.clone();
            let value = value.clone();
            let host = ip_lookup.get(&machine_id).cloned();
            async move {
                let host = host.ok_or_else(|| {
                    UecmError::InvalidInput(format!("machine {} not in lookup", machine_id))
                })?;
                tokio::task::spawn_blocking(move || {
                    ini_editor::set_key_with_credential(
                        &host,
                        &file_path,
                        &section,
                        &name,
                        &value,
                        &cred.username,
                        &cred.password,
                    )
                    .map(|_backup| ())
                })
                .await
                .map_err(|e| UecmError::OperationFailed(format!("join error: {}", e)))?
            }
        }
    })
    .await;

    while let Some(ev) = rx.recv().await {
        let _ = app.emit(BATCH_EVENT_NAME, ev);
    }
    Ok(())
}
