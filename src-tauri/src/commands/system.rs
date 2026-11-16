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
pub fn get_system_info(state: tauri::State<'_, AppState>) -> Result<SystemInfo, AppError> {
    let now = chrono::Local::now();
    let today = now.format("%Y-%m-%d").to_string();
    let time_zone = format!("{:?}", now.offset());

    Ok(SystemInfo {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        app_data_dir: state.app_data_dir.clone(),
        time_zone,
        today,
        os_target: std::env::consts::OS.to_string(),
    })
}

#[tauri::command]
pub fn get_storage_status(
    state: tauri::State<'_, AppState>,
) -> Result<crate::db::StorageStatus, AppError> {
    state.db.get_status()
}
