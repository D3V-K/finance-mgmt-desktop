use crate::error::{AppError, sanitize_error_message};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StorageStatus {
    pub status: String,
    pub database_path: String,
    pub is_wal_enabled: bool,
    pub migration_version: u32,
}

pub struct Database {
    conn: Mutex<Connection>,
    db_path: String,
}

impl Database {
    /// Open a persistent SQLite database at the given path with WAL mode and foreign keys enabled.
    pub fn open(path: &Path) -> Result<Self, AppError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path)?;
        let db = Self::configure_connection(conn, sanitize_error_message(&path.to_string_lossy()))?;
        Ok(db)
    }

    /// Open an in-memory SQLite database for testing.
    pub fn open_in_memory() -> Result<Self, AppError> {
        let conn = Connection::open_in_memory()?;
        let db = Self::configure_connection(conn, ":memory:".to_string())?;
        Ok(db)
    }

    fn configure_connection(conn: Connection, db_path: String) -> Result<Self, AppError> {
        // Enforce foreign key constraints
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        // Enforce reasonable busy timeout for concurrent desktop interactions
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;

        // Enable write-ahead logging (WAL) for persistent DBs to provide concurrent read/write
        let is_memory = db_path == ":memory:";
        if !is_memory {
            let journal_mode: String =
                conn.query_row("PRAGMA journal_mode = WAL;", [], |row| row.get(0))?;
            log::info!("SQLite journal mode initialized: {}", journal_mode);
            conn.execute_batch("PRAGMA synchronous = NORMAL;")?;
        }

        // Initialize schema migration tracking table
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS _schema_migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TEXT NOT NULL
            );",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
            db_path,
        })
    }

    /// Get current applied migration version from `_schema_migrations`.
    pub fn current_migration_version(&self) -> Result<u32, AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::storage("MUTEX_POISONED", "Failed to acquire database lock"))?;

        let version: Option<u32> = conn
            .query_row("SELECT MAX(version) FROM _schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap_or(None);

        Ok(version.unwrap_or(0))
    }

    /// Execute a closure with exclusive access to the underlying SQLite connection.
    pub fn with_conn<F, T>(&self, f: F) -> Result<T, AppError>
    where
        F: FnOnce(&Connection) -> Result<T, AppError>,
    {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::storage("MUTEX_POISONED", "Failed to acquire database lock"))?;
        f(&conn)
    }

    /// Retrieve the live storage status for diagnostics and health monitoring.
    pub fn get_status(&self) -> Result<StorageStatus, AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::storage("MUTEX_POISONED", "Failed to acquire database lock"))?;

        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode;", [], |row| row.get(0))
            .unwrap_or_else(|_| "unknown".to_string());

        let is_wal_enabled = journal_mode.to_uppercase() == "WAL" || self.db_path == ":memory:";

        let version: Option<u32> = conn
            .query_row("SELECT MAX(version) FROM _schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap_or(None);

        Ok(StorageStatus {
            status: "ready".to_string(),
            database_path: self.db_path.clone(),
            is_wal_enabled,
            migration_version: version.unwrap_or(0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_in_memory_database_initialization() {
        let db = Database::open_in_memory().expect("in-memory db open failed");
        let status = db.get_status().expect("get_status failed");
        assert_eq!(status.status, "ready");
        assert_eq!(status.migration_version, 0);
        assert_eq!(status.database_path, ":memory:");
    }

    #[test]
    fn test_file_database_initialization_with_wal() {
        let dir = tempdir().expect("tempdir failed");
        let db_file = dir.path().join("finance.db");

        let db = Database::open(&db_file).expect("file db open failed");
        let status = db.get_status().expect("get_status failed");
        assert_eq!(status.status, "ready");
        assert!(status.is_wal_enabled);
        assert_eq!(status.migration_version, 0);

        // Record a mock migration and verify version
        db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO _schema_migrations (version, name, applied_at) VALUES (?1, ?2, datetime('now'))",
                rusqlite::params![1, "initial_schema"],
            )?;
            Ok(())
        })
        .expect("insert migration failed");

        assert_eq!(db.current_migration_version().unwrap(), 1);
        let updated_status = db.get_status().unwrap();
        assert_eq!(updated_status.migration_version, 1);
    }
}
