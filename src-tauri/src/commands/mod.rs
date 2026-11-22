//! Tauri command transport (#6). Handlers validate wire input, check that boot
//! succeeded, and delegate to the domain/database layers; they own no financial
//! rules. Every command registered here must also be listed in `build.rs` and
//! granted in `capabilities/default.json`, or the renderer cannot invoke it.

pub mod ledger;
pub mod profile;
pub mod subscriptions;
pub mod system;
pub mod wire;

/// Single source for `build.rs` (ACL manifest) and the registration test.
pub const COMMANDS: &[&str] = &[
    "get_app_status",
    "get_system_info",
    "get_storage_status",
    "get_profile",
    "list_accounts",
    "list_categories",
    "list_financial_events",
    "list_subscriptions",
    "get_subscription",
    "create_subscription",
    "update_subscription",
    "revise_subscription_price",
    "change_subscription_status",
    "list_subscription_occurrences",
    "override_subscription_occurrence",
    "reconcile_subscription_occurrence",
    "reconcile_subscription_batch",
    "match_subscription_occurrence",
    "unlink_subscription_occurrence",
];
