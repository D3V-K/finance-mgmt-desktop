<!--
  Securities valuations and the true financial position. Net worth is liquid
  cash + brokerage cash + securities − card debt, computed in Rust; it stays
  "Incomplete" while any balance or valuation is unknown or stale. This is not
  the legacy "net balance" series, which lives under Ledger start & history.
-->
<script lang="ts">
  import { app, resource, WriteSession } from '../lib/app.svelte';
  import { getFinancialPosition, getPositionHistory, listValuations, markHoldingsChange, recordValuation } from '../lib/finance';
  import { addMonths, formatDate, formatInstant, formatMonth, monthStart } from '../lib/money';
  import { focusFirstInvalid, parseFor } from '../lib/forms';
  import type { Id } from '../lib/types';
  import AsyncView from '../lib/components/AsyncView.svelte';
  import Dialog from '../lib/components/Dialog.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';
  import FormStatus from '../lib/components/FormStatus.svelte';
  import Money from '../lib/components/Money.svelte';
  import SeriesChart from '../lib/components/SeriesChart.svelte';
  import StateTag from '../lib/components/StateTag.svelte';
  import YenField from '../lib/components/YenField.svelte';

  const SECURITIES_STATUS: Record<string, string> = { recorded: 'Valued', missing: 'No valuation', stale: 'Out of date' };

  let asOf = $state('');
  $effect(() => {
    if (!asOf) asOf = app.today;
  });
  const brokers = $derived((app.accounts ?? []).filter((a) => a.kind === 'BROKERAGE_CASH'));
  let selected = $state<Id>('');
  $effect(() => {
    if (brokers.length && !brokers.some((b) => b.id === selected)) selected = brokers[0]!.id;
  });

  const position = resource(() => getFinancialPosition(asOf || app.today));
  const history = resource(() => getPositionHistory(addMonths(monthStart(app.today), -11), app.today));
  const valuations = resource(() => (selected ? listValuations(selected) : Promise.resolve([])));

  type Mode = { kind: 'valuation' } | { kind: 'holdings' };
  let mode = $state<Mode | null>(null);
  const session = new WriteSession();
  let attempted = $state(false);
  let form: HTMLFormElement | undefined = $state();
  let amountRaw = $state('');
  let date = $state('');
  let note = $state('');
  let gainRaw = $state('');

  function open(next: Mode) {
    mode = next;
    attempted = false;
    session.reset();
    amountRaw = '';
    gainRaw = '';
    note = '';
    date = app.today;
  }

  const parsed = $derived(parseFor('nonnegative', amountRaw));
  const gain = $derived(gainRaw.trim() ? parseFor('signed', gainRaw) : null);
  const errors = $derived.by(() => {
    const e: Record<string, string> = {};
    if (!mode) return e;
    if (!date) e.date = 'Choose the date.';
    if (mode.kind === 'valuation' && !parsed.ok) e.marketValue = parsed.reason;
    if (mode.kind === 'holdings' && !note.trim()) e.reason = 'Describe what changed, e.g. “Bought 10 shares”.';
    if (mode.kind === 'holdings' && gain && !gain.ok) e.userEnteredRealizedGain = gain.reason;
    return e;
  });
  const show = (f: string) => session.fieldMessage(f) ?? (attempted ? errors[f] : undefined);

  async function save(ev: Event) {
    ev.preventDefault();
    if (!mode) return;
    attempted = true;
    if (Object.keys(errors).length) return focusFirstInvalid(form);
    const r =
      mode.kind === 'valuation'
        ? await session.run({ accountId: selected, asOfDate: date, marketValue: parsed.ok ? parsed.value : 0, note: note.trim() || null }, recordValuation)
        : await session.run({ accountId: selected, occurredOn: date, reason: note.trim(), userEnteredRealizedGain: gain?.ok ? gain.value : null }, markHoldingsChange);
    if (r) {
      if (!r.isReplay) app.toast(mode.kind === 'valuation' ? 'Valuation recorded.' : 'Holdings change noted. Record a new valuation to bring net worth up to date.');
      mode = null;
    }
  }
</script>

<header class="page-head">
  <div>
    <h1 tabindex="-1">Investments & position</h1>
    <p class="muted">Securities are valued by hand. Net worth is shown only when every part is known; otherwise it says what is missing.</p>
  </div>
  <div class="field inline"><label class="field-label"><span>As of</span><input type="date" bind:value={asOf} max={app.today} /></label></div>
</header>

<div class="stack">
  <AsyncView state={position.current} isEmpty={() => false} onretry={position.reload} label="financial position">
    {#snippet empty()}<span></span>{/snippet}
    {#snippet children(p)}
      {@const incomplete = p.coverage !== 'complete'}
      <div class="cols-3">
        <section class="panel figure-card" aria-labelledby="nw">
          <p class="eyebrow" id="nw">Net worth {#if incomplete}<StateTag state="incomplete" />{/if}</p>
          <Money value={p.netWorth} size="figure" state={incomplete ? 'incomplete' : 'actual'} unknownLabel="Incomplete" />
          <p class="note">As of {formatDate(p.asOfDate)}</p>
        </section>
        <section class="panel figure-card" aria-labelledby="sec">
          <p class="eyebrow" id="sec">Securities value</p>
          <Money value={p.securitiesValue} size="figure" state={p.securitiesValue === null ? 'incomplete' : 'actual'} />
          <p class="note">Manual valuations only</p>
        </section>
        <section class="panel figure-card" aria-labelledby="ret">
          <p class="eyebrow" id="ret">Investment return</p>
          <Money value={p.investmentReturn} size="figure" signed tone="direction" state={p.investmentReturn === null ? 'incomplete' : 'actual'} />
          <p class="note">Valuation change <Money value={p.valuationChange} signed /> · realized gains you entered <Money value={p.userEnteredRealizedGain} signed /></p>
        </section>
      </div>

      <section class="panel" aria-labelledby="parts">
        <div class="panel-head"><h2 id="parts">How net worth is made up</h2></div>
        <table class="data">
          <caption class="sr-only">Net worth components as of {p.asOfDate}</caption>
          <thead><tr><th scope="col">Part</th><th scope="col" class="right">Amount</th></tr></thead>
          <tbody>
            <tr><td>Cash and bank</td><td class="right"><Money value={p.liquidCash} /></td></tr>
            <tr><td>Brokerage cash</td><td class="right"><Money value={p.brokerageCash} /></td></tr>
            <tr><td>Securities (manual valuation)</td><td class="right"><Money value={p.securitiesValue} /></td></tr>
            <tr><td>Card balances (debt is negative)</td><td class="right"><Money value={p.signedCardBalances} tone={(p.signedCardBalances ?? 0) < 0 ? 'debt' : 'plain'} /></td></tr>
            <tr class="total"><th scope="row">Net worth</th><td class="right"><Money value={p.netWorth} size="lg" state={incomplete ? 'incomplete' : 'actual'} unknownLabel="Incomplete" /></td></tr>
          </tbody>
        </table>
        <p class="panel-body muted small">Contributions <Money value={p.cumulativeContributions} /> · withdrawals <Money value={p.cumulativeWithdrawals} />. Money set aside and forecasts are not part of net worth.</p>
        {#if incomplete}
          <div class="panel-body"><div class="notice"><strong>Why it is incomplete</strong><ul>
            {#each p.accounts.filter((a) => a.status !== 'complete') as a (a.accountId)}<li>{a.name}: balance not complete</li>{/each}
            {#each p.accounts.filter((a) => a.securities && a.securities.status !== 'recorded') as a (a.accountId)}<li>{a.name}: securities {SECURITIES_STATUS[a.securities!.status] ?? a.securities!.status}</li>{/each}
          </ul></div></div>
        {/if}
      </section>
    {/snippet}
  </AsyncView>

  <section class="panel" aria-labelledby="hist">
    <div class="panel-head"><h2 id="hist">Net worth by month</h2><span class="muted small">Month-end, last 12 months</span></div>
    <AsyncView state={history.current} isEmpty={(h) => h.length === 0} onretry={history.reload} label="position history">
      {#snippet empty()}<EmptyState title="No history yet" body="Month-end positions appear once accounts have balances." />{/snippet}
      {#snippet children(rows)}
        <SeriesChart
          kind="line"
          caption="Month-end net worth and cash"
          series={[{ name: 'Net worth', color: 'var(--accent)' }, { name: 'Cash and bank', color: 'var(--chart-in)' }]}
          points={rows.map((r) => ({
            key: r.asOfDate,
            label: formatMonth(r.asOfDate.slice(0, 7)).replace(/ \d{4}$/, ''),
            fullLabel: `${formatMonth(r.asOfDate.slice(0, 7))} (${formatDate(r.asOfDate)})`,
            values: [r.netWorth, r.liquidCash],
            state: r.coverage === 'complete' ? 'actual' : 'incomplete',
            note: r.coverage === 'complete' ? undefined : 'Net worth incomplete for this month',
          }))}
        />
      {/snippet}
    </AsyncView>
  </section>

  <section class="panel" aria-labelledby="val">
    <div class="panel-head">
      <h2 id="val">Securities valuations</h2>
      {#if brokers.length}
        <div class="row">
          {#if brokers.length > 1}<label><span class="sr-only">Brokerage account</span><select bind:value={selected} class="compact">{#each brokers as b (b.id)}<option value={b.id}>{b.name}</option>{/each}</select></label>{/if}
          <button class="btn small" onclick={() => open({ kind: 'holdings' })}>Holdings changed…</button>
          <button class="btn small primary" onclick={() => open({ kind: 'valuation' })}>Record valuation</button>
        </div>
      {/if}
    </div>
    {#if app.accounts !== undefined && brokers.length === 0}
      <div class="panel-body"><EmptyState title="No brokerage accounts" body="Add a brokerage cash account to record the market value of the securities held there." /></div>
    {:else}
      <AsyncView state={valuations.current} isEmpty={(v) => v.length === 0} onretry={valuations.reload} label="valuations">
        {#snippet empty()}<EmptyState title="No valuations yet" body="Record the total market value of the securities in this account (not its cash). Net worth stays incomplete until you do." />{/snippet}
        {#snippet children(list)}
          <table class="data">
            <thead><tr><th scope="col">As of</th><th scope="col" class="right">Market value</th><th scope="col">Note</th><th scope="col">Recorded</th></tr></thead>
            <tbody>
              {#each [...list].reverse() as v (v.id)}
                <tr><td>{formatDate(v.asOfDate)}</td><td class="right"><Money value={v.marketValue} /></td><td>{v.note ?? ''}</td><td class="muted">{formatInstant(v.recordedAt)}</td></tr>
              {/each}
            </tbody>
          </table>
        {/snippet}
      </AsyncView>
    {/if}
  </section>
</div>

<Dialog open={!!mode} title={mode?.kind === 'valuation' ? `Record valuation · ${app.accountName(selected)}` : `Holdings changed · ${app.accountName(selected)}`} width="500px" onclose={() => (mode = null)}>
  {#if mode}
    <form bind:this={form} class="stacked" novalidate onsubmit={save}>
      {#if mode.kind === 'valuation'}
        <div class="form-grid">
          <YenField label="Market value of securities" bind:value={amountRaw} mode="nonnegative" autofocus error={show('marketValue')} hint="Securities only; brokerage cash is tracked as an account balance" />
          <div class="field"><label class="field-label"><span>As of</span><input type="date" bind:value={date} aria-invalid={show('date') || show('asOfDate') ? 'true' : undefined} /></label>{#if show('date') ?? show('asOfDate')}<span class="error">{show('date') ?? show('asOfDate')}</span>{/if}</div>
          <div class="field wide"><label class="field-label"><span>Note <span class="muted">(optional)</span></span><input bind:value={note} maxlength="500" /></label></div>
        </div>
      {:else}
        <p class="muted small">Mark that you bought or sold securities. Earlier valuations become out of date, so net worth shows as incomplete until you record a new value.</p>
        <div class="form-grid">
          <div class="field"><label class="field-label"><span>Date</span><input type="date" bind:value={date} data-autofocus aria-invalid={show('date') || show('occurredOn') ? 'true' : undefined} /></label>{#if show('date') ?? show('occurredOn')}<span class="error">{show('date') ?? show('occurredOn')}</span>{/if}</div>
          <YenField label="Realized gain (optional)" bind:value={gainRaw} mode="signed" error={show('userEnteredRealizedGain')} hint="As you calculate it; negative for a loss" />
          <div class="field wide"><label class="field-label"><span>What changed</span><input bind:value={note} maxlength="500" aria-invalid={show('reason') ? 'true' : undefined} /></label>{#if show('reason')}<span class="error">{show('reason')}</span>{/if}</div>
        </div>
      {/if}
      <FormStatus {session} fields={['marketValue', 'date', 'asOfDate', 'occurredOn', 'reason', 'userEnteredRealizedGain']} />
      <div class="row end">
        <button type="button" class="btn" onclick={() => (mode = null)}>Cancel</button>
        <button type="submit" class="btn primary" disabled={session.submitting}>{session.submitting ? 'Saving…' : 'Save'}</button>
      </div>
    </form>
  {/if}
</Dialog>

<style>
  .inline { grid-auto-flow: column; align-items: center; gap: var(--space-2); }
  .compact { min-height: 30px; border: 1px solid var(--border-strong); border-radius: var(--radius-md); padding: 0 var(--space-2); background: var(--surface); font-size: var(--text-sm); }
  tr.total th { text-align: left; font-size: var(--text-md); text-transform: none; letter-spacing: 0; color: var(--ink); position: static; padding: var(--space-2) var(--space-3); }
</style>
