/**
 * Frontend spelling of the Tauri command contract.
 * Source of truth: docs/contracts/v1-core-ledger.md (+ v1 transport addendum).
 */

/**
 * - storage: SQLite or file system issues. Renders as app-level banner.
 * - validation: user input invalid. Renders field-level inline errors.
 * - not_found: missing entity.
 * - conflict: concurrency, constraint, idempotency collision or second instance.
 * - internal: unexpected failure. Never includes paths or data.
 */
export type ErrorCategory = 'storage' | 'validation' | 'not_found' | 'conflict' | 'internal';

/** Stable codes from `src-tauri/src/error.rs` `codes`. Branch on these, never on `message`. */
export type ErrorCode =
  | 'INVALID_ID'
  | 'INVALID_DATE'
  | 'INVALID_AMOUNT'
  | 'INVALID_TEXT'
  | 'INVALID_PAGE'
  | 'INVALID_REQUEST'
  | 'MISSING_OPENING'
  | 'ACCOUNT_ARCHIVED'
  | 'UNRESOLVED_MAPPING'
  | 'IDEMPOTENCY_CONFLICT'
  | 'NOT_FOUND'
  | 'STORAGE_FAILURE'
  | 'STORAGE_NOT_READY'
  | 'STORAGE_UNAVAILABLE'
  | 'MIGRATION_FAILED'
  | 'DATABASE_TOO_NEW'
  | 'INSTANCE_LOCKED'
  | 'INTERNAL'
  | 'COMMAND_FAILED'
  | 'TAURI_NOT_AVAILABLE';

export interface AppError {
  code: ErrorCode | (string & {});
  message: string;
  category: ErrorCategory;
  field?: string;
}

export interface AppStatus {
  state: 'ready' | 'failed';
  error: AppError | null;
}

export interface SystemInfo {
  appVersion: string;
  osTarget: string;
}

export interface StorageStatus {
  status: 'ready' | 'uninitialized' | 'error';
  databasePath: string;
  isWalEnabled: boolean;
  migrationVersion: number;
}

// ---- v1 core ledger wire types --------------------------------------------

export type Id = string; // UUID; validated by Rust
export type DateOnly = string; // YYYY-MM-DD in the profile time zone
export type Yen = number; // safe integer whole yen

export interface Profile {
  timeZone: string; // IANA, e.g. Asia/Tokyo
  today: DateOnly;
}

export interface Write<T> {
  idempotencyKey: Id;
  payload: T;
}
export interface WriteResult<T> {
  isReplay: boolean;
  result: T;
}

export type AccountKind = 'CASH' | 'BANK' | 'CREDIT_CARD' | 'BROKERAGE_CASH';
export type EventKind =
  | 'INCOME'
  | 'EXPENSE'
  | 'TRANSFER'
  | 'CARD_REPAYMENT'
  | 'REFUND'
  | 'BALANCE_ADJUSTMENT';
export type DateBasis = 'known' | 'confirmed_purchase_date' | 'legacy_recorded_date';

export interface Account {
  id: Id;
  name: string;
  kind: AccountKind;
  currency: 'JPY';
  archivedAt: string | null;
}
export interface Category {
  id: Id;
  name: string;
  type: 'income' | 'expense';
  parentId: Id | null;
  archivedAt: string | null;
}
export interface Opening {
  accountId: Id;
  amount: Yen;
  asOfDate: DateOnly;
  revision: number;
}
export interface Balance {
  accountId: Id;
  asOfDate: DateOnly;
  balance: Yen | null;
  amountOwed: Yen | null;
  creditBalance: Yen | null;
  status: 'unavailable' | 'incomplete_legacy_mapping' | 'complete';
  openingAsOfDate: DateOnly | null;
  reason: string | null;
}
export interface Posting {
  accountId: Id;
  delta: Yen;
  effectiveDate: DateOnly;
}
export interface FinancialEvent {
  id: Id;
  kind: EventKind;
  amount: Yen;
  categoryId: Id | null;
  description: string | null;
  occurredOn: DateOnly | null;
  dateBasis: DateBasis;
  recordedAt: string;
  originalEventId: Id | null;
  reversesEventId: Id | null;
  replacesEventId: Id | null;
  isReversal: boolean;
  postings: Posting[];
}
export interface EventInput {
  kind: EventKind;
  amount: Yen;
  accountId: Id;
  destinationAccountId?: Id;
  categoryId?: Id;
  description?: string;
  occurredOn: DateOnly;
  effectiveDate: DateOnly;
  originalEventId?: Id;
  adjustmentDelta?: Yen;
}
export interface Page<T> {
  items: T[];
  total: number;
  page: number;
  pageSize: number;
}
export interface MonthlyPoint {
  month: DateOnly;
  income: Yen;
  grossSpending: Yen;
  refunds: Yen;
  netSpending: Yen;
  dateBasis: 'known_only';
}
