<!--
  Accounts: balances (master) and the history behind them (detail). "All
  accounts" is the cross-account history. Earlier Cash/Bank records are not
  mixed in here; they live under Ledger start & history.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import { app, resource } from '../lib/app.svelte';
  import { listOpeningRevisions } from '../lib/finance';
  import { getAccountBalance, listFinancialEvents } from '../lib/ledger';
  import { formatDate, formatInstant } from '../lib/money';
  import type { Account, Balance, EventKind, FinancialEvent, Id } from '../lib/types';
  import AsyncView from '../lib/components/AsyncView.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';
  import Money from '../lib/components/Money.svelte';
  import StateTag from '../lib/components/StateTag.svelte';
  import AccountDialogs, { type AccountDialog } from './AccountDialogs.svelte';
  import EventDialog from './EventDialog.svelte';
  import { ACCOUNT_KIND_LABEL, balanceReason, DATE_BASIS_LABEL, KIND_LABEL, MOVED } from './labels';

  let { selectedId }: { selectedId: Id | null } = $props();

  const PAGE_SIZE = 50;
  let showArchived = $state(false);
  let page = $state(1);
  let search = $state('');
  let kindFilter = $state<EventKind | ''>('');
  let dialog = $state<AccountDialog | null>(null);
  let openEvent = $state<FinancialEvent | null>(null);
  let searchInput: HTMLInputElement | undefined = $state();
  let listEl: HTMLUListElement | undefined = $state();

  const visible = $derived((app.accounts ?? []).filter((a) => showArchived || a.archivedAt === null));
  const selected = $derived(app.account(selectedId));

  const balances = resource(async () => {
    const ids = (app.accounts ?? []).map((a) => a.id);
    const today = app.today;
    const list = await Promise.all(ids.map((id) => getAccountBalance(id, today)));
    return new Map(list.map((b) => [b.accountId, b]));
  });
  const history = resource(() => listFinancialEvents({ ...(selectedId ? { accountId: selectedId } : {}), page, pageSize: PAGE_SIZE }));
  const revisions = resource(() => (selectedId ? listOpeningRevisions(selectedId) : Promise.resolve([])));

  $effect(() => {
    void selectedId;
    page = 1;
    search = '';
    kindFilter = '';
  });

  const reversedIds = $derived(new Set((history.current.data?.items ?? []).map((e) => e.reversesEventId).filter((x): x is string => !!x)));

  const filtered = $derived.by(() => {
    const items = history.current.data?.items ?? [];
    const q = search.trim().toLowerCase();
    return items.filter(
      (e) =>
        (!kindFilter || e.kind === kindFilter) &&
        (!q || [e.description ?? '', app.categoryName(e.categoryId), KIND_LABEL[e.kind]].some((s) => s.toLowerCase().includes(q))),
    );
  });

  function select(id: Id | null) {
    app.go({ name: 'accounts', accountId: id });
  }

  async function onListKeydown(e: KeyboardEvent) {
    const ids: (Id | null)[] = [null, ...visible.map((a) => a.id)];
    const index = ids.indexOf(selectedId);
    const next = e.key === 'ArrowDown' ? index + 1 : e.key === 'ArrowUp' ? index - 1 : e.key === 'Home' ? 0 : e.key === 'End' ? ids.length - 1 : null;
    if (next === null) return;
    e.preventDefault();
    select(ids[Math.max(0, Math.min(ids.length - 1, next))] ?? null);
    await tick();
    listEl?.querySelector<HTMLElement>('[aria-current="true"]')?.focus();
  }

  function onWindowKeydown(e: KeyboardEvent) {
    if (e.key === '/' && !(e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement || e.target instanceof HTMLTextAreaElement) && !document.querySelector('dialog[open]')) {
      e.preventDefault();
      searchInput?.focus();
    }
  }

  /** This account's side of an event; across all accounts, the event amount. */
  function delta(e: FinancialEvent): number | null {
    if (!selectedId) return null;
    return e.postings.filter((p) => p.accountId === selectedId).reduce((s, p) => s + p.delta, 0);
  }

  function effective(e: FinancialEvent) {
    return e.postings.find((p) => !selectedId || p.accountId === selectedId)?.effectiveDate ?? null;
  }

  function figureFor(account: Account, b: Balance | undefined) {
    if (!b) return { value: null as number | null, tone: 'plain' as const, suffix: '' };
    if (account.kind === 'CREDIT_CARD') {
      if (b.creditBalance) return { value: b.creditBalance, tone: 'plain' as const, suffix: 'credit' };
      return { value: b.amountOwed, tone: 'debt' as const, suffix: 'owed' };
    }
    return { value: b.balance, tone: 'plain' as const, suffix: '' };
  }

  const pages = $derived(Math.max(1, Math.ceil((history.current.data?.total ?? 0) / PAGE_SIZE)));
</script>

<svelte:window onkeydown={onWindowKeydown} />

<header class="page-head">
  <div>
    <h1 tabindex="-1">Accounts</h1>
    <p class="muted">Balances as of the start of today and the entries behind them. Card debt is shown as an amount owed, never as cash.</p>
  </div>
  <button class="btn" onclick={() => (dialog = { kind: 'create' })}>Add account</button>
</header>

{#if app.accounts && app.accounts.length === 0}
  <section class="panel">
    <div class="panel-body">
      <EmptyState title="No accounts yet" body="Add the wallets, bank accounts, cards and brokerage cash accounts you want to track. Then record an opening balance for each — or leave it unknown until you have the figure.">
        {#snippet action()}<button class="btn primary" onclick={() => (dialog = { kind: 'create' })}>Add your first account</button>{/snippet}
      </EmptyState>
    </div>
  </section>
{:else}
  <div class="md">
    <section class="panel list" aria-label="Account list">
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <ul class="list-plain" bind:this={listEl} onkeydown={onListKeydown}>
        <li>
          <button class="acct" aria-current={selectedId === null ? 'true' : undefined} tabindex={selectedId === null ? 0 : -1} onclick={() => select(null)}>
            <span class="name">All accounts</span>
            <span class="muted small">Cross-account history</span>
          </button>
        </li>
        {#each visible as a (a.id)}
          {@const b = balances.current.data?.get(a.id)}
          {@const f = figureFor(a, b)}
          <li>
            <button class="acct" aria-current={selectedId === a.id ? 'true' : undefined} tabindex={selectedId === a.id ? 0 : -1} onclick={() => select(a.id)}>
              <span class="name">{a.name}{#if a.archivedAt}<span class="muted"> · archived</span>{/if}</span>
              <span class="muted small">{ACCOUNT_KIND_LABEL[a.kind]}</span>
              <span class="fig">
                {#if b}<Money value={f.value} tone={f.tone} state={b.status === 'complete' ? 'actual' : 'incomplete'} />{#if f.suffix}<span class="muted small"> {f.suffix}</span>{/if}{:else}<span class="muted small">…</span>{/if}
              </span>
            </button>
          </li>
        {/each}
      </ul>
      <label class="check archived"><input type="checkbox" bind:checked={showArchived} /> Show archived</label>
    </section>

    <section class="stack detail" aria-label="Account detail">
      {#if selected}
        {@const b = balances.current.data?.get(selected.id)}
        {@const f = figureFor(selected, b)}
        <div class="panel">
          <div class="figure-card">
            <p class="eyebrow">{ACCOUNT_KIND_LABEL[selected.kind]} · {selected.kind === 'CREDIT_CARD' ? (f.suffix === 'credit' ? 'credit balance' : 'amount owed') : 'balance'} today
              {#if b && b.status !== 'complete'}<StateTag state="incomplete" />{/if}
              {#if selected.archivedAt}<StateTag state="neutral" label="Archived" />{/if}
            </p>
            <h2 class="sr-only">{selected.name}</h2>
            <Money value={f.value} size="figure" tone={f.tone} state={b?.status === 'complete' ? 'actual' : 'incomplete'} />
            {#if b && balanceReason(b)}<p class="note">{balanceReason(b)}.{#if b.status === 'incomplete_legacy_mapping'} <button class="link" onclick={() => app.go({ name: 'start' })}>Finish ledger start</button>{/if}</p>{/if}
            {#if selected.kind === 'CREDIT_CARD'}<p class="note">Card debt is shown separately from cash and is not spending until purchases are recorded.</p>{/if}
          </div>
          <div class="row actions">
            <button class="btn small" onclick={() => (dialog = { kind: 'opening', account: selected, balance: b ?? null })}>{b?.openingAsOfDate ? 'Revise opening…' : 'Set opening…'}</button>
            {#if selected.archivedAt === null}<button class="btn small" onclick={() => (app.entryOpen = true)}>New entry here</button>{/if}
            {#if selected.kind === 'CREDIT_CARD'}<button class="btn small" onclick={() => app.go({ name: 'cards', cardId: selected.id })}>Payment schedule</button>{/if}
            <button class="btn ghost small" onclick={() => (dialog = { kind: 'archive', account: selected })}>{selected.archivedAt ? 'Restore…' : 'Archive…'}</button>
          </div>
          {#if (revisions.current.data ?? []).length > 0}
            <details class="revisions">
              <summary>Opening history ({revisions.current.data!.length} revision{revisions.current.data!.length === 1 ? '' : 's'})</summary>
              <table class="data">
                <thead><tr><th scope="col">Revision</th><th scope="col">As of</th><th scope="col" class="right">Amount</th><th scope="col">Reason</th><th scope="col">Recorded</th></tr></thead>
                <tbody>
                  {#each revisions.current.data! as r (r.id)}
                    <tr><td>{r.revision}</td><td>{formatDate(r.asOfDate)}</td><td class="right"><Money value={r.amount} /></td><td>{r.reason}</td><td class="muted">{formatInstant(r.recordedAt)}</td></tr>
                  {/each}
                </tbody>
              </table>
            </details>
          {/if}
        </div>
      {/if}

      <div class="panel">
        <div class="panel-head">
          <h2>{selected ? 'History' : 'All entries'}</h2>
          <div class="row">
            <label><span class="sr-only">Type</span>
              <select bind:value={kindFilter} class="filter">
                <option value="">All types</option>
                {#each Object.entries(KIND_LABEL) as [value, label] (value)}<option {value}>{label}</option>{/each}
              </select>
            </label>
            <label><span class="sr-only">Search this page</span>
              <input bind:this={searchInput} bind:value={search} class="filter" type="search" placeholder="Search  /" aria-keyshortcuts="/" />
            </label>
          </div>
        </div>
        <AsyncView state={history.current} isEmpty={(p) => p.total === 0} onretry={history.reload} label="history">
          {#snippet empty()}
            <EmptyState title="No entries yet" body={selected ? `Entries posted to ${selected.name} appear here. Earlier Cash/Bank records are kept separately under Ledger start & history.` : 'Every entry across your accounts appears here, newest first.'}>
              {#snippet action()}<button class="btn primary" onclick={() => (app.entryOpen = true)}>New entry</button>{/snippet}
            </EmptyState>
          {/snippet}
          {#snippet children(data)}
            {#if filtered.length === 0}
              <div class="panel-body"><EmptyState title="No entries match" body="Nothing on this page matches the search or type filter.">
                {#snippet action()}<button class="btn small" onclick={() => { search = ''; kindFilter = ''; }}>Clear filters</button>{/snippet}
              </EmptyState></div>
            {:else}
              <table class="data">
                <caption class="sr-only">{selected ? `${selected.name} history` : 'All entries'}, page {data.page} of {pages}</caption>
                <thead><tr><th scope="col">Date</th><th scope="col">Details</th><th scope="col">Type</th><th scope="col" class="right">{selected ? 'Change' : 'Amount'}</th></tr></thead>
                <tbody>
                  {#each filtered as e (e.id)}
                    {@const d = delta(e)}
                    <tr class:struck={reversedIds.has(e.id) || e.isReversal}>
                      <td>
                        {#if e.occurredOn}{formatDate(e.occurredOn)}{:else}<StateTag state="warning" label="Purchase date unknown" /><br /><span class="muted small">Posted {formatDate(effective(e))}</span>{/if}
                        {#if e.occurredOn && effective(e) && effective(e) !== e.occurredOn}<br /><span class="muted small">Posted {formatDate(effective(e))}</span>{/if}
                      </td>
                      <td>
                        <button class="link" onclick={() => (openEvent = e)}>{e.description ?? (e.categoryId ? app.categoryName(e.categoryId) : KIND_LABEL[e.kind])}</button>
                        <span class="muted small">
                          {#if e.categoryId && e.description}{app.categoryName(e.categoryId)}{/if}
                          {#if MOVED.has(e.kind)}{e.postings.map((p) => app.accountName(p.accountId)).join(' → ')}{/if}
                          {#if !selected && !MOVED.has(e.kind)} · {e.postings.map((p) => app.accountName(p.accountId)).join(', ')}{/if}
                          {#if e.dateBasis !== 'known'} · {DATE_BASIS_LABEL[e.dateBasis]}{/if}
                        </span>
                      </td>
                      <td>
                        <span class:muted={MOVED.has(e.kind)}>{KIND_LABEL[e.kind]}{MOVED.has(e.kind) ? ' · moved' : ''}</span>
                        {#if e.isReversal}<StateTag state="warning" label="Reversal" />{/if}
                        {#if reversedIds.has(e.id)}<StateTag state="warning" label="Reversed" />{/if}
                        {#if e.replacesEventId}<StateTag state="neutral" label="Correction" />{/if}
                      </td>
                      <td class="right">
                        {#if d !== null}<Money value={d} signed tone="direction" />{:else}<Money value={e.amount} />{/if}
                      </td>
                    </tr>
                  {/each}
                </tbody>
              </table>
              {#if pages > 1}
                <div class="row pager">
                  <button class="btn small" disabled={page <= 1} onclick={() => (page -= 1)}>Newer</button>
                  <span class="muted small">Page {data.page} of {pages} · {data.total} entries</span>
                  <button class="btn small" disabled={page >= pages} onclick={() => (page += 1)}>Older</button>
                </div>
              {/if}
            {/if}
          {/snippet}
        </AsyncView>
      </div>
    </section>
  </div>
{/if}

<AccountDialogs {dialog} onclose={() => (dialog = null)} oncreated={(a) => select(a.id)} />
<EventDialog event={openEvent} {reversedIds} onclose={() => (openEvent = null)} />

<style>
  .md { display: grid; grid-template-columns: 280px minmax(0, 1fr); gap: var(--space-5); align-items: start; }
  .list { padding: var(--space-2); position: sticky; top: 0; }
  .list ul { display: grid; gap: 2px; }
  .acct { width: 100%; display: grid; grid-template-columns: 1fr auto; grid-template-areas: 'name fig' 'kind fig'; text-align: left; gap: 0 var(--space-2); padding: var(--space-2) var(--space-3); border: 0; border-radius: var(--radius-md); background: transparent; cursor: pointer; }
  .acct:hover { background: var(--surface-sunken); }
  .acct[aria-current='true'] { background: var(--accent-soft); box-shadow: inset 3px 0 0 var(--accent); }
  .acct .name { grid-area: name; font-weight: 560; }
  .acct .small { grid-area: kind; }
  .acct .fig { grid-area: fig; align-self: center; text-align: right; }
  .archived { padding: var(--space-2) var(--space-3); }
  .actions { padding: 0 var(--space-5) var(--space-4); }
  .revisions { padding: 0 var(--space-5) var(--space-4); }
  .revisions summary { cursor: pointer; font-size: var(--text-sm); color: var(--ink-2); margin-bottom: var(--space-2); }
  .filter { min-height: 30px; border: 1px solid var(--border-strong); border-radius: var(--radius-md); padding: 0 var(--space-2); background: var(--surface); font-size: var(--text-sm); }
  td .small { display: block; }
  tr.struck td:not(:last-child) { color: var(--ink-3); }
  tr.struck :global(.money) { text-decoration: line-through; }
  .pager { justify-content: center; padding: var(--space-3); border-top: 1px solid var(--border); }
</style>
