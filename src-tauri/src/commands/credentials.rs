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
    // Write to Credential Manager first; if that fails, don't pollute SQLite.
    core_creds::store(&alias, &username, &password)?;

    // DPAPI is best-effort: if it fails, cmdkey still gives us SMB auth, and
    // the SQLite row is still useful for listing. Log + continue.
    if let Err(e) = core_creds::store_password(&alias, &password) {
        tracing::warn!(
            alias = %alias,
            error = %e,
            "DPAPI store_password failed; per-call WinRM auth via this alias will fail"
        );
    }

    let record = CredentialRecord {
        id: None,
        alias: alias.clone(),
        kind,
        username,
    };
    // If alias already exists, delete + re-insert for an effective upsert.
    if data_creds::find_by_alias(&db, &alias)?.is_some() {
        data_creds::delete_by_alias(&db, &alias)?;
    }
    data_creds::insert(&db, &record)
}

#[tauri::command]
pub fn delete_credential(db: State<'_, Db>, alias: String) -> UecmResult<()> {
    // Try Credential Manager first; if delete fails, still clean SQLite so the
    // UI doesn't display a phantom alias.
    let cm_result = core_creds::delete(&alias);
    data_creds::delete_by_alias(&db, &alias)?;

    // Best-effort DPAPI cleanup. A leftover entry is harmless (orphan key,
    // never resolved) so don't fail the command on this.
    if let Err(e) = core_creds::delete_password(&alias) {
        tracing::warn!(
            alias = %alias,
            error = %e,
            "DPAPI delete_password failed; orphan entry will remain in creds.bin"
        );
    }

    cm_result
}
