# #5 SQLite model proposal — review before migration

**Status:** proposed, not an installed schema or an accepted cross-layer change. Review with the #6 command owner and maintainer against `docs/contracts/v1-core-ledger.md` before creating `src-tauri/migrations/` or implementing #7. Reference: the Python models in `finance-mgmt-backend-lite/src/models/` at `e474d383dc42d298ea9ba513bfd955fa3e212e2c`, plus `fixtures/parity/v1/cases.json`. This proposal covers the first slice; it is not a substitute for mapping every Aurora table for #14.

## Decisions requested

1. **Migration runner:** #5 asks for SQLx migrations; the merged #4 scaffold currently opens SQLite through `rusqlite` and creates `_schema_migrations` on first open. Adopt SQLx for migrations (and decide whether #7 also migrates database access), or explicitly revise #5's acceptance criterion to permit a transactional `rusqlite` runner. Do **not** maintain competing version tables or run the proposed SQL by `create_all` in tests. Existing scaffold database files with migration version 0 need an upgrade path, not deletion.
2. **Stable IDs and import provenance:** local profile gets a new UUID; `source_user_id` is nullable until import and is never a login. Import retains source UUIDs. A second source user must be refused, not silently merged. UUID spelling is validated in Rust and stored as canonical lowercase text for new records; imported source spelling/identity should be checked before normalization. Decide whether preserving byte-equivalent uppercase UUID spelling matters to #14.
3. **Legacy accounting:** `CASH`/`BANK` on legacy rows are labels, not foreign keys to named accounts. Legacy openings/transfers/reports remain separate. Canonical cutover creates explicit provenance links, not copies with invented purchase dates. Opening revision is an append-only audit record; the current snapshot may be rebuilt from its greatest revision.
4. **Time and integer wire:** `INTEGER` is signed i64 yen, `TEXT` dates are calendar ISO `YYYY-MM-DD` in the profile IANA zone (initial `Asia/Tokyo`), UTC instants are RFC 3339, and unknown dates/balances remain nullable. App-layer validation is required for real dates, UUIDs, IANA zones, and `Number.isSafeInteger` wire bounds. SQLite `CHECK` constraints only defend shape and local invariants.

## Candidate first-slice DDL (illustrative, not yet a migration)

The following is intended for one transactional migration after reviewing the runner. `PRAGMA foreign_keys=ON` must be enabled on **every** connection before writes; test it with an on-disk database. Table names are deliberately distinct from the Python legacy/canonical concepts.

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
CREATE INDEX events_occurred_idx ON financial_events(occurred_on);

CREATE TABLE account_postings (
  id TEXT PRIMARY KEY,
  event_id TEXT NOT NULL REFERENCES financial_events(id) ON DELETE RESTRICT,
  account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
  delta INTEGER NOT NULL CHECK (delta <> 0),
  effective_date TEXT NOT NULL CHECK (length(effective_date) = 10)
);
CREATE INDEX postings_account_date_idx ON account_postings(account_id, effective_date, event_id);

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
- Each migration is numbered, checksummed, transactional, and applied exactly once; reject unknown newer versions and checksum mismatch without writing. Upgrade tests use **on-disk** SQLite and real migration files, with foreign keys enabled on each connection and `PRAGMA foreign_key_check` after upgrade. Simulate interruption before commit and verify prior state survives.
- Before a potentially destructive upgrade, create and verify a consistent SQLite backup using the SQLite backup API (not a blind copy of a live WAL database), then retain it until the upgrade and verification succeed. Do not overwrite an existing backup. Coordinate backup ownership with #15.
- Run integrity checks and version validation before exposing any financial command; on failure present a safe non-financial error state, never an empty new database. Test fresh install, one-version upgrade, second launch, duplicate receipt, invalid FK, and recovery from interrupted migration.
