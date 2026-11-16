# UI prototype (#3)

Clickable, offline Svelte 5 + TypeScript prototype of the redesigned desktop
finance UI. All data is synthetic. It is a design artifact for review before
#12/#13 — not the app shell (#4), not parity fixtures (#2), and not a command
contract proposal.

Design decisions, navigation patterns, the Tauri webview validation plan and
the Slint fallback evaluation are in
[`docs/ui/prototype-decision.md`](../docs/ui/prototype-decision.md).

## Run

```sh
cd prototype
npm ci
npm run dev        # http://127.0.0.1:5173 (dev server only)
npm test           # vitest + Testing Library (jsdom)
npm run check      # svelte-check / TypeScript
npm run build      # static dist/ with bundled assets only
```

Use the dashed **Prototype** controls in the top bar to switch between sample
data, an empty install, slow storage and a storage error, and between system,
light and dark themes. Entries you add live in memory until reload or
**Reset sample**.

## Layout

```
src/
  App.svelte              shell: sidebar, top bar, shortcuts, toasts
  lib/
    tokens.css, base.css  design tokens (light/dark) and primitives
    money.ts              JPY formatting/parsing; unknown ≠ zero
    fixtures.ts           synthetic accounts, entries, cash-flow months
    commands.ts           in-memory async stand-in for Tauri commands
    app.svelte.ts         routing, demo mode, async resource helper
    derive.ts             presentation-only aggregates for the prototype
    components/           Money, StateTag, Dialog, AsyncView, EmptyState
  screens/                Overview, Accounts (+ history), CashFlow, QuickEntry, ShortcutHelp
```

When #4 lands, `lib/tokens.css`, `lib/base.css`, `lib/money.ts` and
`lib/components/` are intended to move into the app's `src/` and
`commands.ts` is replaced by the typed Tauri command client.
