<script lang="ts">
  import { app, resource, type Route } from '../lib/app.svelte';
  import { commands } from '../lib/commands';
  import { cardOwed, cashPosition, monthFlow } from '../lib/derive';
  import { KIND_LABEL, TODAY } from '../lib/fixtures';
  import { formatDate, formatMonth } from '../lib/money';
  import AsyncView from '../lib/components/AsyncView.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';
  import Money from '../lib/components/Money.svelte';
  import StateTag from '../lib/components/StateTag.svelte';
  import EntryList from './EntryList.svelte';

  let { onnavigate }: { onnavigate: (route: Route) => void } = $props();

  const accounts = resource(() => commands.listAccounts());
  const entries = resource(() => commands.listEntries());
  const report = resource(() => commands.cashFlowReport());

  const month = TODAY.slice(0, 7);
</script>

<header class="page-head">
  <div>
    <h1 tabindex="-1">Overview</h1>
    <p class="muted">Where things stand on {formatDate(TODAY)}.</p>
  </div>
</header>

<section aria-label="Position" class="positions">
  <AsyncView state={accounts.current} isEmpty={(a) => a.length === 0} onretry={accounts.reload} label="balances" skeletonRows={3}>
    {#snippet empty()}
      <EmptyState title="No accounts yet" body="Add the bank accounts, cash and cards you use. Balances you don’t know yet can stay unknown — they’re never counted as zero.">
        {#snippet action()}<button class="btn small" onclick={() => onnavigate({ name: 'accounts', accountId: null })}>Go to Accounts</button>{/snippet}
      </EmptyState>
    {/snippet}
    {#snippet children(list)}
      {@const cash = cashPosition(list)}
      {@const owed = cardOwed(list)}
      <div class="cards">
        <article class="figure-card">
          <div class="label"><h2>Cash in accounts</h2><StateTag state={cash.unknownCount ? 'incomplete' : 'actual'} /></div>
          <Money value={cash.total ?? cash.knownSubtotal} state={cash.unknownCount ? 'incomplete' : 'actual'} size="figure" />
          <p class="foot">
            {#if cash.unknownCount}
              Known balances only · <strong>{cash.unknownCount} of {cash.count}</strong> accounts unknown
            {:else}
              Across {cash.count} bank, cash and brokerage-cash accounts
            {/if}
          </p>
        </article>
        <article class="figure-card">
          <div class="label"><h2>Owed on cards</h2><StateTag state="actual" /></div>
          <Money value={owed.total} size="figure" tone="debt" />
          <p class="foot">Not yet repaid · shown separately from cash</p>
        </article>
        <AsyncView state={entries.current} isEmpty={() => false} onretry={entries.reload} label="this month" skeletonRows={3}>
          {#snippet empty()}{/snippet}
          {#snippet children(list)}
            {@const flow = monthFlow(list, month)}
            <article class="figure-card">
              <div class="label"><h2>{formatMonth(month)} so far</h2><StateTag state="actual" /></div>
              <dl class="flow">
                <div><dt>Money in</dt><dd><Money value={flow.moneyIn} signed tone="direction" size="lg" /></dd></div>
                <div><dt>Spending</dt><dd><Money value={-flow.spending} size="lg" /></dd></div>
              </dl>
              <p class="foot">Transfers and card repayments aren’t spending{flow.excludedMoves ? ` (${flow.excludedMoves} excluded)` : ''}</p>
            </article>
          {/snippet}
        </AsyncView>
      </div>
      <p class="not-net-worth muted">
        These figures are not net worth. Investments, other assets and future commitments aren’t combined here.
      </p>
    {/snippet}
  </AsyncView>
</section>

<div class="columns">
  <section class="panel" aria-labelledby="ov-accounts">
    <div class="panel-head">
      <h2 id="ov-accounts">Accounts</h2>
      <button class="btn ghost small" onclick={() => onnavigate({ name: 'accounts', accountId: null })}>All accounts →</button>
    </div>
    <AsyncView state={accounts.current} isEmpty={(a) => a.length === 0} onretry={accounts.reload} label="accounts">
      {#snippet empty()}<EmptyState title="Nothing here yet" body="Accounts you add will be listed with their latest balance." />{/snippet}
      {#snippet children(list)}
        <ul class="acct-list">
          {#each list as a (a.id)}
            <li>
              <button onclick={() => onnavigate({ name: 'accounts', accountId: a.id })}>
                <span class="name"><strong>{a.name}</strong><span class="muted">{KIND_LABEL[a.kind]}{a.asOf !== TODAY ? ` · as of ${formatDate(a.asOf)}` : ''}</span></span>
                <span class="amt">
                  {#if a.kind === 'CREDIT_CARD' && a.balance !== null}
                    <Money value={-a.balance} tone="debt" /><span class="muted small">owed</span>
                  {:else}
                    <Money value={a.balance} />
                    {#if a.status === 'incomplete'}<StateTag state="incomplete" label={a.statusNote ?? 'Incomplete'} />{/if}
                  {/if}
                </span>
              </button>
            </li>
          {/each}
        </ul>
      {/snippet}
    </AsyncView>
  </section>

  <section class="panel" aria-labelledby="ov-recent">
    <div class="panel-head">
      <h2 id="ov-recent">Recent activity</h2>
      <button class="btn ghost small" onclick={() => onnavigate({ name: 'accounts', accountId: null })}>History →</button>
    </div>
    <AsyncView state={entries.current} isEmpty={(e) => e.length === 0} onretry={entries.reload} label="activity">
      {#snippet empty()}
        <EmptyState title="No activity recorded" body="Your first entry will appear here.">
          {#snippet action()}<button class="btn small primary" onclick={() => (app.entryOpen = true)}>New entry</button>{/snippet}
        </EmptyState>
      {/snippet}
      {#snippet children(list)}
        <AsyncView state={accounts.current} isEmpty={() => false} onretry={accounts.reload} label="account names">
          {#snippet empty()}{/snippet}
          {#snippet children(accts)}<EntryList entries={list.slice(0, 6)} accounts={accts} accountId={null} compact />{/snippet}
        </AsyncView>
      {/snippet}
    </AsyncView>
  </section>
</div>

<section class="panel forecast-strip" aria-labelledby="ov-ahead">
  <div class="panel-head">
    <h2 id="ov-ahead">Looking ahead</h2>
    <button class="btn ghost small" onclick={() => onnavigate({ name: 'cashflow' })}>Cash flow →</button>
  </div>
  <AsyncView state={report.current} isEmpty={(r) => !r.some((m) => m.state === 'forecast')} onretry={report.reload} label="forecast" skeletonRows={2}>
    {#snippet empty()}<EmptyState title="No forecast yet" body="Once you have a few months of history and a payday set, expected income and spending appear here — always labelled as forecast." />{/snippet}
    {#snippet children(months)}
      <ul class="ahead">
        {#each months.filter((m) => m.state === 'forecast') as m (m.month)}
          <li>
            <span class="m">{formatMonth(m.month)} <StateTag state="forecast" /></span>
            <span><span class="muted small">In</span> <Money value={m.moneyIn} state="forecast" signed /></span>
            <span><span class="muted small">Spending</span> <Money value={-m.spending} state="forecast" /></span>
            {#if m.note}<span class="muted small">{m.note}</span>{/if}
          </li>
        {/each}
      </ul>
      <p class="muted small pad">Forecasts are estimates, not money you have. They never change actual balances.</p>
    {/snippet}
  </AsyncView>
</section>

<style>
  .page-head { margin-bottom: var(--space-5); }
  h1:focus { outline: none; }
  .positions { margin-bottom: var(--space-2); }
  .cards { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: var(--space-4); }
  .figure-card { background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: var(--space-4) var(--space-5); display: grid; gap: var(--space-2); align-content: start; min-height: 148px; }
  .label { display: flex; justify-content: space-between; align-items: center; gap: var(--space-2); }
  .label h2 { font-size: var(--text-sm); font-weight: 600; color: var(--ink-2); }
  .foot { font-size: var(--text-sm); color: var(--ink-3); }
  .flow { display: grid; grid-template-columns: 1fr 1fr; gap: var(--space-3); margin: 0; }
  .flow dt { font-size: var(--text-xs); color: var(--ink-3); font-weight: 600; }
  .flow dd { margin: 0; }
  .not-net-worth { font-size: var(--text-sm); margin: var(--space-3) 0 var(--space-5); }
  .columns { display: grid; grid-template-columns: minmax(0, 5fr) minmax(0, 7fr); gap: var(--space-4); margin-bottom: var(--space-4); }
  .acct-list { list-style: none; margin: 0; padding: var(--space-1) 0; }
  .acct-list button { width: 100%; display: flex; justify-content: space-between; align-items: center; gap: var(--space-3); padding: var(--space-2) var(--space-5); min-height: var(--row-height); border: 0; background: none; text-align: left; cursor: pointer; }
  .acct-list button:hover { background: var(--surface-sunken); }
  .name { display: grid; }
  .name .muted { font-size: var(--text-xs); }
  .amt { display: flex; flex-direction: column; align-items: end; gap: 2px; }
  .small { font-size: var(--text-xs); }
  .ahead { list-style: none; margin: 0; padding: var(--space-2) var(--space-5); display: grid; gap: var(--space-2); }
  .ahead li { display: grid; grid-template-columns: 180px 180px 200px 1fr; align-items: center; gap: var(--space-3); min-height: 36px; border-bottom: 1px dashed var(--border); }
  .ahead li:last-child { border-bottom: 0; }
  .m { display: flex; gap: var(--space-2); align-items: center; font-weight: 560; }
  .pad { padding: 0 var(--space-5) var(--space-4); }
</style>
