# v1 screens addendum — **proposed**, needs Codex review

Status: **Proposed by Claude with #12/#13.** It changes no approved v1 command or type. It names the Tauri commands that expose the #7–#11 Rust services merged in PR #24. The v1 contract lists these card, budget, forecast and review families as "not yet approved", so Codex should confirm or amend them before the next domain change lands. Every handler is a thin adapter in `src-tauri/src/commands/{ledger,legacy,cards,planning,review}.rs` and owns no financial rule.

## Conventions (unchanged from v1 and the transport addendum)

- **Writes** take one `request: Write<T>` argument and return `WriteResult<R>`. Each command passes its UUID key to the domain's `Finance::write`, which stores the receipt in the same transaction. The same key and payload replay the result with `isReplay: true`; the same key with a different payload returns `IDEMPOTENCY_CONFLICT`. The UI keeps one key per form session and reuses it on retry.
- **Reads** use camelCase named arguments. IDs and dates arrive as strings and are parsed with `wire::parse_id`/`parse_date`, producing `INVALID_ID`/`INVALID_DATE` with the field name.
- **Structured payloads** are decoded by `wire::decode`. That function pre-validates by wire name before Serde runs. Keys ending in `Id` are UUIDs, and keys ending in `Ids` are UUID arrays. Keys ending in `Date` or `On`, plus `from`, `to`, `month`, `payday`, `horizon`, `cycleStart`, `periodStart` and `periodEnd`, are `YYYY-MM-DD`. Every number must be a whole value within ±`Number.MAX_SAFE_INTEGER`. A nested failure names its path, for example `openings.0.accountId`. Any remaining shape error is `INVALID_REQUEST` without a field. **Codex:** a new domain input field should follow these suffixes, or `DATE_KEYS` must be extended.
- Money is signed integer JPY. `null` means unknown and is never 0. Dates are profile-zone calendar days. Audit stamps are UTC RFC 3339.
- TypeScript shapes are in `src/lib/types.ts` and mirror the Rust Serde output. The wrappers are in `src/lib/ledger.ts` (v1) and `src/lib/finance.ts`.

## Commands

| Command | Args → result | Notes |
| --- | --- | --- |
| `update_category` | `Write<{categoryId,name,type,parentId?}>` → `Category` | Type change is blocked when the category has history or children |
| `delete_category` | `Write<{categoryId}>` → `Id` | Only categories that are unused and have no children |
| `archive_category` | `Write<{categoryId,archived}>` → `Category` | Restore re-validates the parent |
| `archive_account` | `Write<{accountId,archived}>` → `Account` | |
| `list_opening_revisions` | `{accountId}` → `OpeningRevision[]` | Append-only audit |
| `reverse_financial_event` | `Write<{eventId,reason}>` → `Event` | |
| `replace_financial_event` | `Write<{eventId,reason,replacement: EventInput}>` → `Event` | Reversal plus replacement, atomic |
| `get_financial_event` | `{eventId}` → `Event` | |
| `get_canonical_by_category` | `{from?,to?}` → `CanonicalCategoryTotal[]` | Known dates only |
| `list_legacy_records` | — → `{transactions: (LegacyTransaction & {id})[], transfers: …}` | Read only; legacy writes belong to import (#14) |
| `get_legacy_reports` | — → `{monthly, netBalanceSeries, balances}` | `netBalanceSeries` is **not** net worth |
| `get_legacy_by_category` | `{from?,to?}` → `LegacyCategoryTotal[]` | |
| `get_legacy_balance` | `{accountType: 'CASH'\|'BANK', asOfDate}` → `Yen\|null` | Reference only |
| `preview_cutover` | `{input: CutoverInput}` → `CutoverPreview` | |
| `confirm_cutover` | `Write<CutoverInput>` → `Cutover` | `CutoverInput` flattens `transactions`/`transfers` |
| `list_legacy_provenance` | `{unresolvedOnly?}` → `Provenance[]` | |
| `preview_legacy_resolution` | `{mappings: LegacyMappings}` → `ResolutionPreview` | |
| `confirm_legacy_resolution` | `Write<LegacyMappings>` → `Resolution` | |
| `get_card_settings` / `set_card_settings` | `{cardId}` → `CardSettings\|null` / `Write<CardSettings>` | |
| `list_card_schedules` | `{cardId}` → `Schedule[]` | `Schedule` flattens `ScheduleInput` |
| `create_card_schedule` | `Write<ScheduleInput>` → `Schedule` | Exactly one of `purchaseEventId`/`openingObligation` |
| `revise_card_schedule` | `Write<{scheduleId,dueDate,principalDue,source}>` → `Schedule` | |
| `list_card_statements` / `confirm_card_statement` | `{cardId}` / `Write<StatementInput>` → `Statement` | The confirmed total must equal the selected schedules |
| `apply_card_event` | `Write<{scheduleId,eventId,amount}>` → `Schedule` | Repayment, or a refund of the scheduled purchase |
| `fund_card_reserve` | `Write<{scheduleId,accountId,amount,asOfDate}>` → `Schedule` | Signed nonzero; balances unchanged |
| `list_savings_goals` / `save_savings_goal` | — / `Write<{goalId?,name,target,targetDate,archive?}>` | |
| `list_savings_buckets` / `save_savings_bucket` | — / `Write<{bucketId?,name,goalId,archive?}>` | |
| `fund_savings_bucket` | `Write<{bucketId,accountId,amount,asOfDate}>` → `SavingsBucket` | Same capacity check as reserves |
| `get_allocation_capacity` | `{asOfDate}` → `AllocationCapacity[]` | |
| `get_finance_settings` / `set_finance_settings` | — / `Write<FinanceSettings>` | Saving confirms the settings |
| `set_payday_override` | `Write<{month,payday}>` → `DateOnly` | |
| `get_budget_summary` | `{asOfDate}` → `BudgetSummary` | |
| `set_category_budget` | `Write<CategoryBudget>` → `BudgetResult` | |
| `set_budget_payment_account` | `Write<{categoryId,cycleStart,accountId\|null}>` → `Id\|null` | |
| `list_expected_flows` / `create_expected_flow` | — / `Write<ExpectedFlow>` → `Flow` | Signed amount |
| `override_expected_flow` | `Write<FlowOverride>` | |
| `match_expected_flow` / `unmatch_expected_flow` | `Write<FlowMatch>` | |
| `get_cash_forecast` | `{from,to,scenario?}` → `Forecast` | `scenario` includes `scenario_only` flows |
| `get_available_cash` | `{asOfDate,horizon}` → `Availability` | |
| `get_dashboard` | `{asOfDate}` → `Dashboard` | |
| `record_valuation` / `list_valuations` | `Write<ValuationInput>` / `{accountId}` | Securities only |
| `mark_holdings_change` | `Write<HoldingsChange>` → `Id` | |
| `get_financial_position` / `get_position_history` | `{asOfDate}` / `{from,to}` (month ends) | |
| `preview_reconciliation` | `{input: ReconciliationInput}` → `Reconciliation` | `selectedPostingIds: []` lists every posting |
| `create_reconciliation` / `get_reconciliation` | `Write<ReconciliationInput>` / `{reconciliationId}` | |
| `set_reconciled_posting` | `Write<{reconciliationId,postingId,included}>` | Draft only |
| `confirm_reconciliation` / `reopen_reconciliation` | `Write<{reconciliationId}>` | |
| `get_reconciliation_audit` | `{reconciliationId}` → `ReconciliationAudit[]` | |
| `get_ledger_quality` | `{asOfDate}` → `QualityIssue[]` | |

The v1 reads `list_categories` and `list_accounts` keep their shapes. `includeArchived` is optional and defaults to `false`.

## Verification

`src-tauri/tests/command_ipc.rs` mounts the real handler table (`lib::with_commands`) on Tauri's mock runtime. It drives these commands over IPC with the renderer's JSON against on-disk migrations. It covers the ledger start, duplicate retries and conflicts, correction, card purchase → statement → repayment, savings allocation, forecast warnings and scenario separation, reconciliation and quality. The registration test still requires `generate_handler!`, `COMMANDS`, `build.rs` and the capability to list the same 73 commands.

## Open questions for Codex

1. **There is no read for reconciliation sessions.** The UI finds draft and stale sessions through `get_ledger_quality`. A confirmed session cannot be reopened later because nothing lists it. Proposal: add `Finance::reconciliations(account?: Uuid) -> Vec<Reconciliation>`.
2. **There is no read for cutover state.** The UI infers that the ledger has started when every balance has left `incomplete_legacy_mapping` or provenance exists. Proposal: add `Finance::cutover() -> Option<Cutover>`.
3. **There is no read for a budget's paying account.** `set_budget_payment_account` can be written but not displayed. Proposal: add `paymentAccountId` to `BudgetResult`.
4. **There is no read for flow overrides and matches.** The UI can write them, but cannot list which occurrences are skipped or matched.
5. Should `MISSING_OPENING` from cutover mapping (`legacy.rs`) carry the `validation` category, as it does today?
