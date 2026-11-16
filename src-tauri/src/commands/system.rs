use crate::AppState;
use crate::error::AppError;
use crate::lifecycle::AppStatus;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub app_version: String,
    pub os_target: String,
}

/// Always available, including after a failed boot, so the renderer can show
/// the blocking startup screen instead of financial screens.
#[tauri::command]
pub fn get_app_status(state: tauri::State<'_, AppState>) -> AppStatus {
    state.boot.status()
}

/// Diagnostics without paths, usernames or data.
#[tauri::command]
pub fn get_system_info() -> SystemInfo {
    SystemInfo {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        os_target: std::env::consts::OS.to_string(),
    }
}

#[tauri::command]
pub fn get_storage_status(
    state: tauri::State<'_, AppState>,
) -> Result<crate::db::StorageStatus, AppError> {
    state.boot.db()?.get_status()
}
