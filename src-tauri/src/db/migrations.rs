use super::Database;
use crate::error::{AppError, codes};
use rusqlite::{TransactionBehavior, params};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const MIGRATIONS: &[(&str, &str)] = &[
    ("core_ledger", include_str!("../../migrations/001_core.sql")),
    (
        "planning_and_review",
        include_str!("../../migrations/002_planning_and_review.sql"),
    ),
];

pub fn migrate(db: &Database) -> Result<(), AppError> {
    db.with_conn_mut(|conn| {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let latest: i64 = tx.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM _schema_migrations", [], |r| r.get(0),
        )?;
        if latest > MIGRATIONS.len() as i64 {
            return Err(AppError::storage(codes::DATABASE_TOO_NEW, "This database requires a newer app."));
        }
        let checksum_column: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('_schema_migrations') WHERE name='checksum')",
            [], |r| r.get(0),
        )?;
        let count: i64 = tx.query_row("SELECT COUNT(*) FROM _schema_migrations", [], |r| r.get(0))?;
        if latest != count || (count > 0 && !checksum_column) {
            return Err(invalid_schema());
        }
        for (index, (name, sql)) in MIGRATIONS.iter().take(latest as usize).enumerate() {
            let installed: (String, Option<String>) = tx.query_row(
                "SELECT name, checksum FROM _schema_migrations WHERE version=?1", [index + 1],
                |r| Ok((r.get(0)?, r.get(1)?)),
            ).map_err(|_| invalid_schema())?;
            if installed.0 != *name || installed.1.as_deref() != Some(&checksum(sql)) {
                return Err(invalid_schema());
            }
        }
        if !checksum_column {
            tx.execute_batch("ALTER TABLE _schema_migrations ADD COLUMN checksum TEXT")?;
        }
        for (index, (name, sql)) in MIGRATIONS.iter().enumerate().skip(latest as usize) {
            tx.execute_batch(sql)?;
            if index == 0 {
                tx.execute("INSERT INTO local_profile (singleton,id) VALUES (1,?1)", [Uuid::new_v4().to_string()])?;
            }
            tx.execute(
                "INSERT INTO _schema_migrations(version,name,applied_at,checksum)
                 VALUES (?1,?2,?3,?4)",
                params![index + 1, name, chrono::Utc::now().to_rfc3339(), checksum(sql)],
            )?;
        }
        let fk_issues = tx.prepare("PRAGMA foreign_key_check")?.exists([])?;
        let integrity: String = tx.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        if fk_issues || integrity != "ok" {
            return Err(invalid_schema());
        }
        tx.commit()?;
        Ok(())
    })
}

fn checksum(sql: &str) -> String {
    format!("{:x}", Sha256::digest(sql.as_bytes()))
}

fn invalid_schema() -> AppError {
    AppError::storage(
        codes::MIGRATION_FAILED,
        "The local database schema could not be verified.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn upgrades_real_version_one_and_keeps_profile_and_accounts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("finance.sqlite");
        let account_id = Uuid::new_v4().to_string();
        let profile_id = Uuid::new_v4().to_string();
        {
            let db = Database::open(&path).unwrap();
            db.with_conn_mut(|conn| {
                let tx = conn.transaction()?;
                let (name, sql) = MIGRATIONS[0];
                tx.execute_batch("ALTER TABLE _schema_migrations ADD COLUMN checksum TEXT")?;
                tx.execute_batch(sql)?;
                tx.execute("INSERT INTO _schema_migrations VALUES (1,?1,'2026-01-01',?2)",params![name,checksum(sql)])?;
                tx.execute("INSERT INTO local_profile (singleton,id) VALUES (1,?1)", [&profile_id])?;
                tx.execute("INSERT INTO accounts (id,name,kind,created_at) VALUES (?1,'Cash','CASH','2026-01-01')", [&account_id])?;
                tx.commit()?;
                Ok(())
            }).unwrap();
        }
        for _ in 0..2 {
            let db = Database::open(&path).unwrap();
            migrate(&db).unwrap();
            assert_eq!(db.current_migration_version().unwrap(), 2);
            db.with_conn(|conn| {
                assert_eq!(
                    conn.query_row("SELECT id FROM local_profile", [], |r| r
                        .get::<_, String>(0))?,
                    profile_id
                );
                assert_eq!(
                    conn.query_row("SELECT id FROM accounts", [], |r| r.get::<_, String>(0))?,
                    account_id
                );
                let foreign_keys: i64 = conn.query_row("PRAGMA foreign_keys", [], |r| r.get(0))?;
                assert_eq!(foreign_keys, 1);
                Ok(())
            })
            .unwrap();
        }
    }

    #[test]
    fn failed_second_migration_rolls_back_the_entire_pending_batch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("finance.sqlite");
        {
            let scaffold = Database::open(&path).unwrap();
            scaffold.with_conn(|conn| {
                conn.execute_batch("CREATE TABLE card_settings (sentinel TEXT); INSERT INTO card_settings VALUES ('keep me');")?;
                Ok(())
            }).unwrap();
            assert!(migrate(&scaffold).is_err());
        }
        let raw = Connection::open(path).unwrap();
        assert_eq!(
            raw.query_row("SELECT sentinel FROM card_settings", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "keep me"
        );
        assert_eq!(
            raw.query_row("SELECT COUNT(*) FROM _schema_migrations", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert!(
            !raw.prepare("SELECT 1 FROM sqlite_master WHERE name='local_profile'")
                .unwrap()
                .exists([])
                .unwrap()
        );
        assert!(
            !raw.prepare(
                "SELECT 1 FROM pragma_table_info('_schema_migrations') WHERE name='checksum'"
            )
            .unwrap()
            .exists([])
            .unwrap()
        );
    }

    #[test]
    fn rejects_a_modified_checksum_and_newer_schema() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(&dir.path().join("finance.sqlite")).unwrap();
        migrate(&db).unwrap();
        db.with_conn(|conn| {
            conn.execute(
                "UPDATE _schema_migrations SET checksum='wrong' WHERE version=1",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        assert_eq!(migrate(&db).unwrap_err().code, codes::MIGRATION_FAILED);
        db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO _schema_migrations VALUES (3,'future','2026-01-01','future')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        assert_eq!(migrate(&db).unwrap_err().code, codes::DATABASE_TOO_NEW);
        assert_eq!(db.current_migration_version().unwrap(), 3);
    }
}
