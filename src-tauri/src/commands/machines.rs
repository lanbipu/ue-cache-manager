//! Tauri command handlers for machine CRUD. These are thin wrappers around
//! the data layer; called from the frontend via `invoke()`.

use crate::data::{machines as data_machines, Db, Machine};
use crate::error::UecmResult;
use tauri::State;

#[tauri::command]
pub fn list_machines(db: State<'_, Db>) -> UecmResult<Vec<Machine>> {
    data_machines::list_all(&db)
}

#[tauri::command]
pub fn add_machine(
    db: State<'_, Db>,
    hostname: String,
    ip: String,
) -> UecmResult<i64> {
    let machine = Machine::new(&hostname, &ip);
    data_machines::insert(&db, &machine)
}

#[tauri::command]
pub fn delete_machine(db: State<'_, Db>, id: i64) -> UecmResult<()> {
    data_machines::delete(&db, id)
}
