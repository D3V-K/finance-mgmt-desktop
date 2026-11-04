<script lang="ts">
  import { tick } from 'svelte';
  import { app, NAV, PLANNED, type Route } from './lib/app.svelte';
  import type { DemoMode } from './lib/commands';
  import Overview from './screens/Overview.svelte';
  import Accounts from './screens/Accounts.svelte';
  import CashFlow from './screens/CashFlow.svelte';
  import QuickEntry from './screens/QuickEntry.svelte';
  import ShortcutHelp from './screens/ShortcutHelp.svelte';

  const isMac = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform);
  const mod = isMac ? '⌘' : 'Ctrl';
  let newEntryButton: HTMLButtonElement | undefined = $state();

  async function navigate(name: Route['name']) {
    app.go(name === 'accounts' ? { name, accountId: null } : { name });
    // Move focus to the new page heading so keyboard and screen-reader users
    // land at the start of the content that changed.
    await tick();
    document.querySelector<HTMLElement>('#main h1')?.focus();
  }

  function isTyping(target: EventTarget | null) {
    return target instanceof HTMLElement && (target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName));
  }

  function onKeydown(e: KeyboardEvent) {
    if (app.entryOpen || app.helpOpen) return;
    const modKey = isMac ? e.metaKey : e.ctrlKey;
    if (modKey && e.key.toLowerCase() === 'n') {
      e.preventDefault();
      app.entryOpen = true;
      return;
    }
    const nav = modKey ? NAV.find((n) => n.shortcut === e.key) : undefined;
    if (nav) {
      e.preventDefault();
      navigate(nav.name);
      return;
    }
    if (e.key === '?' && !isTyping(e.target)) {
      e.preventDefault();
      app.helpOpen = true;
    }
  }

  const title = $derived(NAV.find((n) => n.name === app.route.name)?.label ?? '');
  const modes: Array<{ value: DemoMode; label: string }> = [
    { value: 'ready', label: 'Sample data' },
    { value: 'empty', label: 'Empty (new install)' },
    { value: 'slow', label: 'Slow storage' },
    { value: 'error', label: 'Storage error' }
  ];
</script>

<svelte:window onkeydown={onKeydown} />
<svelte:head><title>{title} · Ledger</title></svelte:head>

<a class="skip" href="#main">Skip to content</a>

<div class="shell">
  <aside class="sidebar">
    <div class="brand"><span class="mark" aria-hidden="true"></span>Ledger</div>

    <nav aria-label="Sections">
      <ul>
        {#each NAV as item (item.name)}
          <li>
            <a
              href="#{item.name}"
              aria-current={app.route.name === item.name ? 'page' : undefined}
              onclick={(e) => { e.preventDefault(); navigate(item.name); }}
            >
              <span>{item.label}</span>
              <kbd aria-hidden="true">{mod}{item.shortcut}</kbd>
            </a>
          </li>
        {/each}
      </ul>
    </nav>

    <div class="planned">
      {#each PLANNED as group (group.group)}
        <p class="eyebrow">{group.group}</p>
        <ul aria-label="{group.group} (planned)">
          {#each group.items as item (item)}<li>{item}<span class="sr-only"> — planned, not in prototype</span></li>{/each}
        </ul>
      {/each}
    </div>

    <div class="offline" role="note">
      <span class="dot" aria-hidden="true"></span>
      <span>Offline · this device only<br /><span class="muted">No account or network used</span></span>
    </div>
  </aside>

  <div class="main-col">
    <header class="topbar">
      <div class="proto" role="group" aria-label="Prototype controls">
        <span class="eyebrow">Prototype</span>
        <label>
          <span class="sr-only">Data state</span>
          <select value={app.demoMode} onchange={(e) => app.setMode(e.currentTarget.value as DemoMode)}>
            {#each modes as m (m.value)}<option value={m.value}>{m.label}</option>{/each}
          </select>
        </label>
        <label>
          <span class="sr-only">Theme</span>
          <select value={app.theme} onchange={(e) => app.setTheme(e.currentTarget.value as typeof app.theme)}>
            <option value="system">System theme</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
          </select>
        </label>
        <button class="btn ghost small" onclick={() => app.resetData()}>Reset sample</button>
      </div>
      <div class="actions">
        <button class="btn ghost small" onclick={() => (app.helpOpen = true)} aria-keyshortcuts="?">Shortcuts <kbd>?</kbd></button>
        <button class="btn primary" bind:this={newEntryButton} onclick={() => (app.entryOpen = true)} aria-keyshortcuts={isMac ? 'Meta+N' : 'Control+N'}>
          New entry <kbd class="on-accent">{mod}N</kbd>
        </button>
      </div>
    </header>

    <main id="main" tabindex="-1">
      {#if app.route.name === 'overview'}
        <Overview onnavigate={(r) => app.go(r)} />
      {:else if app.route.name === 'accounts'}
        <Accounts selectedId={app.route.accountId} onselect={(accountId) => app.go({ name: 'accounts', accountId })} />
      {:else}
        <CashFlow />
      {/if}
    </main>

    <footer class="statusbar">
      <span>Synthetic sample data — not connected to a ledger</span>
      <span>Amounts in whole JPY · dates are local calendar days</span>
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

<style>
  .skip { position: absolute; left: -999px; top: 8px; z-index: 10; background: var(--accent); color: var(--accent-ink); padding: 8px 12px; border-radius: var(--radius-md); }
  .skip:focus { left: 8px; }
  .shell { display: grid; grid-template-columns: var(--sidebar-width) 1fr; height: 100vh; }

  .sidebar { background: var(--surface-sunken); border-right: 1px solid var(--border); padding: var(--space-4) var(--space-3); display: flex; flex-direction: column; gap: var(--space-5); overflow-y: auto; }
  .brand { display: flex; align-items: center; gap: var(--space-2); font-weight: 680; font-size: var(--text-lg); padding: 0 var(--space-2); }
  .mark { width: 18px; height: 18px; border-radius: 5px; background: conic-gradient(var(--accent) 0 75%, var(--chart-in) 0); }
  nav ul, .planned ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 2px; }
  nav a { display: flex; justify-content: space-between; align-items: center; padding: 7px var(--space-2); border-radius: var(--radius-md); color: var(--ink-2); text-decoration: none; font-weight: 560; }
  nav a:hover { background: var(--surface); color: var(--ink); }
  nav a[aria-current='page'] { background: var(--surface); color: var(--ink); box-shadow: inset 3px 0 0 var(--accent), var(--shadow); }
  nav kbd { opacity: 0.7; }
  .planned { display: grid; gap: var(--space-1); padding: 0 var(--space-2); }
  .planned .eyebrow { margin-top: var(--space-2); }
  .planned li { color: var(--ink-3); font-size: var(--text-sm); padding: 3px 0; }
  .offline { margin-top: auto; display: flex; gap: var(--space-2); font-size: var(--text-xs); padding: var(--space-2); color: var(--ink-2); }
  .dot { width: 8px; height: 8px; margin-top: 4px; border-radius: 50%; background: var(--state-ok); flex: none; }

  .main-col { display: grid; grid-template-rows: auto 1fr auto; min-width: 0; min-height: 0; }
  .topbar { display: flex; justify-content: space-between; align-items: center; gap: var(--space-4); padding: var(--space-2) var(--space-5); border-bottom: 1px solid var(--border); background: var(--surface); }
  .proto { display: flex; align-items: center; gap: var(--space-2); padding: 2px var(--space-2); border: 1px dashed var(--border-strong); border-radius: var(--radius-md); }
  .proto select { min-height: 28px; font-size: var(--text-sm); border: 1px solid var(--border); border-radius: var(--radius-sm); background: var(--surface); padding: 0 6px; }
  .actions { display: flex; gap: var(--space-2); align-items: center; }
  kbd.on-accent { background: transparent; color: inherit; border-color: currentColor; opacity: 0.8; }

  main { overflow-y: auto; padding: var(--space-5) var(--space-6) var(--space-7); outline: none; }
  .statusbar { display: flex; justify-content: space-between; padding: 4px var(--space-5); font-size: var(--text-xs); color: var(--ink-3); border-top: 1px solid var(--border); background: var(--surface-sunken); }

  .toasts { position: fixed; right: var(--space-5); bottom: var(--space-7); display: grid; gap: var(--space-2); z-index: 20; }
  .toast { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-2) var(--space-2) var(--space-2) var(--space-4); border-radius: var(--radius-md); background: var(--ink); color: var(--bg); box-shadow: var(--shadow); font-weight: 560; }
  .toast .btn { color: inherit; }
  .toast.error { background: var(--state-error); color: #fff; }
</style>
