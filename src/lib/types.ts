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

// ---- Screens addendum (docs/contracts/v1-screens-addendum.md) --------------
// Mirrors `src-tauri/src/domain/*` Serde output. Money stays whole yen; `null`
// means unknown and is never coerced to zero.

export type BalanceStatus = Balance['status'];

export interface OpeningRevision extends Opening {
  id: Id;
  recordedAt: string;
  reason: string;
}
export interface CanonicalCategoryTotal {
  categoryId: Id;
  categoryName: string;
  grossSpending: Yen;
  refunds: Yen;
  netSpending: Yen;
  dateBasis: 'known_only';
}

// #8 earlier Cash/Bank history and cutover
export type LegacyAccount = 'CASH' | 'BANK';
export interface LegacyTransaction {
  id: Id;
  amount: Yen;
  description: string | null;
  transactionDate: DateOnly;
  categoryId: Id;
  accountType: LegacyAccount;
}
export interface LegacyTransfer {
  id: Id;
  amount: Yen;
  description: string | null;
  transferDate: DateOnly;
  from: LegacyAccount;
  to: LegacyAccount;
}
export interface LegacyRecords {
  transactions: LegacyTransaction[];
  transfers: LegacyTransfer[];
}
export interface LegacyReports {
  monthly: { month: DateOnly; income: Yen; expenses: Yen; net: Yen }[];
  /** Legacy opening CASH+BANK plus cumulative net cash flow. Not net worth. */
  netBalanceSeries: { month: DateOnly; balance: Yen }[];
  balances: { cashBalance: Yen; bankBalance: Yen; totalBalance: Yen };
}
export interface TransactionMapping {
  transactionId: Id;
  accountId: Id;
  effectiveDate: DateOnly;
  occurredOn: DateOnly | null;
}
export interface TransferMapping {
  transferId: Id;
  fromAccountId: Id;
  toAccountId: Id;
  effectiveDate: DateOnly;
}
export interface LegacyMappings {
  transactions: TransactionMapping[];
  transfers: TransferMapping[];
}
export interface CutoverInput extends LegacyMappings {
  asOfDate: DateOnly;
  openings: { accountId: Id; amount: Yen }[];
}
export interface CutoverPreview {
  asOfDate: DateOnly;
  legacyTransactionCount: number;
  legacyTransferCount: number;
  categorySums: Record<Id, Yen>;
  legacyDateMin: DateOnly | null;
  legacyDateMax: DateOnly | null;
  unresolvedTransactionIds: Id[];
  unresolvedTransferIds: Id[];
  unresolvedCount: number;
  mappedCount: number;
  openingTotal: Yen;
  legacyAccountTypeIsUnverified: boolean;
  purchaseDateBasis: string;
}
export interface Cutover {
  id: Id;
  asOfDate: DateOnly;
  unresolvedCount: number;
  status: string;
}
export interface Provenance {
  sourceTable: string;
  sourceId: Id;
  recordedDate: DateOnly;
  amount: Yen;
  categoryId: Id | null;
  status: string;
  eventId: Id | null;
}
export interface ResolutionPreview {
  asOfDate: DateOnly;
  remainingBefore: number;
  records: Provenance[];
  writes: number;
}
export interface Resolution {
  resolved: number;
  remaining: number;
  status: string;
}

// #9 cards, reserves and savings
export interface CardSettings {
  cardId: Id;
  settlementAccountId: Id;
  closingDay: number;
  paymentDay: number;
}
export type ScheduleSource = 'estimate' | 'actual';
export interface ScheduleInput {
  cardId: Id;
  purchaseEventId: Id | null;
  openingObligation: string | null;
  dueDate: DateOnly;
  principalDue: Yen;
  source: ScheduleSource;
}
export type ScheduleStatus = 'planned' | 'confirmed' | 'partially_paid' | 'paid';
export interface Schedule extends ScheduleInput {
  id: Id;
  statementId: Id | null;
  paid: Yen;
  refunded: Yen;
  outstanding: Yen;
  status: ScheduleStatus | (string & {});
  fundedReserve: Yen;
  unfunded: Yen;
}
export interface StatementInput {
  cardId: Id;
  closingOn: DateOnly;
  dueDate: DateOnly;
  principalConfirmed: Yen;
  scheduleIds: Id[];
}
export interface Statement extends StatementInput {
  id: Id;
}
export interface Funding {
  accountId: Id;
  amount: Yen;
  asOfDate: DateOnly;
}
export interface SavingsGoal {
  id: Id;
  name: string;
  target: Yen;
  targetDate: DateOnly | null;
  archivedAt: string | null;
}
export interface SavingsBucket {
  id: Id;
  name: string;
  goalId: Id | null;
  funded: Yen;
  archivedAt: string | null;
}
export interface AllocationCapacity {
  balance: Balance;
  funded: Yen;
  allocatable: Yen | null;
  shortfall: Yen | null;
}

// #10 payday, budgets, expected flows, forecast, dashboard
export interface FinanceSettings {
  timeZone: string;
  paydayDay: number;
  defaultAccountId: Id | null;
}
export interface Cycle {
  start: DateOnly;
  endExclusive: DateOnly;
}
export interface CategoryBudget {
  categoryId: Id;
  cycleStart: DateOnly;
  planned: Yen;
}
export interface BudgetResult extends CategoryBudget {
  id: Id;
  categoryName: string;
  grossSpending: Yen;
  refunds: Yen;
  netSpending: Yen;
  remaining: Yen;
}
export interface BudgetSummary extends Cycle {
  budgets: BudgetResult[];
  unknownPurchaseDateCount: number;
  coverage: 'complete' | 'incomplete' | (string & {});
}
export type Recurrence = 'none' | 'monthly';
export type Certainty = 'confirmed' | 'estimated' | 'scenario_only';
export interface ExpectedFlow {
  accountId: Id;
  onDate: DateOnly;
  /** Signed: positive money in, negative money out. */
  amount: Yen;
  label: string;
  recurrence: Recurrence;
  certainty: Certainty;
  scheduleId: Id | null;
  budgetCategoryId: Id | null;
}
export interface Flow extends ExpectedFlow {
  id: Id;
}
export interface FlowOverride {
  flowId: Id;
  scheduledDate: DateOnly;
  overrideDate: DateOnly | null;
  skipped: boolean;
}
export interface FlowMatch {
  flowId: Id;
  scheduledDate: DateOnly;
  eventId: Id;
}
export interface Movement {
  id: Id;
  amount: Yen;
  source: string;
  label: string;
  certainty: string;
}
export interface ForecastDay {
  accountId: Id;
  date: DateOnly;
  startBalance: Yen | null;
  endBalance: Yen | null;
  movements: Movement[];
  intradayOrderUncertain: boolean;
  shortfall: boolean;
  coverage: BalanceStatus;
}
export interface Forecast {
  from: DateOnly;
  to: DateOnly;
  currency: 'JPY';
  scenario: string;
  days: ForecastDay[];
  lowestByAccount: Record<Id, { date: DateOnly; balance: Yen }>;
  coverage: string;
  overdueScheduleIds: Id[];
  warnings: string[];
}
export interface AvailableAccount {
  accountId: Id;
  liquidBalance: Yen | null;
  fundedEarmarks: Yen;
  unfundedKnownDue: Yen;
  rawAvailable: Yen | null;
  spendable: Yen | null;
  reserveShortfall: Yen;
  coverage: BalanceStatus;
}
export interface Availability {
  asOfDate: DateOnly;
  horizon: DateOnly;
  currency: 'JPY';
  accounts: AvailableAccount[];
  remainingDiscretionaryBudget: Yen | null;
  coverage: string;
  overdueScheduleIds: Id[];
  warnings: string[];
}
export interface Dashboard {
  asOfDate: DateOnly;
  availableCash: Availability;
  livingBudget: BudgetSummary;
  forecast: Forecast;
  financialPosition: Position;
  monthlyActuals: MonthlyPoint[];
}

// #11 investments, position, reconciliation, quality
export interface ValuationInput {
  accountId: Id;
  asOfDate: DateOnly;
  marketValue: Yen;
  note: string | null;
}
export interface Valuation extends ValuationInput {
  id: Id;
  scope: string;
  recordedAt: string;
}
export interface HoldingsChange {
  accountId: Id;
  occurredOn: DateOnly;
  reason: string;
  userEnteredRealizedGain: Yen | null;
}
export interface SecuritiesPosition {
  value: Yen | null;
  asOfDate: DateOnly | null;
  scope: string;
  status: 'recorded' | 'missing' | 'stale' | (string & {});
}
export interface PositionAccount {
  accountId: Id;
  name: string;
  kind: AccountKind;
  balance: Yen | null;
  status: BalanceStatus;
  securities: SecuritiesPosition | null;
}
export interface Position {
  asOfDate: DateOnly;
  currency: 'JPY';
  liquidCash: Yen | null;
  brokerageCash: Yen | null;
  securitiesValue: Yen | null;
  signedCardBalances: Yen | null;
  netWorth: Yen | null;
  coverage: 'complete' | 'incomplete' | (string & {});
  accounts: PositionAccount[];
  cumulativeContributions: Yen;
  cumulativeWithdrawals: Yen;
  userEnteredRealizedGain: Yen;
  valuationChange: Yen | null;
  investmentReturn: Yen | null;
}
export interface ReconciliationInput {
  accountId: Id;
  periodStart: DateOnly;
  periodEnd: DateOnly;
  statementOpening: Yen;
  statementClosing: Yen;
  selectedPostingIds: Id[] | null;
}
export interface ReviewPosting {
  id: Id;
  eventId: Id;
  delta: Yen;
  effectiveDate: DateOnly;
}
export interface Reconciliation {
  id: Id;
  accountId: Id;
  periodStart: DateOnly;
  periodEnd: DateOnly;
  statementOpening: Yen;
  statementClosing: Yen;
  calculatedClosing: Yen;
  allPostingsClosing: Yen;
  difference: Yen;
  baselineBeforePeriod: Yen | null;
  openingMismatch: boolean;
  selectedPostingIds: Id[];
  unmatchedPostings: ReviewPosting[];
  status: 'draft' | 'confirmed' | 'stale' | (string & {});
  capturedAt: string;
}
export interface ReconciliationAudit {
  action: string;
  postingId: Id | null;
  recordedAt: string;
}
export interface QualityIssue {
  kind: string;
  recordId: Id;
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
