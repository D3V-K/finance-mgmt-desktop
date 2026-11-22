use chrono::NaiveDate;
use finance_desktop_lib::db::{Database, migrations::migrate};
use finance_desktop_lib::domain::{
    Finance, cards::*, ledger::*, legacy::*, planning::*, subscriptions::*,
};
use finance_desktop_lib::error::{ErrorCategory, codes};
use uuid::Uuid;

fn id() -> Uuid {
    Uuid::new_v4()
}

fn day(raw: &str) -> NaiveDate {
    raw.parse().unwrap()
}

fn setup() -> (tempfile::TempDir, Database) {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(&dir.path().join("finance.sqlite")).unwrap();
    migrate(&db).unwrap();
    (dir, db)
}

fn bootstrap_accounts(f: &Finance<'_>) -> (Uuid, Uuid, Uuid) {
    let bank = f
        .create_account(id(), "Main Bank", AccountKind::Bank)
        .unwrap()
        .result
        .id;
    let card = f
        .create_account(id(), "Visa Card", AccountKind::CreditCard)
        .unwrap()
        .result
        .id;
    let software = f
        .create_category(id(), "Subscriptions", CategoryType::Expense, None)
        .unwrap()
        .result
        .id;
    f.confirm_cutover(
        id(),
        CutoverInput {
            as_of_date: day("2026-01-01"),
            openings: vec![
                CutoverOpening {
                    account_id: bank,
                    amount: 200_000,
                },
                CutoverOpening {
                    account_id: card,
                    amount: 0,
                },
            ],
            mappings: LegacyMappings::default(),
        },
    )
    .unwrap();
    f.set_card_settings(
        id(),
        CardSettings {
            card_id: card,
            settlement_account_id: bank,
            closing_day: 15,
            payment_day: 10,
        },
    )
    .unwrap();
    f.set_finance_settings(
        id(),
        FinanceSettings {
            time_zone: "Asia/Tokyo".into(),
            payday_day: 25,
            default_account_id: Some(bank),
        },
    )
    .unwrap();
    (bank, card, software)
}

#[test]
fn cadence_clamping_plan_revisions_and_pause_cancel_boundaries() {
    let (_dir, db) = setup();
    let f = Finance::new(&db);
    let (_bank, card, cat) = bootstrap_accounts(&f);

    let sub = f
        .create_subscription(
            id(),
            CreateSubscriptionInput {
                name: "ChatGPT Plus".into(),
                merchant: Some("OpenAI".into()),
                account_id: card,
                category_id: cat,
                currency: BillingCurrency::Usd,
                amount_minor: 2000,
                cadence: SubscriptionCadence::Monthly,
                billing_day: Some(31),
                billing_month: None,
                start_date: day("2026-01-31"),
                note: Some("AI tools".into()),
            },
        )
        .unwrap()
        .result;

    // Price increase effective 2026-03-01 ($20.00 -> $25.00)
    let rev = f
        .revise_subscription_price(
            id(),
            PriceRevisionInput {
                subscription_id: sub.subscription.id,
                effective_from: day("2026-03-01"),
                amount_minor: 2500,
                reason: "Tier price increase".into(),
            },
        )
        .unwrap()
        .result;
    assert_eq!(rev.revision, 2);

    // Append-only trigger rejects update/delete on revisions
    db.with_conn(|conn| {
        assert!(
            conn.execute("UPDATE subscription_plan_revisions SET amount_minor=1", [])
                .is_err()
        );
        assert!(
            conn.execute("DELETE FROM subscription_plan_revisions", [])
                .is_err()
        );
        Ok(())
    })
    .unwrap();

    // Check clamped billing dates (Jan 31, Feb 28, Mar 31, Apr 30) and revision pricing
    let occs = f
        .list_subscription_occurrences(
            day("2026-01-01"),
            day("2026-04-30"),
            day("2026-03-31"),
            Some(sub.subscription.id),
            false,
        )
        .unwrap();
    let summary: Vec<_> = occs
        .iter()
        .map(|o| (o.scheduled_date, o.billed_amount_minor, o.status))
        .collect();
    assert_eq!(
        summary,
        vec![
            (
                day("2026-01-31"),
                2000,
                OccurrenceStatus::NeedsReconciliation
            ),
            (
                day("2026-02-28"),
                2000,
                OccurrenceStatus::NeedsReconciliation
            ),
            (
                day("2026-03-31"),
                2500,
                OccurrenceStatus::NeedsReconciliation
            ),
            (day("2026-04-30"), 2500, OccurrenceStatus::Upcoming),
        ]
    );

    // Pause effective 2026-04-01 stops April generation, keeps Jan-Mar
    f.change_subscription_status(
        id(),
        StatusChangeInput {
            subscription_id: sub.subscription.id,
            status: SubscriptionStatus::Paused,
            effective_on: day("2026-04-01"),
        },
    )
    .unwrap();
    let paused_occs = f
        .list_subscription_occurrences(
            day("2026-01-01"),
            day("2026-06-30"),
            day("2026-05-01"),
            Some(sub.subscription.id),
            false,
        )
        .unwrap();
    assert_eq!(paused_occs.len(), 3);

    // Cancel effective 2026-03-15 stops March 31 too, and cannot be reactivated
    f.change_subscription_status(
        id(),
        StatusChangeInput {
            subscription_id: sub.subscription.id,
            status: SubscriptionStatus::Canceled,
            effective_on: day("2026-03-15"),
        },
    )
    .unwrap();
    assert_eq!(
        f.change_subscription_status(
            id(),
            StatusChangeInput {
                subscription_id: sub.subscription.id,
                status: SubscriptionStatus::Active,
                effective_on: day("2026-04-01"),
            },
        )
        .unwrap_err()
        .code,
        codes::SUBSCRIPTION_INACTIVE
    );
}

#[test]
fn month_end_batch_reconciliation_is_atomic_idempotent_and_creates_card_schedules() {
    let (_dir, db) = setup();
    let f = Finance::new(&db);
    let (_bank, card, cat) = bootstrap_accounts(&f);

    let sub1 = f
        .create_subscription(
            id(),
            CreateSubscriptionInput {
                name: "ChatGPT Plus".into(),
                merchant: Some("OpenAI".into()),
                account_id: card,
                category_id: cat,
                currency: BillingCurrency::Usd,
                amount_minor: 2000,
                cadence: SubscriptionCadence::Monthly,
                billing_day: Some(10),
                billing_month: None,
                start_date: day("2026-01-10"),
                note: None,
            },
        )
        .unwrap()
        .result
        .subscription
        .id;

    let sub2 = f
        .create_subscription(
            id(),
            CreateSubscriptionInput {
                name: "GitHub Copilot".into(),
                merchant: Some("GitHub".into()),
                account_id: card,
                category_id: cat,
                currency: BillingCurrency::Usd,
                amount_minor: 1000,
                cadence: SubscriptionCadence::Monthly,
                billing_day: Some(20),
                billing_month: None,
                start_date: day("2026-01-20"),
                note: None,
            },
        )
        .unwrap()
        .result
        .subscription
        .id;

    // Missing actual_jpy on second USD item must roll back the entire batch!
    let bad_batch = ReconcileBatchInput {
        items: vec![
            ReconcileOccurrenceInput {
                subscription_id: sub1,
                scheduled_date: day("2026-01-10"),
                actual_jpy: Some(3_142),
                settled_on: day("2026-01-12"),
                occurred_on: None,
                billed_amount_minor: None,
                create_card_schedule: Some(true),
                description: None,
            },
            ReconcileOccurrenceInput {
                subscription_id: sub2,
                scheduled_date: day("2026-01-20"),
                actual_jpy: None, // invalid for USD!
                settled_on: day("2026-01-22"),
                occurred_on: None,
                billed_amount_minor: None,
                create_card_schedule: Some(true),
                description: None,
            },
        ],
    };
    let err = f.reconcile_subscription_batch(id(), bad_batch).unwrap_err();
    assert_eq!(err.code, codes::INVALID_AMOUNT);
    assert_eq!(f.list_events(None, 1, 10).unwrap().total, 0);

    // Valid batch reconciliation at month-end
    let batch_key = id();
    let good_batch = ReconcileBatchInput {
        items: vec![
            ReconcileOccurrenceInput {
                subscription_id: sub1,
                scheduled_date: day("2026-01-10"),
                actual_jpy: Some(3_142),
                settled_on: day("2026-01-12"),
                occurred_on: None, // defaults to billing date 2026-01-10
                billed_amount_minor: None,
                create_card_schedule: Some(true),
                description: None,
            },
            ReconcileOccurrenceInput {
                subscription_id: sub2,
                scheduled_date: day("2026-01-20"),
                actual_jpy: Some(1_571),
                settled_on: day("2026-01-22"),
                occurred_on: None,
                billed_amount_minor: None,
                create_card_schedule: Some(true),
                description: None,
            },
        ],
    };
    let posted = f
        .reconcile_subscription_batch(batch_key, good_batch.clone())
        .unwrap();
    assert!(!posted.is_replay);
    assert_eq!(posted.result.len(), 2);
    assert_eq!(posted.result[0].event.amount, 3_142);
    assert_eq!(posted.result[0].event.occurred_on, Some(day("2026-01-10")));
    assert_eq!(
        posted.result[0].event.description.as_deref(),
        Some("ChatGPT Plus ($20.00)")
    );
    assert!(posted.result[0].occurrence.schedule_id.is_some());
    assert!(posted.result[1].occurrence.schedule_id.is_some());

    // Exact replay with same idempotency key
    let replay = f
        .reconcile_subscription_batch(batch_key, good_batch.clone())
        .unwrap();
    assert!(replay.is_replay);
    assert_eq!(replay.result, posted.result);
    assert_eq!(f.list_events(None, 1, 10).unwrap().total, 2);

    // Same key, changed payload -> IDEMPOTENCY_CONFLICT
    let mut changed = good_batch;
    changed.items[0].actual_jpy = Some(3_200);
    assert_eq!(
        f.reconcile_subscription_batch(batch_key, changed)
            .unwrap_err()
            .code,
        codes::IDEMPOTENCY_CONFLICT
    );

    // Card balance reflects actual JPY deducted (-4,713 owed)
    let bal = f.account_balance(card, day("2026-01-31")).unwrap();
    assert_eq!(bal.amount_owed, Some(4_713));

    // Card schedules: Jan 10 closes Jan 15 -> due Feb 10; Jan 20 closes Feb 15 -> due Mar 10
    let schedules = f.card_schedules(card).unwrap();
    assert_eq!(schedules.len(), 2);
    assert_eq!(schedules[0].terms.due_date, day("2026-02-10"));
    assert_eq!(schedules[0].terms.principal_due, 3_142);
    assert_eq!(schedules[1].terms.due_date, day("2026-03-10"));
    assert_eq!(schedules[1].terms.principal_due, 1_571);

    // Overview surfaces last_reconciled_jpy on each subscription and last month reconciled foreign JPY
    let feb_overview = f.list_subscriptions(false, day("2026-02-05")).unwrap();
    assert_eq!(feb_overview.last_month_reconciled_foreign_jpy, 4_713);
    assert_eq!(feb_overview.pending_reconciliation_count, 0);
    let chatgpt = feb_overview
        .items
        .iter()
        .find(|i| i.subscription.id == sub1)
        .unwrap();
    assert_eq!(chatgpt.last_reconciled_jpy, Some(3_142));
}

#[test]
fn forecast_availability_avoid_fake_usd_and_avoid_card_double_counting() {
    let (_dir, db) = setup();
    let f = Finance::new(&db);
    let (bank, card, cat) = bootstrap_accounts(&f);

    // JPY liquid subscription (Bank, ¥5,000 on 15th)
    let jpy_bank = f
        .create_subscription(
            id(),
            CreateSubscriptionInput {
                name: "Fiber Internet".into(),
                merchant: None,
                account_id: bank,
                category_id: cat,
                currency: BillingCurrency::Jpy,
                amount_minor: 5_000,
                cadence: SubscriptionCadence::Monthly,
                billing_day: Some(15),
                billing_month: None,
                start_date: day("2026-02-15"),
                note: None,
            },
        )
        .unwrap()
        .result
        .subscription
        .id;

    // JPY card subscription (Visa, ¥1,300 on 5th -> closes 15th -> due Mar 10 on bank)
    let jpy_card = f
        .create_subscription(
            id(),
            CreateSubscriptionInput {
                name: "iCloud+".into(),
                merchant: Some("Apple".into()),
                account_id: card,
                category_id: cat,
                currency: BillingCurrency::Jpy,
                amount_minor: 1_300,
                cadence: SubscriptionCadence::Monthly,
                billing_day: Some(5),
                billing_month: None,
                start_date: day("2026-02-05"),
                note: None,
            },
        )
        .unwrap()
        .result
        .subscription
        .id;

    // USD subscription ($20.00 on 10th) -> NEVER produces fake JPY movement
    let _usd_card = f
        .create_subscription(
            id(),
            CreateSubscriptionInput {
                name: "Claude Pro".into(),
                merchant: Some("Anthropic".into()),
                account_id: card,
                category_id: cat,
                currency: BillingCurrency::Usd,
                amount_minor: 2000,
                cadence: SubscriptionCadence::Monthly,
                billing_day: Some(10),
                billing_month: None,
                start_date: day("2026-02-10"),
                note: None,
            },
        )
        .unwrap()
        .result
        .subscription
        .id;

    // Forecast Feb 1 .. Mar 15:
    // - Feb 15: -5,000 (Fiber Internet, confirmed)
    // - Mar 10: -1,300 (iCloud+ card estimate due from Feb 5 billing)
    // - Mar 15: -5,000 (Fiber Internet, confirmed)
    // - Unreconciled foreign: 2 USD charges ($40.00: Feb 10 & Mar 10) -> warning, zero JPY movements!
    let fc = f
        .cash_forecast(day("2026-02-01"), day("2026-03-15"), false)
        .unwrap();
    let feb15 = fc
        .days
        .iter()
        .find(|d| d.account_id == bank && d.date == day("2026-02-15"))
        .unwrap();
    assert_eq!(feb15.movements.len(), 1);
    assert_eq!(feb15.movements[0].amount, -5_000);
    assert_eq!(feb15.movements[0].source, "subscription");

    let mar10 = fc
        .days
        .iter()
        .find(|d| d.account_id == bank && d.date == day("2026-03-10"))
        .unwrap();
    assert_eq!(mar10.movements.len(), 1);
    assert_eq!(mar10.movements[0].amount, -1_300);
    assert_eq!(mar10.movements[0].source, "subscription_card_estimate");

    assert_eq!(fc.unreconciled_foreign_subscriptions.len(), 1);
    assert_eq!(fc.unreconciled_foreign_subscriptions[0].count, 2);
    assert_eq!(fc.unreconciled_foreign_subscriptions[0].total_minor, 4000);
    assert!(
        fc.warnings
            .iter()
            .any(|w| w.contains("USD subscription charge(s) ($40.00)"))
    );

    // Available cash Feb 1 .. Feb 28 subtracts confirmed JPY liquid subscription (¥5,000) only
    let avail = f
        .available_cash(day("2026-02-01"), day("2026-02-28"))
        .unwrap();
    let bank_avail = avail
        .accounts
        .iter()
        .find(|a| a.account_id == bank)
        .unwrap();
    assert_eq!(bank_avail.unfunded_known_due, 5_000);
    assert_eq!(bank_avail.spendable, Some(195_000));
    assert_eq!(avail.unreconciled_foreign_subscriptions.len(), 1);
    assert_eq!(
        avail.unreconciled_foreign_subscriptions[0].total_minor,
        2000
    );

    // Now reconcile the Feb 5 iCloud+ JPY card charge (defaults actual_jpy = 1_300 & creates card_schedule for Mar 10)
    f.reconcile_subscription_occurrence(
        id(),
        ReconcileOccurrenceInput {
            subscription_id: jpy_card,
            scheduled_date: day("2026-02-05"),
            actual_jpy: None,
            settled_on: day("2026-02-05"),
            occurred_on: None,
            billed_amount_minor: None,
            create_card_schedule: Some(true),
            description: None,
        },
    )
    .unwrap();

    // Forecast Mar 10 must show ONLY the real card_schedule (-1,300), NOT double-counted!
    let fc_after = f
        .cash_forecast(day("2026-02-06"), day("2026-03-15"), false)
        .unwrap();
    let mar10_after = fc_after
        .days
        .iter()
        .find(|d| d.account_id == bank && d.date == day("2026-03-10"))
        .unwrap();
    assert_eq!(mar10_after.movements.len(), 1);
    assert_eq!(mar10_after.movements[0].amount, -1_300);
    assert_eq!(mar10_after.movements[0].source, "card_schedule");

    // Reconciling JPY bank subscription also avoids double-counting on Feb 15
    f.reconcile_subscription_occurrence(
        id(),
        ReconcileOccurrenceInput {
            subscription_id: jpy_bank,
            scheduled_date: day("2026-02-15"),
            actual_jpy: None,
            settled_on: day("2026-02-15"),
            occurred_on: None,
            billed_amount_minor: None,
            create_card_schedule: None,
            description: None,
        },
    )
    .unwrap();
    let fc_feb = f
        .cash_forecast(day("2026-02-01"), day("2026-02-20"), false)
        .unwrap();
    let feb15_after = fc_feb
        .days
        .iter()
        .find(|d| d.account_id == bank && d.date == day("2026-02-15"))
        .unwrap();
    assert_eq!(feb15_after.movements.len(), 1);
    assert_eq!(feb15_after.movements[0].source, "actual");
}

#[test]
fn match_unlink_archive_guards_and_ledger_quality() {
    let (_dir, db) = setup();
    let f = Finance::new(&db);
    let (bank, card, cat) = bootstrap_accounts(&f);

    let sub_id = f
        .create_subscription(
            id(),
            CreateSubscriptionInput {
                name: "Linear".into(),
                merchant: Some("Linear".into()),
                account_id: card,
                category_id: cat,
                currency: BillingCurrency::Usd,
                amount_minor: 800,
                cadence: SubscriptionCadence::Monthly,
                billing_day: Some(15),
                billing_month: None,
                start_date: day("2026-01-15"),
                note: None,
            },
        )
        .unwrap()
        .result
        .subscription
        .id;

    // Cannot archive account or category referenced by an active subscription
    assert_eq!(
        f.archive_account(id(), card, true).unwrap_err().code,
        codes::SUBSCRIPTION_ACTIVE_REFERENCE
    );
    assert_eq!(
        f.archive_category(id(), cat, true).unwrap_err().code,
        codes::SUBSCRIPTION_ACTIVE_REFERENCE
    );

    // On Feb 5, Jan 15 USD cycle is unreconciled from prior completed month -> ledger_quality flags it!
    let issues = f.ledger_quality(day("2026-02-05")).unwrap();
    assert!(
        issues
            .iter()
            .any(|i| i.kind == "unreconciled_subscription" && i.record_id == sub_id)
    );

    // User manually recorded an expense of ¥1,248 earlier -> match it to the Jan 15 cycle
    let manual = f
        .post_event(
            id(),
            EventInput {
                kind: EventKind::Expense,
                amount: 1_248,
                account_id: card,
                destination_account_id: None,
                category_id: Some(cat),
                description: Some("Linear Jan".into()),
                occurred_on: day("2026-01-15"),
                effective_date: day("2026-01-17"),
                original_event_id: None,
                adjustment_delta: None,
            },
        )
        .unwrap()
        .result;

    // Wrong account expense cannot be matched
    let bank_exp = f
        .post_event(
            id(),
            EventInput {
                kind: EventKind::Expense,
                amount: 1_248,
                account_id: bank,
                destination_account_id: None,
                category_id: Some(cat),
                description: Some("Other".into()),
                occurred_on: day("2026-01-15"),
                effective_date: day("2026-01-15"),
                original_event_id: None,
                adjustment_delta: None,
            },
        )
        .unwrap()
        .result;
    assert_eq!(
        f.match_subscription_occurrence(
            id(),
            MatchOccurrenceInput {
                subscription_id: sub_id,
                scheduled_date: day("2026-01-15"),
                event_id: bank_exp.id,
                billed_amount_minor: None,
            },
        )
        .unwrap_err()
        .category,
        ErrorCategory::Validation
    );

    let matched = f
        .match_subscription_occurrence(
            id(),
            MatchOccurrenceInput {
                subscription_id: sub_id,
                scheduled_date: day("2026-01-15"),
                event_id: manual.id,
                billed_amount_minor: None,
            },
        )
        .unwrap()
        .result;
    assert_eq!(matched.status, OccurrenceStatus::Reconciled);
    assert_eq!(matched.actual_jpy, Some(1_248));

    // Prior-month quality issue clears
    let issues_clean = f.ledger_quality(day("2026-02-05")).unwrap();
    assert!(
        !issues_clean
            .iter()
            .any(|i| i.kind == "unreconciled_subscription" && i.record_id == sub_id)
    );

    // If the linked event is reversed later, ledger_quality flags reversed_subscription_event
    f.reverse_event(id(), manual.id, "Duplicate entry").unwrap();
    let issues_rev = f.ledger_quality(day("2026-02-05")).unwrap();
    assert!(
        issues_rev
            .iter()
            .any(|i| i.kind == "reversed_subscription_event" && i.record_id == sub_id)
    );

    // Unlink returns cycle to needs_reconciliation cleanly
    let unlinked = f
        .unlink_subscription_occurrence(
            id(),
            UnlinkOccurrenceInput {
                subscription_id: sub_id,
                scheduled_date: day("2026-01-15"),
            },
        )
        .unwrap()
        .result;
    assert_eq!(unlinked.status, OccurrenceStatus::NeedsReconciliation);
    assert_eq!(unlinked.event_id, None);
    assert_eq!(unlinked.actual_jpy, None);

    // Canceling subscription unblocks archiving account/category
    f.change_subscription_status(
        id(),
        StatusChangeInput {
            subscription_id: sub_id,
            status: SubscriptionStatus::Canceled,
            effective_on: day("2026-02-01"),
        },
    )
    .unwrap();
    assert!(f.archive_account(id(), card, true).is_ok());
    assert!(f.archive_category(id(), cat, true).is_ok());
}
