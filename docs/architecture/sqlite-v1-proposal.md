# #5 SQLite model proposal — review before migration

**Status:** proposed, not an installed schema. The maintainer selected **rusqlite** for the offline single-user app on 2026-11-16. The #6 command/UI owner reviewed the first-slice boundary in [PR #22](https://github.com/D3V-K/finance-mgmt-desktop/pull/22#issuecomment-6917216107) with no blocking disagreement; this revision resolves the review requests below. The maintainer still decides whether to merge the schema proposal before a separate migration implementation. Reference: the Python models in `finance-mgmt-backend-lite/src/models/` at `e474d383dc42d298ea9ba513bfd955fa3e212e2c`, plus `fixtures/parity/v1/cases.json`. This proposal covers the first slice; it is not a substitute for mapping every Aurora table for #14.

## Decisions and review resolution

1. **Migration runner (decided):** use `rusqlite` for migrations and #7 database access. This intentionally supersedes #5's SQLx wording by maintainer decision; all other #5 acceptance criteria remain. Extend the scaffold's existing `_schema_migrations` table rather than creating a competing version table. Version-0 scaffold databases must upgrade in place, not be deleted. Tests execute the actual migration SQL on on-disk SQLite, never `create_all` or an in-memory substitute.
2. **Stable IDs and import provenance:** local profile gets a new UUID; `source_user_id` is nullable until import and is never a login. A second source user must be refused, not silently merged. Every UUID identifier and foreign-key reference, including imported IDs and idempotency keys, is parsed as a UUID then stored/looked up in canonical lowercase hyphenated form (`Uuid::to_string()`); this preserves UUID identity without making differently cased spellings different database keys. The optional original textual spelling belongs in #14's import manifest/raw provenance, not the canonical ID columns. `commands/wire.rs::parse_id` in #23 already returns `Uuid`, so the adapter must pass that parsed value rather than the raw string into database operations. Test mixed-case input and a case-variant idempotency replay.
3. **Legacy accounting:** `CASH`/`BANK` on legacy rows are labels, not foreign keys to named accounts. Legacy openings/transfers/reports remain separate. Canonical cutover creates explicit provenance links, not copies with invented purchase dates. Opening revision is an append-only audit record; the current snapshot may be rebuilt from its greatest revision.
4. **Time and integer wire:** `INTEGER` is signed i64 yen, `TEXT` dates are calendar ISO `YYYY-MM-DD` in the profile IANA zone (initial `Asia/Tokyo`), UTC instants are RFC 3339, and unknown dates/balances remain nullable. App-layer validation is required for real dates, UUIDs, IANA zones, and `Number.isSafeInteger` wire bounds. SQLite `CHECK` constraints only defend shape and local invariants.
5. **Event ordering:** unfiltered history orders by `occurred_on DESC, recorded_at DESC, id DESC`; SQLite puts `NULL` last for descending dates, without inventing a purchase date. Account-filtered history joins postings and orders by `effective_date DESC, recorded_at DESC, event.id DESC` (one posting per event/account is a domain invariant). Both need matching indexes and stable keyset/tie-break behavior for `Page<T>`. An unknown-date legacy event can therefore appear by effective date in a particular account's history but last in unfiltered occurrence-date history. If a future UI wants a different sort, version the contract rather than changing it silently.
6. **Names and imported text:** allow duplicate account and category names, as the Python source does; IDs disambiguate pickers, and the UI may warn. A new uniqueness rule would reject valid historical/imported rows and is not part of approved v1. The UI/transport cap new names and descriptions, but the DDL must not cap imported legacy descriptions or strip multiline/control text. Preserve source data exactly, sanitize at display boundaries.
7. **Profile and boot:** create the singleton local profile inside migration 1 with a generated UUID and `Asia/Tokyo`, before the #23 boot gate returns ready. The #23 `get_profile` command should then read its stored IANA zone. Its lifecycle already owns owner-only app-data/DB/sidecar permissions and the app lock; #5 does not duplicate that code.
8. **Errors:** agree with `docs/contracts/v1-transport-addendum.md` in #23: `MISSING_OPENING` is a validation error **on writes only**; reads before an opening return `Balance { balance: null, status: unavailable }`. `ACCOUNT_ARCHIVED`, `IDEMPOTENCY_CONFLICT`, and `UNRESOLVED_MAPPING` are conflicts. A newer schema returns `DATABASE_TOO_NEW` (storage) unchanged through boot. Other failures become `MIGRATION_FAILED`; integrity-check failures can use that code with the same safe copy for now, but must not claim an upgrade occurred.

## Candidate first-slice DDL (illustrative, not yet a migration)

The following is intended for one transactional `rusqlite` migration after table review. `PRAGMA foreign_keys=ON` must be enabled on **every** connection before writes; test it with an on-disk database. Table names are deliberately distinct from the Python legacy/canonical concepts.

```sql
CREATE TABLE local_profile (
  singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
  id TEXT NOT NULL UNIQUE,
  source_user_id TEXT UNIQUE,
  time_zone TEXT NOT NULL DEFAULT 'Asia/Tokyo' CHECK (length(time_zone) > 0),
  currency TEXT NOT NULL DEFAULT 'JPY' CHECK (currency = 'JPY')
);

CREATE TABLE categories (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL CHECK (length(trim(name)) > 0),
  type TEXT NOT NULL CHECK (type IN ('income', 'expense')),
  parent_id TEXT REFERENCES categories(id) ON DELETE RESTRICT,
  archived_at TEXT,
  created_at TEXT NOT NULL,
  CHECK (parent_id IS NULL OR parent_id <> id)
);
CREATE INDEX categories_parent_idx ON categories(parent_id);

CREATE TABLE accounts (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL CHECK (length(trim(name)) > 0),
  kind TEXT NOT NULL CHECK (kind IN ('CASH', 'BANK', 'CREDIT_CARD', 'BROKERAGE_CASH')),
  currency TEXT NOT NULL DEFAULT 'JPY' CHECK (currency = 'JPY'),
  archived_at TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE account_opening_revisions (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
  amount INTEGER NOT NULL,
  as_of_date TEXT NOT NULL CHECK (length(as_of_date) = 10),
  revision INTEGER NOT NULL CHECK (revision > 0),
  reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),
  recorded_at TEXT NOT NULL,
  UNIQUE (account_id, revision)
);
CREATE INDEX opening_account_revision_idx ON account_opening_revisions(account_id, revision DESC);

CREATE TABLE financial_events (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('INCOME', 'EXPENSE', 'TRANSFER', 'CARD_REPAYMENT', 'REFUND', 'BALANCE_ADJUSTMENT')),
  amount INTEGER NOT NULL CHECK (amount > 0),
  category_id TEXT REFERENCES categories(id) ON DELETE RESTRICT,
  description TEXT,
  occurred_on TEXT CHECK (occurred_on IS NULL OR length(occurred_on) = 10),
  date_basis TEXT NOT NULL CHECK (date_basis IN ('known', 'confirmed_purchase_date', 'legacy_recorded_date')),
  recorded_at TEXT NOT NULL,
  original_event_id TEXT REFERENCES financial_events(id) ON DELETE RESTRICT,
  reverses_event_id TEXT REFERENCES financial_events(id) ON DELETE RESTRICT,
  replaces_event_id TEXT REFERENCES financial_events(id) ON DELETE RESTRICT,
  is_reversal INTEGER NOT NULL DEFAULT 0 CHECK (is_reversal IN (0, 1)),
  CHECK ((occurred_on IS NULL AND date_basis = 'legacy_recorded_date') OR occurred_on IS NOT NULL)
);
CREATE UNIQUE INDEX one_reversal_per_event ON financial_events(reverses_event_id) WHERE reverses_event_id IS NOT NULL;
CREATE INDEX events_history_idx ON financial_events(occurred_on DESC, recorded_at DESC, id DESC);

CREATE TABLE account_postings (
  id TEXT PRIMARY KEY,
  event_id TEXT NOT NULL REFERENCES financial_events(id) ON DELETE RESTRICT,
  account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
  delta INTEGER NOT NULL CHECK (delta <> 0),
  effective_date TEXT NOT NULL CHECK (length(effective_date) = 10)
);
CREATE INDEX postings_account_date_idx ON account_postings(account_id, effective_date DESC, event_id);
CREATE UNIQUE INDEX one_posting_per_event_account ON account_postings(event_id, account_id);

CREATE TABLE write_receipts (
  idempotency_key TEXT PRIMARY KEY,
  operation TEXT NOT NULL,
  payload_hash TEXT NOT NULL,
  result_json TEXT NOT NULL,
  recorded_at TEXT NOT NULL
);

CREATE TABLE legacy_transactions (
  id TEXT PRIMARY KEY,
  source_user_id TEXT,
  amount INTEGER NOT NULL,
  description TEXT,
  transaction_date TEXT NOT NULL,
  recorded_at TEXT,
  category_id TEXT NOT NULL REFERENCES categories(id) ON DELETE RESTRICT,
  account_type TEXT NOT NULL CHECK (account_type IN ('CASH', 'BANK'))
);
CREATE TABLE legacy_openings (
  id TEXT PRIMARY KEY,
  source_user_id TEXT,
  account_type TEXT NOT NULL CHECK (account_type IN ('CASH', 'BANK')),
  amount INTEGER NOT NULL,
  as_of_date TEXT NOT NULL,
  UNIQUE (account_type)
);
CREATE TABLE legacy_transfers (
  id TEXT PRIMARY KEY,
  source_user_id TEXT,
  from_account_type TEXT NOT NULL CHECK (from_account_type IN ('CASH', 'BANK')),
  to_account_type TEXT NOT NULL CHECK (to_account_type IN ('CASH', 'BANK')),
  amount INTEGER NOT NULL,
  transfer_date TEXT NOT NULL,
  CHECK (from_account_type <> to_account_type)
);

CREATE TABLE ledger_cutover (
  singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
  id TEXT NOT NULL UNIQUE,
  as_of_date TEXT NOT NULL,
  confirmed_at TEXT NOT NULL,
  unresolved_count INTEGER NOT NULL CHECK (unresolved_count >= 0),
  request_hash TEXT NOT NULL
);
CREATE TABLE legacy_provenance (
  id TEXT PRIMARY KEY,
  source_table TEXT NOT NULL CHECK (source_table IN ('legacy_transactions', 'legacy_transfers')),
  source_id TEXT NOT NULL,
  recorded_date TEXT NOT NULL,
  amount INTEGER NOT NULL,
  category_id TEXT REFERENCES categories(id) ON DELETE RESTRICT,
  status TEXT NOT NULL CHECK (status IN ('mapped', 'unresolved')),
  event_id TEXT UNIQUE REFERENCES financial_events(id) ON DELETE RESTRICT,
  UNIQUE (source_table, source_id),
  CHECK ((status = 'mapped' AND event_id IS NOT NULL) OR (status = 'unresolved' AND event_id IS NULL))
);
```

`write_receipts` is inserted in the **same transaction** as each write. A duplicate key first compares `operation` and canonical `payload_hash`: exact replay returns stored result with `isReplay=true`; changed payload or operation gives `conflict`. New key for a deliberate opening correction appends revision N+1. Rust rejects archived account writes, missing openings, category cycles/type mismatch, overlapping reversal/refund, incorrect posting directions and totals, and links to an inappropriate event kind. These are cross-row rules that the DDL alone cannot prove. In particular, a mapped provenance row must also be verified against its matching source record and the cutover's effective date; no arbitrary `source_id` is accepted merely because it is a UUID.

## Extension map, not first-slice tables

| Lane | Future tables / preserved links | Owner issue |
| --- | --- | --- |
| Cards | card settings, confirmed statements, payment schedules, schedule payments/refunds, funding allocations; purchase/repayment/refund events remain FKs | #9 |
| Budgets and reserves | payday settings/overrides, category budgets/cash assumptions, savings goals/buckets; allocations do not post to accounts | #9/#10 |
| Forecast | expected flows and occurrence overrides, matched event/schedule references, certainty/status | #10 |
| Investments | dated securities valuations and holdings changes, separate from brokerage cash postings | #11 |
| Reconciliation | sessions, posting matches and audit rows, immutable confirmed-period guard | #11 |
| Import | per-source export manifest, source-user provenance and import receipts; every Python table/relationship mapped before acceptance | #14 |

Do not add placeholder columns for those lanes to the first migration merely to imply coverage. Complete the relationship map before #14 acceptance; preserve nulls, IDs and cross-row audit associations exactly.

## Migration and recovery policy to implement after review

- On first run create app-data directory outside the bundle; restrict directory/file access to the current user where the OS supports it. Create the local profile once, in a transaction. No auth token or network dependency.
- Keep the existing `_schema_migrations(version, name, applied_at)` table. Before migration 1, add a checksum column **inside the upgrade transaction** compatible with a version-0 scaffold database (for example, nullable in DDL but required by the runner for every applied version); reject a preexisting applied row lacking a verified checksum rather than guessing what schema it represents. No second migration ledger.
- #23 calls `Migrator = fn(&Database) -> Result<(), AppError>` before presenting the UI. Add a `Database::with_conn_mut` closure under the existing mutex, so `rusqlite::Connection::transaction_with_behavior(Immediate)` can borrow `&mut Connection`; do not open a second, uncoordinated connection or use an unchecked transaction through `&Connection`.
- Embed ordered SQL files in the Rust binary. Validate installed versions/checksums and reject newer versions, gaps or mismatches **before any write**. Apply **all pending migrations together in one immediate transaction**, including checksum-column upgrade, profile insertion, migration ledger rows, `PRAGMA foreign_key_check`, and `PRAGMA integrity_check` before commit. A failure rolls back the entire pending batch; one transaction per version could leave an earlier version committed and would make #23's “No changes were applied” error copy false. Do not put `BEGIN`/`COMMIT` in embedded SQL or execute transaction-control SQL from `execute_batch`. Upgrade tests use **on-disk** SQLite and the real embedded SQL with foreign keys enabled on each connection. Simulate interruption before commit and verify prior state survives; a future destructive upgrade takes a verified backup first.
- Before a potentially destructive upgrade, create and verify a consistent SQLite backup using the SQLite backup API (not a blind copy of a live WAL database), then retain it until the upgrade and verification succeed. Do not overwrite an existing backup. Coordinate backup ownership with #15.
- Run integrity checks and version validation before exposing any financial command; on failure present a safe non-financial error state, never an empty new database. Test fresh install, one-version upgrade, second launch, duplicate receipt, invalid FK, and recovery from interrupted migration.
