//! Startup, single-writer locking and shutdown for the local profile (#6).
//!
//! Boot order: create the private app-data directory → take the exclusive
//! instance lock → open SQLite → run migrations. Any failure produces
//! [`Boot::Failed`] with a stable, data-free [`AppError`] instead of a crash,
//! and every data command refuses to run until boot is [`Boot::Ready`].

use crate::db::Database;
use crate::error::{AppError, ErrorCategory, codes};
use serde::Serialize;
use std::fs::{File, TryLockError};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const DATABASE_FILE: &str = "finance.db";
pub const LOCK_FILE: &str = "finance.lock";

/// Applies schema migrations. Owned by the database layer (#5); it must apply each
/// migration inside a transaction and return `DATABASE_TOO_NEW` for a newer schema.
pub type Migrator = fn(&Database) -> Result<(), AppError>;

/// Placeholder until #5 lands `db::migrations`; the scaffold has no domain schema.
pub fn no_domain_schema_yet(_: &Database) -> Result<(), AppError> {
    Ok(())
}

/// Exclusive advisory lock held for the lifetime of the process. The OS releases
/// it if the process dies, so a crash never leaves a stale lock behind.
#[derive(Debug)]
pub struct InstanceLock {
    _file: File,
}

pub struct Ready {
    pub db: Arc<Database>,
    _lock: InstanceLock,
}

pub enum Boot {
    Ready(Ready),
    Failed(AppError),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    /// `ready` or `failed`. Financial screens render only when `ready`.
    pub state: &'static str,
    pub error: Option<AppError>,
}

impl Boot {
    pub fn db(&self) -> Result<&Arc<Database>, AppError> {
        match self {
            Boot::Ready(ready) => Ok(&ready.db),
            Boot::Failed(_) => Err(AppError::storage(
                codes::STORAGE_NOT_READY,
                "Local storage is not available.",
            )),
        }
    }

    pub fn status(&self) -> AppStatus {
        match self {
            Boot::Ready(_) => AppStatus {
                state: "ready",
                error: None,
            },
            Boot::Failed(err) => AppStatus {
                state: "failed",
                error: Some(err.clone()),
            },
        }
    }

    /// Flush the WAL into the main database file on a clean exit. Interrupted
    /// exits stay safe: SQLite replays or discards the WAL on next open.
    pub fn shutdown(&self) {
        if let Boot::Ready(ready) = self {
            let result = ready.db.with_conn(|conn| {
                conn.query_row("PRAGMA wal_checkpoint(TRUNCATE);", [], |_| Ok(()))?;
                Ok(())
            });
            if let Err(err) = result {
                log::warn!("WAL checkpoint on exit failed: {}", err.log_summary());
            }
        }
    }
}

pub fn boot(data_dir: &Path, migrate: Migrator) -> Boot {
    match try_boot(data_dir, migrate) {
        Ok(ready) => Boot::Ready(ready),
        Err(err) => {
            log::error!("Startup failed: {}", err.log_summary());
            Boot::Failed(err)
        }
    }
}

fn try_boot(data_dir: &Path, migrate: Migrator) -> Result<Ready, AppError> {
    create_private_dir(data_dir)?;
    let lock = acquire_instance_lock(data_dir)?;

    let db_path = data_dir.join(DATABASE_FILE);
    // Restrict before SQLite opens it: `-wal`/`-shm` inherit the database file's
    // mode when created, so a fresh database must already be owner-only.
    File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&db_path)?;
    restrict_database_files(&db_path)?;
    let db = Database::open(&db_path).map_err(|_| {
        AppError::storage(
            codes::STORAGE_UNAVAILABLE,
            "The local database could not be opened.",
        )
    })?;
    // Repairs sidecars created by earlier builds with the default umask.
    restrict_database_files(&db_path)?;

    migrate(&db).map_err(|err| {
        if err.code == codes::DATABASE_TOO_NEW {
            err
        } else {
            AppError::storage(
                codes::MIGRATION_FAILED,
                "The local database could not be upgraded. No changes were applied. \
                 Quit and reopen the app; if this continues, restore from a backup.",
            )
        }
    })?;

    Ok(Ready {
        db: Arc::new(db),
        _lock: lock,
    })
}

pub fn acquire_instance_lock(data_dir: &Path) -> Result<InstanceLock, AppError> {
    let path: PathBuf = data_dir.join(LOCK_FILE);
    let file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)?;
    restrict_file(&path)?;
    match file.try_lock() {
        Ok(()) => Ok(InstanceLock { _file: file }),
        Err(TryLockError::WouldBlock) => Err(AppError::new(
            codes::INSTANCE_LOCKED,
            "Finance Desktop is already open in another window or process. \
             Close it before opening the app again.",
            ErrorCategory::Conflict,
        )),
        Err(TryLockError::Error(err)) => Err(err.into()),
    }
}

fn create_private_dir(dir: &Path) -> Result<(), AppError> {
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn restrict_database_files(db_path: &Path) -> Result<(), AppError> {
    restrict_file(db_path)?;
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut sidecar = db_path.as_os_str().to_owned();
        sidecar.push(suffix);
        let sidecar = PathBuf::from(sidecar);
        if sidecar.exists() {
            restrict_file(&sidecar)?;
        }
    }
    Ok(())
}

/// Owner-only read/write.
fn restrict_file(path: &Path) -> Result<(), AppError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn failing_migration(db: &Database) -> Result<(), AppError> {
        db.with_conn(|conn| {
            conn.execute_batch(
                "BEGIN;
                 CREATE TABLE half_applied (id INTEGER PRIMARY KEY);
                 INSERT INTO missing_table VALUES (1);
                 COMMIT;",
            )
            .inspect_err(|_| {
                let _ = conn.execute_batch("ROLLBACK;");
            })?;
            Ok(())
        })
    }

    fn too_new(_: &Database) -> Result<(), AppError> {
        Err(AppError::storage(
            codes::DATABASE_TOO_NEW,
            "This database was created by a newer version of Finance Desktop.",
        ))
    }

    #[test]
    fn boots_ready_with_wal_and_lock() {
        let dir = tempdir().unwrap();
        let boot = boot(dir.path(), no_domain_schema_yet);
        assert_eq!(boot.status().state, "ready");
        assert!(boot.db().unwrap().get_status().unwrap().is_wal_enabled);
        assert!(dir.path().join(LOCK_FILE).exists());
    }

    #[test]
    fn second_instance_gets_clear_conflict_and_first_is_unaffected() {
        let dir = tempdir().unwrap();
        let first = boot(dir.path(), no_domain_schema_yet);
        let second = boot(dir.path(), no_domain_schema_yet);

        let status = second.status();
        assert_eq!(status.state, "failed");
        let err = status.error.unwrap();
        assert_eq!(err.code, codes::INSTANCE_LOCKED);
        assert_eq!(err.category, ErrorCategory::Conflict);
        assert_eq!(second.db().err().unwrap().code, codes::STORAGE_NOT_READY);
        assert!(first.db().is_ok());
    }

    #[test]
    fn lock_is_released_when_first_instance_exits() {
        let dir = tempdir().unwrap();
        let first = boot(dir.path(), no_domain_schema_yet);
        first.shutdown();
        drop(first);
        assert_eq!(
            boot(dir.path(), no_domain_schema_yet).status().state,
            "ready"
        );
    }

    #[test]
    fn failed_migration_blocks_data_access_and_leaves_schema_unchanged() {
        let dir = tempdir().unwrap();
        let boot_result = boot(dir.path(), failing_migration);
        let err = boot_result.status().error.unwrap();
        assert_eq!(err.code, codes::MIGRATION_FAILED);
        assert_eq!(err.category, ErrorCategory::Storage);
        assert!(!err.message.contains("missing_table"));
        assert!(boot_result.db().is_err());
        drop(boot_result);

        let conn = rusqlite::Connection::open(dir.path().join(DATABASE_FILE)).unwrap();
        let tables: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name = 'half_applied'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tables, 0, "failed migration must roll back");
    }

    #[test]
    fn newer_schema_is_reported_distinctly() {
        let dir = tempdir().unwrap();
        let err = boot(dir.path(), too_new).status().error.unwrap();
        assert_eq!(err.code, codes::DATABASE_TOO_NEW);
    }

    #[test]
    fn interrupted_session_reopens_with_committed_data() {
        let dir = tempdir().unwrap();
        {
            let first = boot(dir.path(), no_domain_schema_yet);
            first
                .db()
                .unwrap()
                .with_conn(|c| {
                    c.execute_batch("CREATE TABLE t (v INTEGER); INSERT INTO t VALUES (42);")?;
                    Ok(())
                })
                .unwrap();
            // Dropped without shutdown(): WAL is not checkpointed, as after a crash.
        }
        let second = boot(dir.path(), no_domain_schema_yet);
        let v: i64 = second
            .db()
            .unwrap()
            .with_conn(|c| Ok(c.query_row("SELECT v FROM t", [], |r| r.get(0))?))
            .unwrap();
        assert_eq!(v, 42);
    }

    #[cfg(unix)]
    #[test]
    fn storage_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let data = dir.path().join("profile");
        let _boot = boot(&data, no_domain_schema_yet);
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&data), 0o700);
        assert_eq!(mode(&data.join(DATABASE_FILE)), 0o600);
        assert_eq!(mode(&data.join(LOCK_FILE)), 0o600);
        // A fresh WAL database must not leak through world-readable sidecars.
        assert_eq!(mode(&data.join("finance.db-wal")), 0o600);
        assert_eq!(mode(&data.join("finance.db-shm")), 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn existing_world_readable_sidecars_are_repaired() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        drop(boot(dir.path(), no_domain_schema_yet));
        let wal = dir.path().join("finance.db-wal");
        std::fs::write(&wal, b"").unwrap();
        std::fs::set_permissions(&wal, std::fs::Permissions::from_mode(0o644)).unwrap();
        let _boot = boot(dir.path(), no_domain_schema_yet);
        assert_eq!(
            std::fs::metadata(&wal).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
