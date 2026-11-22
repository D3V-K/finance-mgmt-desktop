//! Wire envelopes and input validation shared by every v1 command adapter.
//!
//! IDs and dates arrive as strings and are validated here rather than by Serde,
//! so a malformed value from the renderer becomes a field-level `AppError`
//! (`INVALID_ID`, `INVALID_DATE`, …) instead of an opaque invoke failure.

use crate::error::{AppError, codes};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Largest integer the renderer can represent exactly (`Number.MAX_SAFE_INTEGER`).
pub const MAX_SAFE_YEN: i64 = 9_007_199_254_740_991;
pub const MAX_PAGE_SIZE: u32 = 200;
pub const MAX_NAME_CHARS: usize = 100;
pub const MAX_DESCRIPTION_CHARS: usize = 500;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WriteRequest<T> {
    pub idempotency_key: String,
    pub payload: T,
}

impl<T> WriteRequest<T> {
    pub fn key(&self) -> Result<Uuid, AppError> {
        parse_id(&self.idempotency_key, "idempotencyKey")
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WriteResponse<T> {
    pub is_replay: bool,
    pub result: T,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub page_size: u32,
}

/// Hyphenated UUID only; the original spelling is preserved by the caller.
pub fn parse_id(raw: &str, field: &str) -> Result<Uuid, AppError> {
    if raw.len() != 36 {
        return Err(invalid_id(field));
    }
    Uuid::try_parse(raw).map_err(|_| invalid_id(field))
}

fn invalid_id(field: &str) -> AppError {
    AppError::invalid(codes::INVALID_ID, field, "This reference is not valid.")
}

/// Strict `YYYY-MM-DD` calendar date in the profile time zone.
pub fn parse_date(raw: &str, field: &str) -> Result<NaiveDate, AppError> {
    let well_formed = raw.len() == 10
        && raw.bytes().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        });
    if !well_formed {
        return Err(invalid_date(field));
    }
    NaiveDate::parse_from_str(raw, "%Y-%m-%d").map_err(|_| invalid_date(field))
}

fn invalid_date(field: &str) -> AppError {
    AppError::invalid(
        codes::INVALID_DATE,
        field,
        "Enter a valid date (YYYY-MM-DD).",
    )
}

/// Signed whole yen (postings, openings, adjustment deltas).
pub fn yen(value: i64, field: &str) -> Result<i64, AppError> {
    if !(-MAX_SAFE_YEN..=MAX_SAFE_YEN).contains(&value) {
        return Err(AppError::invalid(
            codes::INVALID_AMOUNT,
            field,
            "The amount is too large.",
        ));
    }
    Ok(value)
}

/// Event amounts are strictly positive; direction comes from the event kind.
pub fn positive_yen(value: i64, field: &str) -> Result<i64, AppError> {
    let value = yen(value, field)?;
    if value <= 0 {
        return Err(AppError::invalid(
            codes::INVALID_AMOUNT,
            field,
            "Enter an amount greater than zero.",
        ));
    }
    Ok(value)
}

/// Strictly positive minor-unit amount (whole yen for JPY, integer cents for USD).
pub fn positive_minor_units(value: i64, field: &str) -> Result<i64, AppError> {
    positive_yen(value, field)
}

/// Supported subscription billing currencies.
pub fn parse_currency(raw: &str, field: &str) -> Result<&'static str, AppError> {
    match raw {
        "JPY" => Ok("JPY"),
        "USD" => Ok("USD"),
        _ => Err(AppError::invalid(
            codes::INVALID_CURRENCY,
            field,
            "Select JPY or USD.",
        )),
    }
}

/// Supported subscription billing cadences.
pub fn parse_cadence(raw: &str, field: &str) -> Result<&'static str, AppError> {
    match raw {
        "weekly" => Ok("weekly"),
        "monthly" => Ok("monthly"),
        "quarterly" => Ok("quarterly"),
        "yearly" => Ok("yearly"),
        _ => Err(AppError::invalid(
            codes::INVALID_CADENCE,
            field,
            "Select weekly, monthly, quarterly, or yearly.",
        )),
    }
}

/// Trimmed single-line text. `None` for optional empty input; never stores control characters.
pub fn text(
    raw: Option<&str>,
    field: &str,
    max_chars: usize,
    required: bool,
) -> Result<Option<String>, AppError> {
    let trimmed = raw.map(str::trim).unwrap_or("");
    if trimmed.is_empty() {
        return if required {
            Err(AppError::invalid(
                codes::INVALID_TEXT,
                field,
                "This field is required.",
            ))
        } else {
            Ok(None)
        };
    }
    if trimmed.chars().count() > max_chars {
        return Err(AppError::invalid(
            codes::INVALID_TEXT,
            field,
            format!("Use {max_chars} characters or fewer."),
        ));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(AppError::invalid(
            codes::INVALID_TEXT,
            field,
            "Remove line breaks and control characters.",
        ));
    }
    Ok(Some(trimmed.to_string()))
}

/// 1-based page number and bounded page size.
pub fn page(page: u32, page_size: u32) -> Result<(u32, u32), AppError> {
    if page == 0 {
        return Err(AppError::invalid(
            codes::INVALID_PAGE,
            "page",
            "Page starts at 1.",
        ));
    }
    if page_size == 0 || page_size > MAX_PAGE_SIZE {
        return Err(AppError::invalid(
            codes::INVALID_PAGE,
            "pageSize",
            format!("Page size must be between 1 and {MAX_PAGE_SIZE}."),
        ));
    }
    Ok((page, page_size))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCategory;

    #[test]
    fn ids_must_be_hyphenated_uuids() {
        assert!(parse_id("0b9f6a7e-3c1d-4e2f-9a8b-7c6d5e4f3a2b", "accountId").is_ok());
        for bad in [
            "",
            "CASH",
            "0b9f6a7e3c1d4e2f9a8b7c6d5e4f3a2b",
            "../../etc/passwd",
        ] {
            let err = parse_id(bad, "accountId").unwrap_err();
            assert_eq!(err.code, codes::INVALID_ID);
            assert_eq!(err.field.as_deref(), Some("accountId"));
            assert_eq!(err.category, ErrorCategory::Validation);
        }
    }

    #[test]
    fn dates_are_strict_calendar_dates() {
        assert_eq!(
            parse_date("2026-02-28", "occurredOn").unwrap(),
            NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()
        );
        for bad in [
            "2026-02-30",
            "2026-2-3",
            "26-02-03",
            "2026/02/03",
            "",
            "2026-02-03T00:00",
        ] {
            assert_eq!(
                parse_date(bad, "occurredOn").unwrap_err().code,
                codes::INVALID_DATE,
                "{bad}"
            );
        }
    }

    #[test]
    fn yen_bounds_match_js_safe_integers() {
        assert_eq!(yen(-MAX_SAFE_YEN, "delta").unwrap(), -MAX_SAFE_YEN);
        assert_eq!(
            yen(MAX_SAFE_YEN + 1, "delta").unwrap_err().code,
            codes::INVALID_AMOUNT
        );
        assert_eq!(
            positive_yen(0, "amount").unwrap_err().code,
            codes::INVALID_AMOUNT
        );
        assert_eq!(
            positive_yen(-5, "amount").unwrap_err().code,
            codes::INVALID_AMOUNT
        );
        assert_eq!(positive_yen(1_200, "amount").unwrap(), 1_200);
    }

    #[test]
    fn text_is_trimmed_bounded_and_single_line() {
        assert_eq!(
            text(Some("  食費 "), "name", 10, true).unwrap().as_deref(),
            Some("食費")
        );
        assert_eq!(text(Some("   "), "description", 10, false).unwrap(), None);
        assert_eq!(
            text(None, "name", 10, true).unwrap_err().code,
            codes::INVALID_TEXT
        );
        assert!(text(Some("12345678901"), "name", 10, true).is_err());
        assert!(text(Some("a\nb"), "name", 10, true).is_err());
    }

    #[test]
    fn page_bounds() {
        assert!(page(1, 50).is_ok());
        assert_eq!(page(0, 50).unwrap_err().field.as_deref(), Some("page"));
        assert_eq!(page(1, 0).unwrap_err().field.as_deref(), Some("pageSize"));
        assert!(page(1, MAX_PAGE_SIZE + 1).is_err());
    }

    #[test]
    fn write_request_wire_shape() {
        let req: WriteRequest<serde_json::Value> = serde_json::from_value(serde_json::json!({
            "idempotencyKey": "0b9f6a7e-3c1d-4e2f-9a8b-7c6d5e4f3a2b",
            "payload": { "name": "Wallet" }
        }))
        .unwrap();
        assert!(req.key().is_ok());

        let unknown: Result<WriteRequest<serde_json::Value>, _> =
            serde_json::from_value(serde_json::json!({
                "idempotencyKey": "x", "payload": {}, "userId": "other"
            }));
        assert!(unknown.is_err(), "unknown envelope fields are rejected");

        let resp = serde_json::to_value(WriteResponse {
            is_replay: true,
            result: 1,
        })
        .unwrap();
        assert_eq!(resp, serde_json::json!({ "isReplay": true, "result": 1 }));
    }

    #[test]
    fn currency_and_cadence_validators_reject_unknown_values() {
        assert_eq!(parse_currency("JPY", "currency").unwrap(), "JPY");
        assert_eq!(parse_currency("USD", "currency").unwrap(), "USD");
        let err = parse_currency("EUR", "currency").unwrap_err();
        assert_eq!(err.code, codes::INVALID_CURRENCY);
        assert_eq!(err.field.as_deref(), Some("currency"));

        for valid in ["weekly", "monthly", "quarterly", "yearly"] {
            assert_eq!(parse_cadence(valid, "cadence").unwrap(), valid);
        }
        let err = parse_cadence("daily", "cadence").unwrap_err();
        assert_eq!(err.code, codes::INVALID_CADENCE);
        assert_eq!(err.field.as_deref(), Some("cadence"));
    }
}
