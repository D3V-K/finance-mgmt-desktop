<!--
  Ledger start (cutover) and earlier Cash/Bank history (#8). Earlier records
  stay separate and read-only; they are never merged into named-account
  history by guessing. Records before the start date are represented by the
  opening balances; records on or after it are mapped explicitly or remain
  visibly unresolved. A purchase date is never inferred from a payment date.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import { app, resource, WriteSession } from '../lib/app.svelte';
  import { toAppError } from '../lib/commands';
  import {
    confirmCutover,
    confirmLegacyResolution,
    getLegacyBalance,
    getLegacyReports,
    listLegacyProvenance,
    listLegacyRecords,
    previewCutover,
    previewLegacyResolution,
  } from '../lib/finance';
  import { getAccountBalance } from '../lib/ledger';
  import { formatDate, formatMonth } from '../lib/money';
  import { parseFor } from '../lib/forms';
  import type { CutoverInput, CutoverPreview, LegacyMappings, ResolutionPreview } from '../lib/types';
  import AsyncView from '../lib/components/AsyncView.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';
  import FormStatus from '../lib/components/FormStatus.svelte';
  import Money from '../lib/components/Money.svelte';
  import StateTag from '../lib/components/StateTag.svelte';
  import YenField from '../lib/components/YenField.svelte';

  const records = resource(() => listLegacyRecords());
  const reports = resource(() => getLegacyReports());
  const provenance = resource(() => listLegacyProvenance(false));
  const statuses = resource(async () => {
    const accounts = (app.accounts ?? []).filter((a) => a.archivedAt === null);
    const today = app.today;
    return Promise.all(accounts.map((a) => getAccountBalance(a.id, today)));
  });

  const started = $derived(
    (provenance.current.data?.length ?? 0) > 0 ||
      ((statuses.current.data?.length ?? 0) > 0 && statuses.current.data!.every((b) => b.status !== 'incomplete_legacy_mapping')),
  );
  const unresolved = $derived((provenance.current.data ?? []).filter((p) => p.status === 'unresolved'));
  const view = $derived(statuses.current.data === undefined || provenance.current.data === undefined ? 'loading' : started ? 'started' : 'setup');

  const txById = $derived(new Map((records.current.data?.transactions ?? []).map((t) => [t.id, t])));
  const trById = $derived(new Map((records.current.data?.transfers ?? []).map((t) => [t.id, t])));
  const accounts = $derived(app.activeAccounts);
  const txAccounts = $derived(accounts.filter((a) => a.kind !== 'BROKERAGE_CASH'));
  const trAccounts = $derived(accounts.filter((a) => a.kind === 'CASH' || a.kind === 'BANK'));

  // ---- Mapping rows (shared by cutover and later resolution) -------------
  type TxRow = { id: string; accountId: string; effectiveDate: string; occurredOn: string };
  type TrRow = { id: string; fromAccountId: string; toAccountId: string; effectiveDate: string };
  let txRows = $state<TxRow[]>([]);
  let trRows = $state<TrRow[]>([]);

  function mappings(): LegacyMappings {
    return {
      transactions: txRows
        .filter((r) => r.accountId)
        .map((r) => {
          const card = app.account(r.accountId)?.kind === 'CREDIT_CARD';
          return { transactionId: r.id, accountId: r.accountId, effectiveDate: r.effectiveDate, occurredOn: card ? r.effectiveDate : r.occurredOn || null };
        }),
      transfers: trRows.filter((r) => r.fromAccountId && r.toAccountId).map((r) => ({ transferId: r.id, fromAccountId: r.fromAccountId, toAccountId: r.toAccountId, effectiveDate: r.effectiveDate })),
    };
  }

  // ---- Cutover -----------------------------------------------------------
  let asOfDate = $state('');
  let openingRows = $state<{ accountId: string; raw: string }[]>([]);
  $effect(() => {
    const ids = accounts.map((a) => a.id);
    untrack(() => {
      openingRows = ids.map((accountId) => openingRows.find((r) => r.accountId === accountId) ?? { accountId, raw: '' });
    });
  });
  let cutPreview = $state<CutoverPreview | null>(null);
  let previewError = $state<string | null>(null);
  let attempted = $state(false);
  let legacyRef = $state<{ cash: number | null; bank: number | null } | null>(null);
  const cutSession = new WriteSession();

  $effect(() => {
    if (!asOfDate && app.today) asOfDate = app.today;
  });

  // Records on/after the start date need an explicit mapping (or stay unresolved).
  $effect(() => {
    if (view !== 'setup' || !records.current.data || !asOfDate) return;
    const boundary = asOfDate;
    const data = records.current.data;
    untrack(() => {
      txRows = data.transactions
        .filter((t) => t.transactionDate >= boundary)
        .map((t) => txRows.find((r) => r.id === t.id) ?? { id: t.id, accountId: '', effectiveDate: t.transactionDate, occurredOn: '' });
      trRows = data.transfers
        .filter((t) => t.transferDate >= boundary)
        .map((t) => trRows.find((r) => r.id === t.id) ?? { id: t.id, fromAccountId: '', toAccountId: '', effectiveDate: t.transferDate });
    });
  });

  $effect(() => {
    if (view !== 'setup' || !asOfDate || !(records.current.data?.transactions.length || records.current.data?.transfers.length)) {
      legacyRef = null;
      return;
    }
    const on = asOfDate;
    Promise.all([getLegacyBalance('CASH', on), getLegacyBalance('BANK', on)]).then(
      ([cash, bank]) => (legacyRef = { cash, bank }),
      () => (legacyRef = null),
    );
  });

  function openingFor(id: string) {
    const card = app.account(id)?.kind === 'CREDIT_CARD';
    const raw = openingRows.find((r) => r.accountId === id)?.raw ?? '';
    const parsed = parseFor(card ? 'nonnegative' : 'signed', raw);
    return { card, parsed, amount: parsed.ok ? (card ? -parsed.value : parsed.value) : null };
  }

  const openingErrors = $derived(Object.fromEntries(accounts.map((a) => [a.id, openingFor(a.id).parsed])));
  const cutoverReady = $derived(!!asOfDate && accounts.length > 0 && accounts.every((a) => openingErrors[a.id]?.ok));

  function cutoverInput(): CutoverInput {
    return { asOfDate, openings: accounts.map((a) => ({ accountId: a.id, amount: openingFor(a.id).amount ?? 0 })), ...mappings() };
  }

  async function runPreview() {
    attempted = true;
    previewError = null;
    cutPreview = null;
    if (!cutoverReady) return;
    try {
      cutPreview = await previewCutover(cutoverInput());
    } catch (err) {
      previewError = toAppError(err).message;
    }
  }

  async function confirm() {
    const r = await cutSession.run(cutoverInput(), confirmCutover);
    if (r) {
      cutPreview = null;
      if (!r.isReplay) app.toast(r.result.unresolvedCount ? `Ledger started. ${r.result.unresolvedCount} earlier records still need mapping.` : 'Ledger started. Balances are now complete.');
    }
  }

  // ---- Later resolution --------------------------------------------------
  let resPreview = $state<ResolutionPreview | null>(null);
  let resError = $state<string | null>(null);
  const resSession = new WriteSession();

  $effect(() => {
    if (view !== 'started') return;
    const pending = unresolved;
    untrack(() => {
      txRows = pending
        .filter((p) => p.sourceTable === 'legacy_transactions')
        .map((p) => txRows.find((r) => r.id === p.sourceId) ?? { id: p.sourceId, accountId: '', effectiveDate: p.recordedDate, occurredOn: '' });
      trRows = pending
        .filter((p) => p.sourceTable === 'legacy_transfers')
        .map((p) => trRows.find((r) => r.id === p.sourceId) ?? { id: p.sourceId, fromAccountId: '', toAccountId: '', effectiveDate: p.recordedDate });
    });
  });

  async function runResolutionPreview() {
    resError = null;
    resPreview = null;
    const m = mappings();
    if (!m.transactions.length && !m.transfers.length) {
      resError = 'Choose an account for at least one record.';
      return;
    }
    try {
      resPreview = await previewLegacyResolution(m);
    } catch (err) {
      resError = toAppError(err).message;
    }
  }

  async function confirmResolution() {
    const r = await resSession.run(mappings(), confirmLegacyResolution);
    if (r) {
      resPreview = null;
      if (!r.isReplay) app.toast(`${r.result.resolved} records mapped; ${r.result.remaining} still unresolved.`);
    }
  }
</script>

<header class="page-head">
  <div>
    <h1 tabindex="-1">Ledger start & history</h1>
    <p class="muted">
      Where named-account balances begin, and the earlier Cash/Bank records from before. Earlier records keep their original dates and
      are never merged into account history by guessing.
    </p>
  </div>
  {#if view === 'started'}<StateTag state={unresolved.length ? 'incomplete' : 'actual'} label={unresolved.length ? `${unresolved.length} records unresolved` : 'Ledger started'} />{/if}
</header>

{#snippet mappingTable(kindLabel: string)}
  {#if txRows.length || trRows.length}
    <table class="data">
      <caption class="sr-only">{kindLabel}</caption>
      <thead><tr><th scope="col">Earlier record</th><th scope="col" class="right">Amount</th><th scope="col">Map to</th><th scope="col">Posts on</th><th scope="col">Purchase date</th></tr></thead>
      <tbody>
        {#each txRows as row (row.id)}
          {@const t = txById.get(row.id)}
          {@const card = app.account(row.accountId)?.kind === 'CREDIT_CARD'}
          <tr>
            <td>{t ? formatDate(t.transactionDate) : ''} · {t?.description ?? app.categoryName(t?.categoryId)}<br /><span class="muted small">Was {t?.accountType === 'CASH' ? 'Cash' : 'Bank'} · {app.categoryName(t?.categoryId)}</span></td>
            <td class="right"><Money value={t?.amount ?? null} /></td>
            <td><label><span class="sr-only">Account for record of {t?.transactionDate}</span>
              <select bind:value={row.accountId}><option value="">Leave unresolved</option>{#each txAccounts as a (a.id)}<option value={a.id}>{a.name}</option>{/each}</select></label></td>
            <td><label><span class="sr-only">Posting date</span><input type="date" bind:value={row.effectiveDate} disabled={!row.accountId} /></label></td>
            <td>
              {#if card}<span class="small muted">Same as posting date (card)</span>
              {:else}<label><span class="sr-only">Purchase date if known</span><input type="date" bind:value={row.occurredOn} disabled={!row.accountId} /></label><br /><span class="small muted">Blank = unknown</span>{/if}
            </td>
          </tr>
        {/each}
        {#each trRows as row (row.id)}
          {@const t = trById.get(row.id)}
          <tr>
            <td>{t ? formatDate(t.transferDate) : ''} · Transfer {t?.from === 'CASH' ? 'Cash' : 'Bank'} → {t?.to === 'CASH' ? 'Cash' : 'Bank'}{#if t?.description}<br /><span class="muted small">{t.description}</span>{/if}</td>
            <td class="right"><Money value={t?.amount ?? null} /></td>
            <td class="pair">
              <label><span class="sr-only">From account</span><select bind:value={row.fromAccountId}><option value="">Leave unresolved</option>{#each trAccounts as a (a.id)}<option value={a.id}>{a.name}</option>{/each}</select></label>
              <label><span class="sr-only">To account</span><select bind:value={row.toAccountId} disabled={!row.fromAccountId}><option value="">To…</option>{#each trAccounts.filter((a) => a.id !== row.fromAccountId) as a (a.id)}<option value={a.id}>{a.name}</option>{/each}</select></label>
            </td>
            <td><label><span class="sr-only">Posting date</span><input type="date" bind:value={row.effectiveDate} disabled={!row.fromAccountId} /></label></td>
            <td><span class="small muted">—</span></td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
{/snippet}

<div class="stack">
  {#if view === 'loading'}
    <section class="panel"><p class="panel-body muted" role="status">Loading…</p></section>
  {:else if view === 'setup'}
    <section class="panel" aria-labelledby="start-title">
      <div class="panel-head"><h2 id="start-title">Start the ledger</h2></div>
      {#if accounts.length === 0}
        <div class="panel-body"><EmptyState title="Add your accounts first" body="The ledger starts with an opening balance for every active account.">
          {#snippet action()}<button class="btn primary" onclick={() => app.go({ name: 'accounts', accountId: null })}>Go to Accounts</button>{/snippet}
        </EmptyState></div>
      {:else}
        <div class="panel-body stack">
          <p>Choose the day your named accounts begin and enter each balance at the <strong>start</strong> of that day. Until this is confirmed, every balance shows as incomplete.</p>
          <div class="field date"><label class="field-label"><span>Ledger starts on</span><input type="date" bind:value={asOfDate} oninput={() => (cutPreview = null)} /></label></div>
          {#if legacyRef}
            <p class="notice info">For reference, earlier records give Cash <Money value={legacyRef.cash} /> and Bank <Money value={legacyRef.bank} /> at that date. These are not applied automatically: a legacy Cash/Bank label does not say which named account it was.</p>
          {/if}
          <table class="data">
            <caption class="sr-only">Opening balances</caption>
            <thead><tr><th scope="col">Account</th><th scope="col">Opening at start of {asOfDate ? formatDate(asOfDate) : 'the start date'}</th></tr></thead>
            <tbody>
              {#each openingRows as row (row.accountId)}
                {@const a = app.account(row.accountId)}
                {@const f = openingFor(row.accountId)}
                <tr>
                  <td>{a?.name}</td>
                  <td>
                    <YenField label={f.card ? `${a?.name} — amount owed` : `${a?.name} — balance`} bind:value={row.raw} mode={f.card ? 'nonnegative' : 'signed'}
                      error={attempted && !f.parsed.ok ? f.parsed.reason : undefined} />
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
          <p class="muted small">Every active account needs an opening here. Archive accounts you no longer use first. Enter 0 for an empty account.</p>
        </div>
        {#if txRows.length || trRows.length}
          <div class="panel-head"><h3>Earlier records on or after the start date</h3><span class="muted small">{txRows.length + trRows.length} records</span></div>
          <p class="panel-body muted small">Records before the start date are already covered by the openings and stay as read-only history. For these, choose the account explicitly — or leave them unresolved; balances then stay incomplete until you map them.</p>
          {@render mappingTable('Earlier records to map')}
        {/if}
        <div class="panel-body stack">
          {#if previewError}<p class="notice danger" role="alert">{previewError} Nothing was saved.</p>{/if}
          {#if cutPreview}
            <div class="preview" aria-live="polite">
              <h3>Check before starting</h3>
              <dl class="dl">
                <dt>Starts on</dt><dd>{formatDate(cutPreview.asOfDate)}</dd>
                <dt>Opening total</dt><dd><Money value={cutPreview.openingTotal} /> <span class="muted small">(cash minus card debt)</span></dd>
                <dt>Earlier records</dt><dd>{cutPreview.legacyTransactionCount} transactions, {cutPreview.legacyTransferCount} transfers{#if cutPreview.legacyDateMin} · {formatDate(cutPreview.legacyDateMin)} – {formatDate(cutPreview.legacyDateMax)}{/if}</dd>
                <dt>Mapped now</dt><dd>{cutPreview.mappedCount}</dd>
                <dt>Left unresolved</dt><dd>{cutPreview.unresolvedCount}{#if cutPreview.unresolvedCount} <StateTag state="incomplete" label="Balances stay incomplete" />{/if}</dd>
              </dl>
              <div class="notice">
                <strong>What starting does</strong>
                <ul>
                  <li>Records these openings and makes earlier Cash/Bank history read-only. It cannot be undone.</li>
                  <li>No entries can be dated before {formatDate(cutPreview.asOfDate)}.</li>
                  {#if cutPreview.legacyAccountTypeIsUnverified}<li>The earlier Cash/Bank labels were never checked against real accounts; they are kept for reference only.</li>{/if}
                  <li>Unknown purchase dates stay unknown ({cutPreview.purchaseDateBasis.replaceAll('_', ' ')}).</li>
                </ul>
              </div>
              <FormStatus session={cutSession} fields={[]} />
              <div class="row end">
                <button class="btn" onclick={() => (cutPreview = null)}>Change something</button>
                <button class="btn primary" disabled={cutSession.submitting} onclick={confirm}>{cutSession.submitting ? 'Starting…' : 'Start the ledger'}</button>
              </div>
            </div>
          {:else}
            <div class="row end"><button class="btn primary" onclick={runPreview}>Review start…</button></div>
            {#if attempted && !cutoverReady}<p class="error" role="alert">Enter an opening for every account (0 is fine).</p>{/if}
          {/if}
        </div>
      {/if}
    </section>
  {:else}
    <section class="panel" aria-labelledby="res-title">
      <div class="panel-head"><h2 id="res-title">Unresolved earlier records</h2></div>
      {#if unresolved.length === 0}
        <div class="panel-body"><EmptyState title="Nothing left to map" body="Every earlier record is either covered by an opening or mapped to a named account." /></div>
      {:else}
        <p class="panel-body notice">These records are dated after the ledger start but are not yet assigned to a named account, so balances stay incomplete. Map them explicitly; nothing is guessed.</p>
        {@render mappingTable('Unresolved records')}
        <div class="panel-body stack">
          {#if resError}<p class="notice danger" role="alert">{resError} Nothing was saved.</p>{/if}
          {#if resPreview}
            <div class="preview" aria-live="polite">
              <p>{resPreview.records.length} records will be mapped ({resPreview.writes} entries written). {resPreview.remainingBefore - resPreview.records.length} will remain unresolved.</p>
              <FormStatus session={resSession} fields={[]} />
              <div class="row end">
                <button class="btn" onclick={() => (resPreview = null)}>Change something</button>
                <button class="btn primary" disabled={resSession.submitting} onclick={confirmResolution}>Map these records</button>
              </div>
            </div>
          {:else}
            <div class="row end"><button class="btn primary" onclick={runResolutionPreview}>Review mapping…</button></div>
          {/if}
        </div>
      {/if}
    </section>
  {/if}

  <section class="panel" aria-labelledby="legacy-title">
    <div class="panel-head"><h2 id="legacy-title">Earlier Cash/Bank reports</h2><span class="muted small">Read-only · original record dates</span></div>
    <AsyncView state={reports.current} isEmpty={(r) => r.monthly.length === 0} onretry={reports.reload} label="earlier reports">
      {#snippet empty()}<EmptyState title="No earlier records" body="Records imported from the previous app appear here, separate from named-account history." />{/snippet}
      {#snippet children(r)}
        <div class="panel-body cols-3">
          <div><p class="eyebrow">Cash (earlier records)</p><Money value={r.balances.cashBalance} size="lg" /></div>
          <div><p class="eyebrow">Bank (earlier records)</p><Money value={r.balances.bankBalance} size="lg" /></div>
          <div><p class="eyebrow">Combined</p><Money value={r.balances.totalBalance} size="lg" /></div>
        </div>
        <div class="cols-2 tables">
          <table class="data">
            <caption>Income and expenses by month</caption>
            <thead><tr><th scope="col">Month</th><th scope="col" class="right">Income</th><th scope="col" class="right">Expenses</th><th scope="col" class="right">Net</th></tr></thead>
            <tbody>{#each r.monthly as m (m.month)}<tr><td>{formatMonth(m.month.slice(0, 7))}</td><td class="right"><Money value={m.income} /></td><td class="right"><Money value={m.expenses} /></td><td class="right"><Money value={m.net} signed /></td></tr>{/each}</tbody>
          </table>
          <table class="data">
            <caption>Net balance series <span class="muted">(not net worth)</span></caption>
            <thead><tr><th scope="col">Month</th><th scope="col" class="right">Net balance</th></tr></thead>
            <tbody>{#each r.netBalanceSeries as m (m.month)}<tr><td>{formatMonth(m.month.slice(0, 7))}</td><td class="right"><Money value={m.balance} /></td></tr>{/each}</tbody>
          </table>
        </div>
        <p class="panel-body muted small">The earlier app called this series “net worth”, but it is only opening Cash + Bank plus cumulative net cash flow. True net worth is under Investments & position.</p>
      {/snippet}
    </AsyncView>
  </section>

  <section class="panel" aria-labelledby="legrec-title">
    <div class="panel-head"><h2 id="legrec-title">Earlier records</h2></div>
    <AsyncView state={records.current} isEmpty={(r) => r.transactions.length + r.transfers.length === 0} onretry={records.reload} label="earlier records">
      {#snippet empty()}<EmptyState title="No earlier records" body="Nothing has been imported from the previous app. Import is handled separately." />{/snippet}
      {#snippet children(r)}
        {@const status = new Map((provenance.current.data ?? []).map((p) => [p.sourceId, p.status]))}
        <details class="panel-body">
          <summary>{r.transactions.length} transactions and {r.transfers.length} transfers</summary>
          <table class="data">
            <thead><tr><th scope="col">Recorded date</th><th scope="col">Details</th><th scope="col">Was</th><th scope="col">Status</th><th scope="col" class="right">Amount</th></tr></thead>
            <tbody>
              {#each r.transactions as t (t.id)}
                <tr><td>{formatDate(t.transactionDate)}</td><td>{t.description ?? ''} <span class="muted small">{app.categoryName(t.categoryId)}</span></td><td>{t.accountType === 'CASH' ? 'Cash' : 'Bank'}</td><td>{status.get(t.id)?.replaceAll('_', ' ') ?? 'history'}</td><td class="right"><Money value={t.amount} /></td></tr>
              {/each}
              {#each r.transfers as t (t.id)}
                <tr><td>{formatDate(t.transferDate)}</td><td>Transfer {t.description ?? ''}</td><td>{t.from} → {t.to}</td><td>{status.get(t.id)?.replaceAll('_', ' ') ?? 'history'}</td><td class="right"><Money value={t.amount} /></td></tr>
              {/each}
            </tbody>
          </table>
          <p class="muted small">The recorded date is when the earlier app saved the entry. It is not treated as a purchase date.</p>
        </details>
      {/snippet}
    </AsyncView>
  </section>
</div>

<style>
  .date { max-width: 240px; }
  .preview { display: grid; gap: var(--space-3); border-top: 1px solid var(--border); padding-top: var(--space-4); }
  .pair { display: grid; gap: var(--space-1); }
  td select, td input[type='date'] { min-height: 30px; border: 1px solid var(--border-strong); border-radius: var(--radius-sm); background: var(--surface); padding: 0 var(--space-2); font-size: var(--text-sm); }
  .tables { padding: 0 var(--space-5) var(--space-4); }
  caption { text-align: left; font-weight: 620; padding: var(--space-2) var(--space-3); }
  details summary { cursor: pointer; color: var(--ink-2); margin-bottom: var(--space-2); }
</style>
