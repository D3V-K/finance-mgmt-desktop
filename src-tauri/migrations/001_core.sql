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
  description TEXT,
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
  status TEXT NOT NULL CHECK (status IN ('historical', 'mapped', 'unresolved')),
  event_id TEXT UNIQUE REFERENCES financial_events(id) ON DELETE RESTRICT,
  UNIQUE (source_table, source_id),
  CHECK ((status = 'mapped' AND event_id IS NOT NULL) OR (status IN ('historical', 'unresolved') AND event_id IS NULL))
);

CREATE TRIGGER immutable_event_update BEFORE UPDATE ON financial_events
BEGIN SELECT RAISE(ABORT, 'Financial events are immutable'); END;
CREATE TRIGGER immutable_event_delete BEFORE DELETE ON financial_events
BEGIN SELECT RAISE(ABORT, 'Financial events are immutable'); END;
CREATE TRIGGER immutable_posting_update BEFORE UPDATE ON account_postings
BEGIN SELECT RAISE(ABORT, 'Account postings are immutable'); END;
CREATE TRIGGER immutable_posting_delete BEFORE DELETE ON account_postings
BEGIN SELECT RAISE(ABORT, 'Account postings are immutable'); END;
CREATE TRIGGER immutable_cutover_update BEFORE UPDATE ON ledger_cutover
BEGIN SELECT RAISE(ABORT, 'Cutover confirmation is immutable'); END;
CREATE TRIGGER immutable_cutover_delete BEFORE DELETE ON ledger_cutover
BEGIN SELECT RAISE(ABORT, 'Cutover confirmation is immutable'); END;

CREATE TRIGGER freeze_legacy_transactions_insert BEFORE INSERT ON legacy_transactions
WHEN EXISTS (SELECT 1 FROM ledger_cutover)
BEGIN SELECT RAISE(ABORT, 'Legacy history is read-only after cutover'); END;

CREATE TRIGGER freeze_legacy_transactions_update BEFORE UPDATE ON legacy_transactions
WHEN EXISTS (SELECT 1 FROM ledger_cutover)
BEGIN SELECT RAISE(ABORT, 'Legacy history is read-only after cutover'); END;

CREATE TRIGGER freeze_legacy_transactions_delete BEFORE DELETE ON legacy_transactions
WHEN EXISTS (SELECT 1 FROM ledger_cutover)
BEGIN SELECT RAISE(ABORT, 'Legacy history is read-only after cutover'); END;

CREATE TRIGGER freeze_legacy_transfers_insert BEFORE INSERT ON legacy_transfers
WHEN EXISTS (SELECT 1 FROM ledger_cutover)
BEGIN SELECT RAISE(ABORT, 'Legacy history is read-only after cutover'); END;

CREATE TRIGGER freeze_legacy_transfers_update BEFORE UPDATE ON legacy_transfers
WHEN EXISTS (SELECT 1 FROM ledger_cutover)
BEGIN SELECT RAISE(ABORT, 'Legacy history is read-only after cutover'); END;

CREATE TRIGGER freeze_legacy_transfers_delete BEFORE DELETE ON legacy_transfers
WHEN EXISTS (SELECT 1 FROM ledger_cutover)
BEGIN SELECT RAISE(ABORT, 'Legacy history is read-only after cutover'); END;

CREATE TRIGGER freeze_legacy_openings_insert BEFORE INSERT ON legacy_openings
WHEN EXISTS (SELECT 1 FROM ledger_cutover)
BEGIN SELECT RAISE(ABORT, 'Legacy history is read-only after cutover'); END;

CREATE TRIGGER freeze_legacy_openings_update BEFORE UPDATE ON legacy_openings
WHEN EXISTS (SELECT 1 FROM ledger_cutover)
BEGIN SELECT RAISE(ABORT, 'Legacy history is read-only after cutover'); END;

CREATE TRIGGER freeze_legacy_openings_delete BEFORE DELETE ON legacy_openings
WHEN EXISTS (SELECT 1 FROM ledger_cutover)
BEGIN SELECT RAISE(ABORT, 'Legacy history is read-only after cutover'); END;

CREATE TRIGGER immutable_opening_update BEFORE UPDATE ON account_opening_revisions
BEGIN SELECT RAISE(ABORT, 'Opening revisions are append-only'); END;
CREATE TRIGGER immutable_opening_delete BEFORE DELETE ON account_opening_revisions
BEGIN SELECT RAISE(ABORT, 'Opening revisions are append-only'); END;
