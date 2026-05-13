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
        "003_machine_ue_installs",
        r#"
        CREATE TABLE IF NOT EXISTS machine_ue_installs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            machine_id INTEGER NOT NULL,
            version TEXT NOT NULL,
            install_path TEXT NOT NULL,
            is_primary INTEGER NOT NULL DEFAULT 0,
            detected_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(machine_id, version),
            FOREIGN KEY (machine_id) REFERENCES machines(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_machine_ue_installs_machine ON machine_ue_installs(machine_id);
        "#,
    ),
    (
        "004_machine_gpus",
        r#"
        CREATE TABLE IF NOT EXISTS machine_gpus (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            machine_id INTEGER NOT NULL,
            gpu_model TEXT NOT NULL,
            driver_version TEXT NOT NULL,
            vendor TEXT NOT NULL DEFAULT 'unknown',
            vram_mb INTEGER,
            detected_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (machine_id) REFERENCES machines(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_machine_gpus_machine ON machine_gpus(machine_id);
        "#,
    ),
    (
        "005_credentials",
        r#"
        CREATE TABLE IF NOT EXISTS credentials (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            alias TEXT NOT NULL UNIQUE,
            kind TEXT NOT NULL,
            username TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_credentials_alias ON credentials(alias);
        "#,
    ),
    (
        "006_share_configs",
        r#"
        CREATE TABLE IF NOT EXISTS share_configs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            host_machine_id INTEGER NOT NULL,
            share_name TEXT NOT NULL,
            unc_path TEXT NOT NULL,
            local_path TEXT NOT NULL,
            mode TEXT NOT NULL,
            credential_alias TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(host_machine_id, share_name),
            FOREIGN KEY (host_machine_id) REFERENCES machines(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_share_configs_host ON share_configs(host_machine_id);
        "#,
    ),
    (
        "007_diagnostics_tables",
        r#"
        CREATE TABLE IF NOT EXISTS scan_runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            scan_type TEXT NOT NULL,                    -- "ini" | "health"
            started_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            finished_at TEXT,
            machine_ids_json TEXT NOT NULL,             -- JSON array of machine ids in scope
            summary_json TEXT                           -- JSON: {critical, warning, healthy, total, ...}
        );
        CREATE INDEX IF NOT EXISTS idx_scan_runs_type_started ON scan_runs(scan_type, started_at DESC);

        CREATE TABLE IF NOT EXISTS ini_findings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            scan_run_id INTEGER NOT NULL,
            machine_id INTEGER NOT NULL,
            rule_id TEXT NOT NULL,                      -- e.g. "R001"
            severity TEXT NOT NULL,                     -- "critical" | "warning" | "healthy" | "info"
            category TEXT NOT NULL,                     -- "project" | "user" | "engine"
            file_path TEXT NOT NULL,                    -- absolute path on the machine
            section TEXT,                               -- INI [section]
            key_name TEXT,                              -- when applicable
            line_number INTEGER,                        -- 1-based, null if N/A
            snippet_before TEXT NOT NULL,               -- multi-line excerpt
            snippet_after TEXT,                         -- suggested fix (null when remove-only)
            recommended_action TEXT NOT NULL,           -- "set" | "remove" | "manual"
            recommended_value TEXT,                     -- payload for "set"
            symptom TEXT NOT NULL,                      -- user-facing description
            rationale TEXT NOT NULL,                    -- "why" explanation
            fixed_at TEXT,                              -- non-null when applied
            skipped_at TEXT,                            -- non-null when user skipped
            FOREIGN KEY (scan_run_id) REFERENCES scan_runs(id) ON DELETE CASCADE,
            FOREIGN KEY (machine_id) REFERENCES machines(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_ini_findings_run ON ini_findings(scan_run_id);
        CREATE INDEX IF NOT EXISTS idx_ini_findings_machine ON ini_findings(machine_id);
        CREATE INDEX IF NOT EXISTS idx_ini_findings_severity ON ini_findings(severity);

        CREATE TABLE IF NOT EXISTS health_check_runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            scan_run_id INTEGER NOT NULL,
            machine_id INTEGER NOT NULL,
            machine_results_json TEXT NOT NULL,         -- JSON: {check_id: {status, message, sample_output}}
            FOREIGN KEY (scan_run_id) REFERENCES scan_runs(id) ON DELETE CASCADE,
            FOREIGN KEY (machine_id) REFERENCES machines(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_health_check_runs_run ON health_check_runs(scan_run_id);
        CREATE INDEX IF NOT EXISTS idx_health_check_runs_machine ON health_check_runs(machine_id);
        CREATE UNIQUE INDEX IF NOT EXISTS uq_health_check_runs_run_machine
            ON health_check_runs(scan_run_id, machine_id);
        "#,
    ),
    (
        "008_operations_table",
        r#"
        CREATE TABLE IF NOT EXISTS operations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            action_type TEXT NOT NULL,
            target_machines TEXT NOT NULL DEFAULT '[]',
            status TEXT NOT NULL DEFAULT 'pending',
            started_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            finished_at TEXT,
            log_text TEXT,
            snapshot_blob BLOB
        );
        CREATE INDEX IF NOT EXISTS idx_operations_action_type ON operations(action_type);
        CREATE INDEX IF NOT EXISTS idx_operations_status ON operations(status);
        "#,
    ),
    (
        "009_projects_table",
        r#"
        CREATE TABLE IF NOT EXISTS projects (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            uproject_name TEXT NOT NULL,
            uproject_stem_lower TEXT NOT NULL UNIQUE,
            uproject_guid TEXT,
            display_name TEXT,
            first_seen_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            last_seen_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_projects_stem ON projects(uproject_stem_lower);
        "#,
    ),
    (
        "010_project_locations_table",
        r#"
        CREATE TABLE IF NOT EXISTS project_locations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id INTEGER NOT NULL,
            machine_id INTEGER NOT NULL,
            abs_path TEXT NOT NULL,
            uproject_path TEXT NOT NULL,
            discovery_status TEXT NOT NULL DEFAULT 'auto',
            discovered_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(project_id, machine_id),
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
            FOREIGN KEY (machine_id) REFERENCES machines(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_project_locations_project ON project_locations(project_id);
        CREATE INDEX IF NOT EXISTS idx_project_locations_machine ON project_locations(machine_id);
        "#,
    ),
    (
        "011_pso_cache_files_table",
        r#"
        CREATE TABLE IF NOT EXISTS pso_cache_files (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id INTEGER NOT NULL,
            source_machine_id INTEGER NOT NULL,
            file_path TEXT NOT NULL,
            file_name TEXT NOT NULL,
            size_bytes INTEGER NOT NULL DEFAULT 0,
            gpu_signature TEXT NOT NULL,
            ue_version TEXT,
            collected_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(project_id, source_machine_id, file_name),
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
            FOREIGN KEY (source_machine_id) REFERENCES machines(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_pso_cache_files_project ON pso_cache_files(project_id);
        CREATE INDEX IF NOT EXISTS idx_pso_cache_files_signature ON pso_cache_files(gpu_signature);
        "#,
    ),
    (
        "012_pso_distributions_table",
        r#"
        CREATE TABLE IF NOT EXISTS pso_distributions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            pso_cache_file_id INTEGER NOT NULL,
            target_machine_id INTEGER NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            bytes_copied INTEGER NOT NULL DEFAULT 0,
            distributed_at TEXT,
            error_message TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(pso_cache_file_id, target_machine_id),
            FOREIGN KEY (pso_cache_file_id) REFERENCES pso_cache_files(id) ON DELETE CASCADE,
            FOREIGN KEY (target_machine_id) REFERENCES machines(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_pso_distributions_file ON pso_distributions(pso_cache_file_id);
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

    #[test]
    fn migrate_creates_machine_ue_installs_table() {
        let db = open_in_memory().unwrap();
        let mut conn = db.lock().unwrap();
        migrate(&mut conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='machine_ue_installs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn migrate_creates_machine_gpus_table() {
        let db = open_in_memory().unwrap();
        let mut conn = db.lock().unwrap();
        migrate(&mut conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='machine_gpus'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn migrate_creates_credentials_table() {
        let db = open_in_memory().unwrap();
        let mut conn = db.lock().unwrap();
        migrate(&mut conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='credentials'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn migrate_creates_share_configs_table() {
        let db = open_in_memory().unwrap();
        let mut conn = db.lock().unwrap();
        migrate(&mut conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='share_configs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn migrate_creates_scan_runs_table() {
        let db = open_in_memory().unwrap();
        let mut conn = db.lock().unwrap();
        migrate(&mut conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='scan_runs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn migrate_creates_ini_findings_table() {
        let db = open_in_memory().unwrap();
        let mut conn = db.lock().unwrap();
        migrate(&mut conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='ini_findings'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn migrate_creates_health_check_runs_table() {
        let db = open_in_memory().unwrap();
        let mut conn = db.lock().unwrap();
        migrate(&mut conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='health_check_runs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
}
