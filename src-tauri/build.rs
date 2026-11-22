// Keep in sync with `src/commands/mod.rs` COMMANDS (checked by a unit test).
const COMMANDS: &[&str] = &[
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

fn main() {
    // An explicit app manifest makes each command a permission (`allow-<command>`),
    // so the renderer can invoke only what `capabilities/default.json` grants.
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("failed to run tauri-build");
}
