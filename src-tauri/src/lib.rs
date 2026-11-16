pub mod commands;
pub mod db;
pub mod error;

use db::Database;
use error::sanitize_error_message;
use std::sync::Arc;
use tauri::Manager;

pub struct AppState {
    pub db: Arc<Database>,
    pub app_data_dir: String,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // Ensure app data directory exists outside the installation directory
            let app_data_dir = app.path().app_data_dir().map_err(|e| {
                Box::new(error::AppError::storage(
                    "APP_DATA_DIR_ERROR",
                    e.to_string(),
                ))
            })?;

            std::fs::create_dir_all(&app_data_dir).map_err(|e| {
                Box::new(error::AppError::storage("CREATE_DIR_ERROR", e.to_string()))
            })?;

            let db_path = app_data_dir.join("finance.db");
            log::info!("Opening offline SQLite database at: {:?}", db_path);

            let database = Database::open(&db_path).map_err(Box::new)?;

            let state = AppState {
                db: Arc::new(database),
                app_data_dir: sanitize_error_message(&app_data_dir.to_string_lossy()),
            };

            app.manage(state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::system::get_system_info,
            commands::system::get_storage_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
