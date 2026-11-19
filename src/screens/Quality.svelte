<!--
  Data quality: everything Rust reports as incomplete, stale or unresolved,
  grouped by kind with what it affects and where to fix it.
-->
<script lang="ts">
  import { app, resource, type Route } from '../lib/app.svelte';
  import { getLedgerQuality } from '../lib/finance';
  import type { QualityIssue } from '../lib/types';
  import AsyncView from '../lib/components/AsyncView.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';
  import StateTag from '../lib/components/StateTag.svelte';
  import { QUALITY_LABEL } from './labels';

  const issues = resource(() => getLedgerQuality(app.today));

  function groups(list: QualityIssue[]) {
    const map = new Map<string, QualityIssue[]>();
    for (const i of list) map.set(i.kind, [...(map.get(i.kind) ?? []), i]);
    return [...map.entries()];
  }

  function target(i: QualityIssue): { label: string; route: Route } {
    switch (i.kind) {
      case 'missing_opening':
        return { label: app.accountName(i.recordId), route: { name: 'accounts', accountId: i.recordId } };
      case 'unresolved_mapping':
        return { label: 'Map on Ledger start', route: { name: 'start' } };
      case 'missing_settlement_account':
        return { label: 'Set up card payments', route: { name: 'cards', cardId: null } };
      case 'unknown_purchase_date':
        return { label: 'View in history', route: { name: 'accounts', accountId: null } };
      default:
        if (i.kind.startsWith('valuation_')) return { label: app.accountName(i.recordId), route: { name: 'investments' } };
        if (i.kind.startsWith('reconciliation_')) return { label: 'Open reconciliation', route: { name: 'reconciliation', reconciliationId: i.recordId } };
        return { label: 'Open', route: { name: 'overview' } };
    }
  }
</script>

<header class="page-head">
  <div>
    <h1 tabindex="-1">Data quality</h1>
    <p class="muted">Gaps that make balances, budgets or net worth incomplete. Nothing here is fixed automatically.</p>
  </div>
  <button class="btn" onclick={issues.reload}>Check again</button>
</header>

<AsyncView state={issues.current} isEmpty={(l) => l.length === 0} onretry={issues.reload} label="data quality">
  {#snippet empty()}
    <section class="panel"><div class="panel-body"><EmptyState title="No issues found" body="Every account has an opening, earlier records are mapped, valuations are current and reconciliations are confirmed." /></div></section>
  {/snippet}
  {#snippet children(list)}
    <div class="stack">
      {#each groups(list) as [kind, items] (kind)}
        {@const info = QUALITY_LABEL[kind] ?? { title: kind.replaceAll('_', ' '), help: '' }}
        <section class="panel" aria-labelledby="q-{kind}">
          <div class="panel-head">
            <h2 id="q-{kind}">{info.title}</h2>
            <StateTag state="warning" label="{items.length} {items.length === 1 ? 'item' : 'items'}" />
          </div>
          {#if info.help}<p class="panel-body muted">{info.help}</p>{/if}
          <ul class="list-plain items">
            {#each items.slice(0, 50) as i (i.recordId)}
              {@const t = target(i)}
              <li><button class="link" onclick={() => app.go(t.route)}>{t.label}</button></li>
            {/each}
            {#if items.length > 50}<li class="muted small">and {items.length - 50} more</li>{/if}
          </ul>
        </section>
      {/each}
    </div>
  {/snippet}
</AsyncView>

<style>
  .items { display: flex; flex-wrap: wrap; gap: var(--space-2) var(--space-4); padding: 0 var(--space-5) var(--space-4); }
</style>
