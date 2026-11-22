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
  | 'INVALID_CURRENCY'
  | 'INVALID_CADENCE'
  | 'MISSING_OPENING'
  | 'ACCOUNT_ARCHIVED'
  | 'UNRESOLVED_MAPPING'
  | 'IDEMPOTENCY_CONFLICT'
  | 'NOT_FOUND'
  | 'SUBSCRIPTION_INACTIVE'
  | 'SUBSCRIPTION_ACTIVE_REFERENCE'
  | 'OCCURRENCE_ALREADY_RECONCILED'
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

// ---- v2 subscription plan wire types --------------------------------------

export type BillingCurrency = 'JPY' | 'USD';
export type SubscriptionCadence = 'weekly' | 'monthly' | 'quarterly' | 'yearly';
export type SubscriptionStatus = 'active' | 'paused' | 'canceled';
export type OccurrenceStatus =
  | 'upcoming'
  | 'due'
  | 'needs_reconciliation'
  | 'skipped'
  | 'reconciled';

export interface PlanRevision {
  id: Id;
  subscriptionId: Id;
  effectiveFrom: DateOnly;
  amountMinor: number;
  revision: number;
  reason: string;
  recordedAt: string;
}

export interface Subscription {
  id: Id;
  name: string;
  merchant: string | null;
  accountId: Id;
  categoryId: Id;
  currency: BillingCurrency;
  cadence: SubscriptionCadence;
  billingDay: number | null;
  billingMonth: number | null;
  startDate: DateOnly;
  status: SubscriptionStatus;
  pausedFrom: DateOnly | null;
  canceledEffectiveOn: DateOnly | null;
  note: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface SubscriptionListItem extends Subscription {
  currentAmountMinor: number;
  nextBillingDate: DateOnly | null;
  lastReconciledJpy: Yen | null;
  lastReconciledOn: DateOnly | null;
  accountName: string;
  accountKind: AccountKind;
  categoryName: string;
}

export interface ForeignCurrencySummary {
  currency: BillingCurrency;
  count: number;
  totalMinor: number;
}

export interface SubscriptionOverview {
  asOfDate: DateOnly;
  items: SubscriptionListItem[];
  jpyMonthlyNormalized: Yen;
  jpyDueThisMonth: Yen;
  foreignMonthlyNormalized: ForeignCurrencySummary[];
  lastMonthReconciledForeignJpy: Yen;
  pendingReconciliationCount: number;
}

export interface SubscriptionOccurrence {
  subscriptionId: Id;
  subscriptionName: string;
  merchant: string | null;
  accountId: Id;
  accountName: string;
  accountKind: AccountKind;
  hasCardSettings: boolean;
  categoryId: Id;
  categoryName: string;
  scheduledDate: DateOnly;
  overrideDate: DateOnly | null;
  effectiveDate: DateOnly;
  status: OccurrenceStatus;
  billedCurrency: BillingCurrency;
  billedAmountMinor: number;
  actualJpy: Yen | null;
  settledOn: DateOnly | null;
  eventId: Id | null;
  scheduleId: Id | null;
  lastReconciledJpy: Yen | null;
}

export interface SubscriptionDetail {
  subscription: SubscriptionListItem;
  revisions: PlanRevision[];
  occurrences: SubscriptionOccurrence[];
}

export interface CreateSubscriptionInput {
  name: string;
  merchant?: string;
  accountId: Id;
  categoryId: Id;
  currency: BillingCurrency;
  amountMinor: number;
  cadence: SubscriptionCadence;
  billingDay?: number;
  billingMonth?: number;
  startDate: DateOnly;
  note?: string;
}

export interface UpdateSubscriptionInput {
  subscriptionId: Id;
  name: string;
  merchant?: string;
  accountId: Id;
  categoryId: Id;
  note?: string;
}

export interface PriceRevisionInput {
  subscriptionId: Id;
  effectiveFrom: DateOnly;
  amountMinor: number;
  reason: string;
}

export interface StatusChangeInput {
  subscriptionId: Id;
  status: SubscriptionStatus;
  effectiveOn: DateOnly;
}

export interface OccurrenceOverrideInput {
  subscriptionId: Id;
  scheduledDate: DateOnly;
  overrideDate?: DateOnly;
  skipped: boolean;
}

export interface ReconcileOccurrenceInput {
  subscriptionId: Id;
  scheduledDate: DateOnly;
  actualJpy?: Yen;
  settledOn: DateOnly;
  occurredOn?: DateOnly;
  billedAmountMinor?: number;
  createCardSchedule?: boolean;
  description?: string;
}

export interface ReconcileBatchInput {
  items: ReconcileOccurrenceInput[];
}

export interface ReconciledCharge {
  occurrence: SubscriptionOccurrence;
  event: FinancialEvent;
}

export interface MatchOccurrenceInput {
  subscriptionId: Id;
  scheduledDate: DateOnly;
  eventId: Id;
  billedAmountMinor?: number;
}

export interface UnlinkOccurrenceInput {
  subscriptionId: Id;
  scheduledDate: DateOnly;
}
