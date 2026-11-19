//! #10 payday, budget, expected-flow, forecast and dashboard adapters.

use super::wire::{WriteRequest, WriteResponse, decode, parse_date};
use super::{Res, St, finance};
use crate::domain::planning::*;
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PaydayOverride {
    month: NaiveDate,
    payday: NaiveDate,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BudgetPaymentAccount {
    category_id: Uuid,
    cycle_start: NaiveDate,
    account_id: Option<Uuid>,
}

#[tauri::command]
pub async fn get_finance_settings(state: St<'_>) -> Res<FinanceSettings> {
    finance(&state)?.finance_settings()
}

#[tauri::command]
pub async fn set_finance_settings(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<FinanceSettings>> {
    let input: FinanceSettings = decode(request.payload.clone())?;
    finance(&state)?.set_finance_settings(request.key()?, input)
}

#[tauri::command]
pub async fn set_payday_override(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<NaiveDate>> {
    let input: PaydayOverride = decode(request.payload.clone())?;
    finance(&state)?.set_payday_override(request.key()?, input.month, input.payday)
}

#[tauri::command]
pub async fn get_budget_summary(state: St<'_>, as_of_date: String) -> Res<BudgetSummary> {
    finance(&state)?.budget_summary(parse_date(&as_of_date, "asOfDate")?)
}

#[tauri::command]
pub async fn set_category_budget(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<BudgetResult>> {
    let input: CategoryBudget = decode(request.payload.clone())?;
    finance(&state)?.set_category_budget(request.key()?, input)
}

#[tauri::command]
pub async fn set_budget_payment_account(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<Option<Uuid>>> {
    let input: BudgetPaymentAccount = decode(request.payload.clone())?;
    finance(&state)?.set_budget_payment_account(
        request.key()?,
        input.category_id,
        input.cycle_start,
        input.account_id,
    )
}

#[tauri::command]
pub async fn list_expected_flows(state: St<'_>) -> Res<Vec<Flow>> {
    finance(&state)?.expected_flows()
}

#[tauri::command]
pub async fn create_expected_flow(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<Flow>> {
    let input: ExpectedFlow = decode(request.payload.clone())?;
    finance(&state)?.create_expected_flow(request.key()?, input)
}

#[tauri::command]
pub async fn override_expected_flow(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<FlowOverride>> {
    let input: FlowOverride = decode(request.payload.clone())?;
    finance(&state)?.override_flow(request.key()?, input)
}

#[tauri::command]
pub async fn match_expected_flow(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<FlowMatch>> {
    let input: FlowMatch = decode(request.payload.clone())?;
    finance(&state)?.match_flow(request.key()?, input)
}

#[tauri::command]
pub async fn unmatch_expected_flow(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<FlowMatch>> {
    let input: FlowMatch = decode(request.payload.clone())?;
    finance(&state)?.unmatch_flow(request.key()?, input)
}

#[tauri::command]
pub async fn get_cash_forecast(
    state: St<'_>,
    from: String,
    to: String,
    scenario: Option<bool>,
) -> Res<Forecast> {
    finance(&state)?.cash_forecast(
        parse_date(&from, "from")?,
        parse_date(&to, "to")?,
        scenario.unwrap_or(false),
    )
}

#[tauri::command]
pub async fn get_available_cash(
    state: St<'_>,
    as_of_date: String,
    horizon: String,
) -> Res<Availability> {
    finance(&state)?.available_cash(
        parse_date(&as_of_date, "asOfDate")?,
        parse_date(&horizon, "horizon")?,
    )
}

#[tauri::command]
pub async fn get_dashboard(state: St<'_>, as_of_date: String) -> Res<Dashboard> {
    finance(&state)?.dashboard(parse_date(&as_of_date, "asOfDate")?)
}
