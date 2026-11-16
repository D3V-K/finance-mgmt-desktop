# Local app boundary and lifecycle (#6)

Status: **Implemented for macOS; awaiting maintainer review.** Owner: Claude (transport, security, lifecycle). Schema and migrations remain Codex's (#5).

## Transport: Tauri commands only

- No HTTP server, TCP/UDP listener or remote origin. The renderer reaches Rust only through `invoke`.
- Verified on the packaged release `.app` (macOS 27.0.1, arm64): `lsof -i` shows **no network sockets**. The only socket is the single-instance plugin's Unix-domain socket (local, not a network interface). Its callback ignores forwarded arguments and only focuses the existing window.
- Commands are declared once in `src-tauri/src/commands/mod.rs` (`COMMANDS`) and mirrored in `build.rs` as a Tauri **app ACL manifest**, so each command becomes an `allow-<command>` permission. `capabilities/default.json` grants exactly those permissions to the `main` window, **with no `core:*`, filesystem, shell, opener, dialog or HTTP permission**. A unit test fails if `generate_handler!`, `COMMANDS` and the capability disagree. Adding a #7 command requires all three.

## Renderer containment

- CSP: `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'; connect-src ipc: http://ipc.localhost`. `connect-src` allows only Tauri IPC. `freezePrototype` is on and `withGlobalTauri` is off.
- The main window is built in `setup` (config `"create": false`) so it can carry `on_navigation`. That hook allows only `tauri://localhost` / `http(s)://tauri.localhost`, plus `http://localhost:1420` in debug builds only. It also has `on_new_window → Deny`. External links are not supported: there is no opener plugin, and the UI should not render clickable external URLs.
- The renderer cannot read files: there is no fs permission, and no command takes a path. IDs, dates, amounts, text and paging are validated in `commands/wire.rs` and return field-level `AppError`s.

## Startup order and outcomes

`setup` runs `lifecycle::boot` **before** the window exists:

1. Create the app-data dir `~/Library/Application Support/com.d3vk.financedesktop/` with mode `0700`.
2. Take an exclusive `flock` on `finance.lock`. The OS releases it if the process dies, so a crash never leaves a stale lock.
3. Pre-create `finance.db` as `0600` so SQLite's `-wal`/`-shm` inherit owner-only mode, open it (WAL, FKs, busy timeout), and repair any older world-readable sidecars.
4. Run the migrator. This is a placeholder until #5. **Requirement for #5:** apply each migration in a transaction, and return `DATABASE_TOO_NEW` when the stored version is newer than the app supports. Back up before destructive migrations (#5/#15).

Any failure yields `Boot::Failed(AppError)` instead of a crash. `get_app_status` is always available. Every data command calls `boot.db()?` and gets `STORAGE_NOT_READY` until boot succeeds. The renderer renders **no financial screen** unless the status is `ready`; otherwise it shows a blocking screen:

| Situation | Code | Outcome |
| --- | --- | --- |
| Second launch, same user | — | Single-instance plugin focuses the first window; the second process exits 0 (verified). |
| Lock held by another process (e.g. dev and release builds, or the plugin bypassed) | `INSTANCE_LOCKED` (conflict) | "Finance Desktop is already open". Data not opened (verified on packaged app). |
| Migration error | `MIGRATION_FAILED` | "Your data could not be upgraded. No changes were applied…". Transactional rollback tested. |
| Newer schema | `DATABASE_TOO_NEW` | Refuses to open; data not modified. |
| Dir/DB cannot open | `STORAGE_UNAVAILABLE` | Generic storage message. |

**Shutdown:** on `RunEvent::Exit` the WAL is checkpointed (`TRUNCATE`); verified via Quit, which leaves a 0-byte WAL. **Interruption:** an un-checkpointed WAL is replayed on the next open (tested), and committed transactions survive.

## Logging and error privacy

- `tauri-plugin-log` is registered **only in debug builds**. Release builds install no log sink.
- Log lines carry `AppError::log_summary()` (code + category) only, never messages, paths, SQL, account names, amounts or descriptions. The DB path is no longer logged.
- `From<rusqlite::Error>` never forwards SQLite text, which can quote SQL, columns or values. It maps to `STORAGE_FAILURE` / `STORAGE_UNAVAILABLE` / `NOT_FOUND` with fixed messages. Remaining free text passes through `sanitize_error_message`, which redacts every home-directory segment.
- `get_system_info` no longer returns the data-directory path.
- The TS transport never displays raw `invoke` rejection strings, because Tauri's arg-deserialization errors can echo values. It maps them to `INVALID_REQUEST` / `COMMAND_FAILED`. The browser dev fallback exists only under `import.meta.env.DEV`.

## Not done / follow-ups

- Visual check of the blocked screen in a real WKWebView. The packaged process was verified to stay up, but nobody has looked at the window yet.
- Windows/Linux validation (future targets).
- #16 owns the packaged offline end-to-end test with networking disabled.
- `get_profile` returns the `Asia/Tokyo` default until #5 persists the profile zone.
