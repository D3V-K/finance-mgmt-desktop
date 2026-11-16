use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable error codes shared with the renderer (`src/lib/types.ts` `ErrorCode`).
///
/// The renderer branches on these codes and never parses `message`. Codes for
/// domain rules (missing opening, archived account, unresolved mapping) are
/// proposed here for the v1 contract so both layers spell them identically;
/// the domain layer (#7) is responsible for raising them.
pub mod codes {
    // Request validation (transport layer, #6)
    pub const INVALID_ID: &str = "INVALID_ID";
    pub const INVALID_DATE: &str = "INVALID_DATE";
    pub const INVALID_AMOUNT: &str = "INVALID_AMOUNT";
    pub const INVALID_TEXT: &str = "INVALID_TEXT";
    pub const INVALID_PAGE: &str = "INVALID_PAGE";
    pub const INVALID_REQUEST: &str = "INVALID_REQUEST";

    // Domain rules (raised by #7; listed here so codes stay stable)
    pub const MISSING_OPENING: &str = "MISSING_OPENING";
    pub const ACCOUNT_ARCHIVED: &str = "ACCOUNT_ARCHIVED";
    pub const UNRESOLVED_MAPPING: &str = "UNRESOLVED_MAPPING";
    pub const IDEMPOTENCY_CONFLICT: &str = "IDEMPOTENCY_CONFLICT";
    pub const NOT_FOUND: &str = "NOT_FOUND";

    // Storage and lifecycle (#6)
    pub const STORAGE_FAILURE: &str = "STORAGE_FAILURE";
    pub const STORAGE_NOT_READY: &str = "STORAGE_NOT_READY";
    pub const STORAGE_UNAVAILABLE: &str = "STORAGE_UNAVAILABLE";
    pub const MIGRATION_FAILED: &str = "MIGRATION_FAILED";
    pub const DATABASE_TOO_NEW: &str = "DATABASE_TOO_NEW";
    pub const INSTANCE_LOCKED: &str = "INSTANCE_LOCKED";

    pub const INTERNAL: &str = "INTERNAL";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    Storage,
    Validation,
    NotFound,
    Conflict,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub category: ErrorCategory,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

impl AppError {
    pub fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        category: ErrorCategory,
    ) -> Self {
        Self {
            code: code.into(),
            message: sanitize_error_message(&message.into()),
            category,
            field: None,
        }
    }

    pub fn with_field(
        code: impl Into<String>,
        message: impl Into<String>,
        category: ErrorCategory,
        field: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            message: sanitize_error_message(&message.into()),
            category,
            field: Some(field.into()),
        }
    }

    pub fn storage(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(code, message, ErrorCategory::Storage)
    }

    pub fn validation(
        code: impl Into<String>,
        message: impl Into<String>,
        field: Option<String>,
    ) -> Self {
        Self {
            code: code.into(),
            message: sanitize_error_message(&message.into()),
            category: ErrorCategory::Validation,
            field,
        }
    }

    /// Validation error tied to a request field (camelCase wire name).
    pub fn invalid(code: &str, field: &str, message: impl Into<String>) -> Self {
        Self::validation(code, message, Some(field.to_string()))
    }

    pub fn internal(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(code, message, ErrorCategory::Internal)
    }

    /// Log-safe one-line summary: code and category only, never the message.
    pub fn log_summary(&self) -> String {
        format!("{} ({:?})", self.code, self.category)
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let category = format!("{:?}", self.category).to_lowercase();
        match self.field {
            Some(ref field) => write!(
                f,
                "[{}] {}: {} (field: {})",
                self.code, self.message, category, field
            ),
            None => write!(f, "[{}] {}: {}", self.code, self.message, category),
        }
    }
}

impl std::error::Error for AppError {}

/// Redact every home-directory segment so messages never disclose a username or path.
pub fn sanitize_error_message(msg: &str) -> String {
    let mut cleaned = msg.to_string();
    for prefix in [
        "/Users/",
        "/home/",
        "C:\\Users\\",
        "c:\\users\\",
        "C:\\users\\",
    ] {
        let sep = if prefix.ends_with('\\') { '\\' } else { '/' };
        while let Some(start) = cleaned.find(prefix) {
            let after = start + prefix.len();
            let end = cleaned[after..]
                .find(sep)
                .map(|i| after + i)
                .unwrap_or(cleaned.len());
            cleaned.replace_range(start..end, "[USER_HOME]");
        }
    }
    cleaned
}

impl From<rusqlite::Error> for AppError {
    /// SQLite messages can quote SQL, column names or values; they are never forwarded.
    /// The renderer receives a stable code and a generic, data-free message.
    fn from(err: rusqlite::Error) -> Self {
        match &err {
            rusqlite::Error::QueryReturnedNoRows => AppError::new(
                codes::NOT_FOUND,
                "The requested record was not found.",
                ErrorCategory::NotFound,
            ),
            rusqlite::Error::SqliteFailure(e, _)
                if matches!(
                    e.code,
                    rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
                ) =>
            {
                AppError::storage(
                    codes::STORAGE_UNAVAILABLE,
                    "Local storage is busy. Try again in a moment.",
                )
            }
            _ => AppError::storage(codes::STORAGE_FAILURE, "A local storage operation failed."),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(_: std::io::Error) -> Self {
        AppError::storage(
            codes::STORAGE_UNAVAILABLE,
            "Local application storage could not be accessed.",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitization_removes_user_paths() {
        let raw = "Failed to open file at /Users/john_doe/Library/Application Support/finance.db";
        let sanitized = sanitize_error_message(raw);
        assert!(!sanitized.contains("john_doe"));
        assert!(sanitized.contains("[USER_HOME]/Library/Application Support/finance.db"));
    }

    #[test]
    fn sanitization_redacts_every_occurrence() {
        let raw = "copy /Users/alice/a.db to /home/bob/b.db and /Users/carol";
        let sanitized = sanitize_error_message(raw);
        for name in ["alice", "bob", "carol"] {
            assert!(!sanitized.contains(name), "{sanitized}");
        }
    }

    #[test]
    fn sanitization_windows_paths() {
        let raw = r"Could not open C:\Users\alice\AppData\Roaming\finance.db";
        let sanitized = sanitize_error_message(raw);
        assert!(!sanitized.contains("alice"));
        assert!(sanitized.contains(r"[USER_HOME]\AppData\Roaming\finance.db"));
    }

    #[test]
    fn rusqlite_not_found_conversion() {
        let app_err: AppError = rusqlite::Error::QueryReturnedNoRows.into();
        assert_eq!(app_err.code, codes::NOT_FOUND);
        assert_eq!(app_err.category, ErrorCategory::NotFound);
    }

    #[test]
    fn sqlite_failure_detail_is_never_forwarded() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t (description TEXT CHECK (length(description) < 3));")
            .unwrap();
        let err = conn
            .execute("INSERT INTO t VALUES ('secret grocery note')", [])
            .unwrap_err();
        let app_err: AppError = err.into();
        assert_eq!(app_err.code, codes::STORAGE_FAILURE);
        assert!(!app_err.message.contains("description"));
        assert!(!app_err.message.contains("secret"));
    }

    #[test]
    fn log_summary_excludes_message() {
        let err = AppError::invalid(codes::INVALID_TEXT, "description", "Too long: lunch at X");
        assert_eq!(err.log_summary(), "INVALID_TEXT (Validation)");
    }
}
