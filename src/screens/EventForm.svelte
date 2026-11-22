<!--
  One form for every manual event kind, used for new entries and for
  corrections (replace = reversal + new event in one Rust transaction). The
  form only collects intent; Rust validates and derives every posting.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import { app, WriteSession } from '../lib/app.svelte';
  import { replaceFinancialEvent } from '../lib/finance';
  import { createFinancialEvent, listFinancialEvents } from '../lib/ledger';
  import { formatDate, formatYen } from '../lib/money';
  import { focusFirstInvalid, parseFor } from '../lib/forms';
  import type { EventInput, EventKind, FinancialEvent } from '../lib/types';
  import FormStatus from '../lib/components/FormStatus.svelte';
  import YenField from '../lib/components/YenField.svelte';

  interface Props {
    /** Prefill from an existing event; with `replacing`, saving corrects it. */
    replacing?: FinancialEvent | null;
    presetAccountId?: string | null;
    onsaved: (event: FinancialEvent) => void;
    oncancel: () => void;
  }
  let { replacing = null, presetAccountId = null, onsaved, oncancel }: Props = $props();

  const KINDS: { value: EventKind; label: string; hint: string }[] = [
    { value: 'EXPENSE', label: 'Spending', hint: 'Money spent from an account or charged to a card.' },
    { value: 'INCOME', label: 'Income', hint: 'Money received into a cash, bank or brokerage-cash account.' },
    { value: 'TRANSFER', label: 'Transfer', hint: 'Money moved between your own accounts. Not spending.' },
    { value: 'CARD_REPAYMENT', label: 'Card payment', hint: 'Paying a card from cash. Reduces card debt; not spending.' },
    { value: 'REFUND', label: 'Refund', hint: 'Money back for an earlier purchase. Reduces that purchase’s spending.' },
    { value: 'BALANCE_ADJUSTMENT', label: 'Adjustment', hint: 'Corrects a balance to match reality. Needs a reason; not income or spending.' },
  ];

  // The form is keyed per session, so props seed it once by design.
  const source = untrack(() => replacing);
  const preset = untrack(() => presetAccountId);
  const firstPosting = source?.postings.find((p) => (source.kind === 'TRANSFER' || source.kind === 'CARD_REPAYMENT' ? p.delta < 0 : true));
  const destPosting = source?.postings.find((p) => p.delta > 0 && p.accountId !== firstPosting?.accountId);

  let kind = $state<EventKind>(source?.kind ?? 'EXPENSE');
  let amountRaw = $state(source ? String(source.amount) : '');
  let direction = $state<'up' | 'down'>(source?.kind === 'BALANCE_ADJUSTMENT' && (firstPosting?.delta ?? 0) < 0 ? 'down' : 'up');
  let description = $state(source?.description ?? '');
  let categoryId = $state(source?.categoryId ?? '');
  let accountId = $state(firstPosting?.accountId ?? preset ?? '');
  let destinationId = $state(destPosting?.accountId ?? '');
  let originalEventId = $state(source?.originalEventId ?? '');
  let occurredOn = $state(source?.occurredOn ?? app.today);
  let separatePosting = $state(!!firstPosting && firstPosting.effectiveDate !== source?.occurredOn);
  let effectiveDate = $state(firstPosting?.effectiveDate ?? app.today);
  let reason = $state('');
  let attempted = $state(false);
  const session = new WriteSession();

  const accounts = $derived(app.activeAccounts);
  const liquid = $derived(accounts.filter((a) => a.kind !== 'CREDIT_CARD'));
  const cards = $derived(accounts.filter((a) => a.kind === 'CREDIT_CARD'));
  const sourceOptions = $derived(kind === 'EXPENSE' || kind === 'REFUND' || kind === 'BALANCE_ADJUSTMENT' ? accounts : liquid);
  const destOptions = $derived(
    kind === 'CARD_REPAYMENT' ? cards : kind === 'TRANSFER' ? liquid.filter((a) => a.id !== accountId) : [],
  );
  const categoryType = $derived(kind === 'INCOME' ? 'income' : 'expense');
  const categoryOptions = $derived((app.categories ?? []).filter((c) => c.archivedAt === null && c.type === categoryType));
  const usesCategory = $derived(kind === 'INCOME' || kind === 'EXPENSE');
  const usesDestination = $derived(kind === 'TRANSFER' || kind === 'CARD_REPAYMENT');

  // Refunds link an earlier, non-reversed purchase. Rust caps the amount.
  let purchases = $state<FinancialEvent[]>([]);
  $effect(() => {
    if (kind !== 'REFUND') return;
    listFinancialEvents({ page: 1, pageSize: 200 }).then(
      (page) => {
        const reversed = new Set(page.items.map((e) => e.reversesEventId).filter(Boolean));
        purchases = page.items.filter((e) => e.kind === 'EXPENSE' && !e.isReversal && !reversed.has(e.id));
      },
      () => (purchases = []),
    );
  });

  $effect(() => {
    if (sourceOptions.length && !sourceOptions.some((a) => a.id === accountId)) accountId = sourceOptions[0]!.id;
    if (usesDestination && !destOptions.some((a) => a.id === destinationId)) destinationId = destOptions[0]?.id ?? '';
    if (usesCategory && !categoryOptions.some((c) => c.id === categoryId)) categoryId = categoryOptions[0]?.id ?? '';
  });

  const parsed = $derived(parseFor('positive', amountRaw));
  const errors = $derived.by(() => {
    const e: Record<string, string> = {};
    if (!parsed.ok) e.amount = parsed.reason;
    if (!occurredOn) e.occurredOn = 'Choose the date it happened.';
    if (separatePosting && !effectiveDate) e.effectiveDate = 'Choose the date it reached the account.';
    if (!accountId) e.accountId = 'Add an account first.';
    if (usesCategory && !categoryId) e.categoryId = `Add an ${categoryType} category first.`;
    if (usesDestination && !destinationId) e.destinationAccountId = kind === 'CARD_REPAYMENT' ? 'Add a card account first.' : 'Add a second account first.';
    if (kind === 'REFUND' && !originalEventId) e.originalEventId = 'Choose the purchase being refunded.';
    if (kind === 'BALANCE_ADJUSTMENT' && !description.trim()) e.description = 'Say why the balance is being adjusted.';
    if (replacing && !reason.trim()) e.reason = 'Say why this entry is being corrected.';
    return e;
  });
  const show = (field: string) => session.fieldMessage(field) ?? (attempted ? errors[field] : undefined);

  function input(): EventInput {
    const value = parsed.ok ? parsed.value : 0;
    return {
      kind,
      amount: value,
      accountId,
      ...(usesDestination ? { destinationAccountId: destinationId } : {}),
      ...(usesCategory ? { categoryId } : {}),
      ...(description.trim() ? { description: description.trim() } : {}),
      occurredOn,
      effectiveDate: separatePosting ? effectiveDate : occurredOn,
      ...(kind === 'REFUND' ? { originalEventId } : {}),
      ...(kind === 'BALANCE_ADJUSTMENT' ? { adjustmentDelta: direction === 'up' ? value : -value } : {}),
    };
  }

  async function submit(e?: Event) {
    e?.preventDefault();
    attempted = true;
    if (Object.keys(errors).length) {
      focusFirstInvalid(form);
      return;
    }
    const result = replacing
      ? await session.run({ eventId: replacing.id, reason: reason.trim(), replacement: input() }, replaceFinancialEvent)
      : await session.run(input(), createFinancialEvent);
    if (!result) {
      queueMicrotask(() => focusFirstInvalid(form));
      return;
    }
    const label = KINDS.find((k) => k.value === kind)!.label.toLowerCase();
    if (!result.isReplay) app.toast(`${replacing ? 'Corrected' : 'Saved'}: ${label} ${formatYen(result.result.amount)}`);
    attempted = false;
    amountRaw = '';
    description = '';
    reason = '';
    onsaved(result.result);
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) void submit(e);
  }

  let form: HTMLFormElement | undefined = $state();
  const accountLabel = $derived(
    { EXPENSE: 'Paid from', INCOME: 'Into', TRANSFER: 'From', CARD_REPAYMENT: 'Paid from', REFUND: 'Refunded to', BALANCE_ADJUSTMENT: 'Account' }[kind],
  );
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<form bind:this={form} class="stacked" novalidate onsubmit={submit} onkeydown={onKeydown} aria-busy={session.submitting}>
  {#if replacing}
    <p class="notice info">
      Correcting keeps the original entry and records a reversal plus this replacement together. Balances and reports use the
      replacement; the history shows all three.
    </p>
  {/if}

  <fieldset class="kinds">
    <legend class="sr-only">Type of entry</legend>
    {#each KINDS as k (k.value)}
      <label class:checked={kind === k.value}>
        <input type="radio" name="kind" value={k.value} bind:group={kind} />
        {k.label}
      </label>
    {/each}
  </fieldset>
  <p class="muted hint">{KINDS.find((k) => k.value === kind)?.hint}</p>

  <div class="form-grid">
    <YenField label="Amount" bind:value={amountRaw} error={show('amount')} autofocus large />

    {#if kind === 'BALANCE_ADJUSTMENT'}
      <fieldset class="field direction">
        <legend>Direction</legend>
        <label class="check"><input type="radio" bind:group={direction} value="up" /> Increase balance</label>
        <label class="check"><input type="radio" bind:group={direction} value="down" /> Decrease balance</label>
      </fieldset>
    {:else}
      <div class="field"><label class="field-label">
        <span>Date it happened</span>
        <input type="date" bind:value={occurredOn} aria-invalid={show('occurredOn') ? 'true' : undefined} aria-describedby="ef-date" /></label>
        {#if show('occurredOn')}<span id="ef-date" class="error">{show('occurredOn')}</span>{:else}<span id="ef-date" class="hint">Purchase or receipt date</span>{/if}
      </div>
    {/if}

    {#if kind === 'BALANCE_ADJUSTMENT'}
      <div class="field"><label class="field-label">
        <span>Date</span>
        <input type="date" bind:value={occurredOn} aria-invalid={show('occurredOn') ? 'true' : undefined} /></label>
        {#if show('occurredOn')}<span class="error">{show('occurredOn')}</span>{/if}
      </div>
    {/if}

    <div class="field"><label class="field-label">
      <span>{accountLabel}</span>
      <select bind:value={accountId} aria-invalid={show('accountId') ? 'true' : undefined}>
        {#each sourceOptions as a (a.id)}<option value={a.id}>{a.name}{a.kind === 'CREDIT_CARD' ? ' (card)' : ''}</option>{/each}
      </select></label>
      {#if show('accountId')}<span class="error">{show('accountId')}</span>{/if}
    </div>

    {#if usesDestination}
      <div class="field"><label class="field-label">
        <span>{kind === 'CARD_REPAYMENT' ? 'Card' : 'To'}</span>
        <select bind:value={destinationId} aria-invalid={show('destinationAccountId') ? 'true' : undefined}>
          {#each destOptions as a (a.id)}<option value={a.id}>{a.name}</option>{/each}
        </select></label>
        {#if show('destinationAccountId')}<span class="error">{show('destinationAccountId')}</span>{/if}
      </div>
    {:else if usesCategory}
      <div class="field"><label class="field-label">
        <span>Category</span>
        <select bind:value={categoryId} aria-invalid={show('categoryId') ? 'true' : undefined}>
          {#each categoryOptions as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
        </select></label>
        {#if show('categoryId')}<span class="error">{show('categoryId')}</span>{/if}
      </div>
    {:else if kind === 'REFUND'}
      <div class="field"><label class="field-label">
        <span>Original purchase</span>
        <select bind:value={originalEventId} aria-invalid={show('originalEventId') ? 'true' : undefined}>
          <option value="">Choose a purchase…</option>
          {#each purchases as p (p.id)}
            <option value={p.id}>{formatDate(p.occurredOn)} · {formatYen(p.amount)} · {p.description ?? app.categoryName(p.categoryId)}</option>
          {/each}
        </select></label>
        {#if show('originalEventId')}<span class="error">{show('originalEventId')}</span>{:else}<span class="hint">The refund uses the purchase’s category.</span>{/if}
      </div>
    {/if}

    <div class="field wide"><label class="field-label">
      <span>{kind === 'BALANCE_ADJUSTMENT' ? 'Reason for adjustment' : 'Description'} {#if kind !== 'BALANCE_ADJUSTMENT'}<span class="muted">(optional)</span>{/if}</span>
      <input bind:value={description} maxlength="500" aria-invalid={show('description') ? 'true' : undefined} /></label>
      {#if show('description')}<span class="error">{show('description')}</span>{/if}
    </div>

    {#if kind !== 'BALANCE_ADJUSTMENT'}
      <label class="check wide"><input type="checkbox" bind:checked={separatePosting} /> It reached the account on a different day</label>
      {#if separatePosting}
        <div class="field"><label class="field-label">
          <span>Date it posted</span>
          <input type="date" bind:value={effectiveDate} aria-invalid={show('effectiveDate') ? 'true' : undefined} /></label>
          {#if show('effectiveDate')}<span class="error">{show('effectiveDate')}</span>{:else}<span class="hint">Balances change on this date</span>{/if}
        </div>
      {/if}
    {/if}

    {#if replacing}
      <div class="field wide"><label class="field-label">
        <span>Why is this being corrected?</span>
        <input bind:value={reason} maxlength="500" aria-invalid={show('reason') ? 'true' : undefined} /></label>
        {#if show('reason')}<span class="error">{show('reason')}</span>{/if}
      </div>
    {/if}
  </div>

  <FormStatus {session} fields={['amount', 'occurredOn', 'effectiveDate', 'accountId', 'destinationAccountId', 'categoryId', 'originalEventId', 'description', 'reason', 'adjustmentDelta']} />

  <div class="row end">
    <button type="button" class="btn" onclick={oncancel}>Cancel</button>
    <button type="submit" class="btn primary" disabled={session.submitting} aria-keyshortcuts="Meta+Enter Control+Enter">
      {session.submitting ? 'Saving…' : session.failure ? 'Try again' : replacing ? 'Save correction' : 'Save entry'}
    </button>
  </div>
</form>

<style>
  .kinds { display: flex; flex-wrap: wrap; border: 1px solid var(--border-strong); border-radius: var(--radius-md); padding: 2px; margin: 0; gap: 2px; width: max-content; max-width: 100%; }
  .kinds label { position: relative; padding: 5px 12px; border-radius: 6px; cursor: pointer; font-weight: 560; font-size: var(--text-sm); }
  .kinds input { position: absolute; opacity: 0; inset: 0; margin: 0; cursor: pointer; }
  .kinds label.checked { background: var(--accent); color: var(--accent-ink); }
  .kinds label:has(input:focus-visible) { outline: 2px solid var(--focus); outline-offset: 2px; }
  .hint { font-size: var(--text-sm); }
  .direction { border: 0; padding: 0; margin: 0; }
  .direction legend { font-size: var(--text-sm); font-weight: 560; color: var(--ink-2); padding: 0; margin-bottom: var(--space-1); }
</style>
