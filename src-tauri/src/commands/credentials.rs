//! Tauri commands for credential management. The SQLite `credentials` row holds
//! the alias metadata (kind + username); the secret lives in the cross-platform
//! SecretStore (AES-GCM). delete_credential also best-effort clears the legacy
//! cmdkey / DPAPI stores until they are removed in P5b.

use crate::core::credentials as core_creds;
use crate::core::secrets::SecretStore;
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

    // Store the secret in the cross-platform SecretStore (AES-GCM), replacing the
    // Windows-only cmdkey + DPAPI writes. If this fails nothing else is written,
    // so the saved state stays consistent (no half-saved alias). SQLite then holds
    // the alias metadata that `list_credentials` surfaces.
    SecretStore::from_config()?.put(&alias, &password)?;

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
    // SQLite metadata is the UI source of truth — always clear it.
    data_creds::delete_by_alias(&db, &alias)?;

    // SecretStore (Share-svc home, P3) + DPAPI: best-effort orphan cleanup.
    // DPAPI is a legacy store retired by the SSH migration, so a stale entry is
    // harmless — warn rather than block the delete.
    if let Err(e) = crate::core::secrets::SecretStore::from_config().and_then(|s| s.delete(&alias)) {
        tracing::warn!(alias = %alias, error = %e, "SecretStore delete failed; orphan secret may remain");
    }
    if let Err(e) = core_creds::delete_password(&alias) {
        tracing::warn!(alias = %alias, error = %e, "DPAPI delete_password failed; orphan entry will remain in creds.bin");
    }

    // cmdkey lives only in the Windows Credential Manager. On the operator host
    // (Windows) surface a genuine delete failure (perms/cmdkey) so the UI never
    // reports a credential gone while its entry lingers, unreclaimable —
    // cred-delete.ps1 is idempotent, so an alias with no entry (e.g. a
    // SecretStore-only Share) returns Ok rather than a spurious failure. On a
    // non-Windows operator there is no such store (and the PowerShell sidecar is
    // Windows-only), so skip it — otherwise delete would always "fail" there
    // after SQLite/SecretStore were already cleared.
    if cfg!(target_os = "windows") {
        core_creds::delete(&alias)?;
    }
    Ok(())
}
