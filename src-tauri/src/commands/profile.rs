use crate::AppState;
use crate::error::{AppError, codes};
use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use serde::Serialize;

/// Initial financial time zone, matching the Python finance settings.
#[cfg(test)]
const DEFAULT_TIME_ZONE: Tz = chrono_tz::Asia::Tokyo;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// IANA name, e.g. `Asia/Tokyo` (never an offset like `+09:00`).
    pub time_zone: String,
    /// `YYYY-MM-DD` in `time_zone`, independent of the OS clock's zone.
    pub today: String,
}

pub fn profile_at(now: DateTime<Utc>, tz: Tz) -> Profile {
    Profile {
        time_zone: tz.name().to_string(),
        today: now
            .with_timezone(&tz)
            .date_naive()
            .format("%Y-%m-%d")
            .to_string(),
    }
}

#[tauri::command]
pub fn get_profile(state: tauri::State<'_, AppState>) -> Result<Profile, AppError> {
    let zone = crate::domain::Finance::new(state.boot.db()?)
        .finance_settings()?
        .time_zone;
    let zone = zone.parse().map_err(|_| {
        AppError::storage(
            codes::STORAGE_FAILURE,
            "The stored financial time zone is invalid.",
        )
    })?;
    Ok(profile_at(Utc::now(), zone))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn today_follows_profile_zone_not_utc() {
        // 2026-11-16 15:30 UTC is already 2026-11-17 in Tokyo.
        let now = Utc.with_ymd_and_hms(2026, 11, 16, 15, 30, 0).unwrap();
        let p = profile_at(now, DEFAULT_TIME_ZONE);
        assert_eq!(p.time_zone, "Asia/Tokyo");
        assert_eq!(p.today, "2026-11-17");
    }

    #[test]
    fn serializes_camel_case() {
        let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let json = serde_json::to_value(profile_at(now, DEFAULT_TIME_ZONE)).unwrap();
        assert_eq!(json["timeZone"], "Asia/Tokyo");
        assert_eq!(json["today"], "2026-01-01");
    }
}
