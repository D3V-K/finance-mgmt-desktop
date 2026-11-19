<!--
  Account writes: create, opening balance (append-only revisions) and
  archive/restore. Each dialog states the consequence before saving.
-->
<script lang="ts" module>
  import type { Account, Balance } from '../lib/types';

  export type AccountDialog =
    | { kind: 'create' }
    | { kind: 'opening'; account: Account; balance: Balance | null }
    | { kind: 'archive'; account: Account };
</script>

<script lang="ts">
  import { app, WriteSession } from '../lib/app.svelte';
  import { archiveAccount } from '../lib/finance';
  import { createAccount, setAccountOpening } from '../lib/ledger';
  import { formatDate, formatYen } from '../lib/money';
  import { focusFirstInvalid, parseFor } from '../lib/forms';
  import type { AccountKind } from '../lib/types';
  import Dialog from '../lib/components/Dialog.svelte';
  import FormStatus from '../lib/components/FormStatus.svelte';
  import YenField from '../lib/components/YenField.svelte';
  import { ACCOUNT_KIND_LABEL } from './labels';

  interface Props {
    dialog: AccountDialog | null;
    onclose: () => void;
    oncreated?: (account: Account) => void;
  }
  let { dialog, onclose, oncreated }: Props = $props();

  let name = $state('');
  let kind = $state<AccountKind>('BANK');
  let amountRaw = $state('');
  let asOfDate = $state('');
  let reason = $state('');
  let attempted = $state(false);
  const session = new WriteSession();
  let form: HTMLFormElement | undefined = $state();

  $effect(() => {
    if (!dialog) return;
    attempted = false;
    session.reset();
    name = '';
    kind = 'BANK';
    reason = '';
    amountRaw = '';
    asOfDate = dialog.kind === 'opening' ? (dialog.balance?.openingAsOfDate ?? app.today) : app.today;
  });

  const isCard = $derived(dialog?.kind === 'opening' && dialog.account.kind === 'CREDIT_CARD');
  const parsedAmount = $derived(parseFor(isCard ? 'nonnegative' : 'signed', amountRaw));

  async function save(e: Event) {
    e.preventDefault();
    if (!dialog) return;
    attempted = true;
    if (dialog.kind === 'create') {
      if (!name.trim()) return focusFirstInvalid(form);
      const r = await session.run({ name: name.trim(), kind }, createAccount);
      if (r) {
        if (!r.isReplay) app.toast(`Added ${r.result.name}. Record its opening balance next.`);
        oncreated?.(r.result);
        onclose();
      }
    } else if (dialog.kind === 'opening') {
      if (!parsedAmount.ok || !asOfDate || !reason.trim()) return focusFirstInvalid(form);
      // Card debt is entered as a positive amount owed and stored signed (negative).
      const amount = isCard ? -parsedAmount.value : parsedAmount.value;
      const r = await session.run({ accountId: dialog.account.id, amount, asOfDate, reason: reason.trim() }, setAccountOpening);
      if (r) {
        if (!r.isReplay) app.toast(`Opening saved (revision ${r.result.revision}).`);
        onclose();
      }
    } else {
      const archived = dialog.account.archivedAt === null;
      const r = await session.run({ accountId: dialog.account.id, archived }, archiveAccount);
      if (r) {
        if (!r.isReplay) app.toast(archived ? `${dialog.account.name} archived.` : `${dialog.account.name} restored.`);
        onclose();
      }
    }
  }

  const title = $derived(
    !dialog ? '' : dialog.kind === 'create' ? 'Add account' : dialog.kind === 'opening' ? `Opening balance · ${dialog.account.name}` : dialog.account.archivedAt ? `Restore ${dialog.account.name}` : `Archive ${dialog.account.name}`,
  );
</script>

<Dialog open={!!dialog} {title} width="520px" {onclose}>
  {#if dialog}
    <form bind:this={form} class="stacked" novalidate onsubmit={save}>
      {#if dialog.kind === 'create'}
        <div class="field"><label class="field-label">
          <span>Name</span>
          <input bind:value={name} maxlength="100" data-autofocus aria-invalid={attempted && !name.trim() ? 'true' : undefined} placeholder="e.g. 三井住友 普通" /></label>
          {#if session.fieldMessage('name') ?? (attempted && !name.trim())}<span class="error">{session.fieldMessage('name') ?? 'Give the account a name.'}</span>{/if}
        </div>
        <fieldset class="field kinds">
          <legend>Type</legend>
          {#each Object.entries(ACCOUNT_KIND_LABEL) as [value, label] (value)}
            <label class="check"><input type="radio" bind:group={kind} {value} /> {label}</label>
          {/each}
        </fieldset>
        <p class="muted small">Credit cards hold debt and are never counted as cash. Amounts are JPY.</p>
      {:else if dialog.kind === 'opening'}
        <p class="notice info">
          The opening is the balance at the <strong>start</strong> of its date. Saving adds a new revision; earlier revisions stay in
          the audit history. Leave this until you know the figure — an unknown opening shows as “Unknown”, never ¥0.
        </p>
        <div class="form-grid">
          <YenField label={isCard ? 'Amount owed' : 'Balance'} bind:value={amountRaw} mode={isCard ? 'nonnegative' : 'signed'} autofocus
            error={session.fieldMessage('amount') ?? (attempted && !parsedAmount.ok ? parsedAmount.reason : undefined)}
            hint={isCard ? 'Positive amount owed on the card; 0 if nothing is owed' : undefined} />
          <div class="field"><label class="field-label">
            <span>As of the start of</span>
            <input type="date" bind:value={asOfDate} aria-invalid={session.fieldMessage('asOfDate') || (attempted && !asOfDate) ? 'true' : undefined} /></label>
            {#if session.fieldMessage('asOfDate')}<span class="error">{session.fieldMessage('asOfDate')}</span>{/if}
          </div>
          <div class="field wide"><label class="field-label">
            <span>Reason</span>
            <input bind:value={reason} maxlength="500" placeholder="e.g. Bank statement 1 Jan" aria-invalid={attempted && !reason.trim() ? 'true' : undefined} /></label>
            {#if attempted && !reason.trim()}<span class="error">Say where the figure comes from.</span>{/if}
          </div>
        </div>
        {#if dialog.balance?.openingAsOfDate}
          <p class="muted small">Current opening starts {formatDate(dialog.balance.openingAsOfDate)}.</p>
        {/if}
        {#if isCard && parsedAmount.ok}<p class="muted small">Stored as a card balance of {formatYen(-parsedAmount.value)}.</p>{/if}
      {:else}
        {#if dialog.account.archivedAt === null}
          <div class="notice">
            <strong>What archiving does</strong>
            <ul>
              <li>Hides {dialog.account.name} from entry forms; no new entries can be posted to it.</li>
              <li>Keeps its history, opening and balance. Reports and net worth still include it.</li>
              <li>You can restore it at any time.</li>
            </ul>
          </div>
        {:else}
          <p class="notice info">Restoring makes {dialog.account.name} available for new entries again. Its history is unchanged.</p>
        {/if}
      {/if}
      <FormStatus {session} fields={['name', 'amount', 'asOfDate']} />
      <div class="row end">
        <button type="button" class="btn" onclick={onclose}>Cancel</button>
        <button type="submit" class="btn primary" disabled={session.submitting} data-autofocus={dialog.kind === 'archive' ? '' : undefined}>
          {session.submitting ? 'Saving…' : dialog.kind === 'create' ? 'Add account' : dialog.kind === 'opening' ? 'Save opening' : dialog.account.archivedAt ? 'Restore' : 'Archive'}
        </button>
      </div>
    </form>
  {/if}
</Dialog>

<style>
  .kinds { border: 0; padding: 0; margin: 0; display: flex; flex-wrap: wrap; gap: var(--space-3); }
  .kinds legend { font-size: var(--text-sm); font-weight: 560; color: var(--ink-2); padding: 0; margin-bottom: var(--space-1); }
</style>
