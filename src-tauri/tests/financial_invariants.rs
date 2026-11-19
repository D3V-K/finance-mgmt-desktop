use chrono::NaiveDate;
use finance_desktop_lib::db::{Database, migrations::migrate};
use finance_desktop_lib::domain::{
    Finance, cards::*, ledger::*, legacy::*, planning::*, review::*,
};
use finance_desktop_lib::error::{ErrorCategory, codes};
use std::sync::{Arc, Barrier};
use uuid::Uuid;

fn id() -> Uuid {
    Uuid::new_v4()
}
fn day(value: &str) -> NaiveDate {
    value.parse().unwrap()
}

fn fixture() -> (tempfile::TempDir, Database) {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(&dir.path().join("finance.sqlite")).unwrap();
    migrate(&db).unwrap();
    (dir, db)
}

fn account(f: &Finance<'_>, name: &str, kind: AccountKind) -> Uuid {
    f.create_account(id(), name, kind).unwrap().result.id
}

fn cutover(f: &Finance<'_>, open: &[(Uuid, i64)]) {
    f.confirm_cutover(
        id(),
        CutoverInput {
            as_of_date: day("2026-01-01"),
            openings: open
                .iter()
                .map(|&(account_id, amount)| CutoverOpening { account_id, amount })
                .collect(),
            mappings: LegacyMappings::default(),
        },
    )
    .unwrap();
}

fn expense(account_id: Uuid, category_id: Uuid, amount: i64, on: NaiveDate) -> EventInput {
    EventInput {
        kind: EventKind::Expense,
        amount,
        account_id,
        destination_account_id: None,
        category_id: Some(category_id),
        description: None,
        occurred_on: on,
        effective_date: on,
        original_event_id: None,
        adjustment_delta: None,
    }
}

#[test]
fn invalid_replacement_rolls_back_reversal_and_receipt() {
    let (_dir, db) = fixture();
    let f = Finance::new(&db);
    let bank = account(&f, "Bank", AccountKind::Bank);
    cutover(&f, &[(bank, 1000)]);
    let food = f
        .create_category(id(), "Food", CategoryType::Expense, None)
        .unwrap()
        .result
        .id;
    let on = day("2026-01-04");
    let input = expense(bank, food, 200, on);
    let original = f.post_event(id(), input.clone()).unwrap().result;
    let retry_key = id();
    let mut replacement = EventInput { amount: 0, ..input };
    assert_eq!(
        f.replace_event(
            retry_key,
            original.id,
            "Correct amount",
            replacement.clone()
        )
        .unwrap_err()
        .code,
        codes::INVALID_AMOUNT
    );
    assert_eq!(f.list_events(None, 1, 10).unwrap().total, 1);
    assert_eq!(f.account_balance(bank, on).unwrap().balance, Some(800));
    replacement.amount = 300;
    let replaced = f
        .replace_event(
            retry_key,
            original.id,
            "Correct amount",
            replacement.clone(),
        )
        .unwrap();
    let replay = f
        .replace_event(retry_key, original.id, "Correct amount", replacement)
        .unwrap();
    assert!(replay.is_replay);
    assert_eq!(replaced.result, replay.result);
    assert_eq!(replaced.result.replaces_event_id, Some(original.id));
    assert_eq!(f.account_balance(bank, on).unwrap().balance, Some(700));
    assert_eq!(
        f.canonical_monthly(None, None).unwrap()[0].gross_spending,
        300
    );
    db.with_conn(|conn| {
        assert!(conn.execute("DELETE FROM financial_events", []).is_err());
        assert!(conn.execute("UPDATE account_postings SET delta=1", []).is_err());
        assert!(conn.execute("UPDATE account_opening_revisions SET amount=0", []).is_err());
        assert!(conn.execute("INSERT INTO account_postings VALUES ('invalid','unknown','unknown',1,'2026-01-01')", []).is_err());
        Ok(())
    }).unwrap();
}

#[test]
fn partial_cutover_and_resolution_keep_original_history_and_conflict_on_changed_evidence() {
    let (_dir, db) = fixture();
    let f = Finance::new(&db);
    let bank = account(&f, "Bank", AccountKind::Bank);
    let card = account(&f, "Card", AccountKind::CreditCard);
    let cat = f
        .create_category(id(), "Food", CategoryType::Expense, None)
        .unwrap()
        .result
        .id;
    let boundary = day("2026-02-01");
    let on = day("2026-02-02");
    let legacy = |when| LegacyTransaction {
        amount: 400,
        description: Some("Receipt".into()),
        transaction_date: when,
        category_id: cat,
        account_type: LegacyAccount::Cash,
    };
    let historical = f
        .save_legacy_transaction(id(), None, legacy(day("2026-01-31")))
        .unwrap()
        .result;
    let unresolved = f
        .save_legacy_transaction(id(), None, legacy(on))
        .unwrap()
        .result;
    let input = CutoverInput {
        as_of_date: boundary,
        openings: vec![
            CutoverOpening {
                account_id: bank,
                amount: 1000,
            },
            CutoverOpening {
                account_id: card,
                amount: -300,
            },
        ],
        mappings: LegacyMappings::default(),
    };
    assert_eq!(
        f.preview_cutover(&input)
            .unwrap()
            .unresolved_transaction_ids,
        vec![unresolved]
    );
    assert!(f.opening_revisions(bank).unwrap().is_empty());
    assert!(f.provenance(false).unwrap().is_empty());
    let confirm_key = id();
    let confirmed = f.confirm_cutover(confirm_key, input.clone()).unwrap();
    assert_eq!(confirmed.result.unresolved_count, 1);
    assert!(
        f.confirm_cutover(confirm_key, input.clone())
            .unwrap()
            .is_replay
    );
    let mut changed = input;
    changed.openings[0].amount = 900;
    assert_eq!(
        f.confirm_cutover(id(), changed).unwrap_err().category,
        ErrorCategory::Conflict
    );
    assert_eq!(f.account_balance(bank, on).unwrap().balance, Some(1000));
    assert_eq!(
        f.account_balance(bank, on).unwrap().status,
        BalanceStatus::IncompleteLegacyMapping
    );
    assert_eq!(f.provenance(false).unwrap()[0].source_id, historical);
    let mut resolve = LegacyMappings {
        transactions: vec![TransactionMapping {
            transaction_id: unresolved,
            account_id: card,
            effective_date: on,
            occurred_on: None,
        }],
        transfers: vec![],
    };
    assert!(f.preview_resolution(&resolve).is_err());
    resolve.transactions[0].account_id = bank;
    assert_eq!(f.preview_resolution(&resolve).unwrap().writes, 0);
    assert_eq!(f.list_events(None, 1, 10).unwrap().total, 0);
    let resolution_key = id();
    assert_eq!(
        f.confirm_resolution(resolution_key, resolve.clone())
            .unwrap()
            .result
            .remaining,
        0
    );
    assert!(
        f.confirm_resolution(resolution_key, resolve.clone())
            .unwrap()
            .is_replay
    );
    assert_eq!(
        f.confirm_resolution(id(), resolve.clone())
            .unwrap()
            .result
            .status,
        "already_mapped"
    );
    resolve.transactions[0].effective_date = day("2026-02-03");
    assert_eq!(
        f.confirm_resolution(id(), resolve).unwrap_err().code,
        codes::UNRESOLVED_MAPPING
    );
    let balance = f.account_balance(bank, on).unwrap();
    assert_eq!(
        (balance.balance, balance.status),
        (Some(600), BalanceStatus::Complete)
    );
    assert!(f.canonical_monthly(None, None).unwrap().is_empty());
    assert_eq!(f.budget_summary(on).unwrap().unknown_purchase_date_count, 1);
    assert_eq!(
        f.available_cash(on, on)
            .unwrap()
            .remaining_discretionary_budget,
        None
    );
    assert_eq!(f.legacy_transactions().unwrap().len(), 2);
    assert!(f.delete_legacy_record(id(), historical, false).is_err());
}

#[test]
fn card_and_savings_funding_compete_across_separate_sqlite_connections() {
    let (dir, db) = fixture();
    let f = Finance::new(&db);
    let bank = account(&f, "Bank", AccountKind::Bank);
    let card = account(&f, "Card", AccountKind::CreditCard);
    let on = day("2026-01-11");
    let goal = f
        .save_savings_goal(
            id(),
            None,
            SavingsGoalInput {
                name: "Trip".into(),
                target: 700,
                target_date: None,
            },
            false,
        )
        .unwrap()
        .result;
    let bucket = f
        .save_savings_bucket(id(), None, "Japan", Some(goal.id), false)
        .unwrap()
        .result
        .id;
    let funding = Funding {
        account_id: bank,
        amount: 700,
        as_of_date: on,
    };
    assert!(
        f.fund_savings_bucket(id(), bucket, funding.clone())
            .is_err()
    );
    cutover(&f, &[(bank, 1000), (card, -900)]);
    let schedule = f
        .create_schedule(
            id(),
            ScheduleInput {
                card_id: card,
                purchase_event_id: None,
                opening_obligation: Some("Existing debt".into()),
                due_date: day("2026-01-27"),
                principal_due: 900,
                source: ScheduleSource::Estimate,
            },
        )
        .unwrap()
        .result
        .id;
    let statement = f
        .confirm_card_statement(
            id(),
            StatementInput {
                card_id: card,
                closing_on: day("2026-01-15"),
                due_date: day("2026-01-27"),
                principal_confirmed: 900,
                schedule_ids: vec![schedule],
            },
        )
        .unwrap()
        .result;
    assert_eq!(
        f.card_schedules(card).unwrap()[0].statement_id,
        Some(statement.id)
    );
    assert!(
        f.revise_schedule(
            id(),
            schedule,
            day("2026-01-28"),
            900,
            ScheduleSource::Actual
        )
        .is_err()
    );
    let barrier = Arc::new(Barrier::new(2));
    let path = dir.path().join("finance.sqlite");
    let first = std::thread::spawn({
        let barrier = barrier.clone();
        let funding = funding.clone();
        move || {
            let db = Database::open(&path).unwrap();
            let f = Finance::new(&db);
            barrier.wait();
            f.fund_card_reserve(id(), schedule, funding).is_ok()
        }
    });
    let bucket_key = id();
    barrier.wait();
    let funded_bucket = f.fund_savings_bucket(bucket_key, bucket, funding.clone());
    assert_ne!(first.join().unwrap(), funded_bucket.is_ok());
    if funded_bucket.is_ok() {
        assert!(
            f.fund_savings_bucket(bucket_key, bucket, funding)
                .unwrap()
                .is_replay
        );
    }
    assert_eq!(f.allocation_capacity(on).unwrap()[0].funded, 700);
    assert_eq!(f.account_balance(bank, on).unwrap().balance, Some(1000));
    assert_eq!(
        f.account_balance(card, day("2026-01-28")).unwrap().balance,
        Some(-900)
    );
    assert_eq!(f.card_schedules(card).unwrap()[0].outstanding, 900);
}

#[test]
fn payday_overrides_calendar_clamping_bill_matching_and_dashboard_share_figures() {
    let (_dir, db) = fixture();
    let f = Finance::new(&db);
    let bank = account(&f, "Bank", AccountKind::Bank);
    cutover(&f, &[(bank, 10000)]);
    let cat = f
        .create_category(id(), "Food", CategoryType::Expense, None)
        .unwrap()
        .result
        .id;
    f.set_finance_settings(
        id(),
        FinanceSettings {
            time_zone: "Asia/Tokyo".into(),
            payday_day: 31,
            default_account_id: Some(bank),
        },
    )
    .unwrap();
    let feb = day("2028-02-29");
    assert_eq!(f.budget_summary(feb).unwrap().cycle.start, feb);
    f.set_payday_override(id(), day("2028-03-01"), day("2028-03-28"))
        .unwrap();
    let end = day("2028-03-28");
    assert_eq!(f.budget_summary(feb).unwrap().cycle.end_exclusive, end);
    f.set_category_budget(
        id(),
        CategoryBudget {
            category_id: cat,
            cycle_start: feb,
            planned: 900,
        },
    )
    .unwrap();
    f.set_budget_payment_account(id(), cat, feb, Some(bank))
        .unwrap();
    let paid = f
        .post_event(id(), expense(bank, cat, 400, feb))
        .unwrap()
        .result;
    let mut refund = expense(bank, cat, 100, feb);
    refund.kind = EventKind::Refund;
    refund.original_event_id = Some(paid.id);
    f.post_event(id(), refund).unwrap();
    let recurring = f
        .create_expected_flow(
            id(),
            ExpectedFlow {
                account_id: bank,
                on_date: day("2028-01-31"),
                amount: 4000,
                label: "Salary".into(),
                recurrence: Recurrence::Monthly,
                certainty: Certainty::Confirmed,
                schedule_id: None,
                budget_category_id: None,
            },
        )
        .unwrap()
        .result
        .id;
    let moved = day("2028-03-05");
    f.override_flow(
        id(),
        FlowOverride {
            flow_id: recurring,
            scheduled_date: feb,
            override_date: Some(moved),
            skipped: false,
        },
    )
    .unwrap();
    let bill = f
        .create_expected_flow(
            id(),
            ExpectedFlow {
                account_id: bank,
                on_date: moved,
                amount: -400,
                label: "Groceries".into(),
                recurrence: Recurrence::None,
                certainty: Certainty::Confirmed,
                schedule_id: None,
                budget_category_id: Some(cat),
            },
        )
        .unwrap()
        .result
        .id;
    let report = f.cash_forecast(feb, end, false).unwrap();
    let on = report.days.iter().find(|d| d.date == moved).unwrap();
    assert!(on.intraday_order_uncertain);
    assert!(on.movements.iter().any(|m| m.id == recurring));
    assert_eq!(
        report
            .days
            .iter()
            .flat_map(|d| &d.movements)
            .filter(|m| m.source == "unspent_budget_assumption")
            .map(|m| m.amount)
            .sum::<i64>(),
        -200
    );
    assert_eq!(
        f.available_cash(feb, end).unwrap().accounts[0].raw_available,
        Some(9300)
    );
    let actual = f
        .post_event(id(), expense(bank, cat, 400, moved))
        .unwrap()
        .result;
    f.match_flow(
        id(),
        FlowMatch {
            flow_id: bill,
            scheduled_date: moved,
            event_id: actual.id,
        },
    )
    .unwrap();
    let report = f.cash_forecast(feb, end, false).unwrap();
    let day = report.days.iter().find(|d| d.date == moved).unwrap();
    assert!(!day.movements.iter().any(|m| m.id == bill));
    assert_eq!(
        day.movements
            .iter()
            .filter(|m| m.source == "actual")
            .count(),
        1
    );
    assert_eq!(
        report
            .days
            .iter()
            .flat_map(|d| &d.movements)
            .filter(|m| m.source == "unspent_budget_assumption")
            .map(|m| m.amount)
            .sum::<i64>(),
        -200
    );
    let available_after_match = f.available_cash(feb, end).unwrap();
    assert_eq!(available_after_match.accounts[0].raw_available, Some(9300));
    assert_eq!(
        available_after_match.remaining_discretionary_budget,
        Some(200)
    );
    let dashboard = f.dashboard(feb).unwrap();
    assert_eq!(dashboard.forecast, report);
    assert_eq!(
        dashboard.available_cash,
        f.available_cash(feb, end).unwrap()
    );
    assert_eq!(dashboard.living_budget, f.budget_summary(feb).unwrap());
    assert!(
        f.cash_forecast(feb, day.date + chrono::Days::new(400), false)
            .is_err()
    );
}

#[test]
fn reconciliation_flags_new_postings_and_records_reopen_and_adjustment_audit() {
    let (_dir, db) = fixture();
    let f = Finance::new(&db);
    let bank = account(&f, "Bank", AccountKind::Bank);
    cutover(&f, &[(bank, 1000)]);
    let start = day("2026-01-02");
    let cat = f
        .create_category(id(), "Food", CategoryType::Expense, None)
        .unwrap()
        .result
        .id;
    let e = f
        .post_event(id(), expense(bank, cat, 100, start))
        .unwrap()
        .result
        .id;
    let input = ReconciliationInput {
        account_id: bank,
        period_start: start,
        period_end: day("2026-01-31"),
        statement_opening: 1000,
        statement_closing: 900,
        selected_posting_ids: None,
    };
    assert_eq!(f.preview_reconciliation(&input).unwrap().difference, 0);
    let session = f.create_reconciliation(id(), input).unwrap().result.id;
    assert_eq!(
        f.confirm_reconciliation(id(), session)
            .unwrap()
            .result
            .status,
        "confirmed"
    );
    assert!(f.reverse_event(id(), e, "Correction").is_err());
    let adjust = |value: i64, description: Option<String>| EventInput {
        kind: EventKind::BalanceAdjustment,
        amount: value.abs(),
        account_id: bank,
        destination_account_id: None,
        category_id: None,
        description,
        occurred_on: start,
        effective_date: start,
        original_event_id: None,
        adjustment_delta: Some(value),
    };
    assert!(f.post_event(id(), adjust(-10, None)).is_err());
    f.post_event(id(), adjust(-10, Some("Statement fee correction".into())))
        .unwrap();
    let stale = f.reconciliation(session).unwrap();
    assert_eq!(stale.status, "stale");
    assert_eq!(stale.unmatched_postings.len(), 1);
    assert!(f.confirm_reconciliation(id(), session).is_err());
    assert_eq!(
        f.reopen_reconciliation(id(), session)
            .unwrap()
            .result
            .status,
        "draft"
    );
    let unmatched = stale.unmatched_postings[0].id;
    assert_eq!(
        f.set_reconciled_posting(id(), session, unmatched, true)
            .unwrap()
            .result
            .difference,
        10
    );
    assert!(f.confirm_reconciliation(id(), session).is_err());
    assert_eq!(
        f.reconciliation_audit(session)
            .unwrap()
            .iter()
            .map(|entry| entry.action.as_str())
            .collect::<Vec<_>>(),
        vec!["draft_created", "confirmed", "reopened", "posting_included"]
    );
    assert_eq!(
        f.canonical_monthly(None, None).unwrap()[0].gross_spending,
        100
    );
}

#[test]
fn overdue_card_payment_keeps_one_commitment_across_separate_funding_accounts() {
    let (_dir, db) = fixture();
    let f = Finance::new(&db);
    let bank = account(&f, "Bank", AccountKind::Bank);
    let cash = account(&f, "Cash", AccountKind::Cash);
    let card = account(&f, "Card", AccountKind::CreditCard);
    cutover(&f, &[(bank, 20_000), (cash, 30_000), (card, -10_000)]);
    let due = day("2026-01-15");
    let on = day("2026-01-16");
    let tomorrow = day("2026-01-17");
    let schedule = f
        .create_schedule(
            id(),
            ScheduleInput {
                card_id: card,
                purchase_event_id: None,
                opening_obligation: Some("Opening card debt".into()),
                due_date: due,
                principal_due: 10_000,
                source: ScheduleSource::Actual,
            },
        )
        .unwrap()
        .result
        .id;
    assert_eq!(
        f.cash_forecast(on, tomorrow, false)
            .unwrap()
            .overdue_schedule_ids,
        vec![schedule]
    );
    f.set_card_settings(
        id(),
        CardSettings {
            card_id: card,
            settlement_account_id: bank,
            closing_day: 5,
            payment_day: 15,
        },
    )
    .unwrap();
    f.fund_card_reserve(
        id(),
        schedule,
        Funding {
            account_id: cash,
            amount: 10_000,
            as_of_date: due,
        },
    )
    .unwrap();

    let available = f.available_cash(on, tomorrow).unwrap();
    assert_eq!(available.overdue_schedule_ids, vec![schedule]);
    let bank_cash = available
        .accounts
        .iter()
        .find(|a| a.account_id == bank)
        .unwrap();
    let wallet = available
        .accounts
        .iter()
        .find(|a| a.account_id == cash)
        .unwrap();
    assert_eq!(
        (bank_cash.unfunded_known_due, bank_cash.raw_available),
        (0, Some(20_000))
    );
    assert_eq!(
        (wallet.funded_earmarks, wallet.raw_available),
        (10_000, Some(20_000))
    );
    assert!(
        available
            .warnings
            .iter()
            .any(|warning| warning.contains("outside"))
    );

    let report = f.cash_forecast(on, tomorrow, false).unwrap();
    assert_eq!(report.overdue_schedule_ids, vec![schedule]);
    let first = report
        .days
        .iter()
        .find(|row| row.date == on && row.account_id == bank)
        .unwrap();
    assert_eq!(first.end_balance, Some(10_000));
    assert!(
        first
            .movements
            .iter()
            .any(|m| m.id == schedule && m.amount == -10_000)
    );

    let payment = f
        .post_event(
            id(),
            EventInput {
                kind: EventKind::CardRepayment,
                amount: 10_000,
                account_id: bank,
                destination_account_id: Some(card),
                category_id: None,
                description: None,
                occurred_on: tomorrow,
                effective_date: tomorrow,
                original_event_id: None,
                adjustment_delta: None,
            },
        )
        .unwrap()
        .result
        .id;
    f.apply_card_event(id(), schedule, payment, 10_000).unwrap();
    let report = f.cash_forecast(on, tomorrow, false).unwrap();
    assert_eq!(report.overdue_schedule_ids, vec![schedule]);
    let debits: Vec<_> = report
        .days
        .iter()
        .filter(|d| d.account_id == bank)
        .flat_map(|d| &d.movements)
        .map(|m| (m.id, m.amount))
        .collect();
    assert_eq!(debits, vec![(payment, -10_000)]);
    assert!(
        f.available_cash(tomorrow, tomorrow)
            .unwrap()
            .overdue_schedule_ids
            .is_empty()
    );
}

#[test]
fn payday_changes_move_category_budgets_and_payment_assumptions_in_place() {
    let (_dir, db) = fixture();
    let f = Finance::new(&db);
    let bank = account(&f, "Bank", AccountKind::Bank);
    cutover(&f, &[(bank, 100_000)]);
    let cat = f
        .create_category(id(), "Food", CategoryType::Expense, None)
        .unwrap()
        .result
        .id;
    let march = day("2028-03-25");
    let april = day("2028-04-25");
    let budget = |start| CategoryBudget {
        category_id: cat,
        cycle_start: start,
        planned: 900,
    };
    let march_id = f
        .set_category_budget(id(), budget(march))
        .unwrap()
        .result
        .id;
    let april_id = f
        .set_category_budget(id(), budget(april))
        .unwrap()
        .result
        .id;
    f.set_budget_payment_account(id(), cat, march, Some(bank))
        .unwrap();

    let new_march = day("2028-03-24");
    let override_key = id();
    f.set_payday_override(override_key, day("2028-03-01"), new_march)
        .unwrap();
    assert!(
        f.set_payday_override(override_key, day("2028-03-01"), new_march)
            .unwrap()
            .is_replay
    );
    let report = f.budget_summary(new_march).unwrap();
    assert_eq!(
        (report.cycle.start, report.budgets[0].id),
        (new_march, march_id)
    );
    let forecast = f
        .cash_forecast(new_march, day("2028-04-24"), false)
        .unwrap();
    let net_amount: i64 = forecast
        .days
        .iter()
        .flat_map(|row| &row.movements)
        .filter(|item| item.id == march_id)
        .map(|item| item.amount)
        .sum();
    assert_eq!(net_amount, -900);

    f.set_finance_settings(
        id(),
        FinanceSettings {
            time_zone: "Asia/Tokyo".into(),
            payday_day: 28,
            default_account_id: None,
        },
    )
    .unwrap();
    assert_eq!(f.budget_summary(new_march).unwrap().budgets[0].id, march_id);
    let new_april = day("2028-04-28");
    assert_eq!(f.budget_summary(new_april).unwrap().budgets[0].id, april_id);
    assert_eq!(
        f.set_category_budget(id(), budget(new_april))
            .unwrap()
            .result
            .id,
        april_id
    );
}

#[test]
fn cutover_preserves_posted_openings_and_excludes_archived_accounts() {
    let (_dir, db) = fixture();
    let f = Finance::new(&db);
    let bank = account(&f, "Bank", AccountKind::Bank);
    let archived = account(&f, "Old card", AccountKind::CreditCard);
    f.archive_account(id(), archived, true).unwrap();
    let food = f
        .create_category(id(), "Food", CategoryType::Expense, None)
        .unwrap()
        .result
        .id;
    let boundary = day("2026-01-01");
    let start = day("2026-01-03");
    let on = day("2026-01-04");
    f.set_opening(
        id(),
        OpeningInput {
            account_id: bank,
            amount: 10_000,
            as_of_date: start,
            reason: "Opening".into(),
        },
    )
    .unwrap();
    let posted = f
        .post_event(id(), expense(bank, food, 1_000, on))
        .unwrap()
        .result;
    let legacy = f
        .save_legacy_transaction(
            id(),
            None,
            LegacyTransaction {
                amount: 2_000,
                description: None,
                transaction_date: start,
                category_id: food,
                account_type: LegacyAccount::Bank,
            },
        )
        .unwrap()
        .result;
    let mut input = CutoverInput {
        as_of_date: boundary,
        openings: vec![CutoverOpening {
            account_id: bank,
            amount: 9_000,
        }],
        mappings: LegacyMappings {
            transactions: vec![TransactionMapping {
                transaction_id: legacy,
                account_id: bank,
                effective_date: start,
                occurred_on: None,
            }],
            transfers: vec![],
        },
    };
    let confirm_key = id();
    assert_eq!(
        f.confirm_cutover(confirm_key, input.clone())
            .unwrap_err()
            .category,
        ErrorCategory::Conflict
    );
    input.openings[0].amount = 10_000;
    input.as_of_date = on;
    assert_eq!(
        f.preview_cutover(&input).unwrap_err().category,
        ErrorCategory::Conflict
    );
    input.as_of_date = boundary;
    input.mappings.transactions[0].effective_date = boundary;
    assert_eq!(
        f.preview_cutover(&input).unwrap_err().code,
        codes::MISSING_OPENING
    );
    input.mappings.transactions[0].effective_date = start;
    assert_eq!(f.preview_cutover(&input).unwrap().mapped_count, 1);
    assert_eq!(f.list_events(None, 1, 10).unwrap().total, 1);

    assert_eq!(
        f.confirm_cutover(confirm_key, input)
            .unwrap()
            .result
            .unresolved_count,
        0
    );
    assert_eq!(f.financial_event(posted.id).unwrap(), posted);
    let revisions = f.opening_revisions(bank).unwrap();
    assert_eq!(revisions.len(), 1);
    assert_eq!(revisions[0].opening.as_of_date, start);
    assert!(f.opening_revisions(archived).unwrap().is_empty());
    let balance = f.account_balance(bank, on).unwrap();
    assert_eq!(
        (balance.balance, balance.status),
        (Some(7_000), BalanceStatus::Complete)
    );
    assert_eq!(f.account_balance(bank, boundary).unwrap().balance, None);
}

#[test]
fn category_references_in_forecasts_and_provenance_guard_change_and_delete() {
    let (_dir, db) = fixture();
    let f = Finance::new(&db);
    let bank = account(&f, "Bank", AccountKind::Bank);
    let bill = f
        .create_category(id(), "Bills", CategoryType::Expense, None)
        .unwrap()
        .result
        .id;
    let archive = f
        .create_category(id(), "Archive", CategoryType::Expense, None)
        .unwrap()
        .result
        .id;
    f.create_expected_flow(
        id(),
        ExpectedFlow {
            account_id: bank,
            on_date: day("2026-01-01"),
            amount: -100,
            label: "Internet".into(),
            recurrence: Recurrence::None,
            certainty: Certainty::Confirmed,
            schedule_id: None,
            budget_category_id: Some(bill),
        },
    )
    .unwrap();
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO legacy_provenance
            (id,source_table,source_id,recorded_date,amount,category_id,status)
            VALUES (?1,'legacy_transactions',?2,'2026-01-01',100,?3,'historical')",
            [id().to_string(), id().to_string(), archive.to_string()],
        )?;
        Ok(())
    })
    .unwrap();
    for category in [bill, archive] {
        assert_eq!(
            f.update_category(id(), category, "Income", CategoryType::Income, None)
                .unwrap_err()
                .category,
            ErrorCategory::Conflict
        );
        assert_eq!(
            f.delete_category(id(), category).unwrap_err().category,
            ErrorCategory::Conflict
        );
    }
}

#[test]
fn event_page_limits_events_and_returns_all_their_postings() {
    let (_dir, db) = fixture();
    let f = Finance::new(&db);
    let bank = account(&f, "Bank", AccountKind::Bank);
    let cash = account(&f, "Cash", AccountKind::Cash);
    cutover(&f, &[(bank, 10_000), (cash, 1_000)]);
    let transfer = |occurred, effective| EventInput {
        kind: EventKind::Transfer,
        amount: 100,
        account_id: bank,
        destination_account_id: Some(cash),
        category_id: None,
        description: None,
        occurred_on: day(occurred),
        effective_date: day(effective),
        original_event_id: None,
        adjustment_delta: None,
    };
    let first = f
        .post_event(id(), transfer("2026-01-02", "2026-01-05"))
        .unwrap()
        .result;
    let second = f
        .post_event(id(), transfer("2026-01-03", "2026-01-04"))
        .unwrap()
        .result;
    let page = f.list_events(None, 1, 1).unwrap();
    assert_eq!(page.total, 2);
    assert_eq!(page.items, vec![second.clone()]);
    for filter in [bank, cash] {
        assert_eq!(
            f.list_events(Some(filter), 1, 1).unwrap().items,
            vec![first.clone()]
        );
        assert_eq!(
            f.list_events(Some(filter), 2, 1).unwrap().items,
            vec![second.clone()]
        );
        assert!(f.list_events(Some(filter), 3, 1).unwrap().items.is_empty());
    }
}

#[test]
fn unexpected_recurrence_and_schedule_source_fail_as_storage_errors() {
    let (_dir, db) = fixture();
    let f = Finance::new(&db);
    let bank = account(&f, "Bank", AccountKind::Bank);
    let card = account(&f, "Card", AccountKind::CreditCard);
    cutover(&f, &[(bank, 1000), (card, -100)]);
    let on = day("2026-01-02");
    f.create_expected_flow(
        id(),
        ExpectedFlow {
            account_id: bank,
            on_date: on,
            amount: 100,
            label: "Salary".into(),
            recurrence: Recurrence::Monthly,
            certainty: Certainty::Confirmed,
            schedule_id: None,
            budget_category_id: None,
        },
    )
    .unwrap();
    f.create_schedule(
        id(),
        ScheduleInput {
            card_id: card,
            purchase_event_id: None,
            opening_obligation: Some("Opening debt".into()),
            due_date: on,
            principal_due: 100,
            source: ScheduleSource::Estimate,
        },
    )
    .unwrap();
    db.with_conn(|conn| {
        conn.execute_batch(
            "PRAGMA ignore_check_constraints=ON;
            UPDATE expected_flows SET recurrence='future';
            UPDATE card_schedules SET source='future';
            PRAGMA ignore_check_constraints=OFF;",
        )?;
        Ok(())
    })
    .unwrap();
    assert_eq!(f.expected_flows().unwrap_err().code, codes::STORAGE_FAILURE);
    assert_eq!(
        f.card_schedules(card).unwrap_err().code,
        codes::STORAGE_FAILURE
    );
}

#[test]
fn payday_change_rolls_back_when_preexisting_budgets_would_collide() {
    let (_dir, db) = fixture();
    let f = Finance::new(&db);
    let cat = f
        .create_category(id(), "Food", CategoryType::Expense, None)
        .unwrap()
        .result
        .id;
    let feb = day("2028-02-25");
    let march = day("2028-03-25");
    for start in [feb, march] {
        f.set_category_budget(
            id(),
            CategoryBudget {
                category_id: cat,
                cycle_start: start,
                planned: 900,
            },
        )
        .unwrap();
    }
    // The earlier implementation could leave an old budget behind after a payday edit.
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO category_budgets (id,category_id,cycle_start,planned)
            VALUES (?1,?2,'2028-03-24',800)",
            [id().to_string(), cat.to_string()],
        )?;
        Ok(())
    })
    .unwrap();
    let result = f.set_finance_settings(
        id(),
        FinanceSettings {
            time_zone: "Asia/Tokyo".into(),
            payday_day: 24,
            default_account_id: None,
        },
    );
    assert_eq!(result.unwrap_err().category, ErrorCategory::Conflict);
    assert_eq!(f.finance_settings().unwrap().payday_day, 25);
    assert_eq!(
        f.budget_summary(feb).unwrap().budgets[0].terms.cycle_start,
        feb
    );
    assert_eq!(
        f.budget_summary(march).unwrap().budgets[0]
            .terms
            .cycle_start,
        march
    );
}
