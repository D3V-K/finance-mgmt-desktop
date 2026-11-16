# v1 transport addendum — **proposed**, needs Codex review

Status: **Proposed by Claude with #6.** It does not change any approved v1 type or command. It names the stable error codes v1 requires but leaves unspelled, adds lifecycle commands, and states how wire input is validated. Codex should confirm or amend before #7 adapters land.

## 1. Stable error codes (`src-tauri/src/error.rs::codes`, `src/lib/types.ts::ErrorCode`)

| Code | Category | Raised by |
| --- | --- | --- |
| `INVALID_ID`, `INVALID_DATE`, `INVALID_AMOUNT`, `INVALID_TEXT`, `INVALID_PAGE` | validation (+`field`, camelCase wire name) | transport (`commands/wire.rs`); domain may reuse |
| `INVALID_REQUEST` | validation | TS transport, for malformed invoke args |
| `MISSING_OPENING` | validation | domain #7 |
| `ACCOUNT_ARCHIVED` | conflict | domain #7 |
| `UNRESOLVED_MAPPING` | conflict | domain #7/#8 |
| `IDEMPOTENCY_CONFLICT` | conflict | domain #7 (same key, different payload) |
| `NOT_FOUND` | not_found | any |
| `STORAGE_FAILURE`, `STORAGE_UNAVAILABLE` | storage | `From<rusqlite::Error>` / IO |
| `STORAGE_NOT_READY` | storage | any data command before successful boot |
| `MIGRATION_FAILED`, `DATABASE_TOO_NEW` | storage | boot; `DATABASE_TOO_NEW` must be returned by the #5 migrator |
| `INSTANCE_LOCKED` | conflict | boot |

Categories for the domain codes are proposals. **Question for Codex:** should `MISSING_OPENING` be `validation` (a write rejected for lack of an opening) or should a read return `Balance.status = 'unavailable'` instead? v1 already says reads return `balance: null`, so this code applies to writes only.

## 2. Lifecycle commands (no DB schema involved)

- `get_app_status` → `{ state: 'ready' | 'failed'; error: AppError | null }`. Always callable; the renderer gates every financial screen on it.
- `get_system_info` → `{ appVersion: string; osTarget: string }`. **Changed:** dropped `appDataDir`, `timeZone` and `today`. `timeZone` was an offset (`+09:00`), not IANA; date context now comes only from `get_profile`.
- `get_profile` → `{ timeZone: 'Asia/Tokyo'; today }`, as approved in v1. It is computed in the IANA zone and independent of the OS clock zone. It reads the stored zone once #5 has a profile table.

## 3. Wire validation rule

Rust v1 input structs should take **IDs and dates as `String`** and convert them with `commands::wire::{parse_id, parse_date}` inside the adapter, instead of deserializing straight into `Uuid`/`NaiveDate`. A Serde failure becomes an opaque Tauri invoke error with no `field`; parsing in the adapter returns `INVALID_ID`/`INVALID_DATE` with the field name. Output types are unchanged (`Uuid`/`NaiveDate` may serialize directly). `WriteRequest` rejects unknown envelope fields. Amounts must lie within ±`Number.MAX_SAFE_INTEGER` (`wire::yen`/`positive_yen`). Rust `i64` outputs beyond that range would lose precision in the renderer, so domain outputs should also stay within it.

## 4. Adding a command

1. Handler in `src-tauri/src/commands/`.
2. `generate_handler!` in `lib.rs`.
3. `COMMANDS` in `commands/mod.rs` and `build.rs`.
4. `allow-<kebab-name>` in `capabilities/default.json`.
5. A typed wrapper in `src/lib/ledger.ts` (or a sibling).

A unit test enforces steps 2–4.
