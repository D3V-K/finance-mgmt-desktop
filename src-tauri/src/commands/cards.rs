//! #9 card schedules, statements, reserves and savings adapters.

use super::wire::{WriteRequest, WriteResponse, decode, parse_date, parse_id};
use super::{Res, St, finance};
use crate::domain::cards::*;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScheduleRevision {
    schedule_id: Uuid,
    due_date: chrono::NaiveDate,
    principal_due: i64,
    source: ScheduleSource,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CardApplication {
    schedule_id: Uuid,
    event_id: Uuid,
    amount: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReserveFunding {
    schedule_id: Uuid,
    #[serde(flatten)]
    funding: Funding,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoalSave {
    goal_id: Option<Uuid>,
    #[serde(flatten)]
    terms: SavingsGoalInput,
    #[serde(default)]
    archive: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BucketSave {
    bucket_id: Option<Uuid>,
    name: String,
    goal_id: Option<Uuid>,
    #[serde(default)]
    archive: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BucketFunding {
    bucket_id: Uuid,
    #[serde(flatten)]
    funding: Funding,
}

#[tauri::command]
pub async fn get_card_settings(state: St<'_>, card_id: String) -> Res<Option<CardSettings>> {
    finance(&state)?.card_settings(parse_id(&card_id, "cardId")?)
}

#[tauri::command]
pub async fn set_card_settings(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<CardSettings>> {
    let input: CardSettings = decode(request.payload.clone())?;
    finance(&state)?.set_card_settings(request.key()?, input)
}

#[tauri::command]
pub async fn list_card_schedules(state: St<'_>, card_id: String) -> Res<Vec<Schedule>> {
    finance(&state)?.card_schedules(parse_id(&card_id, "cardId")?)
}

#[tauri::command]
pub async fn create_card_schedule(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<Schedule>> {
    let input: ScheduleInput = decode(request.payload.clone())?;
    finance(&state)?.create_schedule(request.key()?, input)
}

#[tauri::command]
pub async fn revise_card_schedule(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<Schedule>> {
    let input: ScheduleRevision = decode(request.payload.clone())?;
    finance(&state)?.revise_schedule(
        request.key()?,
        input.schedule_id,
        input.due_date,
        input.principal_due,
        input.source,
    )
}

#[tauri::command]
pub async fn list_card_statements(state: St<'_>, card_id: String) -> Res<Vec<Statement>> {
    finance(&state)?.card_statements(parse_id(&card_id, "cardId")?)
}

#[tauri::command]
pub async fn confirm_card_statement(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<Statement>> {
    let input: StatementInput = decode(request.payload.clone())?;
    finance(&state)?.confirm_card_statement(request.key()?, input)
}

#[tauri::command]
pub async fn apply_card_event(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<Schedule>> {
    let input: CardApplication = decode(request.payload.clone())?;
    finance(&state)?.apply_card_event(
        request.key()?,
        input.schedule_id,
        input.event_id,
        input.amount,
    )
}

#[tauri::command]
pub async fn fund_card_reserve(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<Schedule>> {
    let input: ReserveFunding = decode(request.payload.clone())?;
    finance(&state)?.fund_card_reserve(request.key()?, input.schedule_id, input.funding)
}

#[tauri::command]
pub async fn list_savings_goals(state: St<'_>) -> Res<Vec<SavingsGoal>> {
    finance(&state)?.savings_goals()
}

#[tauri::command]
pub async fn save_savings_goal(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<SavingsGoal>> {
    let input: GoalSave = decode(request.payload.clone())?;
    finance(&state)?.save_savings_goal(request.key()?, input.goal_id, input.terms, input.archive)
}

#[tauri::command]
pub async fn list_savings_buckets(state: St<'_>) -> Res<Vec<SavingsBucket>> {
    finance(&state)?.savings_buckets()
}

#[tauri::command]
pub async fn save_savings_bucket(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<SavingsBucket>> {
    let input: BucketSave = decode(request.payload.clone())?;
    finance(&state)?.save_savings_bucket(
        request.key()?,
        input.bucket_id,
        &input.name,
        input.goal_id,
        input.archive,
    )
}

#[tauri::command]
pub async fn fund_savings_bucket(
    state: St<'_>,
    request: WriteRequest<Value>,
) -> Res<WriteResponse<SavingsBucket>> {
    let input: BucketFunding = decode(request.payload.clone())?;
    finance(&state)?.fund_savings_bucket(request.key()?, input.bucket_id, input.funding)
}

#[tauri::command]
pub async fn get_allocation_capacity(
    state: St<'_>,
    as_of_date: String,
) -> Res<Vec<AllocationCapacity>> {
    finance(&state)?.allocation_capacity(parse_date(&as_of_date, "asOfDate")?)
}
