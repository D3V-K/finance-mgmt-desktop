<script lang="ts">
  import { app, resource } from '../lib/app.svelte';
  import { commands } from '../lib/commands';
  import { EVENT_LABEL, KIND_LABEL, type EventKind } from '../lib/fixtures';
  import { formatDate } from '../lib/money';
  import AsyncView from '../lib/components/AsyncView.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';
  import Money from '../lib/components/Money.svelte';
  import StateTag from '../lib/components/StateTag.svelte';
  import EntryList from './EntryList.svelte';

  let { selectedId, onselect }: { selectedId: string | null; onselect: (id: string | null) => void } = $props();

  const accounts = resource(() => commands.listAccounts());
  const entries = resource(() => commands.listEntries({ accountId: selectedId ?? undefined }));

  let kind = $state<EventKind | 'all'>('all');
  let query = $state('');
  let search: HTMLInputElement | undefined = $state();
  let listEl: HTMLUListElement | undefined = $state();

  // Roving focus through the account list with arrow keys / Home / End.
  function onListKeydown(e: KeyboardEvent) {
    const items = [...(listEl?.querySelectorAll<HTMLButtonElement>('button') ?? [])];
    const i = items.indexOf(document.activeElement as HTMLButtonElement);
    const next = e.key === 'ArrowDown' ? i + 1 : e.key === 'ArrowUp' ? i - 1 : e.key === 'Home' ? 0 : e.key === 'End' ? items.length - 1 : null;
    if (next === null || i === -1) return;
    e.preventDefault();
    items[Math.max(0, Math.min(items.length - 1, next))]?.focus();
  }

  function onWindowKeydown(e: KeyboardEvent) {
    if (e.key === '/' && !(e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement) && !app.entryOpen) {
      e.preventDefault();
      search?.focus();
    }
  }

  const acct = $derived(accounts.current.data?.find((a) => a.id === selectedId));

  const visible = $derived.by(() => {
    const list = entries.current.data ?? [];
    const q = query.trim().toLowerCase();
    return list.filter(
      (e) => (kind === 'all' || e.kind === kind) && (!q || e.description.toLowerCase().includes(q) || (e.category ?? '').toLowerCase().includes(q))
    );
  });
</script>

<svelte:window onkeydown={onWindowKeydown} />

<header class="page-head">
  <h1 tabindex="-1">Accounts</h1>
  <p class="muted">Balances and the history behind them.</p>
</header>

<div class="split">
  <nav class="panel list-pane" aria-label="Accounts">
    <AsyncView state={accounts.current} isEmpty={(a) => a.length === 0} onretry={accounts.reload} label="accounts">
      {#snippet empty()}<EmptyState title="No accounts" body="Add an account to start recording. You can leave the opening balance unknown until you check it." />{/snippet}
      {#snippet children(list)}
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
        <ul bind:this={listEl} onkeydown={onListKeydown}>
          <li>
            <button aria-current={selectedId === null ? 'true' : undefined} onclick={() => onselect(null)}>
              <span class="nm"><strong>All accounts</strong><span class="muted small">Every recorded movement</span></span>
            </button>
          </li>
          {#each list as a (a.id)}
            <li>
              <button aria-current={selectedId === a.id ? 'true' : undefined} onclick={() => onselect(a.id)}>
                <span class="nm"><strong>{a.name}</strong><span class="muted small">{KIND_LABEL[a.kind]}</span></span>
                {#if a.kind === 'CREDIT_CARD' && a.balance !== null}
                  <span class="amt"><Money value={-a.balance} tone="debt" /><span class="muted small">owed</span></span>
                {:else}
                  <span class="amt"><Money value={a.balance} />{#if a.status === 'incomplete'}<span class="inc small">Incomplete</span>{/if}</span>
                {/if}
              </button>
            </li>
          {/each}
        </ul>
      {/snippet}
    </AsyncView>
  </nav>

  <section class="panel history" aria-labelledby="hist-title">
    <div class="hist-head">
      <div>
        <h2 id="hist-title">{acct ? acct.name : 'All accounts'}</h2>
        {#if acct}
          <div class="bal">
            {#if acct.kind === 'CREDIT_CARD' && acct.balance !== null}
              <Money value={-acct.balance} tone="debt" size="lg" /> <span class="muted">owed as of {formatDate(acct.asOf)}</span>
            {:else}
              <Money value={acct.balance} size="lg" />
              <span class="muted">as of {formatDate(acct.asOf)}</span>
              <StateTag state={acct.status === 'complete' ? 'actual' : 'incomplete'} label={acct.statusNote} />
            {/if}
          </div>
        {:else}
          <p class="muted small">Transfers between your own accounts are shown as money moved, not spent.</p>
        {/if}
      </div>
      <div class="filters" role="search" aria-label="Filter history">
        <label class="field">
          <span>Search <kbd aria-hidden="true">/</kbd></span>
          <input bind:this={search} type="search" bind:value={query} placeholder="Description or category" aria-keyshortcuts="/" />
        </label>
        <label class="field">
          <span>Type</span>
          <select bind:value={kind}>
            <option value="all">All types</option>
            {#each Object.entries(EVENT_LABEL) as [value, label] (value)}<option {value}>{label}</option>{/each}
          </select>
        </label>
      </div>
    </div>

    <AsyncView state={entries.current} isEmpty={(e) => e.length === 0} onretry={entries.reload} label="history" skeletonRows={6}>
      {#snippet empty()}
        <EmptyState title="No history yet" body={acct ? `Nothing has been recorded for ${acct.name}.` : 'Entries you record will be listed here, newest first.'}>
          {#snippet action()}<button class="btn small primary" onclick={() => (app.entryOpen = true)}>New entry</button>{/snippet}
        </EmptyState>
      {/snippet}
      {#snippet children()}
        {#if visible.length === 0}
          <div class="no-match" role="status">
            <p><strong>No entries match these filters.</strong></p>
            <button class="btn small" onclick={() => { query = ''; kind = 'all'; }}>Clear filters</button>
          </div>
        {:else}
          <p class="sr-only" role="status">{visible.length} entries shown</p>
          <EntryList entries={visible} accounts={accounts.current.data ?? []} accountId={selectedId} caption="History for {acct?.name ?? 'all accounts'}" />
        {/if}
      {/snippet}
    </AsyncView>
  </section>
</div>

<style>
  .page-head { margin-bottom: var(--space-5); }
  h1:focus { outline: none; }
  .split { display: grid; grid-template-columns: 280px minmax(0, 1fr); gap: var(--space-4); align-items: start; }
  .list-pane ul { list-style: none; margin: 0; padding: var(--space-2); display: grid; gap: 2px; }
  .list-pane button { width: 100%; display: flex; justify-content: space-between; align-items: center; gap: var(--space-2); padding: var(--space-2) var(--space-3); border: 0; border-radius: var(--radius-md); background: none; text-align: left; cursor: pointer; min-height: 48px; }
  .list-pane button:hover { background: var(--surface-sunken); }
  .list-pane button[aria-current] { background: var(--accent-soft); box-shadow: inset 3px 0 0 var(--accent); }
  .nm { display: grid; min-width: 0; }
  .amt { display: grid; justify-items: end; }
  .inc { color: var(--state-incomplete); font-weight: 600; }
  .small { font-size: var(--text-xs); }
  .hist-head { display: flex; justify-content: space-between; align-items: end; gap: var(--space-4); padding: var(--space-4) var(--space-5); border-bottom: 1px solid var(--border); }
  .bal { display: flex; align-items: baseline; gap: var(--space-2); margin-top: var(--space-1); flex-wrap: wrap; }
  .filters { display: flex; gap: var(--space-3); }
  .filters input { width: 220px; }
  .no-match { padding: var(--space-6) var(--space-5); display: grid; gap: var(--space-2); justify-items: start; }
</style>
