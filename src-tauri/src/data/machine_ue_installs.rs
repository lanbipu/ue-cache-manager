//! CRUD for the `machine_ue_installs` table.

use crate::data::Db;
use crate::error::UecmResult;
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UeInstall {
    pub id: Option<i64>,
    pub machine_id: i64,
    pub version: String,        // e.g. "5.4", "5.5"
    pub install_path: String,   // e.g. "C:\\Program Files\\Epic Games\\UE_5.4"
    pub is_primary: bool,
}

pub fn upsert(db: &Db, install: &UeInstall) -> UecmResult<i64> {
    let conn = db.lock().unwrap();
    conn.execute(
        "INSERT INTO machine_ue_installs (machine_id, version, install_path, is_primary)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(machine_id, version) DO UPDATE SET
            install_path = excluded.install_path,
            is_primary = excluded.is_primary,
            detected_at = CURRENT_TIMESTAMP",
        params![
            install.machine_id,
            install.version,
            install.install_path,
            install.is_primary as i32,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list_for_machine(db: &Db, machine_id: i64) -> UecmResult<Vec<UeInstall>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, machine_id, version, install_path, is_primary
         FROM machine_ue_installs WHERE machine_id = ? ORDER BY version DESC",
    )?;
    let rows = stmt.query_map(params![machine_id], |row| {
        Ok(UeInstall {
            id: Some(row.get(0)?),
            machine_id: row.get(1)?,
            version: row.get(2)?,
            install_path: row.get(3)?,
            is_primary: row.get::<_, i32>(4)? != 0,
        })
    })?;
    let mut result = Vec::new();
    for r in rows {
        result.push(r?);
    }
    Ok(result)
}

pub fn delete_for_machine(db: &Db, machine_id: i64) -> UecmResult<()> {
    let conn = db.lock().unwrap();
    conn.execute(
        "DELETE FROM machine_ue_installs WHERE machine_id = ?",
        params![machine_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{machines, open_in_memory, schema, Machine};

    fn setup() -> (Db, i64) {
        let db = open_in_memory().unwrap();
        {
            let mut conn = db.lock().unwrap();
            schema::migrate(&mut conn).unwrap();
        }
        let machine_id = machines::insert(
            &db,
            &Machine::new("RENDER-01", "192.168.10.21"),
        )
        .unwrap();
        (db, machine_id)
    }

    #[test]
    fn upsert_inserts_when_new() {
        let (db, machine_id) = setup();
        let install = UeInstall {
            id: None,
            machine_id,
            version: "5.4".to_string(),
            install_path: "C:\\UE_5.4".to_string(),
            is_primary: true,
        };
        let id = upsert(&db, &install).unwrap();
        assert!(id > 0);
    }

    #[test]
    fn upsert_updates_when_machine_version_exists() {
        let (db, machine_id) = setup();
        let install = UeInstall {
            id: None,
            machine_id,
            version: "5.4".to_string(),
            install_path: "C:\\OldPath".to_string(),
            is_primary: false,
        };
        upsert(&db, &install).unwrap();

        let updated = UeInstall {
            install_path: "C:\\NewPath".to_string(),
            is_primary: true,
            ..install
        };
        upsert(&db, &updated).unwrap();

        let installs = list_for_machine(&db, machine_id).unwrap();
        assert_eq!(installs.len(), 1);
        assert_eq!(installs[0].install_path, "C:\\NewPath");
        assert!(installs[0].is_primary);
    }

    #[test]
    fn list_for_machine_returns_all_versions_desc() {
        let (db, machine_id) = setup();
        upsert(&db, &UeInstall { id: None, machine_id, version: "5.3".into(), install_path: "C:\\A".into(), is_primary: false }).unwrap();
        upsert(&db, &UeInstall { id: None, machine_id, version: "5.5".into(), install_path: "C:\\C".into(), is_primary: false }).unwrap();
        upsert(&db, &UeInstall { id: None, machine_id, version: "5.4".into(), install_path: "C:\\B".into(), is_primary: true }).unwrap();

        let installs = list_for_machine(&db, machine_id).unwrap();
        assert_eq!(installs.len(), 3);
        assert_eq!(installs[0].version, "5.5");
        assert_eq!(installs[1].version, "5.4");
        assert_eq!(installs[2].version, "5.3");
    }

    #[test]
    fn delete_for_machine_removes_all_installs() {
        let (db, machine_id) = setup();
        upsert(&db, &UeInstall { id: None, machine_id, version: "5.4".into(), install_path: "C:\\A".into(), is_primary: true }).unwrap();
        delete_for_machine(&db, machine_id).unwrap();
        let installs = list_for_machine(&db, machine_id).unwrap();
        assert!(installs.is_empty());
    }
}
