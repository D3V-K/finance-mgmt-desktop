use crate::AppState;
use crate::error::AppError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub app_version: String,
    pub app_data_dir: String,
    pub time_zone: String,
    pub today: String,
    pub os_target: String,
}

#[tauri::command]
pub fn get_system_info() -> Result<SystemInfo, AppError> {
    let now = chrono::Utc::now()
        .with_timezone(&chrono::FixedOffset::east_opt(9 * 3600).expect("valid JST offset"));
    let today = now.format("%Y-%m-%d").to_string();

    Ok(SystemInfo {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        app_data_dir: "Local application data".to_string(),
        time_zone: "Asia/Tokyo".to_string(),
        today,
        os_target: std::env::consts::OS.to_string(),
    })
}

#[tauri::command]
pub fn get_storage_status(
    state: tauri::State<'_, AppState>,
) -> Result<crate::db::StorageStatus, AppError> {
    let mut status = state.db.get_status()?;
    status.database_path = "Local SQLite database".to_string();
    Ok(status)
}
