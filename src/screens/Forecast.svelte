<!--
  Forecast and available cash, both computed in Rust from an explicit as-of
  date. Projected balances are estimates, never actuals; what-if flows only
  appear when the scenario switch is on and are labelled as such.
-->
<script lang="ts">
  import { app, resource, WriteSession } from '../lib/app.svelte';
  import { createExpectedFlow, getAvailableCash, getCashForecast, listExpectedFlows, matchExpectedFlow, overrideExpectedFlow, unmatchExpectedFlow } from '../lib/finance';
  import { listFinancialEvents } from '../lib/ledger';
  import { addDays, formatDate, formatYen } from '../lib/money';
  import { focusFirstInvalid, parseFor } from '../lib/forms';
  import type { Certainty, Flow, Movement, Recurrence } from '../lib/types';
  import AsyncView from '../lib/components/AsyncView.svelte';
  import Dialog from '../lib/components/Dialog.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';
  import FormStatus from '../lib/components/FormStatus.svelte';
  import Money from '../lib/components/Money.svelte';
  import SeriesChart from '../lib/components/SeriesChart.svelte';
  import StateTag from '../lib/components/StateTag.svelte';
  import YenField from '../lib/components/YenField.svelte';

  const CERTAINTY: Record<Certainty, string> = { confirmed: 'Confirmed', estimated: 'Estimated', scenario_only: 'What-if only' };
  const SOURCE: Record<string, string> = {
    expected: 'Expected',
    actual: 'Recorded',
    card_schedule: 'Card payment',
    unspent_budget_assumption: 'Budget still to spend',
  };

  let days = $state(60);
  let scenario = $state(false);
  let accountId = $state('');
  const to = $derived(addDays(app.today, days));

  const forecast = resource(() => getCashForecast(app.today, to, scenario));
  const available = resource(() => getAvailableCash(app.today, to));
  const flows = resource(() => listExpectedFlows());

  const forecastAccounts = $derived([...new Set((forecast.current.data?.days ?? []).map((d) => d.accountId))]);
  $effect(() => {
    if (forecastAccounts.length && !forecastAccounts.includes(accountId)) accountId = forecastAccounts[0]!;
  });
  const accountDays = $derived((forecast.current.data?.days ?? []).filter((d) => d.accountId === accountId));

  type Mode = { kind: 'flow' } | { kind: 'override'; flow: Flow } | { kind: 'match'; flow: Flow; unmatch: boolean };
  let mode = $state<Mode | null>(null);
  const session = new WriteSession();
  let attempted = $state(false);
  let form: HTMLFormElement | undefined = $state();
  let flowAccount = $state('');
  let onDate = $state('');
  let direction = $state<'in' | 'out'>('out');
  let amountRaw = $state('');
  let label = $state('');
  let recurrence = $state<Recurrence>('none');
  let certainty = $state<Certainty>('estimated');
  let budgetCategoryId = $state('');
  let scheduledDate = $state('');
  let overrideDate = $state('');
  let skip = $state(false);
  let eventId = $state('');
  let candidates = $state<{ id: string; label: string }[]>([]);

  const liquid = $derived(app.activeAccounts.filter((a) => a.kind !== 'CREDIT_CARD'));
  const parsed = $derived(parseFor('positive', amountRaw));

  function open(next: Mode) {
    mode = next;
    attempted = false;
    session.reset();
    if (next.kind === 'flow') {
      flowAccount = liquid[0]?.id ?? '';
      onDate = addDays(app.today, 1);
      direction = 'out';
      amountRaw = '';
      label = '';
      recurrence = 'none';
      certainty = 'estimated';
      budgetCategoryId = '';
    } else {
      scheduledDate = next.flow.onDate;
      overrideDate = next.flow.onDate;
      skip = false;
      eventId = '';
      if (next.kind === 'match') {
        candidates = [];
        listFinancialEvents({ accountId: next.flow.accountId, page: 1, pageSize: 100 }).then((p) => {
          candidates = p.items
            .filter((e) => !e.isReversal)
            .map((e) => ({ id: e.id, label: `${e.occurredOn ? formatDate(e.occurredOn) : 'date unknown'} · ${formatYen(e.amount)} · ${e.description ?? app.categoryName(e.categoryId)}` }));
          eventId = candidates[0]?.id ?? '';
        });
      }
    }
  }

  const errors = $derived.by(() => {
    const e: Record<string, string> = {};
    if (!mode) return e;
    if (mode.kind === 'flow') {
      if (!flowAccount) e.accountId = 'Add a cash or bank account first.';
      if (!onDate) e.onDate = 'Choose the date.';
      if (!parsed.ok) e.amount = parsed.reason;
      if (!label.trim()) e.label = 'Name the flow, e.g. Rent.';
    } else {
      if (!scheduledDate) e.scheduledDate = 'Choose the scheduled occurrence.';
      if (mode.kind === 'override' && !skip && !overrideDate) e.overrideDate = 'Choose the new date, or skip it.';
      if (mode.kind === 'match' && !eventId) e.eventId = 'Choose the recorded entry.';
    }
    return e;
  });
  const show = (f: string) => session.fieldMessage(f) ?? (attempted ? errors[f] : undefined);

  async function save(ev: Event) {
    ev.preventDefault();
    if (!mode) return;
    attempted = true;
    if (Object.keys(errors).length) return focusFirstInvalid(form);
    let r: { isReplay: boolean } | null = null;
    let message = '';
    if (mode.kind === 'flow') {
      const amount = parsed.ok ? (direction === 'in' ? parsed.value : -parsed.value) : 0;
      r = await session.run(
        { accountId: flowAccount, onDate, amount, label: label.trim(), recurrence, certainty, scheduleId: null, budgetCategoryId: direction === 'out' && budgetCategoryId ? budgetCategoryId : null },
        createExpectedFlow,
      );
      message = `Expected flow added: ${label.trim()}.`;
    } else if (mode.kind === 'override') {
      r = await session.run({ flowId: mode.flow.id, scheduledDate, overrideDate: skip ? null : overrideDate, skipped: skip }, overrideExpectedFlow);
      message = skip ? 'Occurrence skipped.' : `Occurrence moved to ${formatDate(overrideDate)}.`;
    } else {
      const payload = { flowId: mode.flow.id, scheduledDate, eventId };
      r = mode.unmatch ? await session.run(payload, unmatchExpectedFlow) : await session.run(payload, matchExpectedFlow);
      message = mode.unmatch ? 'Match removed; the occurrence is expected again.' : 'Matched: the forecast now uses the recorded entry.';
    }
    if (r) {
      if (!r.isReplay) app.toast(message);
      mode = null;
    }
  }

  function movementState(m: Movement) {
    return m.source === 'actual' ? 'actual' : 'forecast';
  }
</script>

<header class="page-head">
  <div>
    <h1 tabindex="-1">Forecast</h1>
    <p class="muted">Projected balances from today’s known balances, expected flows, card payments and remaining budgets. Estimates only — not money you have.</p>
  </div>
  <div class="row">
    <div class="field inline"><label class="field-label"><span>Horizon</span>
      <select bind:value={days}>
        <option value={30}>30 days</option><option value={60}>60 days</option><option value={90}>90 days</option><option value={180}>180 days</option><option value={365}>1 year</option>
      </select></label>
    </div>
    <label class="check"><input type="checkbox" bind:checked={scenario} /> Include what-if flows</label>
  </div>
</header>

<div class="stack">
  {#if forecast.current.data && forecast.current.data.warnings.length}
    <div class="notice" role="note" data-testid="forecast-warnings">
      <strong>This forecast rests on assumptions</strong>
      <ul>{#each forecast.current.data.warnings as w (w)}<li>{w}</li>{/each}</ul>
    </div>
  {/if}
  {#if scenario}<p class="notice info">What-if flows are included. They are hypothetical and never count as income or balances.</p>{/if}

  <section class="panel" aria-labelledby="fc-chart">
    <div class="panel-head">
      <h2 id="fc-chart">Projected balance</h2>
      <div class="row">
        <StateTag state="forecast" />
        {#if forecastAccounts.length}
          <label><span class="sr-only">Account</span>
            <select bind:value={accountId} class="compact">{#each forecastAccounts as a (a)}<option value={a}>{app.accountName(a)}</option>{/each}</select>
          </label>
        {/if}
      </div>
    </div>
    <AsyncView state={forecast.current} isEmpty={(f) => f.days.length === 0} onretry={forecast.reload} label="forecast" skeletonRows={6}>
      {#snippet empty()}<EmptyState title="Nothing to project yet" body="Forecasts need cash or bank accounts with a known opening balance." />{/snippet}
      {#snippet children(f)}
        {@const low = f.lowestByAccount[accountId]}
        <SeriesChart
          kind="line"
          caption="Projected end-of-day balance for {app.accountName(accountId)}"
          series={[{ name: 'End of day', color: 'var(--chart-forecast)' }]}
          points={accountDays.map((d) => ({
            key: d.date,
            label: d.date.slice(5).replace('-', '/'),
            fullLabel: formatDate(d.date),
            values: [d.endBalance],
            state: d.coverage === 'complete' ? 'forecast' : 'incomplete',
            note: [d.shortfall ? 'Below zero' : '', d.intradayOrderUncertain ? 'Same-day order uncertain' : '', ...d.movements.map((m) => `${m.label} ${m.amount > 0 ? '+' : '−'}${Math.abs(m.amount).toLocaleString('ja-JP')}`)].filter(Boolean).join(' · ') || undefined,
          }))}
        />
        {#if low}<p class="panel-body">Lowest: <Money value={low.balance} state="forecast" tone={low.balance < 0 ? 'debt' : 'plain'} /> on {formatDate(low.date)}{#if low.balance < 0} <StateTag state="warning" label="Shortfall" />{/if}</p>{/if}
        <details class="panel-body">
          <summary>Days with movements</summary>
          <table class="data">
            <caption class="sr-only">Forecast movements for {app.accountName(accountId)}</caption>
            <thead><tr><th scope="col">Date</th><th scope="col">Movement</th><th scope="col">Kind</th><th scope="col" class="right">Amount</th><th scope="col" class="right">End of day</th></tr></thead>
            <tbody>
              {#each accountDays.filter((d) => d.movements.length || d.shortfall) as d (d.date)}
                {#each d.movements as m, i (m.id + i)}
                  <tr>
                    <td>{i === 0 ? formatDate(d.date) : ''}</td>
                    <td>{m.label}{#if m.certainty === 'scenario_only'} <StateTag state="forecast" label="What-if" />{/if}</td>
                    <td class="muted">{SOURCE[m.source] ?? m.source} · {CERTAINTY[m.certainty as Certainty] ?? m.certainty}</td>
                    <td class="right"><Money value={m.amount} signed tone="direction" state={movementState(m)} /></td>
                    <td class="right">{#if i === d.movements.length - 1}<Money value={d.endBalance} state="forecast" tone={(d.endBalance ?? 0) < 0 ? 'debt' : 'plain'} />{/if}</td>
                  </tr>
                {/each}
              {/each}
            </tbody>
          </table>
        </details>
      {/snippet}
    </AsyncView>
  </section>

  <section class="panel" aria-labelledby="av-title">
    <div class="panel-head"><h2 id="av-title">Available cash</h2><span class="muted small">Today, with commitments due by {formatDate(to)}</span></div>
    <AsyncView state={available.current} isEmpty={(a) => a.accounts.length === 0} onretry={available.reload} label="available cash">
      {#snippet empty()}<EmptyState title="No cash accounts" body="Available cash is worked out for cash, bank and brokerage-cash accounts." />{/snippet}
      {#snippet children(a)}
        <table class="data">
          <caption class="sr-only">Available cash by account</caption>
          <thead><tr><th scope="col">Account</th><th scope="col" class="right">Balance</th><th scope="col" class="right">Set aside</th><th scope="col" class="right">Due, not set aside</th><th scope="col" class="right">Spendable</th></tr></thead>
          <tbody>
            {#each a.accounts as row (row.accountId)}
              <tr>
                <td>{app.accountName(row.accountId)} {#if row.coverage !== 'complete'}<StateTag state="incomplete" />{/if}</td>
                <td class="right"><Money value={row.liquidBalance} /></td>
                <td class="right"><Money value={row.fundedEarmarks} /></td>
                <td class="right"><Money value={row.unfundedKnownDue} /></td>
                <td class="right"><Money value={row.spendable} state={row.coverage === 'complete' ? 'actual' : 'incomplete'} />{#if row.reserveShortfall}<br /><span class="small">Reserve short <Money value={row.reserveShortfall} tone="debt" /></span>{/if}</td>
              </tr>
            {/each}
          </tbody>
        </table>
        <p class="panel-body muted small">
          Remaining discretionary budget: <Money value={a.remainingDiscretionaryBudget} />. Spendable is the balance minus money set aside and known payments due before {formatDate(a.horizon)}.
          {#if a.warnings.length}<br />{a.warnings.join(' · ')}{/if}
        </p>
      {/snippet}
    </AsyncView>
  </section>

  <section class="panel" aria-labelledby="flows-title">
    <div class="panel-head"><h2 id="flows-title">Expected flows</h2><button class="btn small primary" onclick={() => open({ kind: 'flow' })}>Add expected flow</button></div>
    <AsyncView state={flows.current} isEmpty={(f) => f.length === 0} onretry={flows.reload} label="expected flows">
      {#snippet empty()}<EmptyState title="No expected flows" body="Add salary, rent and other known future amounts. They shape the forecast but never count as actual income or spending." />{/snippet}
      {#snippet children(list)}
        <table class="data">
          <thead><tr><th scope="col">Label</th><th scope="col">Account</th><th scope="col">From</th><th scope="col">Repeats</th><th scope="col">Certainty</th><th scope="col" class="right">Amount</th><th scope="col"><span class="sr-only">Actions</span></th></tr></thead>
          <tbody>
            {#each list as fl (fl.id)}
              <tr>
                <td>{fl.label}</td>
                <td>{app.accountName(fl.accountId)}</td>
                <td>{formatDate(fl.onDate)}</td>
                <td>{fl.recurrence === 'monthly' ? 'Monthly' : 'Once'}</td>
                <td><StateTag state={fl.certainty === 'confirmed' ? 'actual' : 'forecast'} label={CERTAINTY[fl.certainty]} /></td>
                <td class="right"><Money value={fl.amount} signed tone="direction" state="forecast" /></td>
                <td class="right">
                  <button class="btn ghost small" onclick={() => open({ kind: 'override', flow: fl })} aria-label="Move or skip an occurrence of {fl.label}">Move/skip…</button>
                  <button class="btn ghost small" onclick={() => open({ kind: 'match', flow: fl, unmatch: false })} aria-label="Match {fl.label} to a recorded entry">Match…</button>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/snippet}
    </AsyncView>
  </section>
</div>

<Dialog open={!!mode} title={mode?.kind === 'flow' ? 'Add expected flow' : mode?.kind === 'override' ? `Move or skip · ${mode.flow.label}` : mode ? `Match · ${mode.flow.label}` : ''} width="540px" onclose={() => (mode = null)}>
  {#if mode}
    <form bind:this={form} class="stacked" novalidate onsubmit={save}>
      {#if mode.kind === 'flow'}
        <fieldset class="plain row"><legend class="sr-only">Direction</legend>
          <label class="check"><input type="radio" bind:group={direction} value="out" /> Money out</label>
          <label class="check"><input type="radio" bind:group={direction} value="in" /> Money in</label>
        </fieldset>
        <div class="form-grid">
          <div class="field wide"><label class="field-label"><span>Label</span><input bind:value={label} maxlength="100" data-autofocus placeholder={direction === 'in' ? 'e.g. Salary' : 'e.g. Rent'} aria-invalid={show('label') ? 'true' : undefined} /></label>{#if show('label')}<span class="error">{show('label')}</span>{/if}</div>
          <YenField label="Amount" bind:value={amountRaw} error={show('amount')} />
          <div class="field"><label class="field-label"><span>{recurrence === 'monthly' ? 'First date' : 'Date'}</span><input type="date" bind:value={onDate} aria-invalid={show('onDate') ? 'true' : undefined} /></label>{#if show('onDate')}<span class="error">{show('onDate')}</span>{/if}</div>
          <div class="field"><label class="field-label"><span>Account</span><select bind:value={flowAccount} aria-invalid={show('accountId') ? 'true' : undefined}>{#each liquid as a (a.id)}<option value={a.id}>{a.name}</option>{/each}</select></label>{#if show('accountId')}<span class="error">{show('accountId')}</span>{/if}</div>
          <div class="field"><label class="field-label"><span>Repeats</span><select bind:value={recurrence}><option value="none">Once</option><option value="monthly">Monthly</option></select></label></div>
          <div class="field"><label class="field-label"><span>Certainty</span>
            <select bind:value={certainty}><option value="confirmed">Confirmed</option><option value="estimated">Estimated</option><option value="scenario_only">What-if only</option></select></label>
            <span class="hint">What-if flows appear only with the scenario switch on.</span>
          </div>
          {#if direction === 'out'}
            <div class="field"><label class="field-label"><span>Budget category <span class="muted">(optional)</span></span>
              <select bind:value={budgetCategoryId}><option value="">None</option>{#each (app.categories ?? []).filter((c) => c.type === 'expense' && c.archivedAt === null) as c (c.id)}<option value={c.id}>{c.name}</option>{/each}</select></label>
            </div>
          {/if}
        </div>
      {:else if mode.kind === 'override'}
        <p class="muted small">Changes one occurrence only. The rest of the series is unchanged.</p>
        <div class="form-grid">
          <div class="field"><label class="field-label"><span>Scheduled occurrence</span><input type="date" bind:value={scheduledDate} data-autofocus aria-invalid={show('scheduledDate') ? 'true' : undefined} /></label>{#if show('scheduledDate')}<span class="error">{show('scheduledDate')}</span>{/if}</div>
          <div class="field"><label class="field-label"><span>New date</span><input type="date" bind:value={overrideDate} disabled={skip} aria-invalid={show('overrideDate') ? 'true' : undefined} /></label>{#if show('overrideDate')}<span class="error">{show('overrideDate')}</span>{/if}</div>
          <label class="check wide"><input type="checkbox" bind:checked={skip} /> Skip this occurrence</label>
        </div>
      {:else}
        <p class="muted small">Matching tells the forecast this occurrence has happened as a recorded entry, so it is not counted twice.</p>
        <div class="form-grid">
          <div class="field"><label class="field-label"><span>Scheduled occurrence</span><input type="date" bind:value={scheduledDate} data-autofocus aria-invalid={show('scheduledDate') ? 'true' : undefined} /></label>{#if show('scheduledDate')}<span class="error">{show('scheduledDate')}</span>{/if}</div>
          <div class="field wide"><label class="field-label"><span>Recorded entry</span>
            <select bind:value={eventId} aria-invalid={show('eventId') ? 'true' : undefined}>{#each candidates as c (c.id)}<option value={c.id}>{c.label}</option>{/each}</select></label>
            {#if show('eventId')}<span class="error">{show('eventId')}</span>{/if}
          </div>
          <label class="check wide"><input type="checkbox" checked={mode.unmatch} onchange={(e) => { if (mode?.kind === 'match') mode = { ...mode, unmatch: e.currentTarget.checked }; }} /> Remove an existing match instead</label>
        </div>
      {/if}
      <FormStatus {session} fields={['accountId', 'onDate', 'amount', 'label', 'scheduledDate', 'overrideDate', 'eventId']} />
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
  .plain { border: 0; padding: 0; margin: 0; }
  details summary { cursor: pointer; color: var(--ink-2); font-size: var(--text-sm); margin-bottom: var(--space-2); }
</style>
