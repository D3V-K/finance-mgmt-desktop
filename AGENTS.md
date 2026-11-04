# Parallel implementation guide

This repository is the **new, single-user, fully offline** finance desktop app. Use Tauri 2, Svelte/TypeScript, Rust, and SQLite. The existing [frontend](https://github.com/D3V-K/finance-mgmt-frontend) and [Python backend](https://github.com/D3V-K/finance-mgmt-backend-lite) are behavior references, **not** components to bundle. The [desktop epic](https://github.com/D3V-K/finance-mgmt-desktop/issues/1) tracks the work; each issue has its own acceptance criteria.

## Assignments

| Agent | Issues | Primary ownership | Deliverable |
| --- | --- | --- | --- |
| **Codex** | [#2](https://github.com/D3V-K/finance-mgmt-desktop/issues/2), [#7](https://github.com/D3V-K/finance-mgmt-desktop/issues/7)–[#11](https://github.com/D3V-K/finance-mgmt-desktop/issues/11) | Rust domain model and financial calculations, parity fixtures/tests; later `src-tauri/src/domain/` (or agreed equivalent) | Document shipped vs. proposed behavior, create golden fixtures, port the ledger and advanced financial rules without changing their meaning. |
| **Claude** | [#3](https://github.com/D3V-K/finance-mgmt-desktop/issues/3), [#12](https://github.com/D3V-K/finance-mgmt-desktop/issues/12), [#13](https://github.com/D3V-K/finance-mgmt-desktop/issues/13) | New Svelte UI, visual system, accessibility, frontend tests; later `src/` (or agreed equivalent) | Prototype and implement a redesigned desktop experience using typed commands and deterministic fixtures. Do not transplant the React UI. |
| **Antigravity CLI** | [#4](https://github.com/D3V-K/finance-mgmt-desktop/issues/4)–[#6](https://github.com/D3V-K/finance-mgmt-desktop/issues/6), [#14](https://github.com/D3V-K/finance-mgmt-desktop/issues/14)–[#16](https://github.com/D3V-K/finance-mgmt-desktop/issues/16) | Tauri shell and command adapters, SQLite schema/migrations, local security/lifecycle, import, backup/restore, packaging; later `src-tauri/src/{db,commands}/`, `src-tauri/migrations/`, and build configuration | Establish the app/runtime boundary and storage, then deliver safe data migration and offline releases. |

These are ownership boundaries, **not** permission to design interfaces independently. Codex specifies domain inputs/outputs and expected behavior; Antigravity owns the Tauri command transport and database migration files; Claude consumes the agreed typed command contract. If the generated project uses different directories, agree on the equivalents before coding.

## Sequence and parallel work

1. **Wave 1 — parallel:** Codex starts #2 (feature inventory, Python parity fixtures); Claude starts #3 (synthetic-data UI prototype); Antigravity starts #4 (Tauri/Svelte/Rust scaffold). Each may open a separate PR. Before the next wave, agree on supported OS targets, the first vertical slice, date/money/unknown-value conventions, typed command names and payloads, error shape, and fixture format. #2 must distinguish code that works today from feature-flagged or merely proposed behavior.
2. **Wave 2 — mostly parallel:** Antigravity implements #5 and #6; Codex starts #7 after the SQLite model and command/domain boundaries are agreed; Claude can build #12 against fixtures and mock command implementations after #3/#4, but cannot mark it complete until the real #7/#8 paths work. Antigravity may start #15 after #5. Merge migrations and contract definitions before their consumers.
3. **Wave 3 — parallel feature lanes:** After #7 lands, Codex handles #8–#11 in small independent PRs rather than one long branch. Claude implements the corresponding #12/#13 screens against agreed contracts. Antigravity develops #14's export/schema mapping and #15 in parallel. Coordinate card schedules (#9) with card-dependent forecast/reconciliation work (#10/#11). #8 is needed to finish the cutover UI in #12. #14 cannot be accepted until the full target schema, relationships, and financial verification are stable.
4. **Wave 4 — integration:** Antigravity handles #16 only after domain parity, real-command UI paths, verified import, and backup/restore pass. The human maintainer decides merge order and release readiness; no agent should silently retire AWS resources.

## Collaboration protocol

- Work in **separate worktrees/branches**, preferably one issue per branch (`codex/7-core-ledger`, `claude/12-core-ui`, `antigravity/5-sqlite-schema`). Never let multiple agents modify the same working tree. Open small PRs targeting the current default branch, reference the issue, and list tests, assumptions, contract changes, and remaining gaps.
- Antigravity owns migration files and Tauri command registration/transport; Codex owns domain calculations and parity fixtures; Claude owns UI components/styles. A change to a shared contract or schema requires review from the affected owners **before** implementation branches diverge. Do not have two agents independently rename commands, alter the DB schema, or edit the same shared files.
- Keep a versioned typed frontend-to-Tauri command contract in the repo once #4 creates the structure. Proposed changes should state Rust input/output types, TypeScript equivalents, errors, money units, date/time-zone meaning, null/incomplete semantics, and whether a write is idempotent. Do not create a second local HTTP backend just to reuse the old API.
- Use #2's golden fixtures as the authority for financial parity. Require Rust unit/integration tests on real SQLite migrations, frontend tests for accessible behavior, and at least one packaged-app offline end-to-end test before calling a feature done. Mocked UI success alone is not enough.
- Integrate frequently. A PR depending on another PR should say so explicitly; rebase or merge the updated default branch after the dependency lands. Do not merge generated schema, frontend, and domain changes independently if the contracts differ. The maintainer resolves conflicting assumptions, not whichever agent merges first.
- Preserve JPY integer amounts, stable IDs, legacy/canonical separation, card-debt signs, correction/audit links, unknown dates, incomplete balances, and the distinction among spending, transfers, repayment, earmarking, and future forecasts. The legacy net-balance series is **not** true net worth.
- No Cognito/login, cloud dependency, telemetry, or network requirement in the installed app. Use scoped Tauri permissions and local commands. Back up Aurora before import; select a source user explicitly, never merge users implicitly; validate counts, balances, reports, and relationships before accepting data. Do not shut down AWS until an imported database and restored backup are verified.

## Immediate handoff checklist

- **Codex:** post #2's first feature/fixture matrix and propose the minimal account/transaction domain types to Antigravity and Claude.
- **Claude:** post #3's navigation and prototype decision; use synthetic fixtures until the command contract is approved.
- **Antigravity:** post #4's repo layout and draft typed-command/SQLite integration boundary; do not lock in a schema without Codex's parity review.
- **Maintainer:** approve the shared contract and merge the foundation PRs before assigning dependent issues as ready.
