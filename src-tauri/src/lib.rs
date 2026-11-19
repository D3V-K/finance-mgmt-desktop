pub mod commands;
pub mod db;
pub mod domain;
pub mod error;
pub mod lifecycle;

use error::{AppError, codes};
use lifecycle::Boot;
use tauri::webview::NewWindowResponse;
use tauri::{Manager, RunEvent, Url, WebviewWindowBuilder};

pub const MAIN_WINDOW: &str = "main";

pub struct AppState {
    pub boot: Boot,
}

/// The webview may only show the bundled app. Every other navigation (links,
/// redirects, injected content) is refused; there is no opener/shell plugin.
pub fn allow_navigation(url: &Url) -> bool {
    let host = url.host_str();
    let bundled = (url.scheme() == "tauri" && host == Some("localhost"))
        || (matches!(url.scheme(), "http" | "https") && host == Some("tauri.localhost"));
    let dev_server = cfg!(debug_assertions)
        && url.scheme() == "http"
        && host == Some("localhost")
        && url.port() == Some(1420);
    bundled || dev_server
}

fn focus_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Registers every renderer-callable command. Shared by the app and the IPC
/// tests so both exercise exactly the same handler table.
pub fn with_commands<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![
        commands::system::get_app_status,
        commands::system::get_system_info,
        commands::system::get_storage_status,
        commands::profile::get_profile,
        commands::ledger::list_categories,
        commands::ledger::create_category,
        commands::ledger::update_category,
        commands::ledger::delete_category,
        commands::ledger::archive_category,
        commands::ledger::list_accounts,
        commands::ledger::create_account,
        commands::ledger::archive_account,
        commands::ledger::set_account_opening,
        commands::ledger::list_opening_revisions,
        commands::ledger::get_account_balance,
        commands::ledger::create_financial_event,
        commands::ledger::reverse_financial_event,
        commands::ledger::replace_financial_event,
        commands::ledger::get_financial_event,
        commands::ledger::list_financial_events,
        commands::ledger::get_canonical_monthly,
        commands::ledger::get_canonical_by_category,
        commands::legacy::list_legacy_records,
        commands::legacy::get_legacy_reports,
        commands::legacy::get_legacy_by_category,
        commands::legacy::get_legacy_balance,
        commands::legacy::preview_cutover,
        commands::legacy::confirm_cutover,
        commands::legacy::list_legacy_provenance,
        commands::legacy::preview_legacy_resolution,
        commands::legacy::confirm_legacy_resolution,
        commands::cards::get_card_settings,
        commands::cards::set_card_settings,
        commands::cards::list_card_schedules,
        commands::cards::create_card_schedule,
        commands::cards::revise_card_schedule,
        commands::cards::list_card_statements,
        commands::cards::confirm_card_statement,
        commands::cards::apply_card_event,
        commands::cards::fund_card_reserve,
        commands::cards::list_savings_goals,
        commands::cards::save_savings_goal,
        commands::cards::list_savings_buckets,
        commands::cards::save_savings_bucket,
        commands::cards::fund_savings_bucket,
        commands::cards::get_allocation_capacity,
        commands::planning::get_finance_settings,
        commands::planning::set_finance_settings,
        commands::planning::set_payday_override,
        commands::planning::get_budget_summary,
        commands::planning::set_category_budget,
        commands::planning::set_budget_payment_account,
        commands::planning::list_expected_flows,
        commands::planning::create_expected_flow,
        commands::planning::override_expected_flow,
        commands::planning::match_expected_flow,
        commands::planning::unmatch_expected_flow,
        commands::planning::get_cash_forecast,
        commands::planning::get_available_cash,
        commands::planning::get_dashboard,
        commands::review::record_valuation,
        commands::review::list_valuations,
        commands::review::mark_holdings_change,
        commands::review::get_financial_position,
        commands::review::get_position_history,
        commands::review::preview_reconciliation,
        commands::review::create_reconciliation,
        commands::review::get_reconciliation,
        commands::review::set_reconciled_posting,
        commands::review::confirm_reconciliation,
        commands::review::reopen_reconciliation,
        commands::review::get_reconciliation_audit,
        commands::review::get_ledger_quality,
    ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Must be the first plugin: a second launch focuses the existing window and
    // exits. Arguments forwarded by the second process are ignored.
    let mut builder =
        tauri::Builder::default().plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            focus_main(app)
        }));

    if cfg!(debug_assertions) {
        builder = builder.plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        );
    }

    let app = with_commands(builder)
        .setup(|app| {
            // Boot completes before the window exists, so no screen can render
            // ahead of storage and migrations.
            let boot = match app.path().app_data_dir() {
                Ok(dir) => lifecycle::boot(&dir, db::migrations::migrate),
                Err(_) => Boot::Failed(AppError::storage(
                    codes::STORAGE_UNAVAILABLE,
                    "Local application storage could not be located.",
                )),
            };
            app.manage(AppState { boot });

            let config = app
                .config()
                .app
                .windows
                .iter()
                .find(|w| w.label == MAIN_WINDOW)
                .cloned()
                .ok_or("main window configuration missing")?;
            WebviewWindowBuilder::from_config(app.handle(), &config)?
                .on_navigation(allow_navigation)
                .on_new_window(|_, _| NewWindowResponse::Deny)
                .build()?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            handle.state::<AppState>().boot.shutdown();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn only_bundled_origins_are_navigable() {
        assert!(allow_navigation(&url("tauri://localhost/index.html")));
        assert!(allow_navigation(&url("http://tauri.localhost/")));
        for blocked in [
            "https://example.com/",
            "http://localhost:8000/api",
            "http://127.0.0.1:1420/",
            "file:///etc/passwd",
            "tauri://evil.example/",
            "javascript:alert(1)",
        ] {
            assert!(!allow_navigation(&url(blocked)), "{blocked}");
        }
    }

    #[test]
    fn dev_server_only_in_debug_builds() {
        assert_eq!(
            allow_navigation(&url("http://localhost:1420/")),
            cfg!(debug_assertions)
        );
    }

    #[test]
    fn every_registered_command_is_in_acl_manifest_and_capability() {
        let src = include_str!("lib.rs");
        let handler = &src[src.find("generate_handler![").unwrap()..];
        let handler = &handler[..handler.find(']').unwrap()];
        let registered: Vec<&str> = handler
            .split(',')
            .filter_map(|s| s.trim().rsplit("::").next())
            .filter(|s| !s.is_empty() && !s.contains('['))
            .collect();
        let mut sorted = registered.clone();
        sorted.sort();
        let mut manifest = commands::COMMANDS.to_vec();
        manifest.sort();
        assert_eq!(
            sorted, manifest,
            "generate_handler! and commands::COMMANDS differ"
        );

        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        let mut granted: Vec<String> = capability["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_str().unwrap().to_string())
            .collect();
        granted.sort();
        let mut expected: Vec<String> = manifest
            .iter()
            .map(|c| format!("allow-{}", c.replace('_', "-")))
            .collect();
        expected.sort();
        assert_eq!(
            granted, expected,
            "capability must grant exactly the app commands"
        );
    }
}
