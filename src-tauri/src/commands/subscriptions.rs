//! Subscription Plan Tauri command adapters (v2 contract).
//!
//! Handlers validate wire strings (`UUID`, `YYYY-MM-DD`, enums) into field-level
//! `AppError`s and delegate all financial rules to `domain::subscriptions`.

use crate::AppState;
use crate::commands::wire::{WriteRequest, WriteResponse, parse_date, parse_id};
use crate::domain::Finance;
use crate::domain::subscriptions::{
    BillingCurrency, CreateSubscriptionInput, MatchOccurrenceInput, OccurrenceOverrideInput,
    PlanRevision, PriceRevisionInput, ReconcileBatchInput, ReconcileOccurrenceInput,
    ReconciledCharge, StatusChangeInput, SubscriptionCadence, SubscriptionDetail,
    SubscriptionListItem, SubscriptionOccurrence, SubscriptionOverview, SubscriptionStatus,
    UnlinkOccurrenceInput, UpdateSubscriptionInput,
};
use crate::error::{AppError, codes};
use chrono::{NaiveDate, Utc};
use chrono_tz::Tz;
use serde::Deserialize;

fn resolve_today(finance: &Finance<'_>, override_date: Option<String>) -> Result<NaiveDate, AppError> {
    if let Some(raw) = override_date {
        return parse_date(&raw, "asOfDate");
    }
    let zone = finance.finance_settings()?.time_zone;
    let tz: Tz = zone.parse().map_err(|_| {
        AppError::storage(
            codes::STORAGE_FAILURE,
            "The stored financial time zone is invalid.",
        )
    })?;
    Ok(Utc::now().with_timezone(&tz).date_naive())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateSubscriptionWire {
    pub name: String,
    pub merchant: Option<String>,
    pub account_id: String,
    pub category_id: String,
    pub currency: String,
    pub amount_minor: i64,
    pub cadence: String,
    pub billing_day: Option<u32>,
    pub billing_month: Option<u32>,
    pub start_date: String,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateSubscriptionWire {
    pub subscription_id: String,
    pub name: String,
    pub merchant: Option<String>,
    pub account_id: String,
    pub category_id: String,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PriceRevisionWire {
    pub subscription_id: String,
    pub effective_from: String,
    pub amount_minor: i64,
    pub reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StatusChangeWire {
    pub subscription_id: String,
    pub status: String,
    pub effective_on: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OccurrenceOverrideWire {
    pub subscription_id: String,
    pub scheduled_date: String,
    pub override_date: Option<String>,
    pub skipped: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReconcileOccurrenceWire {
    pub subscription_id: String,
    pub scheduled_date: String,
    pub actual_jpy: Option<i64>,
    pub settled_on: String,
    pub occurred_on: Option<String>,
    pub billed_amount_minor: Option<i64>,
    pub create_card_schedule: Option<bool>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReconcileBatchWire {
    pub items: Vec<ReconcileOccurrenceWire>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MatchOccurrenceWire {
    pub subscription_id: String,
    pub scheduled_date: String,
    pub event_id: String,
    pub billed_amount_minor: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnlinkOccurrenceWire {
    pub subscription_id: String,
    pub scheduled_date: String,
}

fn parse_reconcile_item(w: ReconcileOccurrenceWire) -> Result<ReconcileOccurrenceInput, AppError> {
    Ok(ReconcileOccurrenceInput {
        subscription_id: parse_id(&w.subscription_id, "subscriptionId")?,
        scheduled_date: parse_date(&w.scheduled_date, "scheduledDate")?,
        actual_jpy: w.actual_jpy,
        settled_on: parse_date(&w.settled_on, "settledOn")?,
        occurred_on: w
            .occurred_on
            .as_deref()
            .map(|raw| parse_date(raw, "occurredOn"))
            .transpose()?,
        billed_amount_minor: w.billed_amount_minor,
        create_card_schedule: w.create_card_schedule,
        description: w.description,
    })
}

#[tauri::command]
pub fn list_subscriptions(
    include_canceled: Option<bool>,
    as_of_date: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<SubscriptionOverview, AppError> {
    let finance = Finance::new(state.boot.db()?);
    let on = resolve_today(&finance, as_of_date)?;
    finance.list_subscriptions(include_canceled.unwrap_or(false), on)
}

#[tauri::command]
pub fn get_subscription(
    subscription_id: String,
    from: Option<String>,
    to: Option<String>,
    as_of_date: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<SubscriptionDetail, AppError> {
    let sub_id = parse_id(&subscription_id, "subscriptionId")?;
    let from = from.as_deref().map(|d| parse_date(d, "from")).transpose()?;
    let to = to.as_deref().map(|d| parse_date(d, "to")).transpose()?;
    let finance = Finance::new(state.boot.db()?);
    let on = resolve_today(&finance, as_of_date)?;
    finance.get_subscription(sub_id, from, to, on)
}

#[tauri::command]
pub fn create_subscription(
    request: WriteRequest<CreateSubscriptionWire>,
    state: tauri::State<'_, AppState>,
) -> Result<WriteResponse<SubscriptionListItem>, AppError> {
    let key = request.key()?;
    let p = request.payload;
    let input = CreateSubscriptionInput {
        name: p.name,
        merchant: p.merchant,
        account_id: parse_id(&p.account_id, "accountId")?,
        category_id: parse_id(&p.category_id, "categoryId")?,
        currency: BillingCurrency::parse_str(&p.currency)?,
        amount_minor: p.amount_minor,
        cadence: SubscriptionCadence::parse_str(&p.cadence)?,
        billing_day: p.billing_day,
        billing_month: p.billing_month,
        start_date: parse_date(&p.start_date, "startDate")?,
        note: p.note,
    };
    Finance::new(state.boot.db()?).create_subscription(key, input)
}

#[tauri::command]
pub fn update_subscription(
    request: WriteRequest<UpdateSubscriptionWire>,
    state: tauri::State<'_, AppState>,
) -> Result<WriteResponse<SubscriptionListItem>, AppError> {
    let key = request.key()?;
    let p = request.payload;
    let input = UpdateSubscriptionInput {
        subscription_id: parse_id(&p.subscription_id, "subscriptionId")?,
        name: p.name,
        merchant: p.merchant,
        account_id: parse_id(&p.account_id, "accountId")?,
        category_id: parse_id(&p.category_id, "categoryId")?,
        note: p.note,
    };
    Finance::new(state.boot.db()?).update_subscription(key, input)
}

#[tauri::command]
pub fn revise_subscription_price(
    request: WriteRequest<PriceRevisionWire>,
    state: tauri::State<'_, AppState>,
) -> Result<WriteResponse<PlanRevision>, AppError> {
    let key = request.key()?;
    let p = request.payload;
    let input = PriceRevisionInput {
        subscription_id: parse_id(&p.subscription_id, "subscriptionId")?,
        effective_from: parse_date(&p.effective_from, "effectiveFrom")?,
        amount_minor: p.amount_minor,
        reason: p.reason,
    };
    Finance::new(state.boot.db()?).revise_subscription_price(key, input)
}

#[tauri::command]
pub fn change_subscription_status(
    request: WriteRequest<StatusChangeWire>,
    state: tauri::State<'_, AppState>,
) -> Result<WriteResponse<SubscriptionListItem>, AppError> {
    let key = request.key()?;
    let p = request.payload;
    let input = StatusChangeInput {
        subscription_id: parse_id(&p.subscription_id, "subscriptionId")?,
        status: SubscriptionStatus::parse_str(&p.status)?,
        effective_on: parse_date(&p.effective_on, "effectiveOn")?,
    };
    Finance::new(state.boot.db()?).change_subscription_status(key, input)
}

#[tauri::command]
pub fn list_subscription_occurrences(
    from: String,
    to: String,
    as_of_date: Option<String>,
    subscription_id: Option<String>,
    unreconciled_only: Option<bool>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SubscriptionOccurrence>, AppError> {
    let from = parse_date(&from, "from")?;
    let to = parse_date(&to, "to")?;
    let subscription_id = subscription_id
        .as_deref()
        .map(|raw| parse_id(raw, "subscriptionId"))
        .transpose()?;
    let finance = Finance::new(state.boot.db()?);
    let on = resolve_today(&finance, as_of_date)?;
    finance.list_subscription_occurrences(from, to, on, subscription_id, unreconciled_only.unwrap_or(false))
}

#[tauri::command]
pub fn override_subscription_occurrence(
    request: WriteRequest<OccurrenceOverrideWire>,
    state: tauri::State<'_, AppState>,
) -> Result<WriteResponse<SubscriptionOccurrence>, AppError> {
    let key = request.key()?;
    let p = request.payload;
    let input = OccurrenceOverrideInput {
        subscription_id: parse_id(&p.subscription_id, "subscriptionId")?,
        scheduled_date: parse_date(&p.scheduled_date, "scheduledDate")?,
        override_date: p
            .override_date
            .as_deref()
            .map(|raw| parse_date(raw, "overrideDate"))
            .transpose()?,
        skipped: p.skipped,
    };
    Finance::new(state.boot.db()?).override_subscription_occurrence(key, input)
}

#[tauri::command]
pub fn reconcile_subscription_occurrence(
    request: WriteRequest<ReconcileOccurrenceWire>,
    state: tauri::State<'_, AppState>,
) -> Result<WriteResponse<ReconciledCharge>, AppError> {
    let key = request.key()?;
    let input = parse_reconcile_item(request.payload)?;
    Finance::new(state.boot.db()?).reconcile_subscription_occurrence(key, input)
}

#[tauri::command]
pub fn reconcile_subscription_batch(
    request: WriteRequest<ReconcileBatchWire>,
    state: tauri::State<'_, AppState>,
) -> Result<WriteResponse<Vec<ReconciledCharge>>, AppError> {
    let key = request.key()?;
    let items = request
        .payload
        .items
        .into_iter()
        .map(parse_reconcile_item)
        .collect::<Result<Vec<_>, _>>()?;
    Finance::new(state.boot.db()?).reconcile_subscription_batch(key, ReconcileBatchInput { items })
}

#[tauri::command]
pub fn match_subscription_occurrence(
    request: WriteRequest<MatchOccurrenceWire>,
    state: tauri::State<'_, AppState>,
) -> Result<WriteResponse<SubscriptionOccurrence>, AppError> {
    let key = request.key()?;
    let p = request.payload;
    let input = MatchOccurrenceInput {
        subscription_id: parse_id(&p.subscription_id, "subscriptionId")?,
        scheduled_date: parse_date(&p.scheduled_date, "scheduledDate")?,
        event_id: parse_id(&p.event_id, "eventId")?,
        billed_amount_minor: p.billed_amount_minor,
    };
    Finance::new(state.boot.db()?).match_subscription_occurrence(key, input)
}

#[tauri::command]
pub fn unlink_subscription_occurrence(
    request: WriteRequest<UnlinkOccurrenceWire>,
    state: tauri::State<'_, AppState>,
) -> Result<WriteResponse<SubscriptionOccurrence>, AppError> {
    let key = request.key()?;
    let p = request.payload;
    let input = UnlinkOccurrenceInput {
        subscription_id: parse_id(&p.subscription_id, "subscriptionId")?,
        scheduled_date: parse_date(&p.scheduled_date, "scheduledDate")?,
    };
    Finance::new(state.boot.db()?).unlink_subscription_occurrence(key, input)
}
