# Desktop parity handoff v1 (#2)

Reference snapshots inspected: `finance-mgmt-backend-lite` at `e474d383dc42d298ea9ba513bfd955fa3e212e2c` and `finance-mgmt-frontend` at `b6fe5c82f4ad372ce52a07606aede61efef673d7` (local checkouts, 2026-11-08). This is **source/test evidence**, not a claim about the deployed AWS environment or any user's data. The backend checkout has unrelated local `.env` and `requirements.txt` edits; neither was used as a behavior reference. No live AWS access was used.

## Shipped code versus rollout

All listed Python routers are registered in `src/main.py` and have at least one backend test. The React frontend's `src/config/features.ts` gates the new screens using `VITE_LEDGER_FEATURES`; an unset flag yields **legacy mode**, and `all` enables every gated screen. Thus "implemented and tested in source" does **not** establish that a production deployment has it enabled. Authentication, API Gateway, Aurora and Cognito belong to the old service, not the desktop app.

| Lane | Python implementation and tests | Frontend rollout / desktop status | Parity rule / issue |
| --- | --- | --- | --- |
| Legacy categories, CASH/BANK transactions, openings, transfers, monthly/category/balance reports | Routers in `src/routers/{categories,transactions,opening_balances,transfers,reports}.py`; tests in corresponding `tests/test_*.py` | Legacy pages ungated; desktop not implemented | Preserve legacy records, original dates, IDs and report basis separately from canonical data; #7/#14 |
| Named accounts and per-account opening revisions | `accounts.py`, `services/balances.py`, `tests/test_accounts.py` | `accounts` flag; desktop scaffold only | Opening is the **start** of its date; no opening or earlier query gives `null` / `unavailable`. Card balance is signed, owed is positive derived; #5/#7 |
| Canonical income, expense, transfer, card repayment, refund, adjustment, correction | `services/ledger.py`, `financial_events.py`, `tests/test_financial_events.py`, `test_reconciliations.py` | `events` flag; desktop not implemented | Positive event amount, signed postings; transfer/repayment not spending; refund linked and capped; reversal negates original postings, replacement writes reversal plus new event atomically; #7/#11 |
| Cutover/provenance | `services/cutover.py`, `ledger_cutovers.py`, `tests/test_cutovers.py` | `cutover` flag; desktop not implemented | Explicit opening/mapping, unknown purchase date remains null, legacy read-only after confirmation, unresolved records visible; #8 |
| Category archive/history | `categories.py`, `tests/test_categories.py` | `category-archive` flag; desktop not implemented | Keep used categories and hierarchy, restrict destructive edits; #7/#8 |
| Card schedules, statements, reserves | `cards.py`, `tests/test_cards.py` | `cards` flag; desktop not implemented | Opening debt is not new spending; earmarking does not change cash balance; confirmed statement replaces estimate; #9 |
| Payday budgets, savings buckets, allocation | `budgets.py`, `tests/test_budgets.py` | `budgets` flag; desktop not implemented | Card purchase counts toward spending on occurrence date; funding a bucket changes allocatable cash, not account balance; #10 |
| Forecast, expected flows, recurrence, dashboard | `forecast.py`, `dashboard.py`, `tests/test_forecast.py`, `test_dashboard.py` | `forecast`/`dashboard` flags; desktop not implemented | Scenario cash is not actual income; warn on missing mappings and uncertain same-day order; #10 |
| Investments and true financial position | `investments.py`, `services/position.py`, `tests/test_investments.py` | `investments` flag; desktop not implemented | Signed card debt plus liquid cash plus securities valuation; missing/stale valuation makes net worth incomplete; #11 |
| Reconciliation/data quality/export | `reconciliations.py`, `tests/test_reconciliations.py` | `reconciliation` flag; desktop not implemented | Confirmed periods guard reversal/opening changes; preserve adjustment audit and per-posting match; #11/#14 |

**Misleading legacy name:** `/reports/net-worth` is an alias for the legacy **net-balance** series (`reports.py:125–182`): opening CASH+BANK plus cumulative legacy net cashflow, with no month absent activity. It is not true net worth. The latter is `/reports/financial-position`. Do not label or merge the two.

## Golden cases and limits

`fixtures/parity/v1/cases.json` contains five isolated synthetic scenarios and 77 checked HTTP steps. `reference.py` executes the backend through FastAPI TestClient against in-memory SQLite with foreign keys, a test-only user, and a `date_trunc` shim; it **forces `DATABASE_URL=sqlite://` before importing the backend**. Results checked include account opening unknown/zero and revision, card signs, idempotent event replay/conflict, refunds, reversal, cutover unknown purchase date, legacy-versus-canonical reports, card scheduling/reserves, forecast scenario separation, valuation and reconciliation guards. Generated UUIDs are referenced within a case, never snapshotted as random bytes. Posting lists are compared by account ID because the reference orders them by random posting UUID.

Run from this repository with a Python environment containing the reference backend's dependencies plus pytest/httpx:

```sh
python fixtures/parity/v1/reference.py /path/to/finance-mgmt-backend-lite
```

On 2026-11-08: all five cases passed; the reference's own suite passed **125 tests** against in-memory SQLite. Environment used Python 3.14, FastAPI 0.128.8 and SQLAlchemy 2.0.57 because the reference checkout's committed FastAPI 0.141.1 pin was unavailable from the accessible package index. Repeat against exact reference pins and PostgreSQL before declaring full parity. The reference's test DB uses `Base.metadata.create_all`, **not** real Alembic or desktop SQLite migrations. Rust parity must replay these cases against the actual #5 migrations; these JSON cases are the initial golden authority, not a claim that Rust already passes.

Still required for #2 completion: broader independent fixtures for category tree/archive, opening correction audit, unresolved resolution, budget cycle/leap day, recurring expected flows, statement reconciliation, full export relationships and migration idempotency. The five cases intentionally cover representative paths, not every branch. The frontend prototype's synthetic values and `derive.ts` are not golden financial rules.

## Import acceptance thresholds (#14)

Select **one source user UUID explicitly** after an Aurora backup; never implicitly merge users or hit AWS during installed-app operation. Compare per-table source/export/import row counts and stable IDs **exactly** (zero missing, duplicate or cross-user rows). Compare every opening revision, event/posting sum, account balance at opening/cutover/latest dates, legacy monthly/category totals and canonical known-date monthly/category totals in **integer JPY with zero difference**; compare coverage/status and unknown/null values exactly, not as zero. Every foreign key, provenance link, reversal/replacement/refund link and card schedule/payment/refund association must resolve; zero broken links. Unresolved cutover records may remain only if their exact count and IDs match the source and are surfaced as incomplete; zero silently dropped records. Reimporting the same export must produce zero new rows and unchanged reports; changed payload under the same import identity must conflict. Backup restore must reproduce the same checks before an imported database is accepted. Any mismatch blocks acceptance and AWS retirement; no arbitrary tolerance or silent auto-fix.
