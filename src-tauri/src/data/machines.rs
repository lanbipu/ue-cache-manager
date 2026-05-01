//! CRUD operations for the `machines` table.

use crate::data::Db;
use crate::error::UecmResult;
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Machine {
    pub id: Option<i64>,
    pub hostname: String,
    pub ip: String,
    pub role: String,        // "host" | "render" | "dev" | "editor" | "unknown"
    pub status: String,      // "online" | "offline" | "unknown"
    pub last_seen_at: Option<String>,
}

impl Machine {
    pub fn new(hostname: &str, ip: &str) -> Self {
        Self {
            id: None,
            hostname: hostname.to_string(),
            ip: ip.to_string(),
            role: "unknown".to_string(),
            status: "unknown".to_string(),
            last_seen_at: None,
        }
    }
}

pub fn insert(db: &Db, machine: &Machine) -> UecmResult<i64> {
    let conn = db.lock().unwrap();
    conn.execute(
        "INSERT INTO machines (hostname, ip, role, status, last_seen_at) VALUES (?, ?, ?, ?, ?)",
        params![
            machine.hostname,
            machine.ip,
            machine.role,
            machine.status,
            machine.last_seen_at,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list_all(db: &Db) -> UecmResult<Vec<Machine>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, hostname, ip, role, status, last_seen_at FROM machines ORDER BY hostname",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Machine {
            id: Some(row.get(0)?),
            hostname: row.get(1)?,
            ip: row.get(2)?,
            role: row.get(3)?,
            status: row.get(4)?,
            last_seen_at: row.get(5)?,
        })
    })?;
    let mut result = Vec::new();
    for r in rows {
        result.push(r?);
    }
    Ok(result)
}

pub fn delete(db: &Db, id: i64) -> UecmResult<()> {
    let conn = db.lock().unwrap();
    conn.execute("DELETE FROM machines WHERE id = ?", params![id])?;
    Ok(())
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
    fn insert_returns_new_id() {
        let db = setup();
        let m = Machine::new("RENDER-01", "192.168.10.21");
        let id = insert(&db, &m).unwrap();
        assert!(id > 0);
    }

    #[test]
    fn list_all_returns_inserted_machines_in_alphabetical_order() {
        let db = setup();
        insert(&db, &Machine::new("RENDER-02", "192.168.10.22")).unwrap();
        insert(&db, &Machine::new("RENDER-01", "192.168.10.21")).unwrap();

        let machines = list_all(&db).unwrap();
        assert_eq!(machines.len(), 2);
        assert_eq!(machines[0].hostname, "RENDER-01");
        assert_eq!(machines[1].hostname, "RENDER-02");
    }

    #[test]
    fn delete_removes_machine() {
        let db = setup();
        let id = insert(&db, &Machine::new("RENDER-01", "192.168.10.21")).unwrap();
        delete(&db, id).unwrap();
        let machines = list_all(&db).unwrap();
        assert!(machines.is_empty());
    }

    #[test]
    fn duplicate_ip_returns_database_error() {
        let db = setup();
        insert(&db, &Machine::new("A", "192.168.10.1")).unwrap();
        let result = insert(&db, &Machine::new("B", "192.168.10.1"));
        assert!(result.is_err());
    }
}
