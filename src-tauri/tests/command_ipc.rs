//! Renderer-shaped end-to-end tests. Every call goes through Tauri IPC with the
//! JSON the Svelte client sends (`{ request }` for writes, camelCase named
//! arguments for reads) into the real handler table and on-disk migrations.

use finance_desktop_lib::db::migrations::migrate;
use finance_desktop_lib::{AppState, lifecycle, with_commands};
use serde_json::{Value, json};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{
    INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets,
};
use tauri::webview::InvokeRequest;
use tauri::{Manager, WebviewWindow, WebviewWindowBuilder};

struct Harness {
    _dir: tempfile::TempDir,
    _app: tauri::App<MockRuntime>,
    window: WebviewWindow<MockRuntime>,
}

impl Harness {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let app = with_commands(mock_builder())
            .build(mock_context(noop_assets()))
            .unwrap();
        app.manage(AppState {
            boot: lifecycle::boot(dir.path(), migrate),
        });
        let window = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        Self {
            _dir: dir,
            _app: app,
            window,
        }
    }

    fn call(&self, cmd: &str, args: Value) -> Result<Value, Value> {
        get_ipc_response(
            &self.window,
            InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "tauri://localhost".parse().unwrap(),
                body: InvokeBody::Json(args),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .map(|body| body.deserialize::<Value>().unwrap())
    }

    fn ok(&self, cmd: &str, args: Value) -> Value {
        self.call(cmd, args)
            .unwrap_or_else(|err| panic!("{cmd} failed: {err}"))
    }

    /// One user intent: a fresh idempotency key, as `newWrite` does in TS.
    fn write(&self, cmd: &str, payload: Value) -> Value {
        self.ok(cmd, json!({ "request": key(payload) }))["result"].clone()
    }
}

fn key(payload: Value) -> Value {
    json!({ "idempotencyKey": uuid::Uuid::new_v4().to_string(), "payload": payload })
}

fn id(value: &Value) -> String {
    value["id"].as_str().unwrap().to_string()
}

struct Ledger {
    bank: String,
    card: String,
    food: String,
}

/// Accounts, categories and a confirmed start (cutover with no legacy rows).
fn started(h: &Harness) -> Ledger {
    let bank = id(&h.write(
        "create_account",
        json!({ "name": "Everyday", "kind": "BANK" }),
    ));
    let card = id(&h.write(
        "create_account",
        json!({ "name": "Visa", "kind": "CREDIT_CARD" }),
    ));
    let food = id(&h.write(
        "create_category",
        json!({ "name": "食費", "type": "expense" }),
    ));
    h.write(
        "confirm_cutover",
        json!({
            "asOfDate": "2026-01-01",
            "openings": [{ "accountId": bank, "amount": 200_000 }, { "accountId": card, "amount": 0 }],
            "transactions": [], "transfers": []
        }),
    );
    Ledger { bank, card, food }
}

fn balance(h: &Harness, account: &str, on: &str) -> Value {
    h.ok(
        "get_account_balance",
        json!({ "accountId": account, "asOfDate": on }),
    )
}

#[test]
fn every_financial_command_requires_ready_storage_and_reports_field_errors() {
    let h = Harness::new();
    assert_eq!(h.ok("get_profile", json!({}))["timeZone"], "Asia/Tokyo");

    let err = h
        .call(
            "get_account_balance",
            json!({ "accountId": "CASH", "asOfDate": "2026-01-01" }),
        )
        .unwrap_err();
    assert_eq!(
        (err["code"].as_str(), err["field"].as_str()),
        (Some("INVALID_ID"), Some("accountId"))
    );

    let err = h
        .call(
            "create_financial_event",
            json!({ "request": key(json!({
                "kind": "EXPENSE", "amount": 100, "accountId": uuid::Uuid::new_v4().to_string(),
                "occurredOn": "2026-02-30", "effectiveDate": "2026-02-01"
            })) }),
        )
        .unwrap_err();
    assert_eq!(
        (err["code"].as_str(), err["field"].as_str()),
        (Some("INVALID_DATE"), Some("occurredOn"))
    );
    assert_eq!(err["category"], "validation");
}

#[test]
fn ledger_start_makes_balances_complete_and_retries_never_post_twice() {
    let h = Harness::new();
    let spare = id(&h.write(
        "create_account",
        json!({ "name": "Old wallet", "kind": "CASH" }),
    ));
    assert_eq!(balance(&h, &spare, "2026-01-05")["status"], "unavailable");
    // Every active account needs an opening at start; archive the unused one.
    let archived = h.write(
        "archive_account",
        json!({ "accountId": spare, "archived": true }),
    );
    assert!(archived["archivedAt"].is_string());

    let l = started(&h);
    let b = balance(&h, &l.bank, "2026-01-05");
    assert_eq!(
        (b["status"].as_str(), b["balance"].as_i64()),
        (Some("complete"), Some(200_000))
    );

    let request = key(json!({
        "kind": "EXPENSE", "amount": 1_200, "accountId": l.bank, "categoryId": l.food,
        "description": "  Supermarket ", "occurredOn": "2026-01-05", "effectiveDate": "2026-01-05"
    }));
    let first = h.ok("create_financial_event", json!({ "request": request }));
    let retry = h.ok("create_financial_event", json!({ "request": request }));
    assert_eq!(
        (first["isReplay"].as_bool(), retry["isReplay"].as_bool()),
        (Some(false), Some(true))
    );
    assert_eq!(first["result"], retry["result"]);
    assert_eq!(first["result"]["description"], "Supermarket");
    assert_eq!(balance(&h, &l.bank, "2026-01-05")["balance"], 198_800);

    let mut changed = request.clone();
    changed["payload"]["amount"] = json!(1_300);
    let err = h
        .call("create_financial_event", json!({ "request": changed }))
        .unwrap_err();
    assert_eq!(err["code"], "IDEMPOTENCY_CONFLICT");

    let page = h.ok(
        "list_financial_events",
        json!({ "accountId": l.bank, "page": 1, "pageSize": 50 }),
    );
    assert_eq!(page["total"], 1);

    // Correction: reversal plus replacement in one write; history keeps all three.
    let event = id(&first["result"]);
    let replacement = h.write(
        "replace_financial_event",
        json!({ "eventId": event, "reason": "Typo", "replacement": {
            "kind": "EXPENSE", "amount": 1_250, "accountId": l.bank, "categoryId": l.food,
            "occurredOn": "2026-01-05", "effectiveDate": "2026-01-05" } }),
    );
    assert_eq!(replacement["replacesEventId"], event.as_str());
    assert_eq!(balance(&h, &l.bank, "2026-01-05")["balance"], 198_750);
    let page = h.ok(
        "list_financial_events",
        json!({ "page": 1, "pageSize": 50 }),
    );
    assert_eq!(page["total"], 3);
}

#[test]
fn card_purchase_statement_review_and_repayment() {
    let h = Harness::new();
    let l = started(&h);
    h.write(
        "set_card_settings",
        json!({ "cardId": l.card, "settlementAccountId": l.bank, "closingDay": 15, "paymentDay": 10 }),
    );
    let purchase = h.write(
        "create_financial_event",
        json!({ "kind": "EXPENSE", "amount": 30_000, "accountId": l.card, "categoryId": l.food,
                "occurredOn": "2026-01-10", "effectiveDate": "2026-01-10" }),
    );
    let card = balance(&h, &l.card, "2026-01-10");
    assert_eq!(
        (card["balance"].as_i64(), card["amountOwed"].as_i64()),
        (Some(-30_000), Some(30_000))
    );
    // Card debt never reduces the bank's cash.
    assert_eq!(balance(&h, &l.bank, "2026-01-10")["balance"], 200_000);

    let schedule = h.write(
        "create_card_schedule",
        json!({ "cardId": l.card, "purchaseEventId": id(&purchase), "dueDate": "2026-02-10",
                "principalDue": 30_000, "source": "estimate" }),
    );
    assert_eq!(schedule["status"], "planned");

    // Statement review: confirming replaces the estimate.
    let statement = h.write(
        "confirm_card_statement",
        json!({ "cardId": l.card, "closingOn": "2026-01-15", "dueDate": "2026-02-10",
                "principalConfirmed": 30_000, "scheduleIds": [id(&schedule)] }),
    );
    let schedules = h.ok("list_card_schedules", json!({ "cardId": l.card }));
    assert_eq!(schedules[0]["statementId"], statement["id"]);
    assert_eq!(
        h.ok("list_card_statements", json!({ "cardId": l.card }))
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let repayment = h.write(
        "create_financial_event",
        json!({ "kind": "CARD_REPAYMENT", "amount": 30_000, "accountId": l.bank,
                "destinationAccountId": l.card, "occurredOn": "2026-02-10", "effectiveDate": "2026-02-10" }),
    );
    let paid = h.write(
        "apply_card_event",
        json!({ "scheduleId": id(&schedule), "eventId": id(&repayment), "amount": 30_000 }),
    );
    assert_eq!(
        (paid["status"].as_str(), paid["outstanding"].as_i64()),
        (Some("paid"), Some(0))
    );
    assert_eq!(balance(&h, &l.card, "2026-02-10")["amountOwed"], 0);
    assert_eq!(balance(&h, &l.bank, "2026-02-10")["balance"], 170_000);

    // A repayment is not spending.
    let months = h.ok(
        "get_canonical_monthly",
        json!({ "from": "2026-01-01", "to": "2026-02-28" }),
    );
    let spent: i64 = months
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["grossSpending"].as_i64().unwrap())
        .sum();
    assert_eq!(spent, 30_000);
}

#[test]
fn savings_allocation_reduces_allocatable_cash_not_the_balance() {
    let h = Harness::new();
    let l = started(&h);
    let goal = h.write(
        "save_savings_goal",
        json!({ "name": "Trip", "target": 100_000, "targetDate": "2026-08-01" }),
    );
    let bucket = h.write(
        "save_savings_bucket",
        json!({ "name": "Trip fund", "goalId": id(&goal) }),
    );
    let funded = h.write(
        "fund_savings_bucket",
        json!({ "bucketId": id(&bucket), "accountId": l.bank, "amount": 50_000, "asOfDate": "2026-01-20" }),
    );
    assert_eq!(funded["funded"], 50_000);

    let capacity = h.ok(
        "get_allocation_capacity",
        json!({ "asOfDate": "2026-01-20" }),
    );
    let bank = capacity
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["balance"]["accountId"] == l.bank.as_str())
        .unwrap();
    assert_eq!(bank["balance"]["balance"], 200_000);
    assert_eq!(bank["funded"], 50_000);
    assert_eq!(bank["allocatable"], 150_000);
    assert_eq!(balance(&h, &l.bank, "2026-01-20")["balance"], 200_000);

    let err = h
        .call(
            "fund_savings_bucket",
            json!({ "request": key(json!({ "bucketId": id(&bucket), "accountId": l.bank,
                "amount": 150_001, "asOfDate": "2026-01-20" })) }),
        )
        .unwrap_err();
    assert_ne!(err["category"], "internal");
}

#[test]
fn forecast_warns_and_keeps_scenarios_out_of_actuals() {
    let h = Harness::new();
    let l = started(&h);
    let forecast = h.ok(
        "get_cash_forecast",
        json!({ "from": "2026-01-20", "to": "2026-02-20" }),
    );
    let warnings: Vec<&str> = forecast["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w.as_str().unwrap())
        .collect();
    assert!(
        warnings.contains(&"Finance settings are unconfirmed"),
        "{warnings:?}"
    );

    h.write(
        "set_finance_settings",
        json!({ "timeZone": "Asia/Tokyo", "paydayDay": 25, "defaultAccountId": l.bank }),
    );
    h.write(
        "create_expected_flow",
        json!({ "accountId": l.bank, "onDate": "2026-01-25", "amount": 300_000, "label": "Bonus",
                "certainty": "scenario_only" }),
    );
    let base = h.ok(
        "get_cash_forecast",
        json!({ "from": "2026-01-20", "to": "2026-02-20" }),
    );
    let scenario = h.ok(
        "get_cash_forecast",
        json!({ "from": "2026-01-20", "to": "2026-02-20", "scenario": true }),
    );
    let low = |f: &Value| f["lowestByAccount"][&l.bank]["balance"].as_i64();
    assert!(
        !base["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w == "Finance settings are unconfirmed")
    );
    let end = |f: &Value| {
        f["days"]
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .find(|d| d["accountId"] == l.bank.as_str())
            .unwrap()["endBalance"]
            .as_i64()
    };
    assert_eq!(end(&scenario).unwrap() - end(&base).unwrap(), 300_000);
    assert_eq!(low(&base), Some(200_000));
    // Projected income never becomes an actual balance or report income.
    assert_eq!(balance(&h, &l.bank, "2026-02-20")["balance"], 200_000);
    let months = h.ok("get_canonical_monthly", json!({}));
    assert!(months.as_array().unwrap().iter().all(|m| m["income"] == 0));

    let dashboard = h.ok("get_dashboard", json!({ "asOfDate": "2026-01-20" }));
    assert_eq!(dashboard["financialPosition"]["liquidCash"], 200_000);
    assert!(dashboard["availableCash"]["accounts"].is_array());
}

#[test]
fn reconciliation_and_quality_review() {
    let h = Harness::new();
    let l = started(&h);
    h.write(
        "create_financial_event",
        json!({ "kind": "EXPENSE", "amount": 5_000, "accountId": l.bank, "categoryId": l.food,
                "occurredOn": "2026-01-03", "effectiveDate": "2026-01-03" }),
    );
    let input = json!({ "accountId": l.bank, "periodStart": "2026-01-01", "periodEnd": "2026-01-31",
        "statementOpening": 200_000, "statementClosing": 195_000 });
    let preview = h.ok("preview_reconciliation", json!({ "input": input }));
    assert_eq!(preview["difference"], 0);
    let session = h.write("create_reconciliation", input);
    let issues = h.ok("get_ledger_quality", json!({ "asOfDate": "2026-01-31" }));
    assert!(
        issues
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["recordId"] == session["id"])
    );

    let confirmed = h.write(
        "confirm_reconciliation",
        json!({ "reconciliationId": id(&session) }),
    );
    assert_eq!(confirmed["status"], "confirmed");
    let audit = h.ok(
        "get_reconciliation_audit",
        json!({ "reconciliationId": id(&session) }),
    );
    assert!(!audit.as_array().unwrap().is_empty());
    let issues = h.ok("get_ledger_quality", json!({ "asOfDate": "2026-01-31" }));
    assert!(
        !issues
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["recordId"] == session["id"])
    );
}
