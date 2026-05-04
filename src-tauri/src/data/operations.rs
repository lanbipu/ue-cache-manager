//! Minimal job history helpers backed by the `operations` table.

use crate::data::Db;
use crate::error::{UecmError, UecmResult};
use rusqlite::params;

pub fn start(db: &Db, action_type: &str, target_machines: &[i64]) -> UecmResult<i64> {
    let target_json = serde_json::to_string(target_machines).map_err(|e| {
        UecmError::OperationFailed(format!("serialize target_machines: {}", e))
    })?;
    let conn = db.lock().unwrap();
    conn.execute(
        "INSERT INTO operations (action_type, target_machines, status)
         VALUES (?, ?, 'running')",
        params![action_type, target_json],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn finish(db: &Db, id: i64, status: &str, log_text: Option<&str>) -> UecmResult<()> {
    let conn = db.lock().unwrap();
    conn.execute(
        "UPDATE operations
         SET status = ?, finished_at = CURRENT_TIMESTAMP, log_text = COALESCE(?, log_text)
         WHERE id = ?",
        params![status, log_text, id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{open_in_memory, schema};

    #[test]
    fn start_and_finish_operation() {
        let db = open_in_memory().unwrap();
        {
            let mut conn = db.lock().unwrap();
            schema::migrate(&mut conn).unwrap();
        }
        let id = start(&db, "ddc_pak.generate", &[1]).unwrap();
        finish(&db, id, "ok", Some("done")).unwrap();
        let conn = db.lock().unwrap();
        let status: String = conn
            .query_row("SELECT status FROM operations WHERE id = ?", [id], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(status, "ok");
    }
}
