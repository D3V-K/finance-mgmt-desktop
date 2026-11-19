<!--
  Card payment schedules, reserves and statements. A card purchase is spending
  on its purchase date; the schedule only plans when cash leaves the bank. A
  reserve earmarks cash for a payment without moving it. Confirming a
  statement replaces the estimate for the schedules it covers.
-->
<script lang="ts">
  import { app, resource, WriteSession } from '../lib/app.svelte';
  import {
    applyCardEvent,
    confirmCardStatement,
    createCardSchedule,
    fundCardReserve,
    getCardSettings,
    listCardSchedules,
    listCardStatements,
    reviseCardSchedule,
    setCardSettings,
  } from '../lib/finance';
  import { getAccountBalance, listFinancialEvents } from '../lib/ledger';
  import { formatDate, formatYen } from '../lib/money';
  import { focusFirstInvalid, parseFor } from '../lib/forms';
  import type { FinancialEvent, Id, Schedule, ScheduleSource } from '../lib/types';
  import AsyncView from '../lib/components/AsyncView.svelte';
  import Dialog from '../lib/components/Dialog.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';
  import FormStatus from '../lib/components/FormStatus.svelte';
  import Money from '../lib/components/Money.svelte';
  import StateTag from '../lib/components/StateTag.svelte';
  import YenField from '../lib/components/YenField.svelte';

  let { cardId }: { cardId: Id | null } = $props();

  type Mode =
    | { kind: 'settings' }
    | { kind: 'schedule' }
    | { kind: 'revise'; schedule: Schedule }
    | { kind: 'apply'; schedule: Schedule }
    | { kind: 'reserve'; schedule: Schedule }
    | { kind: 'statement' };

  const STATUS: Record<string, { label: string; state: 'actual' | 'forecast' | 'warning' | 'neutral' }> = {
    planned: { label: 'Estimated', state: 'forecast' },
    confirmed: { label: 'Confirmed', state: 'actual' },
    partially_paid: { label: 'Partly paid', state: 'warning' },
    paid: { label: 'Paid', state: 'neutral' },
  };

  const cards = $derived((app.accounts ?? []).filter((a) => a.kind === 'CREDIT_CARD' && a.archivedAt === null));
  const card = $derived(cards.find((c) => c.id === cardId) ?? cards[0]);
  const id = $derived(card?.id ?? null);

  const settings = resource(() => (id ? getCardSettings(id) : Promise.resolve(null)));
  const schedules = resource(() => (id ? listCardSchedules(id) : Promise.resolve([])));
  const statements = resource(() => (id ? listCardStatements(id) : Promise.resolve([])));
  const balance = resource(() => (id ? getAccountBalance(id, app.today) : Promise.resolve(null)));
  const cardEvents = resource(() => (id ? listFinancialEvents({ accountId: id, page: 1, pageSize: 200 }) : Promise.resolve(null)));

  let mode = $state<Mode | null>(null);
  const session = new WriteSession();
  let attempted = $state(false);
  let form: HTMLFormElement | undefined = $state();
  let settlementAccountId = $state('');
  let closingDay = $state('15');
  let paymentDay = $state('10');
  let basis = $state<'purchase' | 'opening'>('purchase');
  let purchaseEventId = $state('');
  let obligation = $state('');
  let dueDate = $state('');
  let amountRaw = $state('');
  let source = $state<ScheduleSource>('estimate');
  let eventId = $state('');
  let accountId = $state('');
  let asOfDate = $state('');
  let closingOn = $state('');
  let statementDue = $state('');
  let selected = $state<Id[]>([]);

  const liquid = $derived(app.activeAccounts.filter((a) => a.kind !== 'CREDIT_CARD'));
  const events = $derived(cardEvents.current.data?.items ?? []);
  const reversed = $derived(new Set(events.map((e) => e.reversesEventId).filter(Boolean)));
  const scheduledPurchases = $derived(new Set((schedules.current.data ?? []).map((s) => s.purchaseEventId).filter(Boolean)));
  const purchases = $derived(events.filter((e) => e.kind === 'EXPENSE' && !e.isReversal && !reversed.has(e.id) && !scheduledPurchases.has(e.id)));
  const unassigned = $derived((schedules.current.data ?? []).filter((s) => s.statementId === null && s.status !== 'paid'));
  const dueGroups = $derived([...new Set(unassigned.map((s) => s.dueDate))].sort());

  function applicable(s: Schedule): FinancialEvent[] {
    return events.filter(
      (e) =>
        !e.isReversal &&
        !reversed.has(e.id) &&
        (e.kind === 'CARD_REPAYMENT' || (e.kind === 'REFUND' && e.originalEventId !== null && e.originalEventId === s.purchaseEventId)) &&
        e.postings.some((p) => p.accountId === id && p.delta === e.amount),
    );
  }

  function open(next: Mode) {
    mode = next;
    attempted = false;
    session.reset();
    amountRaw = '';
    asOfDate = app.today;
    switch (next.kind) {
      case 'settings':
        settlementAccountId = settings.current.data?.settlementAccountId ?? liquid[0]?.id ?? '';
        closingDay = String(settings.current.data?.closingDay ?? 15);
        paymentDay = String(settings.current.data?.paymentDay ?? 10);
        break;
      case 'schedule':
        basis = purchases.length ? 'purchase' : 'opening';
        purchaseEventId = purchases[0]?.id ?? '';
        obligation = '';
        dueDate = app.today;
        source = 'estimate';
        amountRaw = purchases[0] ? String(purchases[0].amount) : '';
        break;
      case 'revise':
        dueDate = next.schedule.dueDate;
        amountRaw = String(next.schedule.principalDue);
        source = next.schedule.source;
        break;
      case 'apply':
        eventId = applicable(next.schedule)[0]?.id ?? '';
        amountRaw = String(next.schedule.outstanding);
        break;
      case 'reserve':
        accountId = settings.current.data?.settlementAccountId ?? liquid[0]?.id ?? '';
        amountRaw = String(next.schedule.unfunded || '');
        break;
      case 'statement':
        statementDue = dueGroups[0] ?? app.today;
        closingOn = '';
        selected = unassigned.filter((s) => s.dueDate === statementDue).map((s) => s.id);
        break;
    }
  }

  $effect(() => {
    if (mode?.kind === 'schedule' && basis === 'purchase') {
      const p = purchases.find((e) => e.id === purchaseEventId);
      if (p && !amountRaw) amountRaw = String(p.amount);
    }
  });

  const parsed = $derived(parseFor(mode?.kind === 'reserve' ? 'signed' : 'positive', amountRaw));
  const statementTotal = $derived(
    (schedules.current.data ?? []).filter((s) => selected.includes(s.id)).reduce((sum, s) => sum + s.principalDue - s.refunded, 0),
  );

  const errors = $derived.by(() => {
    const e: Record<string, string> = {};
    if (!mode) return e;
    const day = (v: string) => Number.isInteger(Number(v)) && Number(v) >= 1 && Number(v) <= 31;
    if (mode.kind === 'settings') {
      if (!settlementAccountId) e.settlementAccountId = 'Add a bank or cash account first.';
      if (!day(closingDay)) e.closingDay = 'Enter a day from 1 to 31.';
      if (!day(paymentDay)) e.paymentDay = 'Enter a day from 1 to 31.';
    }
    if (mode.kind === 'schedule') {
      if (basis === 'purchase' && !purchaseEventId) e.purchaseEventId = 'Choose a card purchase.';
      if (basis === 'opening' && !obligation.trim()) e.openingObligation = 'Describe the earlier debt, e.g. “Balance at start”.';
    }
    if ((mode.kind === 'schedule' || mode.kind === 'revise') && !dueDate) e.dueDate = 'Choose the payment due date.';
    if (mode.kind !== 'settings' && !parsed.ok) e.amount = parsed.reason;
    if (mode.kind === 'apply' && !eventId) e.eventId = 'Record the card payment or refund first, then apply it here.';
    if (mode.kind === 'reserve' && !accountId) e.accountId = 'Choose where the money is held.';
    if (mode.kind === 'statement') {
      if (!closingOn) e.closingOn = 'Enter the statement closing date.';
      else if (closingOn >= statementDue) e.closingOn = 'Closing must be before the due date.';
      if (selected.length === 0) e.scheduleIds = 'Select the payments on this statement.';
      else if (parsed.ok && parsed.value !== statementTotal) e.amount = 'Must match the selected payments.';
    }
    return e;
  });
  const show = (f: string) => session.fieldMessage(f) ?? (attempted ? errors[f] : undefined);

  async function save(ev: Event) {
    ev.preventDefault();
    if (!mode || !id) return;
    attempted = true;
    if (Object.keys(errors).length) return focusFirstInvalid(form);
    const value = parsed.ok ? parsed.value : 0;
    let r: { isReplay: boolean } | null = null;
    let message = '';
    switch (mode.kind) {
      case 'settings':
        r = await session.run({ cardId: id, settlementAccountId, closingDay: Number(closingDay), paymentDay: Number(paymentDay) }, setCardSettings);
        message = 'Card settings saved.';
        break;
      case 'schedule':
        r = await session.run(
          {
            cardId: id,
            purchaseEventId: basis === 'purchase' ? purchaseEventId : null,
            openingObligation: basis === 'opening' ? obligation.trim() : null,
            dueDate,
            principalDue: value,
            source,
          },
          createCardSchedule,
        );
        message = `Payment of ${formatYen(value)} scheduled for ${formatDate(dueDate)}.`;
        break;
      case 'revise':
        r = await session.run({ scheduleId: mode.schedule.id, dueDate, principalDue: value, source }, reviseCardSchedule);
        message = 'Schedule revised.';
        break;
      case 'apply':
        r = await session.run({ scheduleId: mode.schedule.id, eventId, amount: value }, applyCardEvent);
        message = `${formatYen(value)} applied to the ${formatDate(mode.schedule.dueDate)} payment.`;
        break;
      case 'reserve':
        r = await session.run({ scheduleId: mode.schedule.id, accountId, amount: value, asOfDate }, fundCardReserve);
        message = value > 0 ? `Set aside ${formatYen(value)} for the ${formatDate(mode.schedule.dueDate)} payment.` : `Released ${formatYen(-value)}.`;
        break;
      case 'statement':
        r = await session.run({ cardId: id, closingOn, dueDate: statementDue, principalConfirmed: value, scheduleIds: selected }, confirmCardStatement);
        message = `Statement confirmed: ${formatYen(value)} due ${formatDate(statementDue)}.`;
        break;
    }
    if (r) {
      if (!r.isReplay) app.toast(message);
      mode = null;
    }
  }

  function scheduleLabel(s: Schedule) {
    if (s.openingObligation) return s.openingObligation;
    const p = events.find((e) => e.id === s.purchaseEventId);
    return p ? `${p.description ?? app.categoryName(p.categoryId)} · ${p.occurredOn ? formatDate(p.occurredOn) : 'date unknown'}` : 'Card purchase';
  }

  const TITLES: Record<Mode['kind'], string> = {
    settings: 'Card settings',
    schedule: 'Schedule a card payment',
    revise: 'Revise payment',
    apply: 'Apply a payment or refund',
    reserve: 'Set cash aside for this payment',
    statement: 'Review and confirm statement',
  };
</script>

<header class="page-head">
  <div>
    <h1 tabindex="-1">Cards & statements</h1>
    <p class="muted">When card debt will be paid, what is already set aside for it, and statement confirmation. Purchases are spending when made; paying the card is not.</p>
  </div>
  {#if cards.length > 1}
    <div class="field"><label class="field-label"><span class="sr-only">Card</span>
      <select value={id} onchange={(e) => app.go({ name: 'cards', cardId: e.currentTarget.value })}>
        {#each cards as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
      </select></label>
    </div>
  {/if}
</header>

{#if app.accounts !== undefined && !card}
  <section class="panel"><div class="panel-body">
    <EmptyState title="No credit cards" body="Add a credit card account to plan its payments. Card debt is tracked separately from cash.">
      {#snippet action()}<button class="btn primary" onclick={() => app.go({ name: 'accounts', accountId: null })}>Go to Accounts</button>{/snippet}
    </EmptyState>
  </div></section>
{:else if card}
  <div class="stack">
    <div class="cols-3">
      <section class="panel figure-card" aria-labelledby="owed">
        <p class="eyebrow" id="owed">{card.name} · owed today {#if balance.current.data && balance.current.data.status !== 'complete'}<StateTag state="incomplete" />{/if}</p>
        <Money value={balance.current.data?.amountOwed ?? null} size="figure" tone="debt" state={balance.current.data?.status === 'complete' ? 'actual' : 'incomplete'} />
        {#if balance.current.data?.creditBalance}<p class="note">Credit balance <Money value={balance.current.data.creditBalance} /></p>{/if}
      </section>
      <section class="panel figure-card" aria-labelledby="sched-out">
        <p class="eyebrow" id="sched-out">Scheduled, not yet paid</p>
        <Money value={(schedules.current.data ?? []).reduce((s, x) => s + x.outstanding, 0)} size="figure" />
        <p class="note">Set aside <Money value={(schedules.current.data ?? []).reduce((s, x) => s + x.fundedReserve, 0)} /> of it</p>
      </section>
      <section class="panel figure-card" aria-labelledby="set">
        <p class="eyebrow" id="set">Paid from {#if settings.current.status === 'ready' && !settings.current.data}<StateTag state="warning" label="Not set" />{/if}</p>
        {#if settings.current.data}
          <strong>{app.accountName(settings.current.data.settlementAccountId)}</strong>
          <p class="note">Closes on day {settings.current.data.closingDay}, paid on day {settings.current.data.paymentDay}</p>
        {:else}
          <p class="note">Without a paying account, forecasts cannot place this card’s payments.</p>
        {/if}
        <div><button class="btn small" onclick={() => open({ kind: 'settings' })}>{settings.current.data ? 'Change…' : 'Set up…'}</button></div>
      </section>
    </div>

    <section class="panel" aria-labelledby="sch-title">
      <div class="panel-head">
        <h2 id="sch-title">Payment schedule</h2>
        <div class="row">
          <button class="btn small" onclick={() => open({ kind: 'statement' })} disabled={unassigned.length === 0}>Confirm statement…</button>
          <button class="btn small primary" onclick={() => open({ kind: 'schedule' })}>Schedule payment</button>
        </div>
      </div>
      <AsyncView state={schedules.current} isEmpty={(s) => s.length === 0} onretry={schedules.reload} label="payment schedule">
        {#snippet empty()}
          <EmptyState title="No payments scheduled" body="Schedule when each card purchase (or debt carried from before the ledger start) will be paid. Forecasts take the cash out on the due date." />
        {/snippet}
        {#snippet children(list)}
          <table class="data">
            <caption class="sr-only">Payment schedule for {card.name}</caption>
            <thead><tr><th scope="col">Due</th><th scope="col">For</th><th scope="col">Status</th><th scope="col" class="right">Due amount</th><th scope="col" class="right">Paid / refunded</th><th scope="col" class="right">Outstanding</th><th scope="col" class="right">Set aside</th><th scope="col"><span class="sr-only">Actions</span></th></tr></thead>
            <tbody>
              {#each [...list].sort((a, b) => a.dueDate.localeCompare(b.dueDate)) as s (s.id)}
                {@const st = STATUS[s.status] ?? { label: s.status, state: 'neutral' as const }}
                <tr>
                  <td>{formatDate(s.dueDate)}{#if s.dueDate < app.today && s.outstanding > 0}<br /><StateTag state="warning" label="Overdue" />{/if}</td>
                  <td>{scheduleLabel(s)}{#if s.statementId}<span class="muted small"> · on statement</span>{/if}</td>
                  <td><StateTag state={st.state} label={st.label} /></td>
                  <td class="right"><Money value={s.principalDue} state={s.source === 'estimate' ? 'forecast' : 'actual'} /></td>
                  <td class="right"><Money value={s.paid} /> / <Money value={s.refunded} /></td>
                  <td class="right"><Money value={s.outstanding} tone={s.outstanding ? 'debt' : 'plain'} /></td>
                  <td class="right"><Money value={s.fundedReserve} />{#if s.unfunded}<br /><span class="muted small">needs <Money value={s.unfunded} /></span>{/if}</td>
                  <td class="right acts">
                    {#if s.outstanding > 0}
                      <button class="btn ghost small" onclick={() => open({ kind: 'apply', schedule: s })}>Apply payment…</button>
                      <button class="btn ghost small" onclick={() => open({ kind: 'reserve', schedule: s })}>Set aside…</button>
                      {#if !s.statementId}<button class="btn ghost small" onclick={() => open({ kind: 'revise', schedule: s })}>Revise…</button>{/if}
                    {/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
          <p class="panel-body muted small">Estimated amounts (violet) are replaced when a statement is confirmed. Setting cash aside does not change any balance.</p>
        {/snippet}
      </AsyncView>
    </section>

    <section class="panel" aria-labelledby="st-title">
      <div class="panel-head"><h2 id="st-title">Confirmed statements</h2></div>
      <AsyncView state={statements.current} isEmpty={(s) => s.length === 0} onretry={statements.reload} label="statements">
        {#snippet empty()}<EmptyState title="No statements confirmed" body="When a card statement arrives, confirm it here to lock in the amount due for the schedules it covers." />{/snippet}
        {#snippet children(list)}
          <table class="data">
            <thead><tr><th scope="col">Closed</th><th scope="col">Due</th><th scope="col" class="right">Confirmed total</th><th scope="col" class="right">Schedules</th></tr></thead>
            <tbody>
              {#each list as st (st.id)}
                <tr><td>{formatDate(st.closingOn)}</td><td>{formatDate(st.dueDate)}</td><td class="right"><Money value={st.principalConfirmed} /></td><td class="right">{st.scheduleIds.length}</td></tr>
              {/each}
            </tbody>
          </table>
        {/snippet}
      </AsyncView>
    </section>
  </div>
{/if}

<Dialog open={!!mode} title={mode ? TITLES[mode.kind] : ''} width="560px" onclose={() => (mode = null)}>
  {#if mode}
    <form bind:this={form} class="stacked" novalidate onsubmit={save}>
      {#if mode.kind === 'settings'}
        <div class="form-grid">
          <div class="field wide"><label class="field-label"><span>Paid from</span>
            <select bind:value={settlementAccountId} data-autofocus aria-invalid={show('settlementAccountId') ? 'true' : undefined}>{#each liquid as a (a.id)}<option value={a.id}>{a.name}</option>{/each}</select></label>
            {#if show('settlementAccountId')}<span class="error">{show('settlementAccountId')}</span>{/if}
          </div>
          <div class="field"><label class="field-label"><span>Statement closes on day</span><input type="number" min="1" max="31" bind:value={closingDay} aria-invalid={show('closingDay') ? 'true' : undefined} /></label>{#if show('closingDay')}<span class="error">{show('closingDay')}</span>{/if}</div>
          <div class="field"><label class="field-label"><span>Payment taken on day</span><input type="number" min="1" max="31" bind:value={paymentDay} aria-invalid={show('paymentDay') ? 'true' : undefined} /></label>{#if show('paymentDay')}<span class="error">{show('paymentDay')}</span>{/if}</div>
        </div>
      {:else if mode.kind === 'schedule'}
        <fieldset class="field plain"><legend>This payment is for</legend>
          <label class="check"><input type="radio" bind:group={basis} value="purchase" disabled={purchases.length === 0} /> A card purchase{purchases.length === 0 ? ' (none unscheduled)' : ''}</label>
          <label class="check"><input type="radio" bind:group={basis} value="opening" /> Debt carried from before the ledger start</label>
        </fieldset>
        <div class="form-grid">
          {#if basis === 'purchase'}
            <div class="field wide"><label class="field-label"><span>Purchase</span>
              <select bind:value={purchaseEventId} onchange={() => { const p = purchases.find((e) => e.id === purchaseEventId); if (p) amountRaw = String(p.amount); }} aria-invalid={show('purchaseEventId') ? 'true' : undefined}>
                {#each purchases as p (p.id)}<option value={p.id}>{p.occurredOn ? formatDate(p.occurredOn) : 'Date unknown'} · {formatYen(p.amount)} · {p.description ?? app.categoryName(p.categoryId)}</option>{/each}
              </select></label>
              {#if show('purchaseEventId')}<span class="error">{show('purchaseEventId')}</span>{:else}<span class="hint">Split a purchase into instalments by scheduling several payments.</span>{/if}
            </div>
          {:else}
            <div class="field wide"><label class="field-label"><span>Description</span><input bind:value={obligation} maxlength="500" placeholder="e.g. Balance at ledger start" aria-invalid={show('openingObligation') ? 'true' : undefined} /></label>
              {#if show('openingObligation')}<span class="error">{show('openingObligation')}</span>{:else}<span class="hint">Uses the card’s opening debt; it is not new spending.</span>{/if}</div>
          {/if}
          <YenField label="Amount due" bind:value={amountRaw} error={show('amount') ?? show('principalDue')} />
          <div class="field"><label class="field-label"><span>Due date</span><input type="date" bind:value={dueDate} aria-invalid={show('dueDate') ? 'true' : undefined} /></label>{#if show('dueDate')}<span class="error">{show('dueDate')}</span>{/if}</div>
          <div class="field"><label class="field-label"><span>Amount is</span><select bind:value={source}><option value="estimate">An estimate</option><option value="actual">Confirmed</option></select></label></div>
        </div>
      {:else if mode.kind === 'revise'}
        <p class="muted small">Revising changes the plan only; recorded purchases and payments are unchanged.</p>
        <div class="form-grid">
          <YenField label="Amount due" bind:value={amountRaw} error={show('amount') ?? show('principalDue')} autofocus />
          <div class="field"><label class="field-label"><span>Due date</span><input type="date" bind:value={dueDate} aria-invalid={show('dueDate') ? 'true' : undefined} /></label>{#if show('dueDate')}<span class="error">{show('dueDate')}</span>{/if}</div>
          <div class="field"><label class="field-label"><span>Amount is</span><select bind:value={source}><option value="estimate">An estimate</option><option value="actual">Confirmed</option></select></label></div>
        </div>
      {:else if mode.kind === 'apply'}
        <p class="muted small">Record the payment itself first (New entry → Card payment), then match it to this schedule. Outstanding: {formatYen(mode.schedule.outstanding)}.</p>
        <div class="form-grid">
          <div class="field wide"><label class="field-label"><span>Payment or refund</span>
            <select bind:value={eventId} aria-invalid={show('eventId') ? 'true' : undefined}>
              {#each applicable(mode.schedule) as e (e.id)}<option value={e.id}>{e.kind === 'REFUND' ? 'Refund' : 'Payment'} · {e.occurredOn ? formatDate(e.occurredOn) : 'date unknown'} · {formatYen(e.amount)}</option>{/each}
            </select></label>
            {#if show('eventId')}<span class="error">{show('eventId')}</span>{/if}
          </div>
          <YenField label="Amount to apply" bind:value={amountRaw} error={show('amount')} />
        </div>
        {#if applicable(mode.schedule).length === 0}
          <button type="button" class="btn small" onclick={() => { mode = null; app.entryOpen = true; }}>Record a card payment</button>
        {/if}
      {:else if mode.kind === 'reserve'}
        <p class="notice info">Earmarks cash for the {formatDate(mode.schedule.dueDate)} payment. The money stays in the account and the balance does not change; it just stops being free to allocate. Use a negative amount to release.</p>
        <div class="form-grid">
          <YenField label="Amount" bind:value={amountRaw} mode="signed" error={show('amount')} autofocus />
          <div class="field"><label class="field-label"><span>Held in</span><select bind:value={accountId} aria-invalid={show('accountId') ? 'true' : undefined}>{#each liquid as a (a.id)}<option value={a.id}>{a.name}</option>{/each}</select></label>
            {#if show('accountId')}<span class="error">{show('accountId')}</span>{/if}</div>
          <div class="field"><label class="field-label"><span>As of</span><input type="date" bind:value={asOfDate} /></label></div>
        </div>
        {#if settings.current.data && accountId && accountId !== settings.current.data.settlementAccountId}
          <p class="notice">This card is paid from {app.accountName(settings.current.data.settlementAccountId)}. Money set aside elsewhere will need a transfer before the due date.</p>
        {/if}
      {:else if mode.kind === 'statement'}
        <p class="muted small">Compare with the paper or online statement. Confirming marks these payments as confirmed and replaces their estimates.</p>
        <div class="form-grid">
          <div class="field"><label class="field-label"><span>Due date on statement</span>
            <select bind:value={statementDue} onchange={() => (selected = unassigned.filter((s) => s.dueDate === statementDue).map((s) => s.id))}>
              {#each dueGroups as d (d)}<option value={d}>{formatDate(d)}</option>{/each}
            </select></label>
          </div>
          <div class="field"><label class="field-label"><span>Closing date</span><input type="date" bind:value={closingOn} max={statementDue} aria-invalid={show('closingOn') ? 'true' : undefined} /></label>{#if show('closingOn')}<span class="error">{show('closingOn')}</span>{/if}</div>
        </div>
        <fieldset class="field plain"><legend>Payments on this statement</legend>
          {#each unassigned.filter((s) => s.dueDate === statementDue) as s (s.id)}
            <label class="check"><input type="checkbox" bind:group={selected} value={s.id} /> {scheduleLabel(s)} · {formatYen(s.principalDue - s.refunded)}</label>
          {/each}
          {#if show('scheduleIds')}<span class="error">{show('scheduleIds')}</span>{/if}
        </fieldset>
        <YenField label="Total on the statement" bind:value={amountRaw} error={show('amount') ?? show('principalConfirmed')} />
        <dl class="dl review">
          <dt>Selected payments</dt><dd><Money value={statementTotal} /></dd>
          {#if parsed.ok}<dt>Statement</dt><dd><Money value={parsed.value} /></dd>
            <dt>Difference</dt><dd><Money value={parsed.value - statementTotal} signed />{#if parsed.value === statementTotal} <StateTag state="actual" label="Matches" />{/if}</dd>{/if}
        </dl>
        {#if parsed.ok && parsed.value !== statementTotal}
          <p class="notice" role="status">The statement differs from the scheduled payments. Revise the payments that differ (or select the right ones) so the totals match, then confirm.</p>
        {/if}
      {/if}
      <FormStatus {session} fields={['settlementAccountId', 'closingDay', 'paymentDay', 'purchaseEventId', 'openingObligation', 'amount', 'principalDue', 'dueDate', 'eventId', 'accountId', 'closingOn', 'scheduleIds', 'principalConfirmed']} />
      <div class="row end">
        <button type="button" class="btn" onclick={() => (mode = null)}>Cancel</button>
        <button type="submit" class="btn primary" disabled={session.submitting}>{session.submitting ? 'Saving…' : mode.kind === 'statement' ? 'Confirm statement' : 'Save'}</button>
      </div>
    </form>
  {/if}
</Dialog>

<style>
  .plain { border: 0; padding: 0; margin: 0; display: grid; gap: var(--space-1); }
  .plain legend { font-size: var(--text-sm); font-weight: 560; color: var(--ink-2); padding: 0; margin-bottom: var(--space-1); }
  .acts { white-space: nowrap; }
</style>
