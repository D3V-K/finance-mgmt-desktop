//! #8 earlier Cash/Bank history and cutover adapters. Legacy records are read
//! only here; they arrive through import (#14), never through manual entry.

use super::wire::{WriteRequest, WriteResponse, decode, parse_date};
use super::{Res, St, finance, optional_date};
use crate::domain::legacy::*;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WithId<T> {
    id: Uuid,
    #[serde(flatten)]
    record: T,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyRecords {
    transactions: Vec<WithId<LegacyTransaction>>,
    transfers: Vec<WithId<LegacyTransfer>>,
}

/// The legacy reports are separate series; the net-balance series is **not**
/// net worth and is named accordingly on the wire.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyReports {
    monthly: Vec<LegacyMonth>,
    net_balance_series: Vec<LegacyNetBalance>,
    balances: LegacyBalanceReport,
}

fn with_ids<T>(rows: Vec<(Uuid, T)>) -> Vec<WithId<T>> {
    rows.into_iter()
        .map(|(id, record)| WithId { id, record })
        .collect()
}

#[tauri::command]
pub async fn list_legacy_records(state: St<'_>) -> Res<LegacyRecords> {
    let f = finance(&state)?;
    Ok(LegacyRecords {
        transactions: with_ids(f.legacy_transactions()?),
        transfers: with_ids(f.legacy_transfers()?),
    })
}

#[tauri::command]
pub async fn get_legacy_reports(state: St<'_>) -> Res<LegacyReports> {
    let f = finance(&state)?;
    Ok(LegacyReports {
        monthly: f.legacy_monthly()?,
        net_balance_series: f.legacy_net_balance()?,
        balances: f.legacy_balance_report()?,
    })
}

#[tauri::command]
pub async fn get_legacy_by_category(
    state: St<'_>,
    from: Option<String>,
    to: Option<String>,
) -> Res<Vec<LegacyCategoryTotal>> {
    finance(&state)?.legacy_by_category(optional_date(from, "from")?, optional_date(to, "to")?)
}

#[tauri::command]
pub async fn get_legacy_balance(
    state: St<'_>,
    account_type: LegacyAccount,
    as_of_date: String,
) -> Res<Option<i64>> {
    finance(&state)?.legacy_balance(account_type, parse_date(&as_of_date, "asOfDate")?)
}

#[tauri::command]
pub async fn preview_cutover(state: St<'_>, input: Value) -> Res<CutoverPreview> {
    finance(&state)?.preview_cutover(&decode(input)?)
}

#[tauri::command]
pub async fn confirm_cutover(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<Cutover>> {
    let input: CutoverInput = decode(request.payload.clone())?;
    finance(&state)?.confirm_cutover(request.key()?, input)
}

#[tauri::command]
pub async fn list_legacy_provenance(
    state: St<'_>,
    unresolved_only: Option<bool>,
) -> Res<Vec<Provenance>> {
    finance(&state)?.provenance(unresolved_only.unwrap_or(false))
}

#[tauri::command]
pub async fn preview_legacy_resolution(state: St<'_>, mappings: Value) -> Res<ResolutionPreview> {
    finance(&state)?.preview_resolution(&decode(mappings)?)
}

#[tauri::command]
pub async fn confirm_legacy_resolution(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<Resolution>> {
    let mappings: LegacyMappings = decode(request.payload.clone())?;
    finance(&state)?.confirm_resolution(request.key()?, mappings)
}
