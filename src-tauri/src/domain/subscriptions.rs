use super::cards::{
    ScheduleInput, ScheduleSource, card_settings_row, day_of_month, insert_schedule, next_due_date,
};
use super::ledger::*;
use super::review::QualityIssue;
use super::*;
use crate::commands::wire::{MAX_DESCRIPTION_CHARS, MAX_NAME_CHARS, positive_minor_units};
use chrono::{Datelike, Days, Months};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const MAX_OCCURRENCE_SPAN_DAYS: i64 = 1_100;
const NEXT_BILLING_LOOKAHEAD_DAYS: i64 = 400;

stored_enum!(BillingCurrency {
    Jpy => "JPY",
    Usd => "USD",
});

impl BillingCurrency {
    pub fn is_foreign(self) -> bool {
        self != Self::Jpy
    }

    pub fn parse_str(raw: &str) -> Result<Self, AppError> {
        match raw {
            "JPY" => Ok(Self::Jpy),
            "USD" => Ok(Self::Usd),
            _ => Err(AppError::invalid(
                codes::INVALID_CURRENCY,
                "currency",
                "Select JPY or USD.",
            )),
        }
    }

    pub fn format_minor(self, minor: i64) -> String {
        match self {
            Self::Jpy => format!("¥{minor}"),
            Self::Usd => format!("${}.{:02}", minor / 100, (minor % 100).abs()),
        }
    }
}

stored_enum!(SubscriptionCadence {
    Weekly => "weekly",
    Monthly => "monthly",
    Quarterly => "quarterly",
    Yearly => "yearly",
});

impl SubscriptionCadence {
    pub fn parse_str(raw: &str) -> Result<Self, AppError> {
        match raw {
            "weekly" => Ok(Self::Weekly),
            "monthly" => Ok(Self::Monthly),
            "quarterly" => Ok(Self::Quarterly),
            "yearly" => Ok(Self::Yearly),
            _ => Err(AppError::invalid(
                codes::INVALID_CADENCE,
                "cadence",
                "Select weekly, monthly, quarterly, or yearly.",
            )),
        }
    }

    /// Monthly normalized equivalent in minor units using integer half-up rounding.
    pub fn monthly_normalized_minor(self, amount_minor: i64) -> Result<i64, AppError> {
        let a = i128::from(amount_minor);
        let value = match self {
            Self::Monthly => a,
            Self::Quarterly => (a * 2 + 3) / 6,
            Self::Yearly => (a * 2 + 12) / 24,
            Self::Weekly => (a * 52 * 2 + 12) / 24,
        };
        yen(
            i64::try_from(value).map_err(|_| invalid("amountMinor", "The amount is too large."))?,
            "amountMinor",
        )
    }
}

stored_enum!(SubscriptionStatus {
    Active => "active",
    Paused => "paused",
    Canceled => "canceled",
});

impl SubscriptionStatus {
    pub fn parse_str(raw: &str) -> Result<Self, AppError> {
        match raw {
            "active" => Ok(Self::Active),
            "paused" => Ok(Self::Paused),
            "canceled" => Ok(Self::Canceled),
            _ => Err(invalid("status", "Select active, paused, or canceled.")),
        }
    }
}

stored_enum!(OccurrenceDbState {
    Scheduled => "scheduled",
    Skipped => "skipped",
    Reconciled => "reconciled",
});

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OccurrenceStatus {
    Upcoming,
    Due,
    NeedsReconciliation,
    Skipped,
    Reconciled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlanRevision {
    pub id: Uuid,
    pub subscription_id: Uuid,
    pub effective_from: NaiveDate,
    pub amount_minor: i64,
    pub revision: u32,
    pub reason: String,
    pub recorded_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    pub id: Uuid,
    pub name: String,
    pub merchant: Option<String>,
    pub account_id: Uuid,
    pub category_id: Uuid,
    pub currency: BillingCurrency,
    pub cadence: SubscriptionCadence,
    pub billing_day: Option<u32>,
    pub billing_month: Option<u32>,
    pub start_date: NaiveDate,
    pub status: SubscriptionStatus,
    pub paused_from: Option<NaiveDate>,
    pub canceled_effective_on: Option<NaiveDate>,
    pub note: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionListItem {
    #[serde(flatten)]
    pub subscription: Subscription,
    pub current_amount_minor: i64,
    pub next_billing_date: Option<NaiveDate>,
    pub last_reconciled_jpy: Option<i64>,
    pub last_reconciled_on: Option<NaiveDate>,
    pub account_name: String,
    pub account_kind: AccountKind,
    pub category_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ForeignCurrencySummary {
    pub currency: BillingCurrency,
    pub count: usize,
    pub total_minor: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionOverview {
    pub as_of_date: NaiveDate,
    pub items: Vec<SubscriptionListItem>,
    pub jpy_monthly_normalized: i64,
    pub jpy_due_this_month: i64,
    pub foreign_monthly_normalized: Vec<ForeignCurrencySummary>,
    pub last_month_reconciled_foreign_jpy: i64,
    pub pending_reconciliation_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionOccurrence {
    pub subscription_id: Uuid,
    pub subscription_name: String,
    pub merchant: Option<String>,
    pub account_id: Uuid,
    pub account_name: String,
    pub account_kind: AccountKind,
    pub has_card_settings: bool,
    pub category_id: Uuid,
    pub category_name: String,
    pub scheduled_date: NaiveDate,
    pub override_date: Option<NaiveDate>,
    pub effective_date: NaiveDate,
    pub status: OccurrenceStatus,
    pub billed_currency: BillingCurrency,
    pub billed_amount_minor: i64,
    pub actual_jpy: Option<i64>,
    pub settled_on: Option<NaiveDate>,
    pub event_id: Option<Uuid>,
    pub schedule_id: Option<Uuid>,
    pub last_reconciled_jpy: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionDetail {
    pub subscription: SubscriptionListItem,
    pub revisions: Vec<PlanRevision>,
    pub occurrences: Vec<SubscriptionOccurrence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateSubscriptionInput {
    pub name: String,
    pub merchant: Option<String>,
    pub account_id: Uuid,
    pub category_id: Uuid,
    pub currency: BillingCurrency,
    pub amount_minor: i64,
    pub cadence: SubscriptionCadence,
    pub billing_day: Option<u32>,
    pub billing_month: Option<u32>,
    pub start_date: NaiveDate,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSubscriptionInput {
    pub subscription_id: Uuid,
    pub name: String,
    pub merchant: Option<String>,
    pub account_id: Uuid,
    pub category_id: Uuid,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PriceRevisionInput {
    pub subscription_id: Uuid,
    pub effective_from: NaiveDate,
    pub amount_minor: i64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StatusChangeInput {
    pub subscription_id: Uuid,
    pub status: SubscriptionStatus,
    pub effective_on: NaiveDate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OccurrenceOverrideInput {
    pub subscription_id: Uuid,
    pub scheduled_date: NaiveDate,
    pub override_date: Option<NaiveDate>,
    pub skipped: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileOccurrenceInput {
    pub subscription_id: Uuid,
    pub scheduled_date: NaiveDate,
    pub actual_jpy: Option<i64>,
    pub settled_on: NaiveDate,
    pub occurred_on: Option<NaiveDate>,
    pub billed_amount_minor: Option<i64>,
    pub create_card_schedule: Option<bool>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileBatchInput {
    pub items: Vec<ReconcileOccurrenceInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReconciledCharge {
    pub occurrence: SubscriptionOccurrence,
    pub event: Event,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MatchOccurrenceInput {
    pub subscription_id: Uuid,
    pub scheduled_date: NaiveDate,
    pub event_id: Uuid,
    pub billed_amount_minor: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UnlinkOccurrenceInput {
    pub subscription_id: Uuid,
    pub scheduled_date: NaiveDate,
}

#[derive(Debug, Clone)]
struct StoredOccurrence {
    override_date: Option<NaiveDate>,
    state: OccurrenceDbState,
    billed_currency: BillingCurrency,
    billed_amount_minor: i64,
    actual_jpy: Option<i64>,
    settled_on: Option<NaiveDate>,
    event_id: Option<Uuid>,
    schedule_id: Option<Uuid>,
}

fn month_start(on: NaiveDate) -> NaiveDate {
    on.with_day(1).expect("valid month")
}

fn clamped_day(month: NaiveDate, day: u32) -> NaiveDate {
    (1..=day)
        .rev()
        .find_map(|d| month.with_day(d))
        .expect("validated day and month")
}

fn subscription_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Subscription> {
    Ok(Subscription {
        id: id(r, 0)?,
        name: r.get(1)?,
        merchant: r.get(2)?,
        account_id: id(r, 3)?,
        category_id: id(r, 4)?,
        currency: BillingCurrency::read(r, 5)?,
        cadence: SubscriptionCadence::read(r, 6)?,
        billing_day: r.get(7)?,
        billing_month: r.get(8)?,
        start_date: r.get(9)?,
        status: SubscriptionStatus::read(r, 10)?,
        paused_from: r.get(11)?,
        canceled_effective_on: r.get(12)?,
        note: r.get(13)?,
        created_at: r.get(14)?,
        updated_at: r.get(15)?,
    })
}

fn load_subscription(db: &Connection, subscription_id: Uuid) -> Result<Subscription, AppError> {
    Ok(db.query_row(
        "SELECT id,name,merchant,account_id,category_id,currency,cadence,billing_day,billing_month,
                start_date,status,paused_from,canceled_effective_on,note,created_at,updated_at
         FROM subscriptions WHERE id=?1",
        [key(&subscription_id)],
        subscription_row,
    )?)
}

fn load_subscriptions(
    db: &Connection,
    include_canceled: bool,
) -> Result<Vec<Subscription>, AppError> {
    rows(
        db,
        "SELECT id,name,merchant,account_id,category_id,currency,cadence,billing_day,billing_month,
                start_date,status,paused_from,canceled_effective_on,note,created_at,updated_at
         FROM subscriptions
         WHERE ?1 OR status <> 'canceled'
         ORDER BY CASE status WHEN 'active' THEN 0 WHEN 'paused' THEN 1 ELSE 2 END, name, id",
        [include_canceled],
        subscription_row,
    )
}

fn load_revisions(db: &Connection, subscription_id: Uuid) -> Result<Vec<PlanRevision>, AppError> {
    rows(
        db,
        "SELECT id,subscription_id,effective_from,amount_minor,revision,reason,recorded_at
         FROM subscription_plan_revisions
         WHERE subscription_id=?1
         ORDER BY effective_from ASC, revision ASC",
        [key(&subscription_id)],
        |r| {
            Ok(PlanRevision {
                id: id(r, 0)?,
                subscription_id: id(r, 1)?,
                effective_from: r.get(2)?,
                amount_minor: r.get(3)?,
                revision: r.get(4)?,
                reason: r.get(5)?,
                recorded_at: r.get(6)?,
            })
        },
    )
}

fn amount_for_date(revisions: &[PlanRevision], on: NaiveDate) -> Result<i64, AppError> {
    revisions
        .iter()
        .rev()
        .find(|rev| rev.effective_from <= on)
        .or_else(|| revisions.first())
        .map(|rev| rev.amount_minor)
        .ok_or_else(|| {
            AppError::storage(
                codes::STORAGE_FAILURE,
                "Subscription is missing a plan revision.",
            )
        })
}

fn load_stored_occurrences(
    db: &Connection,
    subscription_id: Uuid,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> Result<BTreeMap<NaiveDate, StoredOccurrence>, AppError> {
    let list = match (from, to) {
        (Some(from), Some(to)) => rows(
            db,
            "SELECT scheduled_date,override_date,state,billed_currency,billed_amount_minor,
                    actual_jpy,settled_on,event_id,schedule_id
             FROM subscription_occurrences
             WHERE subscription_id=?1
               AND (scheduled_date BETWEEN ?2 AND ?3 OR override_date BETWEEN ?2 AND ?3)",
            params![key(&subscription_id), from, to],
            parse_stored_occurrence,
        )?,
        _ => rows(
            db,
            "SELECT scheduled_date,override_date,state,billed_currency,billed_amount_minor,
                    actual_jpy,settled_on,event_id,schedule_id
             FROM subscription_occurrences
             WHERE subscription_id=?1",
            [key(&subscription_id)],
            parse_stored_occurrence,
        )?,
    };
    Ok(list.into_iter().collect())
}

fn parse_stored_occurrence(
    r: &rusqlite::Row<'_>,
) -> rusqlite::Result<(NaiveDate, StoredOccurrence)> {
    Ok((
        r.get(0)?,
        StoredOccurrence {
            override_date: r.get(1)?,
            state: OccurrenceDbState::read(r, 2)?,
            billed_currency: BillingCurrency::read(r, 3)?,
            billed_amount_minor: r.get(4)?,
            actual_jpy: r.get(5)?,
            settled_on: r.get(6)?,
            event_id: optional_id(r, 7)?,
            schedule_id: optional_id(r, 8)?,
        },
    ))
}

fn load_stored_occurrence(
    db: &Connection,
    subscription_id: Uuid,
    scheduled_date: NaiveDate,
) -> Result<Option<StoredOccurrence>, AppError> {
    Ok(db
        .query_row(
            "SELECT scheduled_date,override_date,state,billed_currency,billed_amount_minor,
                    actual_jpy,settled_on,event_id,schedule_id
             FROM subscription_occurrences
             WHERE subscription_id=?1 AND scheduled_date=?2",
            params![key(&subscription_id), scheduled_date],
            parse_stored_occurrence,
        )
        .optional()?
        .map(|(_, occ)| occ))
}

fn last_reconciled(
    db: &Connection,
    subscription_id: Uuid,
) -> Result<(Option<i64>, Option<NaiveDate>), AppError> {
    Ok(db
        .query_row(
            "SELECT actual_jpy, settled_on FROM subscription_occurrences
             WHERE subscription_id=?1 AND state='reconciled'
             ORDER BY scheduled_date DESC, settled_on DESC LIMIT 1",
            [key(&subscription_id)],
            |r| {
                Ok((
                    r.get::<_, Option<i64>>(0)?,
                    r.get::<_, Option<NaiveDate>>(1)?,
                ))
            },
        )
        .optional()?
        .unwrap_or((None, None)))
}

fn active_end_exclusive(sub: &Subscription) -> Option<NaiveDate> {
    match sub.status {
        SubscriptionStatus::Active => None,
        SubscriptionStatus::Paused => sub.paused_from,
        SubscriptionStatus::Canceled => sub.canceled_effective_on,
    }
}

fn generated_dates(
    sub: &Subscription,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Vec<NaiveDate>, AppError> {
    let mut upper = to;
    if let Some(stop) = active_end_exclusive(sub) {
        if stop <= from {
            return Ok(Vec::new());
        }
        if let Some(prev) = stop.pred_opt() {
            upper = upper.min(prev);
        } else {
            return Ok(Vec::new());
        }
    }
    if upper < sub.start_date || upper < from {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    match sub.cadence {
        SubscriptionCadence::Weekly => {
            let start = sub.start_date;
            let offset_days = (from - start).num_days().max(0);
            let weeks = (offset_days + 6) / 7;
            let mut current = start
                .checked_add_days(Days::new((weeks * 7) as u64))
                .ok_or_else(|| invalid("to", "Date range is too large."))?;
            while current <= upper {
                if current >= from {
                    out.push(current);
                }
                current = match current.checked_add_days(Days::new(7)) {
                    Some(next) => next,
                    None => break,
                };
            }
        }
        SubscriptionCadence::Monthly => {
            let anchor_day = sub.billing_day.unwrap_or(sub.start_date.day());
            let mut month = month_start(from.max(sub.start_date));
            while month <= upper {
                let on = clamped_day(month, anchor_day);
                if on >= sub.start_date && on >= from && on <= upper {
                    out.push(on);
                }
                month = match month.checked_add_months(Months::new(1)) {
                    Some(next) => next,
                    None => break,
                };
            }
        }
        SubscriptionCadence::Quarterly => {
            let anchor_day = sub.billing_day.unwrap_or(sub.start_date.day());
            let start_m = month_start(sub.start_date);
            let target_m = month_start(from.max(sub.start_date));
            let diff_months = (target_m.year() - start_m.year()) * 12
                + (target_m.month() as i32 - start_m.month() as i32);
            let steps = ((diff_months.max(0) + 2) / 3) as u32;
            let mut month = start_m
                .checked_add_months(Months::new(steps * 3))
                .ok_or_else(|| invalid("to", "Date range is too large."))?;
            // Step back one quarter in case target_m was already on the anchor month boundary
            if steps > 0
                && let Some(prev) = month.checked_sub_months(Months::new(3))
                && clamped_day(prev, anchor_day) >= from
            {
                month = prev;
            }
            while month <= upper {
                let on = clamped_day(month, anchor_day);
                if on >= sub.start_date && on >= from && on <= upper {
                    out.push(on);
                }
                month = match month.checked_add_months(Months::new(3)) {
                    Some(next) => next,
                    None => break,
                };
            }
        }
        SubscriptionCadence::Yearly => {
            let anchor_month = sub.billing_month.unwrap_or(sub.start_date.month());
            let anchor_day = sub.billing_day.unwrap_or(sub.start_date.day());
            for year in from.max(sub.start_date).year()..=upper.year() {
                if let Some(month) = NaiveDate::from_ymd_opt(year, anchor_month, 1) {
                    let on = clamped_day(month, anchor_day);
                    if on >= sub.start_date && on >= from && on <= upper {
                        out.push(on);
                    }
                }
            }
        }
    }
    Ok(out)
}

fn is_valid_scheduled_date(sub: &Subscription, target: NaiveDate) -> bool {
    if target < sub.start_date {
        return false;
    }
    if let Some(stop) = active_end_exclusive(sub)
        && target >= stop
    {
        return false;
    }
    match sub.cadence {
        SubscriptionCadence::Weekly => (target - sub.start_date).num_days() % 7 == 0,
        SubscriptionCadence::Monthly => {
            let anchor_day = sub.billing_day.unwrap_or(sub.start_date.day());
            target == clamped_day(month_start(target), anchor_day)
        }
        SubscriptionCadence::Quarterly => {
            let anchor_day = sub.billing_day.unwrap_or(sub.start_date.day());
            let start_m = month_start(sub.start_date);
            let target_m = month_start(target);
            let diff = (target_m.year() - start_m.year()) * 12
                + (target_m.month() as i32 - start_m.month() as i32);
            diff >= 0 && diff % 3 == 0 && target == clamped_day(target_m, anchor_day)
        }
        SubscriptionCadence::Yearly => {
            let anchor_month = sub.billing_month.unwrap_or(sub.start_date.month());
            let anchor_day = sub.billing_day.unwrap_or(sub.start_date.day());
            target.month() == anchor_month && target == clamped_day(month_start(target), anchor_day)
        }
    }
}

fn build_occurrences_for_subscription(
    db: &Connection,
    sub: &Subscription,
    from: NaiveDate,
    to: NaiveDate,
    as_of_date: NaiveDate,
) -> Result<Vec<SubscriptionOccurrence>, AppError> {
    let revisions = load_revisions(db, sub.id)?;
    let stored = load_stored_occurrences(db, sub.id, Some(from), Some(to))?;
    let acc = account(db, sub.account_id)?;
    let cat = category(db, sub.category_id)?;
    let has_card_settings =
        acc.kind == AccountKind::CreditCard && card_settings_row(db, sub.account_id)?.is_some();
    let (last_jpy, _) = last_reconciled(db, sub.id)?;

    let mut candidate_dates: BTreeSet<NaiveDate> =
        generated_dates(sub, from, to)?.into_iter().collect();
    candidate_dates.extend(stored.keys().copied());

    let mut result = Vec::new();
    for scheduled_date in candidate_dates {
        let row = stored.get(&scheduled_date);
        let override_date = row.and_then(|r| r.override_date);
        let effective_date = override_date.unwrap_or(scheduled_date);
        if !(from..=to).contains(&effective_date) && !(from..=to).contains(&scheduled_date) {
            continue;
        }
        let billed_currency = row.map_or(sub.currency, |r| r.billed_currency);
        let billed_amount_minor = match row {
            Some(r) => r.billed_amount_minor,
            None => amount_for_date(&revisions, scheduled_date)?,
        };
        let status = match row.map(|r| r.state) {
            Some(OccurrenceDbState::Skipped) => OccurrenceStatus::Skipped,
            Some(OccurrenceDbState::Reconciled) => OccurrenceStatus::Reconciled,
            None | Some(OccurrenceDbState::Scheduled) => {
                if effective_date > as_of_date {
                    OccurrenceStatus::Upcoming
                } else if billed_currency.is_foreign() {
                    OccurrenceStatus::NeedsReconciliation
                } else {
                    OccurrenceStatus::Due
                }
            }
        };
        result.push(SubscriptionOccurrence {
            subscription_id: sub.id,
            subscription_name: sub.name.clone(),
            merchant: sub.merchant.clone(),
            account_id: sub.account_id,
            account_name: acc.name.clone(),
            account_kind: acc.kind,
            has_card_settings,
            category_id: sub.category_id,
            category_name: cat.name.clone(),
            scheduled_date,
            override_date,
            effective_date,
            status,
            billed_currency,
            billed_amount_minor,
            actual_jpy: row.and_then(|r| r.actual_jpy),
            settled_on: row.and_then(|r| r.settled_on),
            event_id: row.and_then(|r| r.event_id),
            schedule_id: row.and_then(|r| r.schedule_id),
            last_reconciled_jpy: last_jpy,
        });
    }
    Ok(result)
}

fn build_list_item(
    db: &Connection,
    sub: &Subscription,
    as_of_date: NaiveDate,
) -> Result<SubscriptionListItem, AppError> {
    let revisions = load_revisions(db, sub.id)?;
    let current_amount_minor = amount_for_date(&revisions, as_of_date)?;
    let acc = account(db, sub.account_id)?;
    let cat = category(db, sub.category_id)?;
    let (last_reconciled_jpy, last_reconciled_on) = last_reconciled(db, sub.id)?;
    let next_billing_date = if sub.status == SubscriptionStatus::Active {
        let horizon = as_of_date
            .checked_add_days(Days::new(NEXT_BILLING_LOOKAHEAD_DAYS as u64))
            .unwrap_or(as_of_date);
        build_occurrences_for_subscription(db, sub, as_of_date, horizon, as_of_date)?
            .into_iter()
            .filter(|o| {
                o.effective_date >= as_of_date
                    && !matches!(
                        o.status,
                        OccurrenceStatus::Skipped | OccurrenceStatus::Reconciled
                    )
            })
            .map(|o| o.effective_date)
            .min()
    } else {
        None
    };

    Ok(SubscriptionListItem {
        subscription: sub.clone(),
        current_amount_minor,
        next_billing_date,
        last_reconciled_jpy,
        last_reconciled_on,
        account_name: acc.name,
        account_kind: acc.kind,
        category_name: cat.name,
    })
}

fn validate_account_and_category(
    db: &Connection,
    account_id: Uuid,
    category_id: Uuid,
) -> Result<(Account, Category), AppError> {
    let acc = account(db, account_id)?;
    active(&acc)?;
    let cat = category(db, category_id)?;
    if cat.kind != CategoryType::Expense || cat.archived_at.is_some() {
        return Err(invalid("categoryId", "Select an active expense category."));
    }
    Ok((acc, cat))
}

fn reconcile_occurrence_in_tx(
    db: &Connection,
    input: &ReconcileOccurrenceInput,
) -> Result<ReconciledCharge, AppError> {
    date(input.scheduled_date)?;
    date(input.settled_on)?;
    if let Some(occ) = input.occurred_on {
        date(occ)?;
    }

    let sub = load_subscription(db, input.subscription_id)?;
    let stored = load_stored_occurrence(db, sub.id, input.scheduled_date)?;
    if stored.is_none() && !is_valid_scheduled_date(&sub, input.scheduled_date) {
        return Err(invalid(
            "scheduledDate",
            "Select a scheduled billing date for this subscription.",
        ));
    }
    if let Some(ref existing) = stored {
        if existing.state == OccurrenceDbState::Reconciled {
            return Err(conflict(
                codes::OCCURRENCE_ALREADY_RECONCILED,
                "This subscription cycle has already been reconciled.",
            ));
        }
        if existing.state == OccurrenceDbState::Skipped {
            return Err(blocked("Restore the skipped cycle before reconciling it."));
        }
    }

    let revisions = load_revisions(db, sub.id)?;
    let default_minor = stored
        .as_ref()
        .map(|s| s.billed_amount_minor)
        .unwrap_or(amount_for_date(&revisions, input.scheduled_date)?);
    let billed_amount_minor = match input.billed_amount_minor {
        Some(v) => positive_minor_units(v, "billedAmountMinor")?,
        None => default_minor,
    };
    let billed_currency = stored
        .as_ref()
        .map(|s| s.billed_currency)
        .unwrap_or(sub.currency);

    let actual_jpy = match input.actual_jpy {
        Some(v) => positive_yen(v, "actualJpy")?,
        None if billed_currency == BillingCurrency::Jpy => {
            positive_yen(billed_amount_minor, "actualJpy")?
        }
        None => {
            return Err(AppError::invalid(
                codes::INVALID_AMOUNT,
                "actualJpy",
                "Enter the actual JPY deducted for foreign-currency subscriptions.",
            ));
        }
    };

    let override_date = stored.as_ref().and_then(|s| s.override_date);
    let billing_date = override_date.unwrap_or(input.scheduled_date);
    let occurred_on = input.occurred_on.unwrap_or(billing_date);
    let description = match text(
        input.description.as_deref(),
        "description",
        MAX_DESCRIPTION_CHARS,
        false,
    )? {
        Some(custom) => custom,
        None if billed_currency.is_foreign() => {
            format!(
                "{} ({})",
                sub.name,
                billed_currency.format_minor(billed_amount_minor)
            )
        }
        None => sub.name.clone(),
    };

    let event = make_event(
        db,
        &EventInput {
            kind: EventKind::Expense,
            amount: actual_jpy,
            account_id: sub.account_id,
            destination_account_id: None,
            category_id: Some(sub.category_id),
            description: Some(description),
            occurred_on,
            effective_date: input.settled_on,
            original_event_id: None,
            adjustment_delta: None,
        },
        None,
    )?;

    let acc = account(db, sub.account_id)?;
    let schedule_id =
        if acc.kind == AccountKind::CreditCard && input.create_card_schedule.unwrap_or(true) {
            if let Some(cfg) = card_settings_row(db, sub.account_id)? {
                let due_date = next_due_date(cfg.closing_day, cfg.payment_day, occurred_on)?;
                let created = insert_schedule(
                    db,
                    &ScheduleInput {
                        card_id: sub.account_id,
                        purchase_event_id: Some(event.id),
                        opening_obligation: None,
                        due_date,
                        principal_due: actual_jpy,
                        source: ScheduleSource::Estimate,
                    },
                )?;
                Some(created.id)
            } else {
                None
            }
        } else {
            None
        };

    let stamp = now();
    let occ_id = Uuid::new_v4();
    db.execute(
        "INSERT INTO subscription_occurrences
            (id,subscription_id,scheduled_date,override_date,state,billed_currency,
             billed_amount_minor,actual_jpy,settled_on,event_id,schedule_id,updated_at)
         VALUES (?1,?2,?3,?4,'reconciled',?5,?6,?7,?8,?9,?10,?11)
         ON CONFLICT(subscription_id,scheduled_date) DO UPDATE SET
            state='reconciled',
            billed_currency=excluded.billed_currency,
            billed_amount_minor=excluded.billed_amount_minor,
            actual_jpy=excluded.actual_jpy,
            settled_on=excluded.settled_on,
            event_id=excluded.event_id,
            schedule_id=excluded.schedule_id,
            updated_at=excluded.updated_at",
        params![
            key(&occ_id),
            key(&sub.id),
            input.scheduled_date,
            override_date,
            billed_currency.as_str(),
            billed_amount_minor,
            actual_jpy,
            input.settled_on,
            key(&event.id),
            schedule_id.as_ref().map(key),
            stamp
        ],
    )?;

    let occurrence = single_occurrence_view(db, &sub, input.scheduled_date, input.settled_on)?;
    Ok(ReconciledCharge { occurrence, event })
}

fn single_occurrence_view(
    db: &Connection,
    sub: &Subscription,
    scheduled_date: NaiveDate,
    as_of_date: NaiveDate,
) -> Result<SubscriptionOccurrence, AppError> {
    let stored = load_stored_occurrence(db, sub.id, scheduled_date)?;
    let revisions = load_revisions(db, sub.id)?;
    let acc = account(db, sub.account_id)?;
    let cat = category(db, sub.category_id)?;
    let has_card_settings =
        acc.kind == AccountKind::CreditCard && card_settings_row(db, sub.account_id)?.is_some();
    let (last_jpy, _) = last_reconciled(db, sub.id)?;
    let override_date = stored.as_ref().and_then(|s| s.override_date);
    let effective_date = override_date.unwrap_or(scheduled_date);
    let billed_currency = stored.as_ref().map_or(sub.currency, |s| s.billed_currency);
    let billed_amount_minor = match stored {
        Some(ref r) => r.billed_amount_minor,
        None => amount_for_date(&revisions, scheduled_date)?,
    };
    let status = match stored.as_ref().map(|s| s.state) {
        Some(OccurrenceDbState::Skipped) => OccurrenceStatus::Skipped,
        Some(OccurrenceDbState::Reconciled) => OccurrenceStatus::Reconciled,
        None | Some(OccurrenceDbState::Scheduled) => {
            if effective_date > as_of_date {
                OccurrenceStatus::Upcoming
            } else if billed_currency.is_foreign() {
                OccurrenceStatus::NeedsReconciliation
            } else {
                OccurrenceStatus::Due
            }
        }
    };

    Ok(SubscriptionOccurrence {
        subscription_id: sub.id,
        subscription_name: sub.name.clone(),
        merchant: sub.merchant.clone(),
        account_id: sub.account_id,
        account_name: acc.name,
        account_kind: acc.kind,
        has_card_settings,
        category_id: sub.category_id,
        category_name: cat.name,
        scheduled_date,
        override_date,
        effective_date,
        status,
        billed_currency,
        billed_amount_minor,
        actual_jpy: stored.as_ref().and_then(|s| s.actual_jpy),
        settled_on: stored.as_ref().and_then(|s| s.settled_on),
        event_id: stored.as_ref().and_then(|s| s.event_id),
        schedule_id: stored.as_ref().and_then(|s| s.schedule_id),
        last_reconciled_jpy: last_jpy,
    })
}

fn summarize_foreign(
    items: impl IntoIterator<Item = (BillingCurrency, i64)>,
) -> Result<Vec<ForeignCurrencySummary>, AppError> {
    let mut grouped = BTreeMap::<String, (BillingCurrency, usize, Vec<i64>)>::new();
    for (currency, minor) in items {
        if !currency.is_foreign() {
            continue;
        }
        let entry = grouped
            .entry(currency.as_str().to_string())
            .or_insert_with(|| (currency, 0, Vec::new()));
        entry.1 += 1;
        entry.2.push(minor);
    }
    grouped
        .into_values()
        .map(|(currency, count, amounts)| {
            Ok(ForeignCurrencySummary {
                currency,
                count,
                total_minor: total(amounts)?,
            })
        })
        .collect()
}

// ---- Cross-module helpers for planning.rs and review.rs --------------------

pub(super) struct SubscriptionForecastBundle {
    pub movements: Vec<(Uuid, NaiveDate, super::planning::Movement)>,
    pub missing_card_settings: bool,
    pub unreconciled_foreign: Vec<ForeignCurrencySummary>,
}

pub(super) fn forecast_items(
    db: &Connection,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<SubscriptionForecastBundle, AppError> {
    let subs = load_subscriptions(db, true)?;
    let mut movements = Vec::new();
    let mut missing_card_settings = false;
    let mut foreign_entries = Vec::new();
    let card_lookback = from.checked_sub_days(Days::new(65)).unwrap_or(from);

    for sub in &subs {
        let acc = account(db, sub.account_id)?;
        let scan_from = if acc.kind == AccountKind::CreditCard {
            card_lookback
        } else {
            from
        };
        let occurrences = build_occurrences_for_subscription(db, sub, scan_from, to, from)?;
        for occ in occurrences {
            if matches!(
                occ.status,
                OccurrenceStatus::Skipped | OccurrenceStatus::Reconciled
            ) {
                continue;
            }
            if occ.billed_currency.is_foreign() {
                if (from..=to).contains(&occ.effective_date) {
                    foreign_entries.push((occ.billed_currency, occ.billed_amount_minor));
                }
                continue;
            }
            if acc.kind.liquid() {
                if (from..=to).contains(&occ.effective_date) {
                    movements.push((
                        sub.account_id,
                        occ.effective_date,
                        super::planning::Movement {
                            id: sub.id,
                            amount: -occ.billed_amount_minor,
                            source: "subscription".into(),
                            label: sub.name.clone(),
                            certainty: "confirmed".into(),
                        },
                    ));
                }
            } else if let Some(cfg) = card_settings_row(db, sub.account_id)? {
                let due = next_due_date(cfg.closing_day, cfg.payment_day, occ.effective_date)?;
                if (from..=to).contains(&due) {
                    movements.push((
                        cfg.settlement_account_id,
                        due,
                        super::planning::Movement {
                            id: sub.id,
                            amount: -occ.billed_amount_minor,
                            source: "subscription_card_estimate".into(),
                            label: format!("{} (card)", sub.name),
                            certainty: "estimated".into(),
                        },
                    ));
                }
            } else if (from..=to).contains(&occ.effective_date) {
                missing_card_settings = true;
            }
        }
    }

    Ok(SubscriptionForecastBundle {
        movements,
        missing_card_settings,
        unreconciled_foreign: summarize_foreign(foreign_entries)?,
    })
}

pub(super) struct SubscriptionAvailabilityBundle {
    pub liquid_jpy_due: BTreeMap<Uuid, Vec<i64>>,
    pub unreconciled_foreign: Vec<ForeignCurrencySummary>,
}

pub(super) fn availability_items(
    db: &Connection,
    on: NaiveDate,
    horizon: NaiveDate,
) -> Result<SubscriptionAvailabilityBundle, AppError> {
    let subs = load_subscriptions(db, true)?;
    let mut liquid_jpy_due = BTreeMap::<Uuid, Vec<i64>>::new();
    let mut foreign_entries = Vec::new();
    for sub in &subs {
        let acc = account(db, sub.account_id)?;
        let occurrences = build_occurrences_for_subscription(db, sub, on, horizon, on)?;
        for occ in occurrences {
            if matches!(
                occ.status,
                OccurrenceStatus::Skipped | OccurrenceStatus::Reconciled
            ) {
                continue;
            }
            if !(on..=horizon).contains(&occ.effective_date) {
                continue;
            }
            if occ.billed_currency.is_foreign() {
                foreign_entries.push((occ.billed_currency, occ.billed_amount_minor));
            } else if acc.kind.liquid() {
                liquid_jpy_due
                    .entry(sub.account_id)
                    .or_default()
                    .push(occ.billed_amount_minor);
            }
        }
    }
    Ok(SubscriptionAvailabilityBundle {
        liquid_jpy_due,
        unreconciled_foreign: summarize_foreign(foreign_entries)?,
    })
}

pub(super) fn confirmed_jpy_bills_in_range(
    db: &Connection,
    category_id: Uuid,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<i64, AppError> {
    let subs = load_subscriptions(db, true)?;
    let mut charges = Vec::new();
    for sub in subs
        .into_iter()
        .filter(|s| s.category_id == category_id && !s.currency.is_foreign())
    {
        let occurrences = build_occurrences_for_subscription(db, &sub, from, to, from)?;
        for occ in occurrences {
            if !matches!(
                occ.status,
                OccurrenceStatus::Skipped | OccurrenceStatus::Reconciled
            ) && (from..=to).contains(&occ.effective_date)
            {
                charges.push(occ.billed_amount_minor);
            }
        }
    }
    total(charges)
}

pub(super) fn quality_issues(
    db: &Connection,
    on: NaiveDate,
) -> Result<Vec<QualityIssue>, AppError> {
    let mut issues = Vec::new();
    // 1. Reconciled occurrences whose linked financial event was later reversed.
    let reversed_links = rows(
        db,
        "SELECT o.subscription_id FROM subscription_occurrences o
         WHERE o.state='reconciled' AND o.event_id IS NOT NULL
           AND EXISTS(SELECT 1 FROM financial_events r WHERE r.reverses_event_id=o.event_id)
         ORDER BY o.subscription_id, o.scheduled_date",
        [],
        |r| id(r, 0),
    )?;
    for sub_id in reversed_links {
        issues.push(QualityIssue {
            kind: "reversed_subscription_event".into(),
            record_id: sub_id,
        });
    }

    // 2. Unreconciled occurrences from completed prior months (before current month start).
    let current_month_start = month_start(on);
    if let Some(prev_end) = current_month_start.pred_opt() {
        let lookback_start = current_month_start
            .checked_sub_months(Months::new(12))
            .unwrap_or(prev_end);
        let subs = load_subscriptions(db, true)?;
        for sub in &subs {
            let scan_from = lookback_start.max(sub.start_date);
            if scan_from > prev_end {
                continue;
            }
            let occurrences = build_occurrences_for_subscription(db, sub, scan_from, prev_end, on)?;
            let has_overdue = occurrences.iter().any(|occ| {
                occ.effective_date < current_month_start
                    && matches!(
                        occ.status,
                        OccurrenceStatus::NeedsReconciliation | OccurrenceStatus::Due
                    )
            });
            if has_overdue {
                issues.push(QualityIssue {
                    kind: "unreconciled_subscription".into(),
                    record_id: sub.id,
                });
            }
        }
    }
    Ok(issues)
}

// ---- Public Finance service methods ----------------------------------------

impl Finance<'_> {
    pub fn list_subscriptions(
        &self,
        include_canceled: bool,
        as_of_date: NaiveDate,
    ) -> Result<SubscriptionOverview, AppError> {
        date(as_of_date)?;
        self.read(|db| {
            let subs = load_subscriptions(db, include_canceled)?;
            let mut items = Vec::with_capacity(subs.len());
            let mut jpy_monthly_parts = Vec::new();
            let mut foreign_monthly_parts = Vec::new();
            for sub in &subs {
                let item = build_list_item(db, sub, as_of_date)?;
                if sub.status == SubscriptionStatus::Active {
                    let normalized = sub
                        .cadence
                        .monthly_normalized_minor(item.current_amount_minor)?;
                    if sub.currency.is_foreign() {
                        foreign_monthly_parts.push((sub.currency, normalized));
                    } else {
                        jpy_monthly_parts.push(normalized);
                    }
                }
                items.push(item);
            }

            let this_month_start = month_start(as_of_date);
            let this_month_end = clamped_day(this_month_start, 31);
            let lookback_start = this_month_start
                .checked_sub_months(Months::new(12))
                .unwrap_or(this_month_start);

            let all_subs = load_subscriptions(db, true)?;
            let mut jpy_due_parts = Vec::new();
            let mut pending_reconciliation_count = 0usize;
            for sub in &all_subs {
                let scan_from = lookback_start.max(sub.start_date);
                if scan_from <= this_month_end {
                    for occ in build_occurrences_for_subscription(
                        db,
                        sub,
                        scan_from,
                        this_month_end,
                        as_of_date,
                    )? {
                        if !occ.billed_currency.is_foreign()
                            && (this_month_start..=this_month_end).contains(&occ.effective_date)
                            && !matches!(occ.status, OccurrenceStatus::Skipped)
                        {
                            jpy_due_parts.push(occ.actual_jpy.unwrap_or(occ.billed_amount_minor));
                        }
                        if occ.status == OccurrenceStatus::NeedsReconciliation {
                            pending_reconciliation_count += 1;
                        }
                    }
                }
            }

            let last_month_reconciled_foreign_jpy =
                if let Some(prev_start) = this_month_start.checked_sub_months(Months::new(1)) {
                    let prev_end = clamped_day(prev_start, 31);
                    amount(
                        db,
                        "SELECT COALESCE(SUM(actual_jpy),0) FROM subscription_occurrences
                         WHERE state='reconciled' AND billed_currency <> 'JPY'
                           AND COALESCE(override_date, scheduled_date) BETWEEN ?1 AND ?2",
                        params![prev_start, prev_end],
                    )?
                } else {
                    0
                };

            Ok(SubscriptionOverview {
                as_of_date,
                items,
                jpy_monthly_normalized: total(jpy_monthly_parts)?,
                jpy_due_this_month: total(jpy_due_parts)?,
                foreign_monthly_normalized: summarize_foreign(foreign_monthly_parts)?,
                last_month_reconciled_foreign_jpy,
                pending_reconciliation_count,
            })
        })
    }

    pub fn get_subscription(
        &self,
        subscription_id: Uuid,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
        as_of_date: NaiveDate,
    ) -> Result<SubscriptionDetail, AppError> {
        date(as_of_date)?;
        let default_from = as_of_date
            .checked_sub_months(Months::new(6))
            .unwrap_or(as_of_date);
        let default_to = as_of_date
            .checked_add_months(Months::new(6))
            .unwrap_or(as_of_date);
        let from = from.unwrap_or(default_from);
        let to = to.unwrap_or(default_to);
        range(from, to, MAX_OCCURRENCE_SPAN_DAYS)?;

        self.read(|db| {
            let sub = load_subscription(db, subscription_id)?;
            let item = build_list_item(db, &sub, as_of_date)?;
            let revisions = load_revisions(db, subscription_id)?;
            let mut occurrences =
                build_occurrences_for_subscription(db, &sub, from, to, as_of_date)?;
            occurrences.sort_by(|a, b| {
                b.effective_date
                    .cmp(&a.effective_date)
                    .then_with(|| b.scheduled_date.cmp(&a.scheduled_date))
            });
            Ok(SubscriptionDetail {
                subscription: item,
                revisions,
                occurrences,
            })
        })
    }

    pub fn create_subscription(
        &self,
        request: Uuid,
        input: CreateSubscriptionInput,
    ) -> Result<WriteResponse<SubscriptionListItem>, AppError> {
        self.write(request, "create_subscription", &input, |db| {
            let name = label(&input.name, "name", MAX_NAME_CHARS)?;
            let merchant = text(input.merchant.as_deref(), "merchant", MAX_NAME_CHARS, false)?;
            let note = text(input.note.as_deref(), "note", MAX_DESCRIPTION_CHARS, false)?;
            date(input.start_date)?;
            let amount_minor = positive_minor_units(input.amount_minor, "amountMinor")?;
            validate_account_and_category(db, input.account_id, input.category_id)?;

            let billing_day = match input.cadence {
                SubscriptionCadence::Weekly => {
                    if input.billing_day.is_some() {
                        return Err(invalid(
                            "billingDay",
                            "Weekly subscriptions anchor on startDate rather than a day of month.",
                        ));
                    }
                    None
                }
                SubscriptionCadence::Monthly
                | SubscriptionCadence::Quarterly
                | SubscriptionCadence::Yearly => {
                    let day = input.billing_day.unwrap_or(input.start_date.day());
                    day_of_month(day)?;
                    Some(day)
                }
            };
            let billing_month = match input.cadence {
                SubscriptionCadence::Yearly => {
                    let month = input.billing_month.unwrap_or(input.start_date.month());
                    if !(1..=12).contains(&month) {
                        return Err(invalid("billingMonth", "Enter a month from 1 through 12."));
                    }
                    Some(month)
                }
                _ => {
                    if input.billing_month.is_some() {
                        return Err(invalid(
                            "billingMonth",
                            "Billing month applies only to yearly subscriptions.",
                        ));
                    }
                    None
                }
            };

            let sub_id = Uuid::new_v4();
            let rev_id = Uuid::new_v4();
            let stamp = now();
            db.execute(
                "INSERT INTO subscriptions
                    (id,name,merchant,account_id,category_id,currency,cadence,billing_day,
                     billing_month,start_date,status,paused_from,canceled_effective_on,note,
                     created_at,updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'active',NULL,NULL,?11,?12,?12)",
                params![
                    key(&sub_id),
                    name,
                    merchant,
                    key(&input.account_id),
                    key(&input.category_id),
                    input.currency.as_str(),
                    input.cadence.as_str(),
                    billing_day,
                    billing_month,
                    input.start_date,
                    note,
                    stamp
                ],
            )?;
            db.execute(
                "INSERT INTO subscription_plan_revisions
                    (id,subscription_id,effective_from,amount_minor,revision,reason,recorded_at)
                 VALUES (?1,?2,?3,?4,1,'Initial plan',?5)",
                params![
                    key(&rev_id),
                    key(&sub_id),
                    input.start_date,
                    amount_minor,
                    stamp
                ],
            )?;
            let sub = load_subscription(db, sub_id)?;
            build_list_item(db, &sub, input.start_date)
        })
    }

    pub fn update_subscription(
        &self,
        request: Uuid,
        input: UpdateSubscriptionInput,
    ) -> Result<WriteResponse<SubscriptionListItem>, AppError> {
        self.write(request, "update_subscription", &input, |db| {
            let existing = load_subscription(db, input.subscription_id)?;
            let name = label(&input.name, "name", MAX_NAME_CHARS)?;
            let merchant = text(input.merchant.as_deref(), "merchant", MAX_NAME_CHARS, false)?;
            let note = text(input.note.as_deref(), "note", MAX_DESCRIPTION_CHARS, false)?;
            validate_account_and_category(db, input.account_id, input.category_id)?;
            db.execute(
                "UPDATE subscriptions SET name=?2,merchant=?3,account_id=?4,category_id=?5,note=?6,updated_at=?7
                 WHERE id=?1",
                params![
                    key(&existing.id),
                    name,
                    merchant,
                    key(&input.account_id),
                    key(&input.category_id),
                    note,
                    now()
                ],
            )?;
            let updated = load_subscription(db, existing.id)?;
            build_list_item(db, &updated, updated.start_date)
        })
    }

    pub fn revise_subscription_price(
        &self,
        request: Uuid,
        input: PriceRevisionInput,
    ) -> Result<WriteResponse<PlanRevision>, AppError> {
        self.write(request, "revise_subscription_price", &input, |db| {
            let sub = load_subscription(db, input.subscription_id)?;
            date(input.effective_from)?;
            let amount_minor = positive_minor_units(input.amount_minor, "amountMinor")?;
            let reason = label(&input.reason, "reason", MAX_DESCRIPTION_CHARS)?;
            if input.effective_from < sub.start_date {
                return Err(invalid(
                    "effectiveFrom",
                    "Revision date cannot precede the subscription start date.",
                ));
            }
            let revisions = load_revisions(db, sub.id)?;
            if revisions
                .iter()
                .any(|r| r.effective_from == input.effective_from)
            {
                return Err(blocked(
                    "A price revision already exists for that effective date.",
                ));
            }
            let next_rev = revisions.iter().map(|r| r.revision).max().unwrap_or(0) + 1;
            let rev_id = Uuid::new_v4();
            let stamp = now();
            db.execute(
                "INSERT INTO subscription_plan_revisions
                    (id,subscription_id,effective_from,amount_minor,revision,reason,recorded_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7)",
                params![
                    key(&rev_id),
                    key(&sub.id),
                    input.effective_from,
                    amount_minor,
                    next_rev,
                    reason,
                    stamp
                ],
            )?;
            db.execute(
                "UPDATE subscriptions SET updated_at=?2 WHERE id=?1",
                params![key(&sub.id), stamp],
            )?;
            Ok(PlanRevision {
                id: rev_id,
                subscription_id: sub.id,
                effective_from: input.effective_from,
                amount_minor,
                revision: next_rev,
                reason,
                recorded_at: stamp,
            })
        })
    }

    pub fn change_subscription_status(
        &self,
        request: Uuid,
        input: StatusChangeInput,
    ) -> Result<WriteResponse<SubscriptionListItem>, AppError> {
        self.write(request, "change_subscription_status", &input, |db| {
            let sub = load_subscription(db, input.subscription_id)?;
            date(input.effective_on)?;
            if input.effective_on < sub.start_date {
                return Err(invalid(
                    "effectiveOn",
                    "Effective date cannot precede the subscription start date.",
                ));
            }
            let (paused_from, canceled_effective_on) = match input.status {
                SubscriptionStatus::Active => {
                    if sub.status == SubscriptionStatus::Canceled {
                        return Err(conflict(
                            codes::SUBSCRIPTION_INACTIVE,
                            "Canceled subscriptions cannot be reactivated; create a new subscription instead.",
                        ));
                    }
                    (None, None)
                }
                SubscriptionStatus::Paused => {
                    if sub.status == SubscriptionStatus::Canceled {
                        return Err(conflict(
                            codes::SUBSCRIPTION_INACTIVE,
                            "Canceled subscriptions cannot be paused.",
                        ));
                    }
                    (Some(input.effective_on), None)
                }
                SubscriptionStatus::Canceled => (None, Some(input.effective_on)),
            };
            db.execute(
                "UPDATE subscriptions SET status=?2,paused_from=?3,canceled_effective_on=?4,updated_at=?5
                 WHERE id=?1",
                params![
                    key(&sub.id),
                    input.status.as_str(),
                    paused_from,
                    canceled_effective_on,
                    now()
                ],
            )?;
            let updated = load_subscription(db, sub.id)?;
            build_list_item(db, &updated, input.effective_on)
        })
    }

    pub fn list_subscription_occurrences(
        &self,
        from: NaiveDate,
        to: NaiveDate,
        as_of_date: NaiveDate,
        subscription_id: Option<Uuid>,
        unreconciled_only: bool,
    ) -> Result<Vec<SubscriptionOccurrence>, AppError> {
        range(from, to, MAX_OCCURRENCE_SPAN_DAYS)?;
        date(as_of_date)?;
        self.read(|db| {
            let subs = match subscription_id {
                Some(id) => vec![load_subscription(db, id)?],
                None => load_subscriptions(db, true)?,
            };
            let mut list = Vec::new();
            for sub in &subs {
                for occ in build_occurrences_for_subscription(db, sub, from, to, as_of_date)? {
                    if unreconciled_only
                        && matches!(
                            occ.status,
                            OccurrenceStatus::Reconciled | OccurrenceStatus::Skipped
                        )
                    {
                        continue;
                    }
                    list.push(occ);
                }
            }
            list.sort_by(|a, b| {
                a.effective_date
                    .cmp(&b.effective_date)
                    .then_with(|| a.subscription_name.cmp(&b.subscription_name))
                    .then_with(|| a.subscription_id.cmp(&b.subscription_id))
            });
            Ok(list)
        })
    }

    pub fn override_subscription_occurrence(
        &self,
        request: Uuid,
        input: OccurrenceOverrideInput,
    ) -> Result<WriteResponse<SubscriptionOccurrence>, AppError> {
        self.write(request, "override_subscription_occurrence", &input, |db| {
            date(input.scheduled_date)?;
            if let Some(on) = input.override_date {
                date(on)?;
            }
            if input.skipped && input.override_date.is_some() {
                return Err(invalid(
                    "overrideDate",
                    "Skip a cycle or move its date, not both.",
                ));
            }
            let sub = load_subscription(db, input.subscription_id)?;
            let existing = load_stored_occurrence(db, sub.id, input.scheduled_date)?;
            if existing.is_none() && !is_valid_scheduled_date(&sub, input.scheduled_date) {
                return Err(invalid(
                    "scheduledDate",
                    "Select a scheduled billing date for this subscription.",
                ));
            }
            if existing
                .as_ref()
                .is_some_and(|e| e.state == OccurrenceDbState::Reconciled)
            {
                return Err(conflict(
                    codes::OCCURRENCE_ALREADY_RECONCILED,
                    "Unlink the reconciled event before modifying this cycle.",
                ));
            }

            let revisions = load_revisions(db, sub.id)?;
            let billed_amount_minor = existing
                .as_ref()
                .map(|e| e.billed_amount_minor)
                .unwrap_or(amount_for_date(&revisions, input.scheduled_date)?);
            let state = if input.skipped {
                OccurrenceDbState::Skipped
            } else {
                OccurrenceDbState::Scheduled
            };
            db.execute(
                "INSERT INTO subscription_occurrences
                    (id,subscription_id,scheduled_date,override_date,state,billed_currency,
                     billed_amount_minor,actual_jpy,settled_on,event_id,schedule_id,updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,NULL,NULL,NULL,NULL,?8)
                 ON CONFLICT(subscription_id,scheduled_date) DO UPDATE SET
                    override_date=excluded.override_date,
                    state=excluded.state,
                    updated_at=excluded.updated_at",
                params![
                    key(&Uuid::new_v4()),
                    key(&sub.id),
                    input.scheduled_date,
                    input.override_date,
                    state.as_str(),
                    sub.currency.as_str(),
                    billed_amount_minor,
                    now()
                ],
            )?;
            single_occurrence_view(
                db,
                &sub,
                input.scheduled_date,
                input.override_date.unwrap_or(input.scheduled_date),
            )
        })
    }

    pub fn reconcile_subscription_occurrence(
        &self,
        request: Uuid,
        input: ReconcileOccurrenceInput,
    ) -> Result<WriteResponse<ReconciledCharge>, AppError> {
        self.write(request, "reconcile_subscription_occurrence", &input, |db| {
            reconcile_occurrence_in_tx(db, &input)
        })
    }

    pub fn reconcile_subscription_batch(
        &self,
        request: Uuid,
        input: ReconcileBatchInput,
    ) -> Result<WriteResponse<Vec<ReconciledCharge>>, AppError> {
        self.write(request, "reconcile_subscription_batch", &input, |db| {
            if input.items.is_empty() {
                return Err(invalid(
                    "items",
                    "Select at least one subscription cycle to reconcile.",
                ));
            }
            let mut seen = BTreeSet::new();
            for item in &input.items {
                if !seen.insert((item.subscription_id, item.scheduled_date)) {
                    return Err(invalid(
                        "items",
                        "Each subscription cycle can appear only once in a batch.",
                    ));
                }
            }
            let mut results = Vec::with_capacity(input.items.len());
            for item in &input.items {
                results.push(reconcile_occurrence_in_tx(db, item)?);
            }
            Ok(results)
        })
    }

    pub fn match_subscription_occurrence(
        &self,
        request: Uuid,
        input: MatchOccurrenceInput,
    ) -> Result<WriteResponse<SubscriptionOccurrence>, AppError> {
        self.write(request, "match_subscription_occurrence", &input, |db| {
            date(input.scheduled_date)?;
            let sub = load_subscription(db, input.subscription_id)?;
            let existing = load_stored_occurrence(db, sub.id, input.scheduled_date)?;
            if existing.is_none() && !is_valid_scheduled_date(&sub, input.scheduled_date) {
                return Err(invalid(
                    "scheduledDate",
                    "Select a scheduled billing date for this subscription.",
                ));
            }
            if let Some(ref occ) = existing {
                if occ.state == OccurrenceDbState::Reconciled {
                    if occ.event_id == Some(input.event_id) {
                        return single_occurrence_view(
                            db,
                            &sub,
                            input.scheduled_date,
                            occ.settled_on.unwrap_or(input.scheduled_date),
                        );
                    }
                    return Err(conflict(
                        codes::OCCURRENCE_ALREADY_RECONCILED,
                        "This subscription cycle is already linked to another event.",
                    ));
                }
                if occ.state == OccurrenceDbState::Skipped {
                    return Err(blocked("Restore the skipped cycle before matching."));
                }
            }

            let e = event(db, input.event_id)?;
            if e.kind != EventKind::Expense || e.is_reversal || reversed(db, e.id)? {
                return Err(invalid(
                    "eventId",
                    "Select a non-reversed expense event.",
                ));
            }
            let posting = e
                .postings
                .iter()
                .find(|p| p.account_id == sub.account_id && p.delta == -e.amount)
                .ok_or_else(|| {
                    invalid(
                        "eventId",
                        "Select an expense posted to this subscription's payment account.",
                    )
                })?;

            if exists(
                db,
                "SELECT 1 FROM subscription_occurrences WHERE event_id=?1",
                [key(&e.id)],
            )? {
                return Err(blocked(
                    "This event is already linked to another subscription cycle.",
                ));
            }

            let revisions = load_revisions(db, sub.id)?;
            let billed_amount_minor = match input.billed_amount_minor {
                Some(v) => positive_minor_units(v, "billedAmountMinor")?,
                None => existing
                    .as_ref()
                    .map(|s| s.billed_amount_minor)
                    .unwrap_or(amount_for_date(&revisions, input.scheduled_date)?),
            };
            let override_date = existing.as_ref().and_then(|s| s.override_date);
            let linked_schedule: Option<Uuid> = db
                .query_row(
                    "SELECT id FROM card_schedules WHERE purchase_event_id=?1 ORDER BY due_date,id LIMIT 1",
                    [key(&e.id)],
                    |r| id(r, 0),
                )
                .optional()?;

            db.execute(
                "INSERT INTO subscription_occurrences
                    (id,subscription_id,scheduled_date,override_date,state,billed_currency,
                     billed_amount_minor,actual_jpy,settled_on,event_id,schedule_id,updated_at)
                 VALUES (?1,?2,?3,?4,'reconciled',?5,?6,?7,?8,?9,?10,?11)
                 ON CONFLICT(subscription_id,scheduled_date) DO UPDATE SET
                    state='reconciled',
                    billed_amount_minor=excluded.billed_amount_minor,
                    actual_jpy=excluded.actual_jpy,
                    settled_on=excluded.settled_on,
                    event_id=excluded.event_id,
                    schedule_id=excluded.schedule_id,
                    updated_at=excluded.updated_at",
                params![
                    key(&Uuid::new_v4()),
                    key(&sub.id),
                    input.scheduled_date,
                    override_date,
                    sub.currency.as_str(),
                    billed_amount_minor,
                    e.amount,
                    posting.effective_date,
                    key(&e.id),
                    linked_schedule.as_ref().map(key),
                    now()
                ],
            )?;

            single_occurrence_view(db, &sub, input.scheduled_date, posting.effective_date)
        })
    }

    pub fn unlink_subscription_occurrence(
        &self,
        request: Uuid,
        input: UnlinkOccurrenceInput,
    ) -> Result<WriteResponse<SubscriptionOccurrence>, AppError> {
        self.write(request, "unlink_subscription_occurrence", &input, |db| {
            date(input.scheduled_date)?;
            let sub = load_subscription(db, input.subscription_id)?;
            let existing = load_stored_occurrence(db, sub.id, input.scheduled_date)?
                .filter(|o| o.state == OccurrenceDbState::Reconciled)
                .ok_or_else(missing)?;
            if existing.override_date.is_some() {
                db.execute(
                    "UPDATE subscription_occurrences
                     SET state='scheduled',actual_jpy=NULL,settled_on=NULL,event_id=NULL,schedule_id=NULL,updated_at=?3
                     WHERE subscription_id=?1 AND scheduled_date=?2",
                    params![key(&sub.id), input.scheduled_date, now()],
                )?;
            } else {
                db.execute(
                    "DELETE FROM subscription_occurrences WHERE subscription_id=?1 AND scheduled_date=?2",
                    params![key(&sub.id), input.scheduled_date],
                )?;
            }
            single_occurrence_view(
                db,
                &sub,
                input.scheduled_date,
                existing.settled_on.unwrap_or(input.scheduled_date),
            )
        })
    }
}
