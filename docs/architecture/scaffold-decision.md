# Desktop Scaffold and Command Boundary Architecture (#4)

Status: **Proposed — ready for review** (Wave 1, Antigravity lane).
Issue: [#4 Scaffold Tauri 2, Svelte, Rust, and offline build workflow](https://github.com/D3V-K/finance-mgmt-desktop/issues/4).

---

## 1. Repository Layout

The desktop application separates the Svelte 5 renderer, Tauri shell adapters, and SQLite storage:

```
├── .github/workflows/ci.yml       # CI: formatting, typecheck, tests, and build smoke check
├── docs/architecture/             # Architectural decision records and contract specifications
├── index.html                     # Offline entrypoint (bundled local assets only)
├── package.json                   # Svelte 5, Vite, Vitest, TypeScript, Tauri 2 CLI
├── src/                           # Svelte renderer
│   ├── App.svelte                 # Top-level shell with system/storage status and error banner
│   ├── App.test.ts                # App shell component tests
│   ├── lib/
│   │   ├── commands.ts            # Typed Tauri command invoker and testing mocks
│   │   └── types.ts               # Command contracts, error shapes, and system info
│   ├── main.ts                    # Svelte 5 entrypoint (mount)
│   └── test-setup.ts              # Vitest setup (jsdom modal dialog mocks)
├── src-tauri/                     # Tauri 2 & Rust backend
│   ├── Cargo.toml                 # Tauri 2, rusqlite (bundled), chrono, serde, thiserror
│   ├── capabilities/default.json  # Scoped minimal Tauri capabilities
│   ├── src/
│   │   ├── commands/              # Tauri command handlers
│   │   │   ├── mod.rs
│   │   │   └── system.rs          # get_system_info, get_storage_status
│   │   ├── db/
│   │   │   └── mod.rs             # SQLite connection pool, WAL mode, migrations tracking
│   │   ├── error.rs               # Actionable errors, categorization, and path redaction
│   │   ├── lib.rs                 # Tauri application setup, lifecycle, command registration
│   │   └── main.rs                # Desktop executable entrypoint
│   └── tauri.conf.json            # Security CSP, window constraints, bundle configuration
├── svelte.config.js               # Svelte compiler configuration
├── tsconfig.json                  # Strict TypeScript configuration
└── vite.config.ts                 # Offline Vite build, strict port, Vitest environment
```

---

## 2. Storage and App Data Boundary

### Storage Location
Application data lives outside the installation directory:
- **macOS:** `~/Library/Application Support/com.d3vk.financedesktop/`
- **Linux:** `~/.local/share/com.d3vk.financedesktop/`
- **Windows:** `%APPDATA%\com.d3vk.financedesktop\`

Database file: `<app_data_dir>/finance.db`.

### SQLite Configuration
- **Journal Mode:** Write-Ahead Logging (`PRAGMA journal_mode = WAL;`) for concurrent read/write throughput.
- **Synchronous:** `PRAGMA synchronous = NORMAL;` safe in WAL mode.
- **Foreign Keys:** Enforced (`PRAGMA foreign_keys = ON;`).
- **Busy Timeout:** 5,000 ms to handle concurrent transactions without locking errors.
- **Migration Tracking:** `_schema_migrations (version INTEGER PRIMARY KEY, name TEXT, applied_at TEXT)`.
- *Note:* Domain tables (accounts, transactions, categories, budgets) will be added in Wave 2 (#5) following Codex's parity review (#2/#7).

---

## 3. Typed Command and Error Contract

### Error Architecture
Errors are serialized as structured JSON matching `AppError`:

```typescript
export type ErrorCategory = 'storage' | 'validation' | 'not_found' | 'conflict' | 'internal';

export interface AppError {
  code: string;
  message: string;
  category: ErrorCategory;
  field?: string;
}
```

- **Redaction of sensitive data:** The Rust layer (`error::sanitize_error_message`) sanitizes filesystem paths (`/Users/...`, `C:\Users\...`) and user credentials into safe placeholders (`[USER_HOME]`) so debug messages never leak PII or local paths.
- **Storage-level vs request-level:** Storage failures carry `category: 'storage'` triggering an app-level system banner rather than fragmented per-component errors (aligning with UI decision in PR #17).

### Shared Conventions
1. **Money:** Integer whole-yen (JPY). Unknown money is `null`, never `0` or partial sums.
2. **Dates:** Backend supplies the local "today" (`YYYY-MM-DD`) and timezone to keep frontend calculations deterministic across system time differences.
3. **Idempotency:** Modifying commands accept `idempotency_key` and return `is_replay: boolean`.

---

## 4. Security and Isolation

- **Zero Remote Dependencies:** Built bundle contains zero `fetch()`, `XMLHttpRequest`, or `WebSocket` connections to external hosts.
- **Strict Content Security Policy (CSP):**
  ```
  default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; object-src 'none'; frame-src 'none'; connect-src 'none';
  ```
- **Scoped Permissions:** `src-tauri/capabilities/default.json` grants only minimal `core:default` permissions.

---

## 5. Build and Developer Workflow

### Prerequisites
- Node.js >= 22 (tested with Node 24)
- Rust toolchain >= 1.90 (2024 edition)

### Commands
```bash
# Frontend typecheck & tests
npm run check
npm test

# Frontend production bundle
npm run build

# Rust tests & clippy checks
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check

# Compile desktop application binary
cargo build --manifest-path src-tauri/Cargo.toml
```
