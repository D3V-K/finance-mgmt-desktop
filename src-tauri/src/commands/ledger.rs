//! Core ledger read adapters needed by desktop screens (v1 contract).

use crate::AppState;
use crate::commands::wire::{Page, parse_id};
use crate::domain::Finance;
use crate::domain::ledger::{Account, Category, Event};
use crate::error::AppError;

#[tauri::command]
pub fn list_accounts(
    include_archived: Option<bool>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<Account>, AppError> {
    Finance::new(state.boot.db()?).list_accounts(include_archived.unwrap_or(false))
}

#[tauri::command]
pub fn list_categories(
    include_archived: Option<bool>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<Category>, AppError> {
    Finance::new(state.boot.db()?).list_categories(include_archived.unwrap_or(false))
}

#[tauri::command]
pub fn list_financial_events(
    account_id: Option<String>,
    page: u32,
    page_size: u32,
    state: tauri::State<'_, AppState>,
) -> Result<Page<Event>, AppError> {
    let account_id = account_id
        .as_deref()
        .map(|raw| parse_id(raw, "accountId"))
        .transpose()?;
    Finance::new(state.boot.db()?).list_events(account_id, page, page_size)
}
