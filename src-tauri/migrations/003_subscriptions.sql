CREATE TABLE subscriptions (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL CHECK (length(trim(name)) > 0),
  merchant TEXT,
  account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
  category_id TEXT NOT NULL REFERENCES categories(id) ON DELETE RESTRICT,
  currency TEXT NOT NULL CHECK (currency IN ('JPY', 'USD')),
  cadence TEXT NOT NULL CHECK (cadence IN ('weekly', 'monthly', 'quarterly', 'yearly')),
  billing_day INTEGER CHECK (billing_day IS NULL OR billing_day BETWEEN 1 AND 31),
  billing_month INTEGER CHECK (billing_month IS NULL OR billing_month BETWEEN 1 AND 12),
  start_date TEXT NOT NULL CHECK (length(start_date) = 10),
  status TEXT NOT NULL CHECK (status IN ('active', 'paused', 'canceled')),
  paused_from TEXT CHECK (paused_from IS NULL OR length(paused_from) = 10),
  canceled_effective_on TEXT CHECK (canceled_effective_on IS NULL OR length(canceled_effective_on) = 10),
  note TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  CHECK (status <> 'paused' OR paused_from IS NOT NULL),
  CHECK (status <> 'canceled' OR canceled_effective_on IS NOT NULL)
);
CREATE INDEX subscriptions_status_idx ON subscriptions(status, account_id);

CREATE TABLE subscription_plan_revisions (
  id TEXT PRIMARY KEY,
  subscription_id TEXT NOT NULL REFERENCES subscriptions(id) ON DELETE RESTRICT,
  effective_from TEXT NOT NULL CHECK (length(effective_from) = 10),
  amount_minor INTEGER NOT NULL CHECK (amount_minor > 0),
  revision INTEGER NOT NULL CHECK (revision > 0),
  reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),
  recorded_at TEXT NOT NULL,
  UNIQUE (subscription_id, revision),
  UNIQUE (subscription_id, effective_from)
);
CREATE INDEX sub_revisions_lookup_idx
  ON subscription_plan_revisions(subscription_id, effective_from DESC, revision DESC);

CREATE TRIGGER immutable_sub_revision_update
  BEFORE UPDATE ON subscription_plan_revisions
  BEGIN SELECT RAISE(ABORT, 'Subscription plan revisions are append-only'); END;
CREATE TRIGGER immutable_sub_revision_delete
  BEFORE DELETE ON subscription_plan_revisions
  BEGIN SELECT RAISE(ABORT, 'Subscription plan revisions are append-only'); END;

CREATE TABLE subscription_occurrences (
  id TEXT PRIMARY KEY,
  subscription_id TEXT NOT NULL REFERENCES subscriptions(id) ON DELETE RESTRICT,
  scheduled_date TEXT NOT NULL CHECK (length(scheduled_date) = 10),
  override_date TEXT CHECK (override_date IS NULL OR length(override_date) = 10),
  state TEXT NOT NULL CHECK (state IN ('scheduled', 'skipped', 'reconciled')),
  billed_currency TEXT NOT NULL CHECK (billed_currency IN ('JPY', 'USD')),
  billed_amount_minor INTEGER NOT NULL CHECK (billed_amount_minor > 0),
  actual_jpy INTEGER CHECK (actual_jpy IS NULL OR actual_jpy > 0),
  settled_on TEXT CHECK (settled_on IS NULL OR length(settled_on) = 10),
  event_id TEXT UNIQUE REFERENCES financial_events(id) ON DELETE RESTRICT,
  schedule_id TEXT REFERENCES card_schedules(id) ON DELETE RESTRICT,
  updated_at TEXT NOT NULL,
  UNIQUE (subscription_id, scheduled_date),
  CHECK (
    (state = 'reconciled' AND event_id IS NOT NULL AND actual_jpy IS NOT NULL AND settled_on IS NOT NULL)
    OR (state <> 'reconciled' AND event_id IS NULL AND actual_jpy IS NULL AND settled_on IS NULL)
  ),
  CHECK (state <> 'skipped' OR override_date IS NULL)
);
CREATE INDEX sub_occurrences_state_idx
  ON subscription_occurrences(state, scheduled_date);
CREATE INDEX sub_occurrences_event_idx
  ON subscription_occurrences(event_id);
