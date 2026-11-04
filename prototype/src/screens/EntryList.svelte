<script lang="ts">
  import { deltaFor } from '../lib/derive';
  import { EVENT_LABEL, type Account, type LedgerEntry } from '../lib/fixtures';
  import { formatDate } from '../lib/money';
  import Money from '../lib/components/Money.svelte';
  import StateTag from '../lib/components/StateTag.svelte';

  interface Props {
    entries: LedgerEntry[];
    accounts: Account[];
    /** When set, amounts are the change to this account; otherwise across all accounts. */
    accountId: string | null;
    compact?: boolean;
    caption?: string;
  }
  let { entries, accounts, accountId, compact = false, caption }: Props = $props();

  const nameOf = (id: string) => accounts.find((a) => a.id === id)?.name ?? 'Unknown account';
  const isMove = (e: LedgerEntry) => e.kind === 'TRANSFER' || e.kind === 'CARD_REPAYMENT';
  function where(e: LedgerEntry) {
    if (e.postings.length > 1) return `${nameOf(e.postings[0]!.accountId)} → ${nameOf(e.postings[1]!.accountId)}`;
    return nameOf(e.postings[0]?.accountId ?? '');
  }
</script>

<table class="data" class:compact>
  {#if caption}<caption class="sr-only">{caption}</caption>{/if}
  <thead>
    <tr>
      <th scope="col" style="width: {compact ? 124 : 170}px">Date</th>
      <th scope="col">Description</th>
      {#if !compact}<th scope="col">Type</th>{/if}
      <th scope="col" class="right">Amount</th>
    </tr>
  </thead>
  <tbody>
    {#each entries as e (e.id)}
      {@const delta = deltaFor(e, accountId)}
      <tr class:move={isMove(e)}>
        <td>
          {#if e.occurredOn}
            <span class="num nowrap">{formatDate(e.occurredOn)}</span>
          {:else}
            <span class="date-unknown">
              <StateTag state="warning" label="Purchase date unknown" />
              {#if !compact}<span class="muted small num">Paid {formatDate(e.effectiveDate)}</span>{/if}
            </span>
          {/if}
        </td>
        <td>
          <span class="desc">
            <span>{e.description}</span>
            <span class="muted small">{e.category ?? EVENT_LABEL[e.kind]} · {where(e)}</span>
          </span>
        </td>
        {#if !compact}
          <td><span class="kind" data-kind={e.kind}>{EVENT_LABEL[e.kind]}</span></td>
        {/if}
        <td class="right">
          {#if accountId === null && e.postings.length > 1}
            <span class="moved"><Money value={delta} /><span class="muted small">moved</span></span>
          {:else}
            <Money value={delta} signed tone="direction" />
          {/if}
        </td>
      </tr>
    {/each}
  </tbody>
</table>

<style>
  .desc { display: grid; }
  .nowrap { white-space: nowrap; }
  .small { font-size: var(--text-xs); }
  .date-unknown { display: grid; gap: 2px; justify-items: start; }
  .compact td { height: 40px; }
  .kind { font-size: var(--text-xs); font-weight: 600; color: var(--ink-2); white-space: nowrap; }
  .kind[data-kind='TRANSFER'], .kind[data-kind='CARD_REPAYMENT'] { color: var(--ink-3); }
  tr.move td { color: var(--ink-2); }
  .moved { display: inline-flex; gap: var(--space-1); align-items: baseline; color: var(--ink-3); }
</style>
