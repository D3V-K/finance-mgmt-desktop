# Rust financial port for issues #7–#11

The port runs in the Tauri process over local SQLite. At boot the app installs the reviewed core ledger and the planning and review tables in one immediate transaction. The migration runner verifies checksums at startup and checks database integrity and foreign keys when applying migrations. Git keeps the embedded SQL files in LF format so their checksums are portable. The runner upgrades a scaffold database in place. Each financial write commits its data and UUID retry receipt together. Planning and dashboard queries share consistent read snapshots.

| Issue | Rust service coverage |
| --- | --- |
| #7 | Accounts and append-only openings, category history, separate legacy Cash/Bank accounting, canonical postings, linked refunds, reversals and atomic replacements. |
| #8 | Cutover preview, immutable confirmation, retained legacy source IDs, and explicit resolution of outstanding mappings. Historical rows remain inspectable and stay outside the opening balance calculation. |
| #9 | Card settings, installment and opening-debt schedules, statements, repayment and refund applications, cash reserves, savings goals and funding buckets. Both allocation types use the same SQLite capacity check. |
| #10 | Local payday cycles and overrides, category budgets, expected flow recurrence and matching, per-account cash forecasts, available cash and dashboard aggregates. |
| #11 | Securities-only manual valuations, holdings-change staleness, position history, reconciliation preview and review, and audit and data-quality reads. |

JPY amounts remain signed where appropriate. Opening dates, effective posting dates, purchase dates and UTC audit timestamps retain their distinct meanings. Cutover stores the original unresolved count on the immutable confirmation and derives current coverage from the provenance rows. Legacy net balance and financial position are separate reports. Forecasts use an explicit as-of date and identify expected flows, actual postings and schedule withdrawals. Schedule-linked flows use the schedule's withdrawal. Confirmed bills use either the forecast occurrence or the matched actual. Salary projections affect forecast cash only.

Cutover requires explicit openings for active accounts. For an account that already has postings, confirmation requires its existing opening amount and a boundary on or before that opening. The original opening date, revisions and postings are retained, and a mapping cannot post before that account's opening. A payday change moves budgets for the affected calendar month to the new payday, preserving their IDs and payment-account assumptions in the same transaction.

Available cash counts a reserve against its schedule across all funding accounts and identifies reserves held outside the settlement account. The forecast projects withdrawals against the configured settlement account. Overdue schedules remain commitments until settled. Their unpaid forecast amount is placed on the first projected date, with the assumption disclosed in the report.

The first-slice command shapes still follow [v1](../contracts/v1-core-ledger.md). This PR exposes typed Rust services and wires migrations and the persisted time zone into the existing boot and profile path. Financial command registration, UI integration and packaged-app offline financial workflows belong to the subsequent integration work. The extended tables are local storage with explicit foreign keys. The later Aurora mapping and import verification remain part of #14.

The Rust test suite replays the seven scenarios in `fixtures/parity/v1/cases.json` against on-disk migrated databases. Additional tests cover a failed migration batch, upgrades, correction rollback, partial cutover resolution, concurrent funding, leap-day payday cycles, forecast matching and reconciliation staleness. Full source parity still requires broader fixtures and the exact-pin/PostgreSQL reference run described in the parity handoff.
