//! Database schema migrations. Runs idempotently on app startup.
//! Each migration is wrapped in a transaction.

use crate::error::UecmResult;
use rusqlite::Connection;

const MIGRATIONS: &[(&str, &str)] = &[
    (
        "001_machines_table",
        r#"
        CREATE TABLE IF NOT EXISTS machines (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            hostname TEXT NOT NULL,
            ip TEXT NOT NULL UNIQUE,
            role TEXT NOT NULL DEFAULT 'unknown',
            status TEXT NOT NULL DEFAULT 'unknown',
            last_seen_at TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_machines_status ON machines(status);
        "#,
    ),
    (
        "002_migrations_table",
        r#"
        CREATE TABLE IF NOT EXISTS schema_migrations (
            name TEXT PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        "#,
    ),
];

pub fn migrate(conn: &mut Connection) -> UecmResult<()> {
    // Bootstrap: ensure migrations table exists.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            name TEXT PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;

    for (name, sql) in MIGRATIONS {
        let already_applied: bool = conn
            .query_row(
                "SELECT 1 FROM schema_migrations WHERE name = ?",
                [name],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if already_applied {
            continue;
        }

        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.execute(
            "INSERT INTO schema_migrations (name) VALUES (?)",
            [name],
        )?;
        tx.commit()?;
        tracing::info!("applied migration: {}", name);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::open_in_memory;

    #[test]
    fn migrate_creates_machines_table() {
        let db = open_in_memory().unwrap();
        let mut conn = db.lock().unwrap();
        migrate(&mut conn).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='machines'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn migrate_is_idempotent() {
        let db = open_in_memory().unwrap();
        let mut conn = db.lock().unwrap();
        migrate(&mut conn).unwrap();
        migrate(&mut conn).unwrap(); // run twice
        // Should not error.
    }

    #[test]
    fn migrate_records_applied_migrations() {
        let db = open_in_memory().unwrap();
        let mut conn = db.lock().unwrap();
        migrate(&mut conn).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM schema_migrations",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(count >= 1);
    }
}
