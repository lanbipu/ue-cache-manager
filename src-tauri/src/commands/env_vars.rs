//! Tauri commands for reading/writing remote env vars on a single machine.

use crate::core::env_vars;
use crate::data::{machines as data_machines, Db};
use crate::error::{UecmError, UecmResult};
use tauri::State;

fn ip_for(db: &Db, machine_id: i64) -> UecmResult<String> {
    Ok(data_machines::find_by_id(db, machine_id)?
        .ok_or_else(|| UecmError::InvalidInput(format!("machine {} not found", machine_id)))?
        .ip)
}

#[tauri::command]
pub fn set_machine_env_var(
    db: State<'_, Db>,
    machine_id: i64,
    name: String,
    value: String,
) -> UecmResult<()> {
    let host = ip_for(&db, machine_id)?;
    env_vars::set(&host, &name, &value)
}

#[tauri::command]
pub fn get_machine_env_var(
    db: State<'_, Db>,
    machine_id: i64,
    name: String,
) -> UecmResult<Option<String>> {
    let host = ip_for(&db, machine_id)?;
    env_vars::get(&host, &name)
}
