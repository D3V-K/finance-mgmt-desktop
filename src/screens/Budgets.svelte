<!--
  Payday cycles, category budgets and savings. Budgets measure spending by
  purchase date within a pay cycle. Setting money aside (savings buckets, card
  reserves) reduces what is allocatable; it never changes an account balance.
-->
<script lang="ts">
  import { app, resource, WriteSession } from '../lib/app.svelte';
  import {
    fundSavingsBucket,
    getAllocationCapacity,
    getBudgetSummary,
    getFinanceSettings,
    listSavingsBuckets,
    listSavingsGoals,
    saveSavingsBucket,
    saveSavingsGoal,
    setBudgetPaymentAccount,
    setCategoryBudget,
    setFinanceSettings,
    setPaydayOverride,
  } from '../lib/finance';
  import { addDays, addMonths, formatDate, formatYen, monthStart } from '../lib/money';
  import { focusFirstInvalid, parseFor } from '../lib/forms';
  import type { BudgetResult, SavingsBucket, SavingsGoal } from '../lib/types';
  import AsyncView from '../lib/components/AsyncView.svelte';
  import Dialog from '../lib/components/Dialog.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';
  import FormStatus from '../lib/components/FormStatus.svelte';
  import Money from '../lib/components/Money.svelte';
  import StateTag from '../lib/components/StateTag.svelte';
  import YenField from '../lib/components/YenField.svelte';

  type Mode =
    | { kind: 'settings' }
    | { kind: 'override' }
    | { kind: 'budget'; existing: BudgetResult | null }
    | { kind: 'payer'; budget: BudgetResult }
    | { kind: 'goal'; existing: SavingsGoal | null }
    | { kind: 'bucket'; existing: SavingsBucket | null }
    | { kind: 'fund'; bucket: SavingsBucket }
    | { kind: 'archiveGoal'; goal: SavingsGoal }
    | { kind: 'archiveBucket'; bucket: SavingsBucket };

  let cycleDate = $state('');
  $effect(() => {
    if (!cycleDate) cycleDate = app.today;
  });

  const settings = resource(() => getFinanceSettings());
  const summary = resource(() => getBudgetSummary(cycleDate || app.today));
  const goals = resource(() => listSavingsGoals());
  const buckets = resource(() => listSavingsBuckets());
  const capacity = resource(() => getAllocationCapacity(app.today));

  let mode = $state<Mode | null>(null);
  const session = new WriteSession();
  let attempted = $state(false);
  let form: HTMLFormElement | undefined = $state();
  // Shared field state; each dialog uses the subset it needs.
  let paydayDay = $state('25');
  let defaultAccountId = $state('');
  let month = $state('');
  let payday = $state('');
  let categoryId = $state('');
  let amountRaw = $state('');
  let accountId = $state('');
  let name = $state('');
  let targetDate = $state('');
  let goalId = $state('');
  let asOfDate = $state('');

  const expenseCategories = $derived((app.categories ?? []).filter((c) => c.type === 'expense' && c.archivedAt === null));
  const liquid = $derived(app.activeAccounts.filter((a) => a.kind !== 'CREDIT_CARD'));
  const activeGoals = $derived((goals.current.data ?? []).filter((g) => g.archivedAt === null));
  const inCurrentCycle = $derived(
    !!summary.current.data && app.today >= summary.current.data.start && app.today < summary.current.data.endExclusive,
  );

  function open(next: Mode) {
    mode = next;
    attempted = false;
    session.reset();
    amountRaw = '';
    name = '';
    asOfDate = app.today;
    switch (next.kind) {
      case 'settings':
        paydayDay = String(settings.current.data?.paydayDay ?? 25);
        defaultAccountId = settings.current.data?.defaultAccountId ?? '';
        break;
      case 'override':
        month = monthStart(app.today);
        payday = app.today;
        break;
      case 'budget': {
        const used = new Set(summary.current.data?.budgets.map((b) => b.categoryId));
        categoryId = next.existing?.categoryId ?? expenseCategories.find((c) => !used.has(c.id))?.id ?? expenseCategories[0]?.id ?? '';
        amountRaw = next.existing ? String(next.existing.planned) : '';
        break;
      }
      case 'payer':
        accountId = '';
        break;
      case 'goal':
        name = next.existing?.name ?? '';
        amountRaw = next.existing ? String(next.existing.target) : '';
        targetDate = next.existing?.targetDate ?? '';
        break;
      case 'bucket':
        name = next.existing?.name ?? '';
        goalId = next.existing?.goalId ?? '';
        break;
      case 'fund':
        accountId = liquid[0]?.id ?? '';
        break;
    }
  }

  const amountMode = $derived(mode?.kind === 'budget' ? 'nonnegative' : mode?.kind === 'fund' ? 'signed' : 'positive');
  const parsed = $derived(parseFor(amountMode, amountRaw));
  const day = $derived(Number(paydayDay));

  const errors = $derived.by(() => {
    const e: Record<string, string> = {};
    if (!mode) return e;
    if (mode.kind === 'settings' && !(Number.isInteger(day) && day >= 1 && day <= 31)) e.paydayDay = 'Enter a day from 1 to 31.';
    if (mode.kind === 'override' && (!month || !payday)) e.payday = 'Choose the month and the actual payday.';
    if ((mode.kind === 'budget' || mode.kind === 'goal' || mode.kind === 'fund') && !parsed.ok) e.amount = parsed.reason;
    if (mode.kind === 'fund' && parsed.ok && parsed.value === 0) e.amount = 'Enter an amount to set aside, or a negative amount to release.';
    if (mode.kind === 'budget' && !categoryId) e.categoryId = 'Add a spending category first.';
    if ((mode.kind === 'goal' || mode.kind === 'bucket') && !name.trim()) e.name = 'Give it a name.';
    if (mode.kind === 'fund' && !accountId) e.accountId = 'Choose the account holding the money.';
    return e;
  });
  const show = (f: string) => session.fieldMessage(f) ?? (attempted ? errors[f] : undefined);

  async function save(ev: Event) {
    ev.preventDefault();
    if (!mode) return;
    attempted = true;
    if (Object.keys(errors).length) return focusFirstInvalid(form);
    const value = parsed.ok ? parsed.value : 0;
    const cycleStart = summary.current.data?.start ?? app.today;
    let r: { isReplay: boolean } | null = null;
    let message = '';
    switch (mode.kind) {
      case 'settings':
        r = await session.run(
          { timeZone: settings.current.data?.timeZone ?? app.timeZone, paydayDay: day, defaultAccountId: defaultAccountId || null },
          setFinanceSettings,
        );
        message = `Payday confirmed: day ${day} of each month.`;
        break;
      case 'override':
        r = await session.run({ month: monthStart(month), payday }, setPaydayOverride);
        message = `Payday for ${month.slice(0, 7)} set to ${formatDate(payday)}.`;
        break;
      case 'budget':
        r = await session.run({ categoryId, cycleStart, planned: value }, setCategoryBudget);
        message = `Budget set: ${app.categoryName(categoryId)} ${formatYen(value)}.`;
        break;
      case 'payer':
        r = await session.run(
          { categoryId: mode.budget.categoryId, cycleStart: mode.budget.cycleStart, accountId: accountId || null },
          setBudgetPaymentAccount,
        );
        message = accountId
          ? `Forecasts will draw ${mode.budget.categoryName} from ${app.accountName(accountId)}.`
          : `${mode.budget.categoryName} no longer assumes a paying account.`;
        break;
      case 'goal':
        r = await session.run(
          { ...(mode.existing ? { goalId: mode.existing.id } : {}), name: name.trim(), target: value, targetDate: targetDate || null },
          saveSavingsGoal,
        );
        message = `Goal saved: ${name.trim()}.`;
        break;
      case 'bucket':
        r = await session.run(
          { ...(mode.existing ? { bucketId: mode.existing.id } : {}), name: name.trim(), goalId: goalId || null },
          saveSavingsBucket,
        );
        message = `Savings bucket saved: ${name.trim()}.`;
        break;
      case 'fund':
        r = await session.run({ bucketId: mode.bucket.id, accountId, amount: value, asOfDate }, fundSavingsBucket);
        message = value > 0 ? `Set aside ${formatYen(value)} for ${mode.bucket.name}.` : `Released ${formatYen(-value)} from ${mode.bucket.name}.`;
        break;
      case 'archiveGoal':
        r = await session.run(
          { goalId: mode.goal.id, name: mode.goal.name, target: mode.goal.target, targetDate: mode.goal.targetDate, archive: true },
          saveSavingsGoal,
        );
        message = `${mode.goal.name} archived.`;
        break;
      case 'archiveBucket':
        r = await session.run(
          { bucketId: mode.bucket.id, name: mode.bucket.name, goalId: mode.bucket.goalId, archive: true },
          saveSavingsBucket,
        );
        message = `${mode.bucket.name} archived.`;
        break;
    }
    if (r) {
      if (!r.isReplay) app.toast(message);
      mode = null;
    }
  }

  function goalFunded(goal: SavingsGoal) {
    return (buckets.current.data ?? []).filter((b) => b.goalId === goal.id && b.archivedAt === null).reduce((s, b) => s + b.funded, 0);
  }

  const TITLES: Record<Mode['kind'], string> = {
    settings: 'Payday and defaults',
    override: 'Move one month’s payday',
    budget: 'Category budget',
    payer: 'Paying account',
    goal: 'Savings goal',
    bucket: 'Savings bucket',
    fund: 'Set money aside',
    archiveGoal: 'Archive goal',
    archiveBucket: 'Archive bucket',
  };
</script>

<header class="page-head">
  <div>
    <h1 tabindex="-1">Budgets & savings</h1>
    <p class="muted">Plan spending per pay cycle and set money aside for goals. Setting money aside never moves it — balances stay the same.</p>
  </div>
  <button class="btn" onclick={() => open({ kind: 'settings' })}>Payday settings</button>
</header>

<div class="stack">
  <section class="panel" aria-labelledby="cycle-title">
    <div class="panel-head">
      <div>
        <h2 id="cycle-title">Pay cycle budget</h2>
        {#if summary.current.data}
          <p class="muted small">
            {formatDate(summary.current.data.start)} – {formatDate(addDays(summary.current.data.endExclusive, -1))}
            {#if settings.current.data} · payday day {settings.current.data.paydayDay}{/if}
          </p>
        {/if}
      </div>
      <div class="row">
        <button class="btn small" onclick={() => (cycleDate = addMonths(summary.current.data?.start ?? cycleDate, -1))}>← Previous cycle</button>
        <button class="btn small" onclick={() => (cycleDate = app.today)} disabled={inCurrentCycle}>Current</button>
        <button class="btn small" onclick={() => (cycleDate = summary.current.data?.endExclusive ?? cycleDate)}>Next cycle →</button>
        <button class="btn small" onclick={() => open({ kind: 'override' })}>Move a payday…</button>
        <button class="btn small primary" onclick={() => open({ kind: 'budget', existing: null })}>Add budget</button>
      </div>
    </div>
    <AsyncView state={summary.current} isEmpty={(s) => s.budgets.length === 0} onretry={summary.reload} label="budget">
      {#snippet empty()}
        <EmptyState title="No budgets for this cycle" body="Add a planned amount for any spending category. Card purchases count on their purchase date; refunds reduce the category.">
          {#snippet action()}<button class="btn primary" onclick={() => open({ kind: 'budget', existing: null })}>Add budget</button>{/snippet}
        </EmptyState>
      {/snippet}
      {#snippet children(s)}
        {#if s.coverage !== 'complete'}
          <p class="panel-body notice">
            {s.unknownPurchaseDateCount} purchase{s.unknownPurchaseDateCount === 1 ? '' : 's'} with an unknown purchase date cannot be placed in any cycle, so spending here may be understated.
          </p>
        {/if}
        <table class="data">
          <caption class="sr-only">Budgets for the cycle starting {s.start}</caption>
          <thead>
            <tr><th scope="col">Category</th><th scope="col" class="right">Planned</th><th scope="col" class="right">Spent</th><th scope="col" class="right">Refunds</th><th scope="col" class="right">Left</th><th scope="col"><span class="sr-only">Actions</span></th></tr>
          </thead>
          <tbody>
            {#each s.budgets as b (b.id)}
              <tr>
                <td>{b.categoryName}</td>
                <td class="right"><Money value={b.planned} /></td>
                <td class="right"><Money value={b.grossSpending} /></td>
                <td class="right"><Money value={b.refunds} /></td>
                <td class="right"><Money value={b.remaining} tone={b.remaining < 0 ? 'debt' : 'plain'} />{#if b.remaining < 0}<span class="muted small"> over</span>{/if}</td>
                <td class="right">
                  <button class="btn ghost small" onclick={() => open({ kind: 'budget', existing: b })} aria-label="Change {b.categoryName} budget">Change</button>
                  <button class="btn ghost small" onclick={() => open({ kind: 'payer', budget: b })} aria-label="Paying account for {b.categoryName}">Paid from…</button>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/snippet}
    </AsyncView>
  </section>

  <div class="cols-2">
    <section class="panel" aria-labelledby="goals-title">
      <div class="panel-head"><h2 id="goals-title">Savings goals</h2><button class="btn small" onclick={() => open({ kind: 'goal', existing: null })}>Add goal</button></div>
      <AsyncView state={goals.current} isEmpty={(g) => g.every((x) => x.archivedAt !== null)} onretry={goals.reload} label="goals">
        {#snippet empty()}<EmptyState title="No goals yet" body="A goal is a target amount, optionally by a date. Link savings buckets to it to track progress." />{/snippet}
        {#snippet children(list)}
          <ul class="list-plain">
            {#each list.filter((g) => g.archivedAt === null) as g (g.id)}
              {@const funded = goalFunded(g)}
              <li class="goal">
                <div class="row"><strong>{g.name}</strong>{#if g.targetDate}<span class="muted small">by {formatDate(g.targetDate)}</span>{/if}</div>
                <progress max={g.target} value={Math.min(funded, g.target)} aria-label="{g.name} progress"></progress>
                <div class="row">
                  <span><Money value={funded} /> <span class="muted small">of</span> <Money value={g.target} /></span>
                  <span class="row end grow">
                    <button class="btn ghost small" onclick={() => open({ kind: 'goal', existing: g })} aria-label="Edit {g.name}">Edit</button>
                    <button class="btn ghost small" onclick={() => open({ kind: 'archiveGoal', goal: g })} aria-label="Archive {g.name}">Archive</button>
                  </span>
                </div>
              </li>
            {/each}
          </ul>
        {/snippet}
      </AsyncView>
    </section>

    <section class="panel" aria-labelledby="buckets-title">
      <div class="panel-head"><h2 id="buckets-title">Savings buckets</h2><button class="btn small" onclick={() => open({ kind: 'bucket', existing: null })}>Add bucket</button></div>
      <AsyncView state={buckets.current} isEmpty={(b) => b.every((x) => x.archivedAt !== null)} onretry={buckets.reload} label="buckets">
        {#snippet empty()}<EmptyState title="No buckets yet" body="A bucket earmarks money in an account for a purpose. Funding it lowers what you can allocate elsewhere, not your balance." />{/snippet}
        {#snippet children(list)}
          <table class="data">
            <thead><tr><th scope="col">Bucket</th><th scope="col" class="right">Set aside</th><th scope="col"><span class="sr-only">Actions</span></th></tr></thead>
            <tbody>
              {#each list.filter((b) => b.archivedAt === null) as b (b.id)}
                <tr>
                  <td>{b.name}{#if b.goalId}<span class="muted small"> · {(goals.current.data ?? []).find((g) => g.id === b.goalId)?.name ?? 'goal'}</span>{/if}</td>
                  <td class="right"><Money value={b.funded} /></td>
                  <td class="right">
                    <button class="btn small" onclick={() => open({ kind: 'fund', bucket: b })} aria-label="Set money aside for {b.name}">Set aside…</button>
                    <button class="btn ghost small" onclick={() => open({ kind: 'bucket', existing: b })} aria-label="Edit {b.name}">Edit</button>
                    <button class="btn ghost small" onclick={() => open({ kind: 'archiveBucket', bucket: b })} aria-label="Archive {b.name}">Archive</button>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/snippet}
      </AsyncView>
    </section>
  </div>

  <section class="panel" aria-labelledby="cap-title">
    <div class="panel-head"><h2 id="cap-title">Free to allocate</h2><span class="muted small">Today</span></div>
    <AsyncView state={capacity.current} isEmpty={(c) => c.length === 0} onretry={capacity.reload} label="allocation">
      {#snippet empty()}<EmptyState title="No cash accounts" body="Savings and card reserves are set aside from cash or bank accounts." />{/snippet}
      {#snippet children(rows)}
        <table class="data">
          <caption class="sr-only">Allocation capacity by account</caption>
          <thead><tr><th scope="col">Account</th><th scope="col" class="right">Balance</th><th scope="col" class="right">Set aside</th><th scope="col" class="right">Free to allocate</th></tr></thead>
          <tbody>
            {#each rows as c (c.balance.accountId)}
              <tr>
                <td>{app.accountName(c.balance.accountId)} {#if c.balance.status !== 'complete'}<StateTag state="incomplete" />{/if}</td>
                <td class="right"><Money value={c.balance.balance} /></td>
                <td class="right"><Money value={c.funded} /></td>
                <td class="right">
                  <Money value={c.allocatable} />
                  {#if c.shortfall}<br /><span class="small">Short by <Money value={c.shortfall} tone="debt" /></span>{/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
        <p class="panel-body muted small">Savings buckets and card reserves draw on the same free amount. Balances are unchanged by setting money aside.</p>
      {/snippet}
    </AsyncView>
  </section>
</div>

<Dialog open={!!mode} title={mode ? TITLES[mode.kind] : ''} width="520px" onclose={() => (mode = null)}>
  {#if mode}
    <form bind:this={form} class="stacked" novalidate onsubmit={save}>
      {#if mode.kind === 'settings'}
        <p class="notice info">
          Saving confirms these settings. Until then, forecasts and available cash say the payday is unconfirmed. Changing the payday
          moves this month’s budgets to the new cycle.
        </p>
        <div class="form-grid">
          <div class="field"><label class="field-label"><span>Payday (day of month)</span>
            <input type="number" min="1" max="31" bind:value={paydayDay} data-autofocus aria-invalid={show('paydayDay') ? 'true' : undefined} /></label>
            {#if show('paydayDay')}<span class="error">{show('paydayDay')}</span>{:else}<span class="hint">Short months use their last day</span>{/if}
          </div>
          <div class="field"><label class="field-label"><span>Default account</span>
            <select bind:value={defaultAccountId}><option value="">None</option>{#each liquid as a (a.id)}<option value={a.id}>{a.name}</option>{/each}</select></label>
          </div>
        </div>
      {:else if mode.kind === 'override'}
        <p class="muted small">Use this when one payday falls on a different date, e.g. before a holiday. Other months keep the usual day.</p>
        <div class="form-grid">
          <div class="field"><label class="field-label"><span>Month</span><input type="month" value={month.slice(0, 7)} onchange={(e) => (month = `${e.currentTarget.value}-01`)} data-autofocus /></label></div>
          <div class="field"><label class="field-label"><span>Actual payday</span><input type="date" bind:value={payday} aria-invalid={show('payday') ? 'true' : undefined} /></label>
            {#if show('payday')}<span class="error">{show('payday')}</span>{/if}</div>
        </div>
      {:else if mode.kind === 'budget'}
        <div class="form-grid">
          <div class="field"><label class="field-label"><span>Category</span>
            <select bind:value={categoryId} disabled={!!mode.existing} aria-invalid={show('categoryId') ? 'true' : undefined}>
              {#each expenseCategories as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
            </select></label>
            {#if show('categoryId')}<span class="error">{show('categoryId')}</span>{/if}
          </div>
          <YenField label="Planned for this cycle" bind:value={amountRaw} mode="nonnegative" error={show('amount') ?? show('planned')} autofocus />
        </div>
        <p class="muted small">Applies to the cycle starting {formatDate(summary.current.data?.start ?? app.today)}.</p>
      {:else if mode.kind === 'payer'}
        <p class="muted small">Forecasts assume {mode.budget.categoryName} spending in this cycle is paid from this account. Choose “No assumption” to leave it out.</p>
        <div class="field"><label class="field-label"><span>Paid from</span>
          <select bind:value={accountId} data-autofocus><option value="">No assumption</option>{#each app.activeAccounts as a (a.id)}<option value={a.id}>{a.name}</option>{/each}</select></label>
          {#if show('accountId')}<span class="error">{show('accountId')}</span>{/if}
        </div>
      {:else if mode.kind === 'goal'}
        <div class="form-grid">
          <div class="field wide"><label class="field-label"><span>Name</span><input bind:value={name} maxlength="100" data-autofocus aria-invalid={show('name') ? 'true' : undefined} /></label>
            {#if show('name')}<span class="error">{show('name')}</span>{/if}</div>
          <YenField label="Target" bind:value={amountRaw} error={show('amount') ?? show('target')} />
          <div class="field"><label class="field-label"><span>Target date <span class="muted">(optional)</span></span><input type="date" bind:value={targetDate} /></label></div>
        </div>
      {:else if mode.kind === 'bucket'}
        <div class="field"><label class="field-label"><span>Name</span><input bind:value={name} maxlength="100" data-autofocus aria-invalid={show('name') ? 'true' : undefined} /></label>
          {#if show('name')}<span class="error">{show('name')}</span>{/if}</div>
        <div class="field"><label class="field-label"><span>Counts toward goal <span class="muted">(optional)</span></span>
          <select bind:value={goalId}><option value="">No goal</option>{#each activeGoals as g (g.id)}<option value={g.id}>{g.name}</option>{/each}</select></label>
        </div>
      {:else if mode.kind === 'fund'}
        <p class="notice info">
          This earmarks money for <strong>{mode.bucket.name}</strong>. It stays in the account and the balance does not change; it just stops
          being free to allocate. Use a negative amount to release money.
        </p>
        <div class="form-grid">
          <YenField label="Amount" bind:value={amountRaw} mode="signed" error={show('amount')} autofocus />
          <div class="field"><label class="field-label"><span>Held in</span>
            <select bind:value={accountId} aria-invalid={show('accountId') ? 'true' : undefined}>{#each liquid as a (a.id)}<option value={a.id}>{a.name}</option>{/each}</select></label>
            {#if show('accountId')}<span class="error">{show('accountId')}</span>{/if}
          </div>
          <div class="field"><label class="field-label"><span>As of</span><input type="date" bind:value={asOfDate} aria-invalid={show('asOfDate') ? 'true' : undefined} /></label>
            {#if show('asOfDate')}<span class="error">{show('asOfDate')}</span>{/if}</div>
        </div>
      {:else if mode.kind === 'archiveGoal'}
        <p class="notice">Archiving hides {mode.goal.name}. Buckets linked to it keep their money set aside until you release it.</p>
      {:else if mode.kind === 'archiveBucket'}
        <p class="notice">Archiving hides {mode.bucket.name}. If money is still set aside, release it first (set aside a negative amount) or it stays earmarked.</p>
      {/if}
      <FormStatus {session} fields={['paydayDay', 'payday', 'amount', 'planned', 'target', 'categoryId', 'name', 'accountId', 'asOfDate']} />
      <div class="row end">
        <button type="button" class="btn" onclick={() => (mode = null)}>Cancel</button>
        <button type="submit" class="btn primary" disabled={session.submitting} data-autofocus={mode.kind.startsWith('archive') ? '' : undefined}>
          {session.submitting ? 'Saving…' : mode.kind.startsWith('archive') ? 'Archive' : mode.kind === 'settings' ? 'Confirm settings' : 'Save'}
        </button>
      </div>
    </form>
  {/if}
</Dialog>

<style>
  .goal { display: grid; gap: var(--space-1); padding: var(--space-3) var(--space-5); border-bottom: 1px solid var(--border); }
  .goal:last-child { border-bottom: 0; }
  progress { width: 100%; height: 8px; accent-color: var(--chart-in); }
  .grow { flex: 1; }
</style>
