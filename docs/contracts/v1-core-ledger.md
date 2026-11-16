# Core ledger command contract v1 — integrator decision

**Decision (2026-11-08): approved for the first macOS vertical slice**, subject to Codex and Claude reviewing this exact shared boundary before their #5/#6/#7/#12 branches diverge. This is a transport/domain contract, **not approval of an unreviewed migration** or a claim that parity is complete. First slice: local profile; categories; named accounts and explicit opening revisions; canonical events/postings and per-account balances/history. Legacy records stay separate and readable for later cutover (#8). The app never bundles Python, Cognito, HTTP, telemetry or cloud access.

## Invariants

- `Money` is signed **integer JPY** (`i64` in Rust/SQLite; TypeScript `number` restricted to `Number.isSafeInteger`, or an explicit decimal-string wire change reviewed by both owners). Event `amount` is strictly positive. Posting `delta` and opening `amount` are signed. Do not coerce `null` to zero or present a partial subtotal as a total. Extending the reference PostgreSQL 32-bit integer range to SQLite `i64` is intentional; imported values must remain exact.
- IDs are stable UUID strings; retain source IDs on import. One local profile has no login; store a selected source-user UUID as import provenance, not a live auth dependency. Legacy CASH/BANK `account_type` is **not** a named account ID and must not be inferred from it.
- Account kinds: `CASH | BANK | CREDIT_CARD | BROKERAGE_CASH`. Card signed balance is negative debt; return `amountOwed = max(0, -balance)` and `creditBalance = max(0, balance)` **from Rust** when known. Liquid cash excludes cards; true net worth requires #11's separate valuation path.
- Dates are ISO `YYYY-MM-DD` calendar dates in the local profile's configured IANA time zone (initial default `Asia/Tokyo`, as in the Python finance settings; macOS is the first OS, **not** a change of financial time zone). Rust supplies `today` and `timeZone`. `recordedAt` is a UTC RFC 3339 instant in the desktop wire format; this normalizes the Python reference's naive `datetime.now()` and is an intentional change. Occurrence/purchase date may be `null` on imported legacy events. Effective posting date is required. Opening applies from the **start** of `asOfDate`, inclusive; prior dates return unknown. Never synthesize a purchase date from a legacy payment date.
- `BalanceRead.status`: `unavailable | incomplete_legacy_mapping | complete`, plus optional reason and `openingAsOfDate`. An account without a known opening has `balance: null` even if its known postings sum to zero. Canonical reports include only events with known `occurredOn`; transfers and repayments are excluded from spending, refunds subtract from gross spending. Future/scenario flows are never actuals.
- Every modifying command has a client-generated `idempotencyKey` (UUID string). Store key and canonical payload hash atomically with the operation; same key/same payload returns the original result with `isReplay: true`, same key/different payload returns `conflict`. This extends reference idempotency beyond event/cutover commands to opening revisions and account/category writes, so retries do not create duplicate revisions. A new key on a deliberate edit is a new write. No empty-success retry.
- Existing `AppError` stays `{ code, message, category, field? }`, category `storage | validation | not_found | conflict | internal`; messages safe for UI, no paths/data. Stable codes are required for invalid amount/date, missing opening, archived account, idempotency collision, unresolved mapping and storage failure. The caller never needs to parse message text. Each command returns typed data or rejects with `AppError`.

## Versioned wire types (TypeScript spelling; Rust Serde uses `rename_all = "camelCase"`)

```ts
type Id = string; // UUID; parsed/validated by Rust
type DateOnly = string; // YYYY-MM-DD in profile time zone
type Yen = number; // safe integer; null only when value unknown
type Write<T> = { idempotencyKey: Id; payload: T };
type WriteResult<T> = { isReplay: boolean; result: T };
type AccountKind = 'CASH' | 'BANK' | 'CREDIT_CARD' | 'BROKERAGE_CASH';
type EventKind = 'INCOME' | 'EXPENSE' | 'TRANSFER' | 'CARD_REPAYMENT' | 'REFUND' | 'BALANCE_ADJUSTMENT';
type DateBasis = 'known' | 'confirmed_purchase_date' | 'legacy_recorded_date';
type Account = { id: Id; name: string; kind: AccountKind; currency: 'JPY'; archivedAt: string | null };
type Category = { id: Id; name: string; type: 'income' | 'expense'; parentId: Id | null; archivedAt: string | null };
type Opening = { accountId: Id; amount: Yen; asOfDate: DateOnly; revision: number };
type Balance = { accountId: Id; asOfDate: DateOnly; balance: Yen | null;
  amountOwed: Yen | null; creditBalance: Yen | null;
  status: 'unavailable' | 'incomplete_legacy_mapping' | 'complete';
  openingAsOfDate: DateOnly | null; reason: string | null };
type Posting = { accountId: Id; delta: Yen; effectiveDate: DateOnly };
type Event = { id: Id; kind: EventKind; amount: Yen; categoryId: Id | null;
  description: string | null; occurredOn: DateOnly | null; dateBasis: DateBasis;
  recordedAt: string; originalEventId: Id | null; reversesEventId: Id | null;
  replacesEventId: Id | null; isReversal: boolean; postings: Posting[] };
type EventInput = { kind: EventKind; amount: Yen; accountId: Id;
  destinationAccountId?: Id; categoryId?: Id; description?: string;
  occurredOn: DateOnly; effectiveDate: DateOnly; originalEventId?: Id;
  adjustmentDelta?: Yen };
type Page<T> = { items: T[]; total: number; page: number; pageSize: number };
```

Rust equivalents are `Uuid`-validated string IDs (UUID representation preserved on the wire), `chrono::NaiveDate` for `DateOnly`, `i64` for `Yen`, `Option<T>` for nullable values, and Serde tagged enum/string variants matching the listed spellings. Persist explicit date basis, signed postings, immutable event audit links and a separate legacy provenance table; do not derive those from UI labels. The reference permits optional event dates and default account for manual writes; this v1 UI command deliberately requires them, leaving import/cutover through separate commands. A missing account/date is a validation error, not silently defaulted.

Concretely, Rust `AccountKind`, `EventKind` and `DateBasis` enums serialize to the listed string variants; `Account`, `Category`, `Opening`, `Balance`, `Posting`, `Event`, `EventInput`, `MonthlyPoint` structs mirror the TypeScript fields with `#[serde(rename_all = "camelCase")]`. `WriteRequest<T> { idempotency_key: Uuid, payload: T }` and `WriteResponse<T> { is_replay: bool, result: T }` mirror `Write<T>`/`WriteResult<T>`; `Page<T>` uses `Vec<T>, u64, u32, u32`. Each handler returns `Result<T, AppError>`, with `AppError { code: String, message: String, category: ErrorCategory, field: Option<String> }`. For example, `create_financial_event(request: WriteRequest<EventInput>, db: State<Db>) -> Result<WriteResponse<Event>, AppError>` and `get_account_balance(account_id: Uuid, as_of_date: NaiveDate, db: State<Db>) -> Result<Balance, AppError>` are transport adapters, not calculation owners. The `request` argument is the single Tauri invoke argument for writes; reads use camelCase named arguments. Use the existing `src/lib/commands.ts` invoker and error transport rather than a second API.

## First-slice commands (Tauri `invoke` args use `{ request }` for writes)

| Command | Input → output | Write semantics |
| --- | --- | --- |
| `get_profile` | none → `{ timeZone: string; today: DateOnly }` | read; default Asia/Tokyo |
| `list_categories` | `{ includeArchived?: boolean }` → `Category[]` | read |
| `create_category` | `Write<{ name: string; type: Category['type']; parentId?: Id }>` → `WriteResult<Category>` | idempotent |
| `list_accounts` | `{ includeArchived?: boolean }` → `Account[]` | read |
| `create_account` | `Write<{ name: string; kind: AccountKind }>` → `WriteResult<Account>` | idempotent; JPY only |
| `set_account_opening` | `Write<{ accountId: Id; amount: Yen; asOfDate: DateOnly; reason: string }>` → `WriteResult<Opening>` | idempotent; append audit revision for a new key |
| `get_account_balance` | `{ accountId: Id; asOfDate: DateOnly }` → `Balance` | read; unknown stays null |
| `create_financial_event` | `Write<EventInput>` → `WriteResult<Event>` | idempotent; postings in one SQLite transaction |
| `list_financial_events` | `{ accountId?: Id; page: number; pageSize: number }` → `Page<Event>` | read; account filter joins postings |
| `get_canonical_monthly` | `{ from?: DateOnly; to?: DateOnly }` → `{ month: DateOnly; income: Yen; grossSpending: Yen; refunds: Yen; netSpending: Yen; dateBasis: 'known_only' }[]` | read; no unknown-date events |

For a first manual event, income needs an income category and a liquid account; expense needs an expense category; refund links the original non-reversed expense and cannot exceed its remaining amount; transfer uses two distinct liquid accounts; repayment moves liquid funds to a card; adjustment requires signed delta whose absolute value equals amount and a reason. The source account gets `+amount` for income/refund, `-amount` for expense/transfer/repayment; destination gets `+amount`. Reversal/replace and legacy-cutover commands are **not** silently implemented by these write shapes: specify and review their own idempotent commands in #8/#11.

## Approval boundaries

Approved: first target macOS; first slice above; JPY/date/null/sign conventions; the named command inputs/outputs and error shape; JSON fixture v1. **Not yet approved:** concrete SQLite DDL, migration order, import format, card/budget/forecast command families, or release readiness. Codex reviews this with #5's schema proposal and Claude with #6's Tauri adapters; neither renames shared types independently. #7 starts against the reviewed #5 model, and #12 may use mocks but cannot claim completion before real #7/#8 paths. The human maintainer still decides PR merge and release readiness.
