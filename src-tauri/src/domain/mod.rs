//! Offline financial services. Each write and its retry receipt share an
//! immediate SQLite transaction. Reporting reads use a consistent snapshot.

pub mod cards;
pub mod ledger;
pub mod legacy;
pub mod planning;
pub mod review;

use crate::commands::wire::{positive_yen, text, yen};
use crate::db::Database;
use crate::error::{AppError, ErrorCategory, codes};
use chrono::{NaiveDate, Utc};
use rusqlite::{Connection, OptionalExtension, Params, Transaction, TransactionBehavior, params};
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub use crate::commands::wire::{Page, WriteResponse};

pub struct Finance<'db> {
    db: &'db Database,
}

impl<'db> Finance<'db> {
    pub fn new(db: &'db Database) -> Self {
        Self { db }
    }

    fn read<T>(
        &self,
        read: impl FnOnce(&Connection) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        self.db.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            let result = read(&tx)?;
            tx.commit()?;
            Ok(result)
        })
    }

    fn write<P: Serialize, T: Serialize + DeserializeOwned>(
        &self,
        key: Uuid,
        operation: &str,
        payload: &P,
        write: impl FnOnce(&Transaction<'_>) -> Result<T, AppError>,
    ) -> Result<WriteResponse<T>, AppError> {
        let hash = digest(payload)?;
        self.db.with_conn_mut(|conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let receipt = tx.query_row(
                "SELECT operation,payload_hash,result_json FROM write_receipts WHERE idempotency_key=?1",
                [key.to_string()], |r| Ok((r.get::<_,String>(0)?, r.get::<_,String>(1)?, r.get::<_,String>(2)?)),
            ).optional()?;
            if let Some((old_operation, old_hash, json)) = receipt {
                if old_operation != operation || old_hash != hash {
                    return Err(conflict(codes::IDEMPOTENCY_CONFLICT, "This write key has already been used."));
                }
                return Ok(WriteResponse { is_replay: true, result: serde_json::from_str(&json).map_err(json_error)? });
            }
            let result = write(&tx)?;
            tx.execute(
                "INSERT INTO write_receipts VALUES (?1,?2,?3,?4,?5)",
                params![key.to_string(), operation, hash, serde_json::to_string(&result).map_err(json_error)?, now()],
            )?;
            tx.commit()?;
            Ok(WriteResponse { is_replay: false, result })
        })
    }
}

fn rows<T>(
    conn: &Connection,
    sql: &str,
    bindings: impl Params,
    map: impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
) -> Result<Vec<T>, AppError> {
    Ok(conn
        .prepare(sql)?
        .query_map(bindings, map)?
        .collect::<Result<_, _>>()?)
}

fn exists(conn: &Connection, sql: &str, bindings: impl Params) -> Result<bool, AppError> {
    Ok(conn.prepare(sql)?.exists(bindings)?)
}

fn amount(conn: &Connection, sql: &str, bindings: impl Params) -> Result<i64, AppError> {
    yen(conn.query_row(sql, bindings, |r| r.get(0))?, "amount")
}

fn total(values: impl IntoIterator<Item = i64>) -> Result<i64, AppError> {
    let sum: i128 = values.into_iter().map(i128::from).sum();
    yen(
        i64::try_from(sum).map_err(|_| invalid("amount", "The amount is too large."))?,
        "amount",
    )
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
}

fn date(date: NaiveDate) -> Result<(), AppError> {
    use chrono::Datelike;
    if (1..=9999).contains(&date.year()) {
        return Ok(());
    }
    Err(AppError::invalid(
        codes::INVALID_DATE,
        "date",
        "Enter a date in years 0001–9999.",
    ))
}

fn range(start: NaiveDate, end: NaiveDate, max_days: i64) -> Result<(), AppError> {
    date(start)?;
    date(end)?;
    if end < start || (end - start).num_days() > max_days {
        return Err(AppError::invalid(
            codes::INVALID_DATE,
            "to",
            "Invalid date range.",
        ));
    }
    Ok(())
}

fn label(value: &str, field: &str, max: usize) -> Result<String, AppError> {
    Ok(text(Some(value), field, max, true)?.expect("required text"))
}

fn digest(value: &impl Serialize) -> Result<String, AppError> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).map_err(json_error)?)
    ))
}

fn json_error(_: serde_json::Error) -> AppError {
    AppError::internal(codes::INTERNAL, "Financial data could not be serialized.")
}

fn invalid(field: &str, message: &str) -> AppError {
    AppError::invalid(codes::INVALID_REQUEST, field, message)
}

fn conflict(code: &str, message: &str) -> AppError {
    AppError::new(code, message, ErrorCategory::Conflict)
}

fn blocked(message: &str) -> AppError {
    conflict(codes::INVALID_REQUEST, message)
}

fn missing() -> AppError {
    rusqlite::Error::QueryReturnedNoRows.into()
}
