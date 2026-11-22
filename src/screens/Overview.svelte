<!--
  Overview from Rust's dashboard. Each headline is one meaning with its own
  label: cash, money set aside, card debt, net worth and forecast are never
  added together here. Unknown values stay "Unknown"; coverage gaps are named.
-->
<script lang="ts">
  import { app, resource } from '../lib/app.svelte';
  import { getDashboard } from '../lib/finance';
  import { addDays, formatDate, formatMonth, sumKnown } from '../lib/money';
  import type { Dashboard } from '../lib/types';
  import AsyncView from '../lib/components/AsyncView.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';
  import Money from '../lib/components/Money.svelte';
  import StateTag from '../lib/components/StateTag.svelte';
  import { balanceReason } from './labels';

  const dashboard = resource(() => getDashboard(app.today));

  function setup(d: Dashboard) {
    const accounts = d.financialPosition.accounts;
    const steps: { label: string; done: boolean; go: () => void }[] = [
      { label: 'Add your accounts', done: accounts.length > 0, go: () => app.go({ name: 'accounts', accountId: null }) },
      {
        label: 'Start the ledger (opening balances and any earlier Cash/Bank history)',
        done: accounts.length > 0 && !accounts.some((a) => a.status === 'incomplete_legacy_mapping'),
        go: () => app.go({ name: 'start' }),
      },
      {
        label: 'Confirm your payday',
        done: !d.availableCash.warnings.includes('Finance settings are unconfirmed'),
        go: () => app.go({ name: 'budgets' }),
      },
    ];
    return steps;
  }

  function warnings(d: Dashboard) {
    return [...new Set([...d.availableCash.warnings, ...d.forecast.warnings])];
  }

  const ownedOnCards = (signed: number | null) => (signed === null ? null : Math.max(0, -signed));
</script>

<header class="page-head">
  <div>
    <h1 tabindex="-1">Overview</h1>
    <p class="muted">Where things stand at the start of {formatDate(app.today)}. These figures are kept separate on purpose.</p>
  </div>
  <button class="btn" onclick={() => (app.entryOpen = true)}>New entry</button>
</header>

<AsyncView state={dashboard.current} isEmpty={() => false} onretry={dashboard.reload} label="overview" skeletonRows={8}>
  {#snippet empty()}<span></span>{/snippet}
  {#snippet children(d)}
    {@const steps = setup(d)}
    {@const pos = d.financialPosition}
    {@const spendable = sumKnown(d.availableCash.accounts.map((a) => a.spendable))}
    {@const earmarked = d.availableCash.accounts.reduce((s, a) => s + a.fundedEarmarks, 0)}
    {@const dueUnfunded = d.availableCash.accounts.reduce((s, a) => s + a.unfundedKnownDue, 0)}
    <div class="stack">
      {#if steps.some((s) => !s.done)}
        <section class="panel setup" aria-labelledby="setup-title">
          <div class="panel-head"><h2 id="setup-title">Finish setting up</h2><span class="muted small">{steps.filter((s) => s.done).length} of {steps.length} done</span></div>
          <ol class="panel-body steps">
            {#each steps as s (s.label)}
              <li class:done={s.done}>
                <span aria-hidden="true">{s.done ? '✓' : '○'}</span>
                <span>{s.label}{#if s.done}<span class="sr-only"> — done</span>{/if}</span>
                {#if !s.done}<button class="btn small" onclick={s.go}>Open</button>{/if}
              </li>
            {/each}
          </ol>
        </section>
      {/if}

      {#if warnings(d).length}
        <div class="notice" role="note">
          <strong>Some figures rest on assumptions</strong>
          <ul>{#each warnings(d) as w (w)}<li>{w}</li>{/each}</ul>
        </div>
      {/if}

      <div class="cols-3">
        <section class="panel figure-card" aria-labelledby="f-cash">
          <p class="eyebrow" id="f-cash">Cash in accounts {#if pos.liquidCash === null}<StateTag state="incomplete" />{/if}</p>
          <Money value={pos.liquidCash} size="figure" state={pos.liquidCash === null ? 'incomplete' : 'actual'} />
          <p class="note">Cash and bank accounts. Cards are not cash.</p>
          {#if pos.brokerageCash !== null && pos.brokerageCash !== 0}<p class="note">Plus <Money value={pos.brokerageCash} /> brokerage cash</p>{/if}
        </section>

        <section class="panel figure-card" aria-labelledby="f-spend">
          <p class="eyebrow" id="f-spend">Spendable after commitments {#if spendable.total === null}<StateTag state="incomplete" />{/if}</p>
          <Money value={spendable.total} size="figure" state={spendable.total === null ? 'incomplete' : 'actual'} />
          {#if spendable.unknownCount}<p class="note">{spendable.unknownCount} of {spendable.count} accounts unknown; known part <Money value={spendable.knownSubtotal} state="incomplete" /></p>{/if}
          <p class="note">Set aside <Money value={earmarked} /> · due by {formatDate(d.availableCash.horizon)} and not yet set aside <Money value={dueUnfunded} /></p>
        </section>

        <section class="panel figure-card" aria-labelledby="f-card">
          <p class="eyebrow" id="f-card">Owed on cards {#if pos.signedCardBalances === null}<StateTag state="incomplete" />{/if}</p>
          <Money value={ownedOnCards(pos.signedCardBalances)} size="figure" tone="debt" state={pos.signedCardBalances === null ? 'incomplete' : 'actual'} />
          <p class="note">Shown separately from cash; paying a card moves money, it is not new spending.</p>
        </section>
      </div>

      <div class="cols-2">
        <section class="panel" aria-labelledby="nw-title">
          <div class="figure-card">
            <p class="eyebrow" id="nw-title">Net worth {#if pos.netWorth === null}<StateTag state="incomplete" />{/if}</p>
            <Money value={pos.netWorth} size="figure" state={pos.netWorth === null ? 'incomplete' : 'actual'} unknownLabel="Incomplete" />
            <p class="note">Cash + brokerage cash + securities valuation − card debt.
              {#if pos.netWorth === null}It stays incomplete until every account has a known balance and securities have a current valuation.{/if}</p>
          </div>
          <div class="row foot"><button class="btn small" onclick={() => app.go({ name: 'investments' })}>Investments & position</button></div>
        </section>

        <section class="panel" aria-labelledby="fc-title">
          <div class="panel-head"><h2 id="fc-title">Lowest projected balance</h2><StateTag state="forecast" /></div>
          {#if Object.keys(d.forecast.lowestByAccount).length === 0}
            <div class="panel-body"><EmptyState title="No projection yet" body="Forecasts appear once accounts have known balances." /></div>
          {:else}
            <table class="data">
              <caption class="sr-only">Lowest projected balance by account to {formatDate(d.forecast.to)}</caption>
              <thead><tr><th scope="col">Account</th><th scope="col">On</th><th scope="col" class="right">Balance</th></tr></thead>
              <tbody>
                {#each Object.entries(d.forecast.lowestByAccount) as [id, low] (id)}
                  <tr><td>{app.accountName(id)}</td><td>{formatDate(low.date)}</td><td class="right"><Money value={low.balance} state="forecast" tone={low.balance < 0 ? 'debt' : 'plain'} /></td></tr>
                {/each}
              </tbody>
            </table>
          {/if}
          <p class="panel-body muted small">Forecasts are estimates to {formatDate(d.forecast.to)}, not money you have. <button class="link" onclick={() => app.go({ name: 'forecast' })}>Open forecast</button></p>
        </section>
      </div>

      <div class="cols-2">
        <section class="panel" aria-labelledby="bud-title">
          <div class="panel-head">
            <h2 id="bud-title">This pay cycle</h2>
            <span class="muted small">{formatDate(d.livingBudget.start)} – {formatDate(addDays(d.livingBudget.endExclusive, -1))}</span>
          </div>
          {#if d.livingBudget.budgets.length === 0}
            <div class="panel-body"><EmptyState title="No budgets this cycle" body="Plan spending per category for each pay cycle under Budgets & savings." /></div>
          {:else}
            <table class="data">
              <thead><tr><th scope="col">Category</th><th scope="col" class="right">Planned</th><th scope="col" class="right">Spent</th><th scope="col" class="right">Left</th></tr></thead>
              <tbody>
                {#each d.livingBudget.budgets as b (b.id)}
                  <tr><td>{b.categoryName}</td><td class="right"><Money value={b.planned} /></td><td class="right"><Money value={b.netSpending} /></td><td class="right"><Money value={b.remaining} tone={b.remaining < 0 ? 'debt' : 'plain'} /></td></tr>
                {/each}
              </tbody>
            </table>
          {/if}
          {#if d.livingBudget.unknownPurchaseDateCount}<p class="panel-body notice">{d.livingBudget.unknownPurchaseDateCount} purchases with unknown dates are not counted in any cycle.</p>{/if}
        </section>

        <section class="panel" aria-labelledby="mo-title">
          <div class="panel-head"><h2 id="mo-title">Recent months</h2><span class="muted small">Known dates only</span></div>
          {#if d.monthlyActuals.length === 0}
            <div class="panel-body"><EmptyState title="No activity yet" body="Income and spending by month appear after your first entries." /></div>
          {:else}
            <table class="data">
              <thead><tr><th scope="col">Month</th><th scope="col" class="right">Income</th><th scope="col" class="right">Net spending</th></tr></thead>
              <tbody>
                {#each d.monthlyActuals.slice(-4).reverse() as m (m.month)}
                  <tr><td>{formatMonth(m.month.slice(0, 7))}</td><td class="right"><Money value={m.income} /></td><td class="right"><Money value={m.netSpending} /></td></tr>
                {/each}
              </tbody>
            </table>
          {/if}
          <p class="panel-body muted small">Transfers and card payments are excluded; refunds reduce spending.</p>
        </section>
      </div>

      <section class="panel" aria-labelledby="acc-title">
        <div class="panel-head"><h2 id="acc-title">Accounts</h2><button class="btn small" onclick={() => app.go({ name: 'accounts', accountId: null })}>All accounts</button></div>
        <table class="data">
          <thead><tr><th scope="col">Account</th><th scope="col">Status</th><th scope="col" class="right">Balance</th></tr></thead>
          <tbody>
            {#each pos.accounts as a (a.accountId)}
              <tr>
                <td><button class="link" onclick={() => app.go({ name: 'accounts', accountId: a.accountId })}>{a.name}</button></td>
                <td>{#if a.status === 'complete'}<StateTag state="actual" />{:else}<StateTag state="incomplete" label={balanceReason({ status: a.status, openingAsOfDate: null }) ?? 'Incomplete'} />{/if}</td>
                <td class="right">
                  {#if a.kind === 'CREDIT_CARD'}<Money value={ownedOnCards(a.balance)} tone="debt" /> <span class="muted small">owed</span>{:else}<Money value={a.balance} />{/if}
                </td>
              </tr>
            {:else}
              <tr><td colspan="3" class="muted">No accounts yet.</td></tr>
            {/each}
          </tbody>
        </table>
      </section>
    </div>
  {/snippet}
</AsyncView>

<style>
  .steps { display: grid; gap: var(--space-2); margin: 0; padding-left: var(--space-5); list-style: none; }
  .steps li { display: grid; grid-template-columns: 20px 1fr auto; align-items: center; gap: var(--space-2); }
  .steps li.done { color: var(--ink-3); }
  .foot { padding: 0 var(--space-5) var(--space-4); }
</style>
