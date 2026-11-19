//! #11 valuation, financial position, reconciliation and data-quality adapters.

use super::wire::{WriteRequest, WriteResponse, decode, parse_date, parse_id};
use super::{Res, St, finance};
use crate::domain::review::*;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionRef {
    reconciliation_id: Uuid,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PostingToggle {
    reconciliation_id: Uuid,
    posting_id: Uuid,
    included: bool,
}

#[tauri::command]
pub async fn record_valuation(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<Valuation>> {
    let input: ValuationInput = decode(request.payload.clone())?;
    finance(&state)?.record_valuation(request.key()?, input)
}

#[tauri::command]
pub async fn list_valuations(state: St<'_>, account_id: String) -> Res<Vec<Valuation>> {
    finance(&state)?.valuations(parse_id(&account_id, "accountId")?)
}

#[tauri::command]
pub async fn mark_holdings_change(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<Uuid>> {
    let input: HoldingsChange = decode(request.payload.clone())?;
    finance(&state)?.mark_holdings_change(request.key()?, input)
}

#[tauri::command]
pub async fn get_financial_position(state: St<'_>, as_of_date: String) -> Res<Position> {
    finance(&state)?.financial_position(parse_date(&as_of_date, "asOfDate")?)
}

#[tauri::command]
pub async fn get_position_history(state: St<'_>, from: String, to: String) -> Res<Vec<Position>> {
    finance(&state)?.position_history(parse_date(&from, "from")?, parse_date(&to, "to")?)
}

#[tauri::command]
pub async fn preview_reconciliation(state: St<'_>, input: Value) -> Res<Reconciliation> {
    finance(&state)?.preview_reconciliation(&decode(input)?)
}

#[tauri::command]
pub async fn create_reconciliation(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<Reconciliation>> {
    let input: ReconciliationInput = decode(request.payload.clone())?;
    finance(&state)?.create_reconciliation(request.key()?, input)
}

#[tauri::command]
pub async fn get_reconciliation(state: St<'_>, reconciliation_id: String) -> Res<Reconciliation> {
    finance(&state)?.reconciliation(parse_id(&reconciliation_id, "reconciliationId")?)
}

#[tauri::command]
pub async fn set_reconciled_posting(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<Reconciliation>> {
    let input: PostingToggle = decode(request.payload.clone())?;
    finance(&state)?.set_reconciled_posting(
        request.key()?,
        input.reconciliation_id,
        input.posting_id,
        input.included,
    )
}

#[tauri::command]
pub async fn confirm_reconciliation(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<Reconciliation>> {
    let input: SessionRef = decode(request.payload.clone())?;
    finance(&state)?.confirm_reconciliation(request.key()?, input.reconciliation_id)
}

#[tauri::command]
pub async fn reopen_reconciliation(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<Reconciliation>> {
    let input: SessionRef = decode(request.payload.clone())?;
    finance(&state)?.reopen_reconciliation(request.key()?, input.reconciliation_id)
}

#[tauri::command]
pub async fn get_reconciliation_audit(
    state: St<'_>,
    reconciliation_id: String,
) -> Res<Vec<ReconciliationAudit>> {
    finance(&state)?.reconciliation_audit(parse_id(&reconciliation_id, "reconciliationId")?)
}

#[tauri::command]
pub async fn get_ledger_quality(state: St<'_>, as_of_date: String) -> Res<Vec<QualityIssue>> {
    finance(&state)?.ledger_quality(parse_date(&as_of_date, "asOfDate")?)
}
