# Subscriptions command contract v2

**Status:** Approved by maintainer via `subscription-plan.md` (Month-End Reconciliation Model).

This contract extends [v1-core-ledger.md](v1-core-ledger.md) and [v1-transport-addendum.md](v1-transport-addendum.md) with offline subscription management across local (`JPY`) and foreign (`USD`) billing currencies without requiring live or manual FX rates.

## Invariants

1. **Whole-integer canonical ledger (`JPY`)**: `accounts.currency`, `financial_events.amount`, and `account_postings.delta` remain strictly whole-integer JPY (`i64`). Foreign-currency amounts (`USD`) are stored in integer **minor units** (cents: `$19.99` → `1999`) exclusively as billing reference metadata on subscription tables.
2. **No estimated FX in canonical actuals or forecasts**:
   - Future or unreconciled `USD` subscription cycles are **never** converted with guessed FX rates and **never** written to `financial_events`.
   - Every non-skipped, unreconciled `USD` billing cycle enters `needs_reconciliation` so the user enters the **actual JPY deducted** from their bank or credit card statement during month-end bookkeeping (or matches an already-posted canonical `EXPENSE` event).
   - Forecasts and available-cash reports include confirmed `JPY` subscriptions directly, while reporting unreconciled foreign-currency subscriptions separately as structured warnings/counts (`unreconciledForeignSubscriptions`) rather than coercing them to `¥0`.
3. **Date semantics**:
   - `scheduledDate`: canonical cycle key (`YYYY-MM-DD` in profile time zone), clamped to month length for days 29–31.
   - `overrideDate`: optional rescheduled billing date for a single cycle.
   - When reconciling at month-end, canonical `EXPENSE.occurredOn` defaults to the cycle's billing date (`overrideDate ?? scheduledDate`) so monthly spending and category budgets attribute to the right billing month, while `account_postings.effectiveDate` (`settledOn`) records the actual deduction/statement date.
4. **Append-only price history**: Price changes append a dated revision in `subscription_plan_revisions` (never mutating prior revisions). Unreconciled cycles (including moved or skipped/restored cycles) always resolve the revision active on their `scheduledDate`, while reconciled cycles preserve the billed snapshot captured at reconciliation.
5. **Pause & cancel history**: Pause windows (`[pausedFrom, resumedOn)`) are preserved across resume and cancel transitions so paused cycles never reappear. Effective status (`active` / `paused` / `canceled`) is evaluated as of `asOfDate` so future-dated pauses/cancellations remain visible until they take effect.
6. **Reconcile unlink & data quality**: Unlinking a cycle created by `reconcile_subscription_*` atomically deletes any unconfirmed auto-created card schedule and reverses its linked `EXPENSE` event so the cycle is never double-counted; unlinking a manually matched cycle detaches the link without reversing the user's pre-existing event. `ledger_quality` flags prior-month unreconciled **foreign-currency** (`needs_reconciliation`) cycles and any reconciled cycle whose linked event was reversed.
7. **Idempotent writes**: Every mutating command accepts `Write<T> = { idempotencyKey: Id; payload: T }` and returns `WriteResult<R> = { isReplay: boolean; result: R }`, committing data and `write_receipts` in one immediate SQLite transaction. Batch month-end reconciliation is all-or-nothing atomic.

## Stable error codes (additions to `AppError.code`)

| Code | Category | Meaning |
| --- | --- | --- |
| `INVALID_CURRENCY` | `validation` | Currency is not one of `JPY`, `USD`. |
| `INVALID_CADENCE` | `validation` | Cadence is not one of `weekly`, `monthly`, `quarterly`, `yearly`. |
| `SUBSCRIPTION_INACTIVE` | `conflict` | Operation requires an active subscription or conflicts with pause/cancel status. |
| `SUBSCRIPTION_ACTIVE_REFERENCE` | `conflict` | Account or category is still referenced by active subscriptions and cannot be archived. |
| `OCCURRENCE_ALREADY_RECONCILED` | `conflict` | Cycle is already reconciled to a canonical event. |

## Wire types (TypeScript spelling; Rust Serde `rename_all = "camelCase"`)

```ts
type BillingCurrency = 'JPY' | 'USD';
type SubscriptionCadence = 'weekly' | 'monthly' | 'quarterly' | 'yearly';
type SubscriptionStatus = 'active' | 'paused' | 'canceled';
type OccurrenceStatus =
  | 'upcoming'
  | 'due'
  | 'needs_reconciliation'
  | 'skipped'
  | 'reconciled';

interface PlanRevision {
  id: Id;
  subscriptionId: Id;
  effectiveFrom: DateOnly;
  amountMinor: number; // integer yen for JPY, integer cents for USD
  revision: number;
  reason: string;
  recordedAt: string;
}

interface Subscription {
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

interface SubscriptionListItem extends Subscription {
  currentAmountMinor: number;
  nextBillingDate: DateOnly | null;
  lastReconciledJpy: Yen | null;
  lastReconciledOn: DateOnly | null;
  accountName: string;
  accountKind: AccountKind;
  categoryName: string;
}

interface ForeignCurrencySummary {
  currency: BillingCurrency;
  count: number;
  totalMinor: number;
}

interface SubscriptionOverview {
  asOfDate: DateOnly;
  items: SubscriptionListItem[];
  jpyMonthlyNormalized: Yen;
  jpyDueThisMonth: Yen;
  foreignMonthlyNormalized: ForeignCurrencySummary[];
  lastMonthReconciledForeignJpy: Yen;
  pendingReconciliationCount: number;
}

interface SubscriptionOccurrence {
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

interface SubscriptionDetail {
  subscription: SubscriptionListItem;
  revisions: PlanRevision[];
  occurrences: SubscriptionOccurrence[];
}

interface ReconciledCharge {
  occurrence: SubscriptionOccurrence;
  event: FinancialEvent;
}
```

## Commands (Tauri `invoke` args use `{ request }` for writes)

| Command | Input → Output | Write semantics |
| --- | --- | --- |
| `list_subscriptions` | `{ includeCanceled?: boolean; asOfDate?: DateOnly }` → `SubscriptionOverview` | read |
| `get_subscription` | `{ subscriptionId: Id; from?: DateOnly; to?: DateOnly; asOfDate?: DateOnly }` → `SubscriptionDetail` | read |
| `create_subscription` | `Write<CreateSubscriptionInput>` → `WriteResult<SubscriptionListItem>` | idempotent |
| `update_subscription` | `Write<UpdateSubscriptionInput>` → `WriteResult<SubscriptionListItem>` | idempotent |
| `revise_subscription_price` | `Write<PriceRevisionInput>` → `WriteResult<PlanRevision>` | idempotent |
| `change_subscription_status` | `Write<StatusChangeInput>` → `WriteResult<SubscriptionListItem>` | idempotent |
| `list_subscription_occurrences` | `{ from: DateOnly; to: DateOnly; asOfDate?: DateOnly; subscriptionId?: Id; unreconciledOnly?: boolean }` → `SubscriptionOccurrence[]` | read |
| `override_subscription_occurrence` | `Write<OccurrenceOverrideInput>` → `WriteResult<SubscriptionOccurrence>` | idempotent |
| `reconcile_subscription_occurrence` | `Write<ReconcileOccurrenceInput>` → `WriteResult<ReconciledCharge>` | idempotent |
| `reconcile_subscription_batch` | `Write<{ items: ReconcileOccurrenceInput[] }>` → `WriteResult<ReconciledCharge[]>` | idempotent, atomic |
| `match_subscription_occurrence` | `Write<MatchOccurrenceInput>` → `WriteResult<SubscriptionOccurrence>` | idempotent |
| `unlink_subscription_occurrence` | `Write<{ subscriptionId: Id; scheduledDate: DateOnly }>` → `WriteResult<SubscriptionOccurrence>` | idempotent |
