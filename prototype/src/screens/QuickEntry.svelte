<script lang="ts">
  import { app, resource } from '../lib/app.svelte';
  import { CommandFailure, commands, type NewEntryInput } from '../lib/commands';
  import { categories, TODAY } from '../lib/fixtures';
  import { formatYen, parseYenInput } from '../lib/money';
  import Dialog from '../lib/components/Dialog.svelte';

  let { open, onclose }: { open: boolean; onclose: () => void } = $props();

  type Kind = NewEntryInput['kind'];
  const KINDS: Array<{ value: Kind; label: string; hint: string }> = [
    { value: 'EXPENSE', label: 'Spending', hint: 'Money spent from an account or card' },
    { value: 'INCOME', label: 'Income', hint: 'Money received into an account' },
    { value: 'TRANSFER', label: 'Transfer', hint: 'Money moved between your own accounts — not spending' }
  ];

  const accounts = resource(() => commands.listAccounts());

  let kind = $state<Kind>('EXPENSE');
  let amountRaw = $state('');
  let description = $state('');
  let category = $state('Groceries');
  let accountId = $state('acc-everyday');
  let toAccountId = $state('acc-savings');
  let occurredOn = $state(TODAY);
  let keepOpen = $state(false);

  let idempotencyKey = $state(crypto.randomUUID());
  let submitting = $state(false);
  let attempted = $state(false);
  let failure = $state<string | null>(null);
  let serverField = $state<{ field: string; message: string } | null>(null);
  let amountInput: HTMLInputElement | undefined = $state();

  // Cards can be spent from but income and transfers go through cash accounts;
  // card repayment is its own flow (#9).
  const fromOptions = $derived((accounts.current.data ?? []).filter((a) => kind === 'EXPENSE' || a.kind !== 'CREDIT_CARD'));
  const toOptions = $derived((accounts.current.data ?? []).filter((a) => a.kind !== 'CREDIT_CARD' && a.id !== accountId));

  const parsed = $derived(parseYenInput(amountRaw));
  const errors = $derived.by(() => {
    const e: Record<string, string> = {};
    if (!parsed.ok) e.amount = parsed.reason;
    if (kind !== 'TRANSFER' && description.trim() === '') e.description = 'Add a short description.';
    if (!occurredOn) e.occurredOn = 'Choose a date.';
    if (serverField) e[serverField.field] = serverField.message;
    return e;
  });
  const show = (field: string) => (attempted || serverField?.field === field) && errors[field];

  // Opening the dialog starts a fresh form session.
  $effect(() => {
    if (open) {
      failure = null;
      attempted = false;
      serverField = null;
    }
  });

  $effect(() => {
    if (!toOptions.some((a) => a.id === toAccountId)) toAccountId = toOptions[0]?.id ?? '';
    if (fromOptions.length && !fromOptions.some((a) => a.id === accountId)) accountId = fromOptions[0]!.id;
  });

  function resetForm() {
    amountRaw = '';
    description = '';
    idempotencyKey = crypto.randomUUID();
    attempted = false;
  }

  async function submit() {
    if (submitting) return;
    attempted = true;
    serverField = null;
    if (Object.keys(errors).length || !parsed.ok) {
      document.querySelector<HTMLElement>('dialog [aria-invalid="true"]')?.focus();
      return;
    }
    submitting = true;
    failure = null;
    try {
      const { entry, replayed } = await commands.createEntry({
        idempotencyKey,
        kind,
        amount: parsed.value,
        description: description.trim() || 'Transfer',
        category: kind === 'TRANSFER' ? null : category,
        occurredOn,
        accountId,
        toAccountId: kind === 'TRANSFER' ? toAccountId : undefined
      });
      app.revision += 1;
      app.toast(`${replayed ? 'Already saved' : 'Saved'}: ${KINDS.find((k) => k.value === kind)!.label.toLowerCase()} ${formatYen(parsed.value)} — ${entry.description}`);
      resetForm();
      if (keepOpen) amountInput?.focus();
      else onclose();
    } catch (err) {
      if (err instanceof CommandFailure && err.error.field) {
        serverField = { field: err.error.field, message: err.error.message };
      } else {
        failure = err instanceof CommandFailure ? err.error.message : 'Something went wrong.';
      }
    } finally {
      submitting = false;
    }
  }

  function onFormKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      submit();
    }
  }
</script>

<Dialog {open} title="New entry" description="Recorded to this device’s ledger. Amounts are whole yen." {onclose}>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <form novalidate onsubmit={(e) => { e.preventDefault(); submit(); }} onkeydown={onFormKeydown} aria-busy={submitting}>
    <fieldset class="kinds">
      <legend class="sr-only">Type of entry</legend>
      {#each KINDS as k (k.value)}
        <label class:checked={kind === k.value}>
          <input type="radio" name="kind" value={k.value} bind:group={kind} />
          {k.label}
        </label>
      {/each}
    </fieldset>
    <p class="hint muted">{KINDS.find((k) => k.value === kind)?.hint}</p>

    <div class="grid">
      <label class="field amount">
        <span>Amount</span>
        <span class="yen-input">
          <span aria-hidden="true">¥</span>
          <input
            bind:this={amountInput}
            bind:value={amountRaw}
            inputmode="numeric"
            autocomplete="off"
            data-autofocus
            placeholder="0"
            aria-invalid={show('amount') ? 'true' : undefined}
            aria-describedby="amt-msg"
          />
        </span>
        {#if show('amount')}
          <span id="amt-msg" class="error">{errors.amount}</span>
        {:else}
          <span id="amt-msg" class="hint num">{parsed.ok ? formatYen(parsed.value) : 'Whole yen, e.g. 1,200'}</span>
        {/if}
      </label>

      <label class="field">
        <span>Date</span>
        <input type="date" bind:value={occurredOn} max={TODAY} aria-invalid={show('occurredOn') ? 'true' : undefined} aria-describedby="date-msg" />
        {#if show('occurredOn')}<span id="date-msg" class="error">{errors.occurredOn}</span>{:else}<span id="date-msg" class="hint">When it happened</span>{/if}
      </label>

      <label class="field wide">
        <span>Description {#if kind === 'TRANSFER'}<span class="muted">(optional)</span>{/if}</span>
        <input bind:value={description} placeholder={kind === 'INCOME' ? 'e.g. Salary' : kind === 'TRANSFER' ? 'e.g. Monthly savings' : 'e.g. Supermarket'} aria-invalid={show('description') ? 'true' : undefined} aria-describedby="desc-msg" />
        {#if show('description')}<span id="desc-msg" class="error">{errors.description}</span>{/if}
      </label>

      <label class="field">
        <span>{kind === 'TRANSFER' ? 'From' : kind === 'INCOME' ? 'Into' : 'Paid from'}</span>
        <select bind:value={accountId}>
          {#each fromOptions as a (a.id)}<option value={a.id}>{a.name}</option>{/each}
        </select>
      </label>

      {#if kind === 'TRANSFER'}
        <label class="field">
          <span>To</span>
          <select bind:value={toAccountId} aria-invalid={show('toAccountId') ? 'true' : undefined} aria-describedby="to-msg">
            {#each toOptions as a (a.id)}<option value={a.id}>{a.name}</option>{/each}
          </select>
          {#if show('toAccountId')}<span id="to-msg" class="error">{errors.toAccountId}</span>{/if}
        </label>
      {:else}
        <label class="field">
          <span>Category</span>
          <select bind:value={category}>
            {#each categories as c (c)}<option>{c}</option>{/each}
          </select>
        </label>
      {/if}
    </div>

    {#if failure}
      <div class="failure" role="alert">
        <strong>Nothing was saved.</strong> {failure} Your entry is still here — try again when ready.
      </div>
    {/if}

    <div class="foot">
      <label class="keep"><input type="checkbox" bind:checked={keepOpen} /> Add another after saving</label>
      <div class="buttons">
        <button type="button" class="btn" onclick={onclose}>Cancel</button>
        <button type="submit" class="btn primary" disabled={submitting} aria-keyshortcuts="Meta+Enter Control+Enter">
          {submitting ? 'Saving…' : failure ? 'Try again' : 'Save entry'}
        </button>
      </div>
    </div>
  </form>
</Dialog>

<style>
  form { display: grid; gap: var(--space-3); }
  .kinds { display: inline-flex; border: 1px solid var(--border-strong); border-radius: var(--radius-md); padding: 2px; margin: 0; gap: 2px; width: max-content; }
  .kinds label { position: relative; padding: 5px 14px; border-radius: 6px; cursor: pointer; font-weight: 560; font-size: var(--text-sm); }
  .kinds input { position: absolute; opacity: 0; inset: 0; margin: 0; cursor: pointer; }
  .kinds label.checked { background: var(--accent); color: var(--accent-ink); }
  .kinds label:has(input:focus-visible) { outline: 2px solid var(--focus); outline-offset: 2px; }
  .hint { font-size: var(--text-sm); }
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: var(--space-3) var(--space-4); }
  .wide { grid-column: 1 / -1; }
  .yen-input { display: flex; align-items: center; border: 1px solid var(--border-strong); border-radius: var(--radius-md); background: var(--surface); padding-left: var(--space-3); }
  .yen-input:has(input[aria-invalid='true']) { border-color: var(--state-error); box-shadow: 0 0 0 1px var(--state-error); }
  .yen-input:focus-within { outline: 2px solid var(--focus); outline-offset: 2px; }
  .field .yen-input input { border: 0; flex: 1; min-width: 0; font-size: var(--text-lg); font-variant-numeric: tabular-nums; outline: none; box-shadow: none; }
  .failure { padding: var(--space-3) var(--space-4); border-radius: var(--radius-md); background: var(--state-error-bg); color: var(--ink); border-left: 3px solid var(--state-error); font-size: var(--text-sm); }
  .foot { display: flex; justify-content: space-between; align-items: center; gap: var(--space-3); padding-top: var(--space-2); }
  .keep { display: flex; align-items: center; gap: var(--space-2); font-size: var(--text-sm); color: var(--ink-2); }
  .buttons { display: flex; gap: var(--space-2); }
</style>
