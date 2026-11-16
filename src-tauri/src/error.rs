use serde::{Deserialize, Serialize};
use std::fmt;

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

    pub fn internal(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(code, message, ErrorCategory::Internal)
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref field) = self.field {
            write!(
                f,
                "[{}] {}: {} (field: {})",
                self.code,
                self.message,
                format!("{:?}", self.category).to_lowercase(),
                field
            )
        } else {
            write!(
                f,
                "[{}] {}: {}",
                self.code,
                self.message,
                format!("{:?}", self.category).to_lowercase()
            )
        }
    }
}

impl std::error::Error for AppError {}

/// Sanitize error messages to remove sensitive paths or personal details.
/// Ensures error messages are actionable without disclosing filesystem or user information.
pub fn sanitize_error_message(msg: &str) -> String {
    let mut cleaned = msg.to_string();

    // Redact Unix home paths like /Users/<username>/... or /home/<username>/...
    if let Some(users_idx) = cleaned.find("/Users/") {
        let after_users = &cleaned[users_idx + 7..];
        if let Some(slash_idx) = after_users.find('/') {
            let user_part = &cleaned[users_idx..users_idx + 7 + slash_idx];
            cleaned = cleaned.replace(user_part, "[USER_HOME]");
        }
    }

    if let Some(home_idx) = cleaned.find("/home/") {
        let after_home = &cleaned[home_idx + 6..];
        if let Some(slash_idx) = after_home.find('/') {
            let user_part = &cleaned[home_idx..home_idx + 6 + slash_idx];
            cleaned = cleaned.replace(user_part, "[USER_HOME]");
        }
    }

    // Redact Windows user paths like C:\Users\<username>\...
    if let Some(win_idx) = cleaned.to_lowercase().find("c:\\users\\") {
        let after_win = &cleaned[win_idx + 9..];
        if let Some(slash_idx) = after_win.find('\\') {
            let user_part = &cleaned[win_idx..win_idx + 9 + slash_idx];
            cleaned = cleaned.replace(user_part, "[USER_HOME]");
        }
    }

    cleaned
}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        match &err {
            rusqlite::Error::SqliteFailure(err_code, msg) => {
                let detail = msg.as_deref().unwrap_or("SQLite operation failed");
                AppError::storage(format!("SQLITE_ERROR_{:?}", err_code.extended_code), detail)
            }
            rusqlite::Error::QueryReturnedNoRows => AppError::new(
                "NOT_FOUND",
                "Requested record was not found",
                ErrorCategory::NotFound,
            ),
            _ => AppError::storage("SQLITE_INTERNAL", err.to_string()),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::storage("IO_ERROR", err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitization_removes_user_paths() {
        let raw = "Failed to open file at /Users/john_doe/Library/Application Support/finance.db";
        let sanitized = sanitize_error_message(raw);
        assert!(!sanitized.contains("john_doe"));
        assert!(sanitized.contains("[USER_HOME]/Library/Application Support/finance.db"));
    }

    #[test]
    fn test_sanitization_windows_paths() {
        let raw = r"Could not open C:\Users\alice\AppData\Roaming\finance.db";
        let sanitized = sanitize_error_message(raw);
        assert!(!sanitized.contains("alice"));
        assert!(sanitized.contains(r"[USER_HOME]\AppData\Roaming\finance.db"));
    }

    #[test]
    fn test_rusqlite_error_conversion() {
        let err = rusqlite::Error::QueryReturnedNoRows;
        let app_err: AppError = err.into();
        assert_eq!(app_err.code, "NOT_FOUND");
        assert_eq!(app_err.category, ErrorCategory::NotFound);
    }
}
