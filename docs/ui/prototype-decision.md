# UI prototype and interaction system — decision record (#3)

Status: **proposed — awaiting maintainer feedback** before #12/#13 build out
screens. Prototype: [`prototype/`](../../prototype/README.md).

This document records the information architecture, navigation patterns,
visual system and state conventions the new desktop UI will use, what has and
has not been validated, and the Slint fallback evaluation. It does not define
command names, payloads, SQLite schema or financial rules — those belong to
#4/#5 (Antigravity) and #2/#7 (Codex). Where the UI needs something from them,
it is listed under [Needs from the contract](#needs-from-the-command-contract)
as a requirement, not a proposal.

## What the prototype covers

| Issue #3 asks for | Where | Status |
| --- | --- | --- |
| Dashboard | Overview | Done (synthetic) |
| Quick transaction entry | Global **New entry** dialog (⌘/Ctrl+N) | Done (in-memory) |
| Account history | Accounts — list + history with search/type filter | Done (synthetic) |
| One report/chart | Cash flow — chart with equivalent table | Done (synthetic) |
| Keyboard navigation | Shortcuts, roving focus, focus management | Done; tested |
| Accessible text/contrast | Tokens with contrast targets; sr-only money text | Done; needs manual AT pass |
| Empty / error / loading states | Every async view; prototype mode switch | Done; tested |
| Large monetary figures | `Money size="figure"`, tabular numerals, safe-integer formatting | Done; tested to 2^53−1 |
| Actual vs forecast vs incomplete | `StateTag` + `Money state=` + chart patterns | Done; tested |
| Validate in Tauri webviews on target OSes | — | **Not done** — see [Webview validation](#tauri-webview-validation) |
| Slint fallback evaluation | [Below](#slint-fallback-evaluation) | Desk evaluation only |
| No React UI copied | Built from scratch; the React app was consulted only for domain terms | Done |

Screenshots: [overview](screenshots/overview-light.png),
[accounts](screenshots/accounts-light.png),
[cash flow (dark)](screenshots/cashflow-dark.png),
[quick entry validation](screenshots/entry-invalid.png),
[empty install](screenshots/overview-empty.png).

## Information architecture

Primary navigation is a persistent left sidebar. Sections are grouped by what
the user is doing, not by data table:

| Group | Section | Purpose | In prototype |
| --- | --- | --- | --- |
| Track | **Overview** | Where things stand today | ✓ |
| | **Accounts** | Balances and the history behind them (master–detail) | ✓ |
| | **Cash flow** | Money in vs spending over time, actual and forecast | ✓ |
| Plan | Budgets & payday · Cards & statements · Forecast | #10, #9 | listed |
| Review | Reconciliation · Investments · Data quality | #11 | listed |
| Data | Import · Backup & restore | #14, #15 | listed |

Planned sections are shown greyed out and non-interactive so the full IA can
be reviewed now. "Activity" is not a separate section: **Accounts → All
accounts** is the cross-account history, which avoids two screens that list
the same entries with different filters.

### Overview never combines unlike figures

The dashboard shows separate, separately labelled figures:

1. **Cash in accounts** — bank, cash and brokerage-cash balances. If any is
   unknown the figure becomes the *known subtotal*, gets an **Incomplete** tag
   and a dotted underline, and says "1 of 4 accounts unknown". It is never
   presented as a total.
2. **Owed on cards** — positive amount owed, in the debt color, explicitly
   "shown separately from cash". Not subtracted from cash.
3. **This month so far** — money in and spending. Transfers and card
   repayments are excluded and the count of excluded moves is stated.
4. **Looking ahead** — forecast months, every value tagged **Forecast**, with
   "Forecasts are estimates, not money you have."

A standing line reads "These figures are not net worth." True net worth will
only appear once #11 provides it, as its own figure.

## Navigation patterns

| Pattern | Behavior |
| --- | --- |
| Section switch | Sidebar links with `aria-current="page"`; ⌘/Ctrl+1–3. Focus moves to the new page's `<h1>` so keyboard and screen-reader users start at the content that changed. |
| Global create | **New entry** is in the top bar on every screen and on ⌘/Ctrl+N. It is a modal, not a page, so the user returns exactly where they were. |
| Master–detail | Accounts list on the left (↑/↓/Home/End roving focus), history on the right. Selecting an account replaces the detail pane; it does not navigate away. |
| In-page search | `/` focuses the history search. Filtering to nothing shows "No entries match" with **Clear filters**, distinct from the true empty state. |
| Dialogs | Native `<dialog>` in modal mode: platform inertness, Esc to close, focus returned to the opener. First field (or first invalid field) is focused. |
| Charts | One tab stop; ←/→/Home/End move between months; each month has a full spoken description; a detail strip mirrors the focused month. A **Chart / Table** toggle gives an equivalent table. |
| Help | `?` opens the shortcut list. |
| Skip link | "Skip to content" is the first tab stop. |

**For #4 (Antigravity):** the native app menu should expose the same
accelerators (New entry, sections) and dispatch to the same actions, so ⌘N in
the menu bar and in the webview do not diverge or double-fire.

## Visual system

Tokens live in `prototype/src/lib/tokens.css` (light and dark, following the
OS with a manual override) and primitives in `base.css`. No web fonts: system
UI stacks with Japanese fallbacks (Hiragino Sans, Noto Sans JP, Yu Gothic UI),
so nothing is fetched at runtime. Motion is minimal and disabled under
`prefers-reduced-motion`.

Contrast targets: body text ≥ 7:1, secondary text and state labels ≥ 4.5:1,
focus rings and control borders ≥ 3:1. These are design targets checked by eye
in screenshots; an automated contrast audit is still to do (see gaps).

### Value-state language

Color is never the only signal. Every state has a text label, and charts add
a pattern.

| State | Text | Number style | Chart |
| --- | --- | --- | --- |
| Actual | "Actual" tag where ambiguity is possible | Ink | Solid fill |
| Forecast | "Forecast" tag; "Forecast →" zone | Violet | Hatched fill, dashed outline, shaded zone |
| Incomplete | "Incomplete" tag plus a reason ("No opening balance recorded") | Dotted underline | Dotted outline around the month |
| Unknown amount | The word "Unknown" (never ¥0, never "—") | Amber, bold | Not plotted |
| Card debt | "owed" suffix | Debt red, positive number | — |
| Transfer / repayment | "moved"; type shown muted | Muted, unsigned across all accounts | Excluded from in/spending |
| Purchase date unknown | "Purchase date unknown" tag plus "Paid <date>" | — | — |

### Money display rules (`Money.svelte`, `money.ts`)

- Amounts are whole-yen safe integers; anything else throws rather than rounds.
- A real minus sign (U+2212), grouped with `ja-JP`, and tabular numerals so
  columns align.
- Screen readers get words ("minus 6,240 yen, forecast"), not glyphs.
- `null` renders as a word. `sumKnown()` returns `total: null` whenever any
  input is unknown, and exposes the known subtotal and unknown count
  separately, so the UI cannot accidentally show a partial sum as a total.
- Input accepts `1,200`, `¥1200` and full-width digits; it rejects decimals,
  signs, zero and out-of-range values with specific messages.

## State conventions

| Situation | Pattern |
| --- | --- |
| First load | Skeleton rows with a polite "Loading …" status. |
| Refresh | Previous data stays visible, dimmed, with `aria-busy`. No layout jump. |
| Empty | Explains what will appear and why, plus the next action. An unknown opening balance is offered as a valid choice ("can stay unknown"). |
| Read failure | `role="alert"` with a safe message, "Nothing has been changed.", and **Try again**. No paths or record contents. |
| Write failure | Form stays open with every field intact; "Nothing was saved."; the button becomes **Try again**. |
| Double submit | Submit is disabled while pending, and the write carries an idempotency key that is generated per form session and reused on retry. The mock replays the original result for a repeated key. Tested. |
| Field-level server error | Mapped to the field (`aria-invalid` + message), e.g. a future date. |

**Design finding to carry into #12:** in "Storage error" mode every panel
shows its own error ([screenshot](screenshots/overview-error.png)). That is
noisy. A storage-level failure (the database can't be opened) should produce
**one app-level banner** with recovery guidance; per-panel errors should be
reserved for failures specific to that panel. This needs the error shape to
distinguish the two (see below).

## Needs from the command contract

These are UI requirements to bring to the Wave 1 contract discussion, not
proposed names or types. Codex (domain) and Antigravity (transport) own the
actual shapes.

1. **Unknown money** is `null` (or an explicit variant), never `0`. Balances
   carry a status and a human-readable reason when incomplete.
2. **Card accounts:** confirm whether the UI receives the signed balance
   (debt negative) and derives "owed", or receives `amount_owed` as the
   legacy API did. The UI should not derive financial results itself.
3. **Dates:** purchase/occurrence date may be `null`, with a date-basis value;
   effective date is always present. The app needs the user's local "today"
   and time zone from the backend, not the webview clock.
4. **Writes** accept a client-generated idempotency key and return the
   original result on replay, with a flag saying it was a replay.
5. **Errors** carry a stable code, a message safe to display, an optional
   field, and a way to tell storage-level failures from request-level ones.
6. **Postings per account**, so history can show each account's side of a
   transfer or repayment.
7. **Cash-flow/report results** carry a per-period state (actual / forecast /
   incomplete) and a reason, computed in Rust. The prototype's
   `derive.ts` averages exist only for the mock and should not be ported.

## Tauri webview validation

**Not yet done.** This machine has no Rust toolchain and the #4 shell does
not exist yet, so the prototype has only been run in headless Chromium, which
is close to WebView2 on Windows but says nothing about WKWebView or
WebKitGTK. The built bundle made **zero non-local requests** (verified by
request logging) and contains no `fetch`/XHR/WebSocket calls.

Once #4 can launch a window, run this checklist on each supported OS
(WKWebView on macOS, WebView2 on Windows, WebKitGTK on Linux) and record the
results here:

| Check | Risk / note |
| --- | --- |
| `<dialog>.showModal()`, `::backdrop`, Esc, focus return | Needs WebKitGTK ≥ 2.38 / Safari 15.4 |
| `:has()` selector (segmented control, amount field focus) | Needs WebKitGTK ≥ 2.42; cheap to replace if missing |
| `<input type="date">` | **Highest risk:** WebKitGTK has historically had no native picker. May need a custom date field. |
| ⌘/Ctrl+N and ⌘/Ctrl+1–3 vs native menu accelerators | Must not double-fire (see the note for #4) |
| Japanese text rendering and font fallback; IME input in amount and description fields | Test with the Kotoeri, MS-IME and Fcitx/IBus input methods |
| `Intl.NumberFormat('ja-JP')` grouping, `Intl.DateTimeFormat` | Should match Chromium |
| Focus-visible rings, tab order, VoiceOver / Narrator / Orca reading of `Money` and charts | Manual AT pass |
| `prefers-color-scheme` live switching; `prefers-reduced-motion` | — |
| SVG chart crispness at 1×/2× and with system text scaling at 150% | — |
| Window at minimum size (960 px wide) | Body has `min-width: 960px` |
| Strict CSP from #4 | Component CSS is bundled to one external file; a few inline `style=` attributes (bar and column widths) need `style-src-attr 'unsafe-inline'` or conversion to classes. Coordinate with #4's CSP. |

## Slint fallback evaluation

A desk evaluation, because no webview failure has been observed yet.

| Criterion | Tauri + Svelte (current) | Slint |
| --- | --- | --- |
| Rendering consistency across OSes | Three different engines; WebKitGTK is the weak one | One renderer everywhere |
| Accessibility | Mature web semantics on all three platform AT bridges | AccessKit-based; improving but less proven for complex widgets such as tables and charts |
| Japanese text, IME | Native through the webview | Supported; needs explicit testing |
| Charts and data tables | SVG and HTML tables; equivalent table is trivial | Custom drawing; tables need hand-built focus and AT support |
| Frontend tests | Testing Library and jsdom, fast, in CI now | Rust-side UI tests; a less mature tooling story |
| Team ownership boundary | Matches AGENTS.md (Svelte `src/`, Rust `src-tauri/`) | Collapses UI into Rust; would change the ownership split |
| Memory / bundle | System webview, ~100 kB JS | Smaller footprint, no webview |
| Cost to switch later | — | Full UI rewrite; tokens and copy carry over, components don't |

**Recommendation:** keep Tauri + Svelte. Consider Slint only if a checklist
item fails on a supported OS *and* cannot be fixed within the webview (custom
control, polyfill, or dropping that OS). The `<input type="date">` and `:has()`
risks both have cheap in-webview fixes, so neither justifies a switch on its
own.

## Known gaps

- Webview validation on real Tauri windows (above). Blocked on #4 and a Rust
  toolchain.
- No automated axe/contrast audit yet. Add one in #12 when the real `src/`
  exists. A manual screen-reader pass is still owed.
- Window resizing below 960 px is out of scope. Desktop sizes only.
- Corrections, reversal, archive and card repayment flows are intentionally
  absent. They depend on #7/#8/#9 semantics.
- `derive.ts` aggregates are prototype-only and are not parity logic.

## Feedback requested

1. Is the Track / Plan / Review / Data grouping right, and is folding
   "Activity" into Accounts → All accounts acceptable?
2. Should Overview keep cash and card debt as separate headline figures, with
   no combined number until #11 provides net worth?
3. Is the value-state language (tags, dotted underline, hatching, "Unknown")
   clear enough, or should forecasts move to a separate screen entirely?
4. Is a global modal the right shape for quick entry, or would you prefer an
   inline entry row at the top of account history?
5. Which OS is the first supported target? That decides where webview
   validation runs first.
