//! Tauri command transport (#6). Handlers validate wire input, check that boot
//! succeeded, and delegate to the domain/database layers; they own no financial
//! rules. Every command registered here must also be listed in `build.rs` and
//! granted in `capabilities/default.json`, or the renderer cannot invoke it.

pub mod cards;
pub mod ledger;
pub mod legacy;
pub mod planning;
pub mod profile;
pub mod review;
pub mod system;
pub mod wire;

/// Single source for `build.rs` (ACL manifest) and the registration test.
pub const COMMANDS: &[&str] = &[
    "get_app_status",
    "get_system_info",
    "get_storage_status",
    "get_profile",
    "list_categories",
    "create_category",
    "update_category",
    "delete_category",
    "archive_category",
    "list_accounts",
    "create_account",
    "archive_account",
    "set_account_opening",
    "list_opening_revisions",
    "get_account_balance",
    "create_financial_event",
    "reverse_financial_event",
    "replace_financial_event",
    "get_financial_event",
    "list_financial_events",
    "get_canonical_monthly",
    "get_canonical_by_category",
    "list_legacy_records",
    "get_legacy_reports",
    "get_legacy_by_category",
    "get_legacy_balance",
    "preview_cutover",
    "confirm_cutover",
    "list_legacy_provenance",
    "preview_legacy_resolution",
    "confirm_legacy_resolution",
    "get_card_settings",
    "set_card_settings",
    "list_card_schedules",
    "create_card_schedule",
    "revise_card_schedule",
    "list_card_statements",
    "confirm_card_statement",
    "apply_card_event",
    "fund_card_reserve",
    "list_savings_goals",
    "save_savings_goal",
    "list_savings_buckets",
    "save_savings_bucket",
    "fund_savings_bucket",
    "get_allocation_capacity",
    "get_finance_settings",
    "set_finance_settings",
    "set_payday_override",
    "get_budget_summary",
    "set_category_budget",
    "set_budget_payment_account",
    "list_expected_flows",
    "create_expected_flow",
    "override_expected_flow",
    "match_expected_flow",
    "unmatch_expected_flow",
    "get_cash_forecast",
    "get_available_cash",
    "get_dashboard",
    "record_valuation",
    "list_valuations",
    "mark_holdings_change",
    "get_financial_position",
    "get_position_history",
    "preview_reconciliation",
    "create_reconciliation",
    "get_reconciliation",
    "set_reconciled_posting",
    "confirm_reconciliation",
    "reopen_reconciliation",
    "get_reconciliation_audit",
    "get_ledger_quality",
];

use crate::AppState;
use crate::domain::Finance;
use crate::error::AppError;

pub(crate) type St<'a> = tauri::State<'a, AppState>;
pub(crate) type Res<T> = Result<T, AppError>;

/// Financial commands fail with `STORAGE_NOT_READY` until boot succeeded.
pub(crate) fn finance<'a>(state: &'a St<'_>) -> Res<Finance<'a>> {
    Ok(Finance::new(state.boot.db()?))
}

pub(crate) fn optional_date(raw: Option<String>, field: &str) -> Res<Option<chrono::NaiveDate>> {
    raw.map(|raw| wire::parse_date(&raw, field)).transpose()
}
