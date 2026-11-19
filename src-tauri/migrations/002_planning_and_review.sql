CREATE TABLE card_settings (
  card_id TEXT PRIMARY KEY REFERENCES accounts(id),
  settlement_account_id TEXT NOT NULL REFERENCES accounts(id),
  closing_day INTEGER NOT NULL CHECK (closing_day BETWEEN 1 AND 31),
  payment_day INTEGER NOT NULL CHECK (payment_day BETWEEN 1 AND 31)
);
CREATE TABLE card_statements (
  id TEXT PRIMARY KEY,
  card_id TEXT NOT NULL REFERENCES accounts(id),
  closing_on TEXT NOT NULL,
  due_date TEXT NOT NULL CHECK (due_date > closing_on),
  principal_confirmed INTEGER NOT NULL CHECK (principal_confirmed >= 0),
  UNIQUE(card_id, closing_on),
  UNIQUE(card_id, due_date)
);
CREATE TABLE card_schedules (
  id TEXT PRIMARY KEY,
  card_id TEXT NOT NULL REFERENCES accounts(id),
  purchase_event_id TEXT REFERENCES financial_events(id),
  opening_obligation TEXT,
  statement_id TEXT REFERENCES card_statements(id),
  due_date TEXT NOT NULL,
  principal_due INTEGER NOT NULL CHECK (principal_due > 0),
  source TEXT NOT NULL CHECK (source IN ('estimate','actual')),
  CHECK ((purchase_event_id IS NULL) <> (opening_obligation IS NULL))
);
CREATE INDEX schedules_card_due_idx ON card_schedules(card_id,due_date);
CREATE INDEX schedules_purchase_idx ON card_schedules(purchase_event_id);
CREATE TABLE card_applications (
  schedule_id TEXT NOT NULL REFERENCES card_schedules(id),
  event_id TEXT NOT NULL REFERENCES financial_events(id),
  kind TEXT NOT NULL CHECK(kind IN ('payment','refund')),
  amount INTEGER NOT NULL CHECK(amount > 0),
  effective_date TEXT NOT NULL,
  PRIMARY KEY(schedule_id,event_id)
);
CREATE INDEX card_applications_event_idx ON card_applications(event_id);

CREATE TABLE savings_goals (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL CHECK(length(trim(name)) > 0),
  target INTEGER NOT NULL CHECK(target > 0),
  target_date TEXT,
  archived_at TEXT
);
CREATE TABLE savings_buckets (
  id TEXT PRIMARY KEY,
  goal_id TEXT REFERENCES savings_goals(id),
  name TEXT NOT NULL CHECK(length(trim(name)) > 0),
  archived_at TEXT
);
CREATE TABLE funding_allocations (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL REFERENCES accounts(id),
  schedule_id TEXT REFERENCES card_schedules(id),
  bucket_id TEXT REFERENCES savings_buckets(id),
  amount INTEGER NOT NULL CHECK(amount <> 0),
  effective_date TEXT NOT NULL,
  reason TEXT NOT NULL CHECK(length(trim(reason)) > 0),
  CHECK ((schedule_id IS NULL) <> (bucket_id IS NULL))
);
CREATE INDEX allocations_account_idx ON funding_allocations(account_id,effective_date);
CREATE INDEX allocations_schedule_idx ON funding_allocations(schedule_id,account_id);
CREATE INDEX allocations_bucket_idx ON funding_allocations(bucket_id,account_id);

CREATE TABLE finance_settings (
  singleton INTEGER PRIMARY KEY CHECK(singleton=1),
  payday_day INTEGER NOT NULL CHECK(payday_day BETWEEN 1 AND 31),
  default_account_id TEXT REFERENCES accounts(id)
);
CREATE TABLE payday_overrides (
  month TEXT PRIMARY KEY,
  actual_date TEXT NOT NULL
);
CREATE TABLE category_budgets (
  id TEXT PRIMARY KEY,
  category_id TEXT NOT NULL REFERENCES categories(id),
  cycle_start TEXT NOT NULL,
  planned INTEGER NOT NULL CHECK(planned >= 0),
  payment_account_id TEXT REFERENCES accounts(id),
  UNIQUE(category_id,cycle_start)
);
CREATE INDEX category_budgets_cycle_idx ON category_budgets(cycle_start);
CREATE TABLE expected_flows (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL REFERENCES accounts(id),
  on_date TEXT NOT NULL,
  amount INTEGER NOT NULL CHECK(amount <> 0),
  label TEXT NOT NULL CHECK(length(trim(label)) > 0),
  recurrence TEXT NOT NULL CHECK(recurrence IN ('none','monthly')),
  certainty TEXT NOT NULL CHECK(certainty IN ('confirmed','estimated','scenario_only')),
  schedule_id TEXT UNIQUE REFERENCES card_schedules(id),
  budget_category_id TEXT REFERENCES categories(id)
);
CREATE TABLE flow_overrides (
  flow_id TEXT NOT NULL REFERENCES expected_flows(id),
  scheduled_date TEXT NOT NULL,
  override_date TEXT,
  skipped INTEGER NOT NULL CHECK(skipped IN (0,1)),
  PRIMARY KEY(flow_id,scheduled_date),
  CHECK(NOT skipped OR override_date IS NULL)
);
CREATE TABLE flow_matches (
  flow_id TEXT NOT NULL REFERENCES expected_flows(id),
  scheduled_date TEXT NOT NULL,
  event_id TEXT NOT NULL REFERENCES financial_events(id),
  PRIMARY KEY(flow_id,scheduled_date),
  UNIQUE(event_id)
);

CREATE TABLE securities_valuations (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL REFERENCES accounts(id),
  as_of_date TEXT NOT NULL,
  market_value INTEGER NOT NULL CHECK(market_value >= 0),
  note TEXT,
  recorded_at TEXT NOT NULL,
  UNIQUE(account_id,as_of_date)
);
CREATE TABLE holdings_changes (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL REFERENCES accounts(id),
  occurred_on TEXT NOT NULL,
  reason TEXT NOT NULL CHECK(length(trim(reason)) > 0),
  realized_gain INTEGER,
  recorded_at TEXT NOT NULL
);
CREATE INDEX holdings_account_date_idx ON holdings_changes(account_id,occurred_on,recorded_at);
CREATE TABLE reconciliations (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL REFERENCES accounts(id),
  period_start TEXT NOT NULL,
  period_end TEXT NOT NULL CHECK(period_end >= period_start),
  statement_opening INTEGER NOT NULL,
  statement_closing INTEGER NOT NULL,
  status TEXT NOT NULL CHECK(status IN ('draft','confirmed')),
  captured_at TEXT NOT NULL,
  UNIQUE(account_id,period_start,period_end)
);
CREATE TABLE reconciled_postings (
  session_id TEXT NOT NULL REFERENCES reconciliations(id),
  posting_id TEXT NOT NULL REFERENCES account_postings(id),
  PRIMARY KEY(session_id,posting_id)
);
CREATE TABLE reconciliation_audit (
  id INTEGER PRIMARY KEY,
  session_id TEXT NOT NULL REFERENCES reconciliations(id),
  action TEXT NOT NULL CHECK(action IN ('draft_created','confirmed','reopened','posting_included','posting_excluded')),
  posting_id TEXT REFERENCES account_postings(id),
  recorded_at TEXT NOT NULL
);
