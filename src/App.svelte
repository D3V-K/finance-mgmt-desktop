<script lang="ts">
  import { onMount, tick } from 'svelte';
  import './lib/tokens.css';
  import './lib/base.css';
  import { app, NAV, NAV_GROUPS, PLANNED, routeFor, type RouteName } from './lib/app.svelte';
  import { getAppStatus, getProfile, getStorageStatus, toAppError } from './lib/commands';
  import type { AppError, StorageStatus } from './lib/types';
  import Overview from './screens/Overview.svelte';
  import Accounts from './screens/Accounts.svelte';
  import CashFlow from './screens/CashFlow.svelte';
  import Categories from './screens/Categories.svelte';
  import Budgets from './screens/Budgets.svelte';
  import Cards from './screens/Cards.svelte';
  import Forecast from './screens/Forecast.svelte';
  import Reconciliation from './screens/Reconciliation.svelte';
  import Investments from './screens/Investments.svelte';
  import Quality from './screens/Quality.svelte';
  import LedgerStart from './screens/LedgerStart.svelte';
  import QuickEntry from './screens/QuickEntry.svelte';
  import ShortcutHelp from './screens/ShortcutHelp.svelte';

  type Phase = 'starting' | 'blocked' | 'ready';

  /** Startup failures are terminal for this launch: copy explains the safe next step. */
  const BLOCKED_COPY: Record<string, { title: string; body: string }> = {
    INSTANCE_LOCKED: {
      title: 'Finance Desktop is already open',
      body: 'Only one copy can use your data at a time. Switch to the open window, or quit it and open the app again.',
    },
    MIGRATION_FAILED: {
      title: 'Your data could not be upgraded',
      body: 'No changes were applied to your data. Quit and reopen the app. If this keeps happening, restore from a backup.',
    },
    DATABASE_TOO_NEW: {
      title: 'This data needs a newer version',
      body: 'Your data was saved by a newer version of Finance Desktop. Install that version to open it; this version will not change it.',
    },
  };
  const DEFAULT_BLOCKED = {
    title: 'Local storage is unavailable',
    body: 'Finance Desktop could not open its data on this computer. Quit and reopen the app. Nothing was sent anywhere.',
  };

  let phase = $state<Phase>('starting');
  let startupError = $state<AppError | null>(null);
  let profileError = $state<AppError | null>(null);
  let storage = $state<StorageStatus | null>(null);

  const blockedCopy = $derived(startupError ? (BLOCKED_COPY[startupError.code] ?? DEFAULT_BLOCKED) : DEFAULT_BLOCKED);
  const isMac = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform);
  const mod = isMac ? '⌘' : 'Ctrl';
  const title = $derived(NAV.find((n) => n.name === app.route.name)?.label ?? '');

  async function start() {
    try {
      const status = await getAppStatus();
      if (status.state !== 'ready') {
        startupError = status.error;
        phase = 'blocked';
        return;
      }
    } catch (err) {
      startupError = toAppError(err);
      phase = 'blocked';
      return;
    }
    await loadProfile();
  }

  async function loadProfile() {
    profileError = null;
    try {
      const profile = await getProfile();
      app.today = profile.today;
      app.timeZone = profile.timeZone;
      app.storageIssue = null;
      phase = 'ready';
      storage = await getStorageStatus().catch(() => null);
    } catch (err) {
      profileError = toAppError(err);
      app.today = '';
      phase = 'ready';
    }
  }

  // Reference data (accounts, categories) refreshes after every write.
  $effect(() => {
    void app.revision;
    if (phase === 'ready' && app.today) void app.loadReferences();
  });

  async function navigate(name: RouteName) {
    app.go(routeFor(name));
    await tick();
    document.querySelector<HTMLElement>('#main h1')?.focus();
  }

  function isTyping(target: EventTarget | null) {
    return target instanceof HTMLElement && (target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName));
  }

  function onKeydown(e: KeyboardEvent) {
    if (phase !== 'ready' || !app.today || e.isComposing) return;
    if (app.entryOpen || app.helpOpen || document.querySelector('dialog[open]')) return;
    const modKey = isMac ? e.metaKey : e.ctrlKey;
    if (modKey && e.key.toLowerCase() === 'n') {
      e.preventDefault();
      app.entryOpen = true;
      return;
    }
    const nav = modKey ? NAV.find((n) => n.shortcut === e.key) : undefined;
    if (nav) {
      e.preventDefault();
      void navigate(nav.name);
      return;
    }
    if (e.key === '?' && !isTyping(e.target)) {
      e.preventDefault();
      app.helpOpen = true;
    }
  }

  onMount(() => {
    void start();
  });
</script>

<svelte:window onkeydown={onKeydown} />
<svelte:head><title>{title ? `${title} · ` : ''}Finance Desktop</title></svelte:head>

{#if phase === 'starting'}
  <div class="center" role="status" data-testid="loading-state"><p>Opening your local data …</p></div>
{:else if phase === 'blocked'}
  <div class="center">
    <section class="blocked" role="alert" aria-labelledby="blocked-title" data-testid="startup-blocked">
      <h1 class="app-title">Finance Desktop</h1>
      <h2 id="blocked-title">{blockedCopy.title}</h2>
      <p>{blockedCopy.body}</p>
      {#if startupError}<p class="muted small">Reference: {startupError.code}</p>{/if}
    </section>
  </div>
{:else if !app.today}
  <div class="center">
    <section class="blocked" role="alert" data-testid="error-banner">
      <h1 class="app-title">Finance Desktop</h1>
      <h2>Your profile could not be read</h2>
      <p>{profileError?.message ?? 'The local profile is unavailable.'} Nothing has been changed.</p>
      {#if profileError}<p class="muted small">Reference: {profileError.code}</p>{/if}
      <div><button class="btn primary" onclick={loadProfile}>Try again</button></div>
    </section>
  </div>
{:else}
  <a class="skip" href="#main">Skip to content</a>
  <div class="shell">
    <aside class="sidebar">
      <div class="brand"><span class="mark" aria-hidden="true"></span><span class="app-title">Finance Desktop</span></div>
      <nav aria-label="Sections">
        {#each NAV_GROUPS as group (group.group)}
          <p class="eyebrow" id="nav-{group.group}">{group.group}</p>
          <ul aria-labelledby="nav-{group.group}">
            {#each group.items as item (item.name)}
              <li>
                <a
                  href="#{item.name}"
                  aria-current={app.route.name === item.name ? 'page' : undefined}
                  aria-keyshortcuts={item.shortcut ? `${isMac ? 'Meta' : 'Control'}+${item.shortcut}` : undefined}
                  onclick={(e) => {
                    e.preventDefault();
                    void navigate(item.name);
                  }}
                >
                  <span>{item.label}</span>
                  {#if item.shortcut}<kbd aria-hidden="true">{mod}{item.shortcut}</kbd>{/if}
                </a>
              </li>
            {/each}
          </ul>
        {/each}
        <ul class="planned" aria-label="Planned">
          {#each PLANNED as item (item)}<li>{item}<span class="muted small"> · coming later</span></li>{/each}
        </ul>
      </nav>
      <div class="offline" role="note">
        <span class="dot" aria-hidden="true"></span>
        <span>Offline · this device only<br /><span class="muted">No account or network used</span></span>
      </div>
    </aside>

    <div class="main-col">
      <header class="topbar">
        <label class="theme">
          <span class="sr-only">Theme</span>
          <select value={app.theme} onchange={(e) => app.setTheme(e.currentTarget.value as typeof app.theme)}>
            <option value="system">System theme</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
          </select>
        </label>
        <div class="actions">
          <button class="btn ghost small" onclick={() => (app.helpOpen = true)} aria-keyshortcuts="?">Shortcuts <kbd>?</kbd></button>
          <button class="btn primary" onclick={() => (app.entryOpen = true)} aria-keyshortcuts={isMac ? 'Meta+N' : 'Control+N'}>
            New entry <kbd class="on-accent">{mod}N</kbd>
          </button>
        </div>
      </header>

      {#if app.storageIssue}
        <div class="storage-banner" role="alert" data-testid="storage-banner">
          <div>
            <strong>Local storage is not responding.</strong>
            {app.storageIssue.message} Nothing was changed.
            <span class="muted small">Reference: {app.storageIssue.code}</span>
          </div>
          <button
            class="btn small"
            onclick={() => {
              app.storageIssue = null;
              app.changed();
            }}>Try again</button
          >
        </div>
      {/if}

      <main id="main" tabindex="-1">
        {#if app.route.name === 'overview'}
          <Overview />
        {:else if app.route.name === 'accounts'}
          <Accounts selectedId={app.route.accountId} />
        {:else if app.route.name === 'cashflow'}
          <CashFlow />
        {:else if app.route.name === 'categories'}
          <Categories />
        {:else if app.route.name === 'budgets'}
          <Budgets />
        {:else if app.route.name === 'cards'}
          <Cards cardId={app.route.cardId} />
        {:else if app.route.name === 'forecast'}
          <Forecast />
        {:else if app.route.name === 'reconciliation'}
          <Reconciliation reconciliationId={app.route.reconciliationId} />
        {:else if app.route.name === 'investments'}
          <Investments />
        {:else if app.route.name === 'quality'}
          <Quality />
        {:else}
          <LedgerStart />
        {/if}
      </main>

      <footer class="statusbar" data-testid="system-meta">
        <span>Today {app.today} ({app.timeZone}) · amounts in whole JPY</span>
        {#if storage}<span>SQLite · schema v{storage.migrationVersion}</span>{/if}
      </footer>
    </div>
  </div>

  <QuickEntry open={app.entryOpen} onclose={() => (app.entryOpen = false)} />
  <ShortcutHelp open={app.helpOpen} {mod} onclose={() => (app.helpOpen = false)} />

  <div class="toasts" role="status" aria-live="polite">
    {#each app.toasts as t (t.id)}
      <div class="toast {t.tone}">
        <span>{t.message}</span>
        <button class="btn ghost small" aria-label="Dismiss notification" onclick={() => app.dismiss(t.id)}>✕</button>
      </div>
    {/each}
  </div>
{/if}

<style>
  .center { min-height: 100vh; display: grid; place-items: center; padding: var(--space-6); }
  .blocked { max-width: 520px; display: grid; gap: var(--space-3); padding: var(--space-6); background: var(--surface); border: 1px solid var(--border); border-left: 4px solid var(--state-error); border-radius: var(--radius-lg); }
  .small { font-size: var(--text-xs); }
  .skip { position: absolute; left: -999px; top: 8px; z-index: 10; background: var(--accent); color: var(--accent-ink); padding: 8px 12px; border-radius: var(--radius-md); }
  .skip:focus { left: 8px; }
  .shell { display: grid; grid-template-columns: var(--sidebar-width) 1fr; height: 100vh; }
  .sidebar { background: var(--surface-sunken); border-right: 1px solid var(--border); padding: var(--space-4) var(--space-3); display: flex; flex-direction: column; gap: var(--space-4); overflow-y: auto; }
  .brand { display: flex; align-items: center; gap: var(--space-2); padding: 0 var(--space-2); }
  .app-title { font-size: var(--text-lg); font-weight: 680; }
  .mark { width: 18px; height: 18px; border-radius: 5px; background: conic-gradient(var(--accent) 0 75%, var(--chart-in) 0); flex: none; }
  nav { display: grid; gap: var(--space-1); }
  nav .eyebrow { padding: var(--space-2) var(--space-2) 2px; }
  nav ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 2px; }
  nav a { display: flex; justify-content: space-between; align-items: center; padding: 6px var(--space-2); border-radius: var(--radius-md); color: var(--ink-2); text-decoration: none; font-weight: 560; }
  nav a:hover { background: var(--surface); color: var(--ink); }
  nav a[aria-current='page'] { background: var(--surface); color: var(--ink); box-shadow: inset 3px 0 0 var(--accent), var(--shadow); }
  nav kbd { opacity: 0.7; }
  nav ul.planned { padding: var(--space-2) var(--space-2) 0; }
  .planned li { color: var(--ink-3); font-size: var(--text-sm); }
  .offline { margin-top: auto; display: flex; gap: var(--space-2); font-size: var(--text-xs); padding: var(--space-2); color: var(--ink-2); }
  .dot { width: 8px; height: 8px; margin-top: 4px; border-radius: 50%; background: var(--state-ok); flex: none; }
  .main-col { display: grid; grid-template-rows: auto auto 1fr auto; min-width: 0; min-height: 0; }
  .topbar { display: flex; justify-content: space-between; align-items: center; gap: var(--space-4); padding: var(--space-2) var(--space-5); border-bottom: 1px solid var(--border); background: var(--surface); }
  .theme select { min-height: 28px; font-size: var(--text-sm); border: 1px solid var(--border); border-radius: var(--radius-sm); background: var(--surface); padding: 0 6px; }
  .actions { display: flex; gap: var(--space-2); align-items: center; }
  kbd.on-accent { background: transparent; color: inherit; border-color: currentColor; opacity: 0.8; }
  .storage-banner { display: flex; justify-content: space-between; align-items: center; gap: var(--space-4); padding: var(--space-3) var(--space-5); background: var(--state-error-bg); border-bottom: 1px solid var(--state-error); }
  main { overflow-y: auto; padding: var(--space-5) var(--space-6) var(--space-7); outline: none; }
  .statusbar { display: flex; justify-content: space-between; padding: 4px var(--space-5); font-size: var(--text-xs); color: var(--ink-3); border-top: 1px solid var(--border); background: var(--surface-sunken); }
  .toasts { position: fixed; right: var(--space-5); bottom: var(--space-7); display: grid; gap: var(--space-2); z-index: 20; }
  .toast { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-2) var(--space-2) var(--space-2) var(--space-4); border-radius: var(--radius-md); background: var(--ink); color: var(--bg); box-shadow: var(--shadow); font-weight: 560; }
  .toast .btn { color: inherit; }
  .toast.error { background: var(--state-error); color: #fff; }
</style>
