//! CRUD for per-machine health-check results.

use crate::data::Db;
use crate::error::{UecmError, UecmResult};
use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthCheckRun {
    pub id: Option<i64>,
    pub scan_run_id: i64,
    pub machine_id: i64,
    pub machine_results: JsonValue,
}

pub fn insert(db: &Db, run: &HealthCheckRun) -> UecmResult<i64> {
    let conn = db.lock().unwrap();
    let machine_results_json = serde_json::to_string(&run.machine_results)
        .map_err(|e| UecmError::OperationFailed(e.to_string()))?;
    conn.execute(
        "INSERT INTO health_check_runs (scan_run_id, machine_id, machine_results_json)
         VALUES (?, ?, ?)",
        params![run.scan_run_id, run.machine_id, machine_results_json],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list_for_run(db: &Db, scan_run_id: i64) -> UecmResult<Vec<HealthCheckRun>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, scan_run_id, machine_id, machine_results_json
         FROM health_check_runs WHERE scan_run_id = ? ORDER BY machine_id",
    )?;
    let rows = stmt.query_map(params![scan_run_id], row_to_run)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn latest_for_machine(db: &Db, machine_id: i64) -> UecmResult<Option<HealthCheckRun>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT h.id, h.scan_run_id, h.machine_id, h.machine_results_json
         FROM health_check_runs h
         JOIN scan_runs s ON s.id = h.scan_run_id
         WHERE h.machine_id = ?
         ORDER BY s.started_at DESC, h.id DESC
         LIMIT 1",
    )?;
    let mut rows = stmt.query(params![machine_id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row_to_run(row)?))
    } else {
        Ok(None)
    }
}

pub fn update_results(db: &Db, id: i64, machine_results: &JsonValue) -> UecmResult<()> {
    let conn = db.lock().unwrap();
    let machine_results_json = serde_json::to_string(machine_results)
        .map_err(|e| UecmError::OperationFailed(e.to_string()))?;
    conn.execute(
        "UPDATE health_check_runs SET machine_results_json = ? WHERE id = ?",
        params![machine_results_json, id],
    )?;
    Ok(())
}

fn row_to_run(row: &Row<'_>) -> rusqlite::Result<HealthCheckRun> {
    let raw: String = row.get(3)?;
    let machine_results = serde_json::from_str(&raw).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(
            3,
            rusqlite::types::Type::Text,
            Box::new(err),
        )
    })?;
    Ok(HealthCheckRun {
        id: Some(row.get(0)?),
        scan_run_id: row.get(1)?,
        machine_id: row.get(2)?,
        machine_results,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{machines, open_in_memory, scan_runs, schema, Machine};

    fn setup() -> (Db, i64, i64) {
        let db = open_in_memory().unwrap();
        {
            let mut conn = db.lock().unwrap();
            schema::migrate(&mut conn).unwrap();
        }
        let machine_id = machines::insert(&db, &Machine::new("RENDER-01", "192.168.10.21")).unwrap();
        let scan_id = scan_runs::insert(&db, "health", &[machine_id]).unwrap();
        (db, scan_id, machine_id)
    }

    #[test]
    fn insert_and_list_round_trip_json() {
        let (db, scan_id, machine_id) = setup();
        insert(
            &db,
            &HealthCheckRun {
                id: None,
                scan_run_id: scan_id,
                machine_id,
                machine_results: serde_json::json!({"smb": {"status": "healthy"}}),
            },
        )
        .unwrap();
        let rows = list_for_run(&db, scan_id).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].machine_results["smb"]["status"], "healthy");
    }

    #[test]
    fn latest_for_machine_returns_recent_row() {
        let (db, scan_id, machine_id) = setup();
        let id = insert(
            &db,
            &HealthCheckRun {
                id: None,
                scan_run_id: scan_id,
                machine_id,
                machine_results: serde_json::json!({"ok": true}),
            },
        )
        .unwrap();
        assert_eq!(latest_for_machine(&db, machine_id).unwrap().unwrap().id, Some(id));
    }
}
