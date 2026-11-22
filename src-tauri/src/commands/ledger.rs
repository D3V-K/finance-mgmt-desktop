//! #7 core ledger adapters. v1 names and shapes are unchanged; the remaining
//! commands are specified in `docs/contracts/v1-screens-addendum.md`.

use super::wire::{
    MAX_DESCRIPTION_CHARS, Page, WriteRequest, WriteResponse, decode, parse_date, parse_id, text,
};
use super::{Res, St, finance, optional_date};
use crate::domain::ledger::*;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CategoryInput {
    name: String,
    #[serde(rename = "type")]
    kind: CategoryType,
    parent_id: Option<Uuid>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CategoryUpdate {
    category_id: Uuid,
    name: String,
    #[serde(rename = "type")]
    kind: CategoryType,
    parent_id: Option<Uuid>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CategoryRef {
    category_id: Uuid,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CategoryArchive {
    category_id: Uuid,
    archived: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountInput {
    name: String,
    kind: AccountKind,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountArchive {
    account_id: Uuid,
    archived: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Reversal {
    event_id: Uuid,
    reason: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Replacement {
    event_id: Uuid,
    reason: String,
    replacement: EventInput,
}

/// Domain writes hash their own canonical payload; trimming here only keeps
/// optional free text from storing whitespace.
fn event_input(mut input: EventInput) -> Result<EventInput, crate::error::AppError> {
    input.description = text(
        input.description.as_deref(),
        "description",
        MAX_DESCRIPTION_CHARS,
        false,
    )?;
    Ok(input)
}

#[tauri::command]
pub async fn list_categories(state: St<'_>, include_archived: Option<bool>) -> Res<Vec<Category>> {
    finance(&state)?.list_categories(include_archived.unwrap_or(false))
}

#[tauri::command]
pub async fn create_category(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<Category>> {
    let input: CategoryInput = decode(request.payload.clone())?;
    finance(&state)?.create_category(request.key()?, &input.name, input.kind, input.parent_id)
}

#[tauri::command]
pub async fn update_category(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<Category>> {
    let input: CategoryUpdate = decode(request.payload.clone())?;
    finance(&state)?.update_category(
        request.key()?,
        input.category_id,
        &input.name,
        input.kind,
        input.parent_id,
    )
}

#[tauri::command]
pub async fn delete_category(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<Uuid>> {
    let input: CategoryRef = decode(request.payload.clone())?;
    finance(&state)?.delete_category(request.key()?, input.category_id)
}

#[tauri::command]
pub async fn archive_category(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<Category>> {
    let input: CategoryArchive = decode(request.payload.clone())?;
    finance(&state)?.archive_category(request.key()?, input.category_id, input.archived)
}

#[tauri::command]
pub async fn list_accounts(state: St<'_>, include_archived: Option<bool>) -> Res<Vec<Account>> {
    finance(&state)?.list_accounts(include_archived.unwrap_or(false))
}

#[tauri::command]
pub async fn create_account(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<Account>> {
    let input: AccountInput = decode(request.payload.clone())?;
    finance(&state)?.create_account(request.key()?, &input.name, input.kind)
}

#[tauri::command]
pub async fn archive_account(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<Account>> {
    let input: AccountArchive = decode(request.payload.clone())?;
    finance(&state)?.archive_account(request.key()?, input.account_id, input.archived)
}

#[tauri::command]
pub async fn set_account_opening(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<Opening>> {
    let input: OpeningInput = decode(request.payload.clone())?;
    finance(&state)?.set_opening(request.key()?, input)
}

#[tauri::command]
pub async fn list_opening_revisions(
    state: St<'_>,
    account_id: String,
) -> Res<Vec<OpeningRevision>> {
    finance(&state)?.opening_revisions(parse_id(&account_id, "accountId")?)
}

#[tauri::command]
pub async fn get_account_balance(
    state: St<'_>,
    account_id: String,
    as_of_date: String,
) -> Res<Balance> {
    finance(&state)?.account_balance(
        parse_id(&account_id, "accountId")?,
        parse_date(&as_of_date, "asOfDate")?,
    )
}

#[tauri::command]
pub async fn create_financial_event(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<Event>> {
    let input = event_input(decode(request.payload.clone())?)?;
    finance(&state)?.post_event(request.key()?, input)
}

#[tauri::command]
pub async fn reverse_financial_event(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<Event>> {
    let input: Reversal = decode(request.payload.clone())?;
    finance(&state)?.reverse_event(request.key()?, input.event_id, &input.reason)
}

#[tauri::command]
pub async fn replace_financial_event(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<Event>> {
    let input: Replacement = decode(request.payload.clone())?;
    let replacement = event_input(input.replacement)?;
    finance(&state)?.replace_event(request.key()?, input.event_id, &input.reason, replacement)
}

#[tauri::command]
pub async fn get_financial_event(state: St<'_>, event_id: String) -> Res<Event> {
    finance(&state)?.financial_event(parse_id(&event_id, "eventId")?)
}

#[tauri::command]
pub async fn list_financial_events(
    state: St<'_>,
    account_id: Option<String>,
    page: u32,
    page_size: u32,
) -> Res<Page<Event>> {
    let account = account_id
        .map(|raw| parse_id(&raw, "accountId"))
        .transpose()?;
    finance(&state)?.list_events(account, page, page_size)
}

#[tauri::command]
pub async fn get_canonical_monthly(
    state: St<'_>,
    from: Option<String>,
    to: Option<String>,
) -> Res<Vec<MonthlyPoint>> {
    finance(&state)?.canonical_monthly(optional_date(from, "from")?, optional_date(to, "to")?)
}

#[tauri::command]
pub async fn get_canonical_by_category(
    state: St<'_>,
    from: Option<String>,
    to: Option<String>,
) -> Res<Vec<CanonicalCategoryTotal>> {
    finance(&state)?.canonical_by_category(optional_date(from, "from")?, optional_date(to, "to")?)
}
