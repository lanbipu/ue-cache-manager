//! Tauri commands for SMB share creation, listing, deletion, and per-client
//! SYSTEM credential injection.
//!
//! Mode A (`Open`) — Guest+Everyone:Full. No svc credential to track.
//! Mode B (`Managed`) — generates a 24-byte URL-safe password, runs the
//! PS script (host-side `New-SmbShare` + local `ddc-svc`), then on success
//! persists the alias to:
//!   1. `cmdkey` (transparent SMB auth on the operator host)
//!   2. DPAPI-encrypted store (so future `inject_share_credential_to_clients`
//!      can read the plaintext back)
//!   3. SQLite `credentials` row (so the alias surfaces in the UI list)
//!
//! Persistence happens AFTER the PS script succeeds — a PS failure leaves
//! SQLite untouched.

use crate::core::{credentials as core_creds, psexec, shares as core_shares};
use crate::data::{
    credentials as data_creds, machines as data_machines, share_configs as data_shares,
    CredentialKind, CredentialRecord, Db, ShareConfig, ShareMode,
};
use crate::error::{UecmError, UecmResult};
use serde::Serialize;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct CreateShareResponse {
    pub share_config_id: i64,
    pub unc_path: String,
    pub mode: ShareMode,
    pub credential_alias: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct InjectionResult {
    pub client_machine_id: i64,
    pub ok: bool,
    pub message: String,
}

fn resolve_operator_creds(
    db: &Db,
    alias: Option<&str>,
) -> UecmResult<(Option<String>, Option<String>)> {
    let Some(alias) = alias else {
        return Ok((None, None));
    };
    if alias.is_empty() {
        return Ok((None, None));
    }
    let cred = data_creds::find_by_alias(db, alias)?.ok_or_else(|| {
        UecmError::InvalidInput(format!("credential alias '{}' not found", alias))
    })?;
    let pwd = core_creds::resolve_password(alias)?;
    Ok((Some(cred.username), Some(pwd)))
}

fn host_ip(db: &Db, machine_id: i64) -> UecmResult<String> {
    Ok(data_machines::find_by_id(db, machine_id)?
        .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", machine_id)))?
        .ip)
}

fn host_hostname(db: &Db, machine_id: i64) -> UecmResult<String> {
    Ok(data_machines::find_by_id(db, machine_id)?
        .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", machine_id)))?
        .hostname)
}

#[tauri::command]
pub fn create_share(
    db: State<'_, Db>,
    host_machine_id: i64,
    mode: ShareMode,
    share_name: String,
    local_path: String,
    operator_credential_alias: Option<String>,
    svc_username: Option<String>,
) -> UecmResult<CreateShareResponse> {
    let host_ip = host_ip(&db, host_machine_id)?;
    let (op_user, op_pass) = resolve_operator_creds(&db, operator_credential_alias.as_deref())?;

    let (unc_path, persisted_alias): (String, Option<String>) = match mode {
        ShareMode::Open => {
            let result = core_shares::create_mode_a(
                &host_ip,
                &share_name,
                &local_path,
                op_user.as_deref(),
                op_pass.as_deref(),
            )?;
            (result.unc_path, None)
        }
        ShareMode::Managed => {
            let svc_user = svc_username
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or("ddc-svc")
                .to_string();
            let svc_pass = core_shares::generate_svc_password();
            let result = core_shares::create_mode_b(
                &host_ip,
                &share_name,
                &local_path,
                &svc_user,
                &svc_pass,
                op_user.as_deref(),
                op_pass.as_deref(),
            )?;
            // PS script succeeded — host-side `ddc-svc` exists and
            // `New-SmbShare` is up. Now persist the alias locally so the
            // operator host can transparently mount the share AND so future
            // injection calls can read the password back.
            let host_hn = host_hostname(&db, host_machine_id)?;
            let alias = format!("UECM:share:{}:{}", host_hn, svc_user);
            // 1) cmdkey — persistent SMB transparent auth on this machine.
            core_creds::store(&alias, &svc_user, &svc_pass)?;
            // 2) DPAPI — per-call WinRM auth used by inject_share_credential_to_clients.
            //    Best-effort: cmdkey already covers SMB on this host; logging is
            //    enough so we don't wedge the whole flow.
            if let Err(e) = core_creds::store_password(&alias, &svc_pass) {
                tracing::warn!(
                    alias = %alias,
                    error = %e,
                    "DPAPI store_password failed for share svc credential"
                );
            }
            // 3) SQLite credential record — idempotent (skip if alias somehow
            //    already exists from a prior partial run).
            if data_creds::find_by_alias(&db, &alias)?.is_none() {
                data_creds::insert(
                    &db,
                    &CredentialRecord {
                        id: None,
                        alias: alias.clone(),
                        kind: CredentialKind::Share,
                        username: svc_user.clone(),
                    },
                )?;
            }
            (result.unc_path, Some(alias))
        }
    };

    let cfg = ShareConfig {
        id: None,
        host_machine_id,
        share_name: share_name.clone(),
        unc_path: unc_path.clone(),
        local_path: local_path.clone(),
        mode,
        credential_alias: persisted_alias.clone(),
    };
    let share_config_id = data_shares::insert(&db, &cfg)?;

    Ok(CreateShareResponse {
        share_config_id,
        unc_path,
        mode,
        credential_alias: persisted_alias,
    })
}

#[tauri::command]
pub fn inject_share_credential_to_clients(
    db: State<'_, Db>,
    share_config_id: i64,
    client_machine_ids: Vec<i64>,
    operator_credential_alias: Option<String>,
) -> UecmResult<Vec<InjectionResult>> {
    let share = data_shares::find_by_id(&db, share_config_id)?.ok_or_else(|| {
        UecmError::InvalidInput(format!("share_config {} not found", share_config_id))
    })?;
    if share.mode != ShareMode::Managed {
        return Err(UecmError::InvalidInput(
            "credential injection only applies to Mode B (managed) shares".to_string(),
        ));
    }
    let svc_alias = share.credential_alias.as_ref().ok_or_else(|| {
        UecmError::OperationFailed("managed share missing credential_alias".to_string())
    })?;
    let svc_cred = data_creds::find_by_alias(&db, svc_alias)?.ok_or_else(|| {
        UecmError::OperationFailed(format!(
            "credential alias '{}' from share row not found in credentials",
            svc_alias
        ))
    })?;
    let svc_pass = core_creds::resolve_password(svc_alias)?;
    let host_hn = host_hostname(&db, share.host_machine_id)?;
    let (op_user, op_pass) = resolve_operator_creds(&db, operator_credential_alias.as_deref())?;

    let mut results = Vec::with_capacity(client_machine_ids.len());
    for client_id in client_machine_ids {
        let client_ip = match host_ip(&db, client_id) {
            Ok(ip) => ip,
            Err(e) => {
                results.push(InjectionResult {
                    client_machine_id: client_id,
                    ok: false,
                    message: e.to_string(),
                });
                continue;
            }
        };
        match psexec::inject_system_credential(
            &client_ip,
            &host_hn,
            &svc_cred.username,
            &svc_pass,
            op_user.as_deref(),
            op_pass.as_deref(),
        ) {
            Ok(msg) => results.push(InjectionResult {
                client_machine_id: client_id,
                ok: true,
                message: msg,
            }),
            Err(e) => results.push(InjectionResult {
                client_machine_id: client_id,
                ok: false,
                message: e.to_string(),
            }),
        }
    }
    Ok(results)
}

#[tauri::command]
pub fn list_shares(db: State<'_, Db>) -> UecmResult<Vec<ShareConfig>> {
    data_shares::list_all(&db)
}

#[tauri::command]
pub fn delete_share(
    db: State<'_, Db>,
    share_config_id: i64,
    also_remove_remote: bool,
) -> UecmResult<()> {
    // TODO(plan-4): when `ps-scripts/remove-share.ps1` lands, branch on
    // `also_remove_remote` to call Remove-SmbShare + (Mode B) Remove-LocalUser.
    // For Plan 3 v1 we delete the SQLite row only.
    let _ = also_remove_remote;
    data_shares::delete(&db, share_config_id)?;
    Ok(())
}
