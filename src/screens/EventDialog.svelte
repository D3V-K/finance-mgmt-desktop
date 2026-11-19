<!--
  Event detail with its postings and audit links. Reversal and correction are
  explicit, explained actions; nothing is edited or deleted in place.
-->
<script lang="ts">
  import { app, WriteSession } from '../lib/app.svelte';
  import { reverseFinancialEvent } from '../lib/finance';
  import { formatDate, formatInstant } from '../lib/money';
  import type { FinancialEvent } from '../lib/types';
  import Dialog from '../lib/components/Dialog.svelte';
  import FormStatus from '../lib/components/FormStatus.svelte';
  import Money from '../lib/components/Money.svelte';
  import StateTag from '../lib/components/StateTag.svelte';
  import EventForm from './EventForm.svelte';
  import { KIND_LABEL, DATE_BASIS_LABEL } from './labels';

  interface Props {
    event: FinancialEvent | null;
    /** IDs known to have been reversed (from the loaded history). */
    reversedIds: Set<string>;
    onclose: () => void;
  }
  let { event, reversedIds, onclose }: Props = $props();

  let mode = $state<'view' | 'reverse' | 'correct'>('view');
  let reason = $state('');
  let attempted = $state(false);
  const session = new WriteSession();

  $effect(() => {
    if (event) {
      mode = 'view';
      reason = '';
      attempted = false;
      session.reset();
    }
  });

  const isReversed = $derived(!!event && reversedIds.has(event.id));
  const canChange = $derived(!!event && !event.isReversal && !isReversed);

  async function reverse(e: Event) {
    e.preventDefault();
    attempted = true;
    if (!event || !reason.trim()) return;
    const result = await session.run({ eventId: event.id, reason: reason.trim() }, reverseFinancialEvent);
    if (result) {
      if (!result.isReplay) app.toast('Entry reversed. The original stays in history.');
      onclose();
    }
  }
</script>

<Dialog open={!!event} title={mode === 'correct' ? 'Correct entry' : mode === 'reverse' ? 'Reverse entry' : 'Entry details'} width="640px" {onclose}>
  {#if event}
    {#if mode === 'correct'}
      <EventForm replacing={event} onsaved={onclose} oncancel={() => (mode = 'view')} />
    {:else}
      <div class="head">
        <Money value={event.amount} size="lg" />
        <StateTag state="neutral" label={KIND_LABEL[event.kind]} />
        {#if event.isReversal}<StateTag state="warning" label="Reversal" />{/if}
        {#if isReversed}<StateTag state="warning" label="Reversed" />{/if}
        {#if event.replacesEventId}<StateTag state="neutral" label="Correction" />{/if}
      </div>
      <dl class="dl">
        <dt>Happened</dt>
        <dd>{event.occurredOn ? formatDate(event.occurredOn) : 'Purchase date unknown'}{#if event.dateBasis !== 'known'} <span class="muted">· {DATE_BASIS_LABEL[event.dateBasis]}</span>{/if}</dd>
        {#if event.categoryId}<dt>Category</dt><dd>{app.categoryName(event.categoryId)}</dd>{/if}
        {#if event.description}<dt>Description</dt><dd>{event.description}</dd>{/if}
        <dt>Recorded</dt><dd>{formatInstant(event.recordedAt)}</dd>
      </dl>

      <table class="data">
        <caption class="sr-only">Postings</caption>
        <thead><tr><th scope="col">Account</th><th scope="col">Posts on</th><th scope="col" class="right">Change</th></tr></thead>
        <tbody>
          {#each event.postings as p (p.accountId)}
            <tr><td>{app.accountName(p.accountId)}</td><td>{formatDate(p.effectiveDate)}</td><td class="right"><Money value={p.delta} signed tone="direction" /></td></tr>
          {/each}
        </tbody>
      </table>

      {#if event.isReversal}
        <p class="notice info">This reversal cancels an earlier entry. Reversals cannot themselves be reversed or corrected.</p>
      {:else if isReversed}
        <p class="notice info">This entry has been reversed. It stays in history for the audit trail and no longer affects balances.</p>
      {/if}

      {#if mode === 'reverse'}
        <form class="stacked" novalidate onsubmit={reverse}>
          <div class="notice danger">
            <strong>What reversing does</strong>
            <ul>
              <li>Records a new entry that exactly cancels this one on every account it touched.</li>
              <li>Keeps this entry visible in history; nothing is deleted.</li>
              <li>If a reconciled period includes it, Rust will refuse until that period is reopened.</li>
            </ul>
          </div>
          <div class="field"><label class="field-label">
            <span>Reason</span>
            <input bind:value={reason} maxlength="500" data-autofocus aria-invalid={attempted && !reason.trim() ? 'true' : undefined} /></label>
            {#if attempted && !reason.trim()}<span class="error">Say why this entry is being reversed.</span>{/if}
          </div>
          <FormStatus {session} />
          <div class="row end">
            <button type="button" class="btn" onclick={() => (mode = 'view')}>Back</button>
            <button type="submit" class="btn primary" disabled={session.submitting}>{session.submitting ? 'Reversing…' : 'Reverse entry'}</button>
          </div>
        </form>
      {:else}
        <div class="row end">
          {#if canChange}
            <button class="btn" onclick={() => (mode = 'reverse')}>Reverse…</button>
            <button class="btn" onclick={() => (mode = 'correct')}>Correct…</button>
          {/if}
          <button class="btn primary" onclick={onclose} data-autofocus>Done</button>
        </div>
      {/if}
    {/if}
  {/if}
</Dialog>

<style>
  .head { display: flex; align-items: center; gap: var(--space-2); }
</style>
