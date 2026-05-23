//! Tauri commands for credential management. Combines the SQLite alias
//! record with the cmdkey side effect (transparent SMB auth) and the DPAPI
//! side effect (per-call WinRM auth).

use crate::core::credentials as core_creds;
use crate::data::{credentials as data_creds, CredentialKind, CredentialRecord, Db};
use crate::error::UecmResult;
use tauri::State;

#[tauri::command]
pub fn list_credentials(db: State<'_, Db>) -> UecmResult<Vec<CredentialRecord>> {
    data_creds::list_all(&db)
}

#[tauri::command]
pub fn save_credential(
    db: State<'_, Db>,
    alias: String,
    kind: CredentialKind,
    username: String,
    password: String,
) -> UecmResult<i64> {
    let username = core_creds::normalize_username_for_storage(&username);

    // Cmdkey first: if this fails, nothing else gets written.
    core_creds::store(&alias, &username, &password)?;

    // DPAPI must succeed for per-call WinRM auth (share creation, batch ops)
    // to work later. Roll back the cmdkey entry if DPAPI fails so the saved
    // state is consistent — half-saved aliases caused user-visible "no DPAPI
    // entry" errors downstream during Plan 3 lanPC E2E.
    if let Err(e) = core_creds::store_password(&alias, &password) {
        if let Err(rollback_err) = core_creds::delete(&alias) {
            tracing::warn!(
                alias = %alias,
                error = %rollback_err,
                "cmdkey rollback after DPAPI failure also failed"
            );
        }
        return Err(e);
    }

    let record = CredentialRecord {
        id: None,
        alias: alias.clone(),
        kind,
        username,
    };
    if data_creds::find_by_alias(&db, &alias)?.is_some() {
        data_creds::delete_by_alias(&db, &alias)?;
    }
    data_creds::insert(&db, &record)
}

#[tauri::command]
pub fn delete_credential(db: State<'_, Db>, alias: String) -> UecmResult<()> {
    // SQLite metadata is the source of truth for the UI list — always clear it.
    data_creds::delete_by_alias(&db, &alias)?;

    // Best-effort secret cleanup across every store; a leftover entry is a
    // harmless orphan, so none failing should wedge the delete (cmdkey is no
    // longer the home for share-svc secrets, so its "no entry" failure is
    // expected for SecretStore-backed aliases).
    //   - SecretStore: current home for Mode B share-svc passwords (P3).
    if let Err(e) = crate::core::secrets::SecretStore::from_config().and_then(|s| s.delete(&alias)) {
        tracing::warn!(alias = %alias, error = %e, "SecretStore delete failed; orphan secret may remain");
    }
    //   - cmdkey / DPAPI: legacy stores for aliases created before the SecretStore move (retired in P5b).
    if let Err(e) = core_creds::delete(&alias) {
        tracing::debug!(alias = %alias, error = %e, "cmdkey delete failed (expected for SecretStore-backed aliases)");
    }
    if let Err(e) = core_creds::delete_password(&alias) {
        tracing::warn!(
            alias = %alias,
            error = %e,
            "DPAPI delete_password failed; orphan entry will remain in creds.bin"
        );
    }

    Ok(())
}
