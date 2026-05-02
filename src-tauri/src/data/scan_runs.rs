//! CRUD for diagnostic scan sessions.

use crate::data::Db;
use crate::error::{UecmError, UecmResult};
use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScanRun {
    pub id: Option<i64>,
    pub scan_type: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub machine_ids: Vec<i64>,
    pub summary: Option<JsonValue>,
}

pub fn insert(db: &Db, scan_type: &str, machine_ids: &[i64]) -> UecmResult<i64> {
    let conn = db.lock().unwrap();
    let machine_ids_json = serde_json::to_string(machine_ids)
        .map_err(|e| UecmError::OperationFailed(e.to_string()))?;
    conn.execute(
        "INSERT INTO scan_runs (scan_type, machine_ids_json) VALUES (?, ?)",
        params![scan_type, machine_ids_json],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn finish(db: &Db, id: i64, summary: &JsonValue) -> UecmResult<()> {
    let conn = db.lock().unwrap();
    let summary_json = serde_json::to_string(summary)
        .map_err(|e| UecmError::OperationFailed(e.to_string()))?;
    conn.execute(
        "UPDATE scan_runs SET finished_at = CURRENT_TIMESTAMP, summary_json = ? WHERE id = ?",
        params![summary_json, id],
    )?;
    Ok(())
}

pub fn find_by_id(db: &Db, id: i64) -> UecmResult<Option<ScanRun>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, scan_type, started_at, finished_at, machine_ids_json, summary_json
         FROM scan_runs WHERE id = ?",
    )?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row_to_scan_run(row)?))
    } else {
        Ok(None)
    }
}

pub fn list_recent(db: &Db, scan_type: &str, limit: i64) -> UecmResult<Vec<ScanRun>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, scan_type, started_at, finished_at, machine_ids_json, summary_json
         FROM scan_runs WHERE scan_type = ? ORDER BY started_at DESC, id DESC LIMIT ?",
    )?;
    let rows = stmt.query_map(params![scan_type, limit], row_to_scan_run)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn row_to_scan_run(row: &Row<'_>) -> rusqlite::Result<ScanRun> {
    let machine_ids_json: String = row.get(4)?;
    let summary_json: Option<String> = row.get(5)?;
    let machine_ids = serde_json::from_str(&machine_ids_json).map_err(json_to_sql)?;
    let summary = summary_json
        .map(|s| serde_json::from_str(&s).map_err(json_to_sql))
        .transpose()?;
    Ok(ScanRun {
        id: Some(row.get(0)?),
        scan_type: row.get(1)?,
        started_at: row.get(2)?,
        finished_at: row.get(3)?,
        machine_ids,
        summary,
    })
}

fn json_to_sql(err: serde_json::Error) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(err),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{open_in_memory, schema};

    fn setup() -> Db {
        let db = open_in_memory().unwrap();
        {
            let mut conn = db.lock().unwrap();
            schema::migrate(&mut conn).unwrap();
        }
        db
    }

    #[test]
    fn insert_returns_new_id_with_started_at() {
        let db = setup();
        let id = insert(&db, "ini", &[1, 2, 3]).unwrap();
        let row = find_by_id(&db, id).unwrap().unwrap();
        assert_eq!(row.scan_type, "ini");
        assert_eq!(row.machine_ids, vec![1, 2, 3]);
        assert!(row.started_at.is_some());
        assert!(row.finished_at.is_none());
    }

    #[test]
    fn finish_updates_summary_and_finished_at() {
        let db = setup();
        let id = insert(&db, "ini", &[1]).unwrap();
        finish(&db, id, &serde_json::json!({"critical": 0, "warning": 1})).unwrap();
        let row = find_by_id(&db, id).unwrap().unwrap();
        assert!(row.finished_at.is_some());
        assert_eq!(row.summary.unwrap()["warning"], 1);
    }

    #[test]
    fn list_recent_returns_descending() {
        let db = setup();
        let _a = insert(&db, "ini", &[1]).unwrap();
        let b = insert(&db, "health", &[1]).unwrap();
        let recent = list_recent(&db, "health", 10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].id, Some(b));
    }
}
