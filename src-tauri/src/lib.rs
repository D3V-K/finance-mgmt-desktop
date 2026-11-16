pub mod commands;
pub mod db;
pub mod error;

use db::Database;
use std::sync::Arc;
use tauri::Manager;

pub struct AppState {
    pub db: Arc<Database>,
}

fn allow_navigation(url: &tauri::Url) -> bool {
    if cfg!(debug_assertions)
        && url.scheme() == "http"
        && url.host_str() == Some("localhost")
        && url.port() == Some(1420)
    {
        return true;
    }
    url.scheme() == "tauri" && url.host_str() == Some("localhost")
        || url.scheme() == "http" && url.host_str() == Some("tauri.localhost")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(
            tauri::plugin::Builder::<tauri::Wry>::new("local-navigation")
                .on_navigation(|_, url| allow_navigation(url))
                .build(),
        )
        .setup(|app| {
            // Ensure app data directory exists outside the installation directory
            let app_data_dir = app.path().app_data_dir().map_err(|_| {
                Box::new(error::AppError::storage(
                    "APP_DATA_DIR_ERROR",
                    "Cannot locate local application storage",
                ))
            })?;

            std::fs::create_dir_all(&app_data_dir).map_err(|e| {
                Box::new(error::AppError::storage("CREATE_DIR_ERROR", e.to_string()))
            })?;

            let db_path = app_data_dir.join("finance.db");
            let database = Database::open(&db_path).map_err(Box::new)?;

            let state = AppState {
                db: Arc::new(database),
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

#[cfg(test)]
mod tests {
    use super::allow_navigation;

    #[test]
    fn navigation_is_local_only() {
        assert!(allow_navigation(
            &"tauri://localhost/index.html".parse().unwrap()
        ));
        assert!(allow_navigation(
            &"http://tauri.localhost/index.html".parse().unwrap()
        ));
        assert!(!allow_navigation(&"https://example.com/".parse().unwrap()));
        assert!(!allow_navigation(
            &"file:///Users/private/secret".parse().unwrap()
        ));
        assert!(!allow_navigation(
            &"http://tauri.localhost.evil.test/".parse().unwrap()
        ));
    }
}
