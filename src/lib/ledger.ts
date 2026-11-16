/** First-slice wire contract: docs/contracts/v1-core-ledger.md. */
import { runCommand } from './commands';
import type { AppError, IdempotentRequest, IdempotentResponse } from './types';

export type Id = string;
export type DateOnly = string;
export type Yen = number;
export type AccountKind = 'CASH' | 'BANK' | 'CREDIT_CARD' | 'BROKERAGE_CASH';
export type EventKind = 'INCOME' | 'EXPENSE' | 'TRANSFER' | 'CARD_REPAYMENT' | 'REFUND' | 'BALANCE_ADJUSTMENT';
export type DateBasis = 'known' | 'confirmed_purchase_date' | 'legacy_recorded_date';

export interface Account { id: Id; name: string; kind: AccountKind; currency: 'JPY'; archivedAt: string | null }
export interface Category { id: Id; name: string; type: 'income' | 'expense'; parentId: Id | null; archivedAt: string | null }
export interface Opening { accountId: Id; amount: Yen; asOfDate: DateOnly; revision: number }
export interface Balance {
  accountId: Id; asOfDate: DateOnly; balance: Yen | null;
  amountOwed: Yen | null; creditBalance: Yen | null;
  status: 'unavailable' | 'incomplete_legacy_mapping' | 'complete';
  openingAsOfDate: DateOnly | null; reason: string | null;
}
export interface Posting { accountId: Id; delta: Yen; effectiveDate: DateOnly }
export interface FinancialEvent {
  id: Id; kind: EventKind; amount: Yen; categoryId: Id | null;
  description: string | null; occurredOn: DateOnly | null; dateBasis: DateBasis;
  recordedAt: string; originalEventId: Id | null; reversesEventId: Id | null;
  replacesEventId: Id | null; isReversal: boolean; postings: Posting[];
}
export interface EventInput {
  kind: EventKind; amount: Yen; accountId: Id; destinationAccountId?: Id;
  categoryId?: Id; description?: string; occurredOn: DateOnly;
  effectiveDate: DateOnly; originalEventId?: Id; adjustmentDelta?: Yen;
}
export interface Page<T> { items: T[]; total: number; page: number; pageSize: number }
export interface MonthlyPoint {
  month: DateOnly; income: Yen; grossSpending: Yen; refunds: Yen;
  netSpending: Yen; dateBasis: 'known_only';
}

function assertWireYen(value: Yen, field: string): void {
  if (!Number.isSafeInteger(value)) {
    throw {
      code: 'INVALID_AMOUNT', message: 'Enter an amount that is a whole number of yen.',
      category: 'validation', field,
    } satisfies AppError;
  }
}

export function getProfile() {
  return runCommand<{ timeZone: string; today: DateOnly }>('get_profile');
}
export function listCategories(includeArchived = false) {
  return runCommand<Category[]>('list_categories', { includeArchived });
}
export function createCategory(request: IdempotentRequest<{ name: string; type: Category['type']; parentId?: Id }>) {
  return runCommand<IdempotentResponse<Category>>('create_category', { request });
}
export function listAccounts(includeArchived = false) {
  return runCommand<Account[]>('list_accounts', { includeArchived });
}
export function createAccount(request: IdempotentRequest<{ name: string; kind: AccountKind }>) {
  return runCommand<IdempotentResponse<Account>>('create_account', { request });
}
export async function setAccountOpening(request: IdempotentRequest<{ accountId: Id; amount: Yen; asOfDate: DateOnly; reason: string }>) {
  assertWireYen(request.payload.amount, 'amount');
  return runCommand<IdempotentResponse<Opening>>('set_account_opening', { request });
}
export function getAccountBalance(accountId: Id, asOfDate: DateOnly) {
  return runCommand<Balance>('get_account_balance', { accountId, asOfDate });
}
export async function createFinancialEvent(request: IdempotentRequest<EventInput>) {
  assertWireYen(request.payload.amount, 'amount');
  if (request.payload.adjustmentDelta !== undefined) assertWireYen(request.payload.adjustmentDelta, 'adjustmentDelta');
  return runCommand<IdempotentResponse<FinancialEvent>>('create_financial_event', { request });
}
export function listFinancialEvents(page: number, pageSize: number, accountId?: Id) {
  return runCommand<Page<FinancialEvent>>('list_financial_events', { page, pageSize, ...(accountId ? { accountId } : {}) });
}
export function getCanonicalMonthly(from?: DateOnly, to?: DateOnly) {
  return runCommand<MonthlyPoint[]>('get_canonical_monthly', { ...(from ? { from } : {}), ...(to ? { to } : {}) });
}
