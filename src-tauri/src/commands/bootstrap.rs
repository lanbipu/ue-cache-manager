//! Tauri commands for first-contact WinRM bootstrap.

use crate::core::{bootstrap as core_bootstrap, credentials as core_credentials};
use crate::data::{credentials as data_credentials, machines as data_machines, CredentialKind, Db};
use crate::error::{UecmError, UecmResult};
use tauri::State;

#[tauri::command]
pub fn get_winrm_bootstrap_script() -> UecmResult<String> {
    Ok(core_bootstrap::manual_winrm_script())
}

#[tauri::command]
pub fn bootstrap_winrm(
    db: State<'_, Db>,
    machine_id: i64,
    credential_alias: String,
    enable_local_account_remote_admin: bool,
) -> UecmResult<core_bootstrap::WinrmBootstrapResult> {
    let machine = data_machines::find_by_id(&db, machine_id)?
        .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", machine_id)))?;
    let credential = data_credentials::find_by_alias(&db, &credential_alias)?.ok_or_else(|| {
        UecmError::InvalidInput(format!("credential alias '{}' not found", credential_alias))
    })?;
    if credential.kind != CredentialKind::Winrm {
        return Err(UecmError::InvalidInput(format!(
            "credential alias '{}' is not a WinRM credential",
            credential_alias
        )));
    }

    let password = core_credentials::resolve_password(&credential_alias)?;
    let result = core_bootstrap::enable_winrm_with_psexec(
        &machine.ip,
        &credential.username,
        &password,
        enable_local_account_remote_admin,
        false,
    )?;

    if result.winrm_ok {
        data_machines::mark_seen(&db, machine_id, "online")?;
    } else {
        data_machines::mark_seen(&db, machine_id, "offline")?;
    }

    Ok(result)
}
