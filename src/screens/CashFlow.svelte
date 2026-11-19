<!--
  Actual income and spending from Rust's canonical reports: known dates only,
  transfers and card payments excluded, refunds netted. Earlier Cash/Bank
  reports are separate (Ledger start & history). Forecasts live on Forecast.
-->
<script lang="ts">
  import { app, resource } from '../lib/app.svelte';
  import { getCanonicalByCategory } from '../lib/finance';
  import { getCanonicalMonthly } from '../lib/ledger';
  import { addMonths, formatMonth, monthStart } from '../lib/money';
  import AsyncView from '../lib/components/AsyncView.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';
  import Money from '../lib/components/Money.svelte';
  import SeriesChart from '../lib/components/SeriesChart.svelte';

  let months = $state(12);
  const from = $derived(addMonths(monthStart(app.today), -(months - 1)));
  const to = $derived(app.today);
  let catFrom = $state('');
  let catTo = $state('');
  $effect(() => {
    catFrom = monthStart(app.today);
    catTo = app.today;
  });

  const monthly = resource(() => getCanonicalMonthly({ from, to }));
  const byCategory = resource(() => getCanonicalByCategory({ ...(catFrom ? { from: catFrom } : {}), ...(catTo ? { to: catTo } : {}) }));
  const thisMonth = $derived(monthStart(app.today));
</script>

<header class="page-head">
  <div>
    <h1 tabindex="-1">Cash flow</h1>
    <p class="muted">Recorded income and spending by month. Only entries with a known date count; transfers and card payments are moves, not spending.</p>
  </div>
  <div class="field inline"><label class="field-label">
    <span>Period</span>
    <select bind:value={months}>
      <option value={6}>Last 6 months</option>
      <option value={12}>Last 12 months</option>
      <option value={24}>Last 24 months</option>
    </select></label>
  </div>
</header>

<div class="stack">
  <section class="panel" aria-labelledby="cf-month">
    <div class="panel-head"><h2 id="cf-month">By month</h2><span class="muted small">Actual</span></div>
    <AsyncView state={monthly.current} isEmpty={(r) => r.length === 0} onretry={monthly.reload} label="monthly cash flow" skeletonRows={6}>
      {#snippet empty()}
        <EmptyState title="No dated activity in this period" body="Months appear after income or spending with a known date is recorded. Earlier Cash/Bank records are reported separately under Ledger start & history." />
      {/snippet}
      {#snippet children(rows)}
        <SeriesChart
          caption="Monthly income and net spending"
          series={[{ name: 'Income', color: 'var(--chart-in)' }, { name: 'Net spending', color: 'var(--chart-out)' }]}
          points={rows.map((m) => ({
            key: m.month,
            label: formatMonth(m.month.slice(0, 7)).replace(/ \d{4}$/, ''),
            fullLabel: formatMonth(m.month.slice(0, 7)),
            values: [m.income, m.netSpending],
            state: 'actual',
            note: [m.refunds ? `Refunds ${m.refunds.toLocaleString('ja-JP')} yen netted` : '', m.month === thisMonth ? 'Month to date' : ''].filter(Boolean).join(' · ') || undefined,
          }))}
        />
        <p class="panel-body muted small">Net spending is gross spending minus linked refunds. Card purchases count on the purchase date, not when the card is paid.</p>
      {/snippet}
    </AsyncView>
  </section>

  <section class="panel" aria-labelledby="cf-cat">
    <div class="panel-head">
      <h2 id="cf-cat">Spending by category</h2>
      <div class="row">
        <div class="field inline"><label class="field-label"><span>From</span><input type="date" bind:value={catFrom} max={catTo || undefined} /></label></div>
        <div class="field inline"><label class="field-label"><span>To</span><input type="date" bind:value={catTo} min={catFrom || undefined} /></label></div>
      </div>
    </div>
    <AsyncView state={byCategory.current} isEmpty={(r) => r.length === 0} onretry={byCategory.reload} label="category totals">
      {#snippet empty()}<EmptyState title="No spending in this range" body="Choose a wider range or record some spending." />{/snippet}
      {#snippet children(rows)}
        <table class="data">
          <caption class="sr-only">Spending by category from {catFrom} to {catTo}</caption>
          <thead><tr><th scope="col">Category</th><th scope="col" class="right">Gross</th><th scope="col" class="right">Refunds</th><th scope="col" class="right">Net</th></tr></thead>
          <tbody>
            {#each rows as r (r.categoryId)}
              <tr><td>{r.categoryName}</td><td class="right"><Money value={r.grossSpending} /></td><td class="right"><Money value={r.refunds === 0 ? 0 : -r.refunds} /></td><td class="right"><Money value={r.netSpending} /></td></tr>
            {/each}
          </tbody>
        </table>
      {/snippet}
    </AsyncView>
    <p class="panel-body muted small">Known purchase dates only. Refunds reduce the category of the original purchase.</p>
  </section>
</div>

<style>
  .inline { grid-auto-flow: column; align-items: center; gap: var(--space-2); }
</style>
