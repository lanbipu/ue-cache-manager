//! Tauri commands for credential management. Combines the SQLite alias
//! record with the cmdkey side effect.

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
    cm_result
}
