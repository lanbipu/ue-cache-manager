//! CRUD for INI diagnostic findings.

use crate::data::Db;
use crate::error::UecmResult;
use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IniFinding {
    pub id: Option<i64>,
    pub scan_run_id: i64,
    pub machine_id: i64,
    pub rule_id: String,
    pub severity: String,
    pub category: String,
    pub file_path: String,
    pub section: Option<String>,
    pub key_name: Option<String>,
    pub line_number: Option<i64>,
    pub snippet_before: String,
    pub snippet_after: Option<String>,
    pub recommended_action: String,
    pub recommended_value: Option<String>,
    pub symptom: String,
    pub rationale: String,
    pub fixed_at: Option<String>,
    pub skipped_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SeverityCounts {
    pub critical: i64,
    pub warning: i64,
    pub healthy: i64,
    pub info: i64,
}

pub fn insert(db: &Db, finding: &IniFinding) -> UecmResult<i64> {
    let conn = db.lock().unwrap();
    conn.execute(
        "INSERT INTO ini_findings
         (scan_run_id, machine_id, rule_id, severity, category, file_path,
          section, key_name, line_number, snippet_before, snippet_after,
          recommended_action, recommended_value, symptom, rationale)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            finding.scan_run_id,
            finding.machine_id,
            finding.rule_id,
            finding.severity,
            finding.category,
            finding.file_path,
            finding.section,
            finding.key_name,
            finding.line_number,
            finding.snippet_before,
            finding.snippet_after,
            finding.recommended_action,
            finding.recommended_value,
            finding.symptom,
            finding.rationale,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn find_by_id(db: &Db, id: i64) -> UecmResult<Option<IniFinding>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(&format!("SELECT {} FROM ini_findings WHERE id = ?", SELECT_COLS))?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row_to_finding(row)?))
    } else {
        Ok(None)
    }
}

pub fn list_for_run(db: &Db, scan_run_id: i64) -> UecmResult<Vec<IniFinding>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM ini_findings WHERE scan_run_id = ? ORDER BY machine_id, severity, file_path, id",
        SELECT_COLS
    ))?;
    let rows = stmt.query_map(params![scan_run_id], row_to_finding)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn list_open_for_machine(db: &Db, machine_id: i64) -> UecmResult<Vec<IniFinding>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM ini_findings
         WHERE machine_id = ? AND fixed_at IS NULL AND skipped_at IS NULL
         ORDER BY id DESC",
        SELECT_COLS
    ))?;
    let rows = stmt.query_map(params![machine_id], row_to_finding)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn mark_fixed(db: &Db, id: i64) -> UecmResult<()> {
    let conn = db.lock().unwrap();
    conn.execute(
        "UPDATE ini_findings SET fixed_at = CURRENT_TIMESTAMP WHERE id = ?",
        params![id],
    )?;
    Ok(())
}

pub fn mark_skipped(db: &Db, id: i64) -> UecmResult<()> {
    let conn = db.lock().unwrap();
    conn.execute(
        "UPDATE ini_findings SET skipped_at = CURRENT_TIMESTAMP WHERE id = ?",
        params![id],
    )?;
    Ok(())
}

pub fn count_by_severity_for_machine(
    db: &Db,
    scan_run_id: i64,
    machine_id: i64,
) -> UecmResult<SeverityCounts> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT severity, COUNT(*) FROM ini_findings
         WHERE scan_run_id = ? AND machine_id = ? AND fixed_at IS NULL AND skipped_at IS NULL
         GROUP BY severity",
    )?;
    let rows = stmt.query_map(params![scan_run_id, machine_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    let mut counts = SeverityCounts::default();
    for row in rows {
        let (severity, count) = row?;
        match severity.as_str() {
            "critical" => counts.critical = count,
            "warning" => counts.warning = count,
            "healthy" => counts.healthy = count,
            "info" => counts.info = count,
            _ => {}
        }
    }
    Ok(counts)
}

const SELECT_COLS: &str = "id, scan_run_id, machine_id, rule_id, severity, category, file_path, \
section, key_name, line_number, snippet_before, snippet_after, recommended_action, \
recommended_value, symptom, rationale, fixed_at, skipped_at";

fn row_to_finding(row: &Row<'_>) -> rusqlite::Result<IniFinding> {
    Ok(IniFinding {
        id: Some(row.get(0)?),
        scan_run_id: row.get(1)?,
        machine_id: row.get(2)?,
        rule_id: row.get(3)?,
        severity: row.get(4)?,
        category: row.get(5)?,
        file_path: row.get(6)?,
        section: row.get(7)?,
        key_name: row.get(8)?,
        line_number: row.get(9)?,
        snippet_before: row.get(10)?,
        snippet_after: row.get(11)?,
        recommended_action: row.get(12)?,
        recommended_value: row.get(13)?,
        symptom: row.get(14)?,
        rationale: row.get(15)?,
        fixed_at: row.get(16)?,
        skipped_at: row.get(17)?,
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
        let scan_id = scan_runs::insert(&db, "ini", &[machine_id]).unwrap();
        (db, scan_id, machine_id)
    }

    fn sample(scan_id: i64, machine_id: i64) -> IniFinding {
        IniFinding {
            id: None,
            scan_run_id: scan_id,
            machine_id,
            rule_id: "R001".into(),
            severity: "critical".into(),
            category: "project".into(),
            file_path: "C:\\Project\\Config\\DefaultEngine.ini".into(),
            section: Some("/Script/UnrealEd.DerivedDataCacheSettings".into()),
            key_name: Some("Path".into()),
            line_number: Some(42),
            snippet_before: "Path=D:\\OldDDC".into(),
            snippet_after: Some("EnvPathOverride=UE-SharedDataCachePath".into()),
            recommended_action: "set".into(),
            recommended_value: Some("UE-SharedDataCachePath".into()),
            symptom: "DDC silently falls back to local".into(),
            rationale: "Hardcoded path overrides env var".into(),
            fixed_at: None,
            skipped_at: None,
        }
    }

    #[test]
    fn list_for_run_returns_inserted_rows() {
        let (db, scan_id, machine_id) = setup();
        insert(&db, &sample(scan_id, machine_id)).unwrap();
        insert(&db, &sample(scan_id, machine_id)).unwrap();
        assert_eq!(list_for_run(&db, scan_id).unwrap().len(), 2);
    }

    #[test]
    fn mark_fixed_sets_timestamp() {
        let (db, scan_id, machine_id) = setup();
        let id = insert(&db, &sample(scan_id, machine_id)).unwrap();
        mark_fixed(&db, id).unwrap();
        assert!(find_by_id(&db, id).unwrap().unwrap().fixed_at.is_some());
    }

    #[test]
    fn mark_skipped_sets_timestamp() {
        let (db, scan_id, machine_id) = setup();
        let id = insert(&db, &sample(scan_id, machine_id)).unwrap();
        mark_skipped(&db, id).unwrap();
        assert!(find_by_id(&db, id).unwrap().unwrap().skipped_at.is_some());
    }

    #[test]
    fn count_by_severity_for_machine_returns_counts() {
        let (db, scan_id, machine_id) = setup();
        insert(&db, &sample(scan_id, machine_id)).unwrap();
        let mut warning = sample(scan_id, machine_id);
        warning.severity = "warning".into();
        insert(&db, &warning).unwrap();
        let counts = count_by_severity_for_machine(&db, scan_id, machine_id).unwrap();
        assert_eq!(counts.critical, 1);
        assert_eq!(counts.warning, 1);
    }
}
