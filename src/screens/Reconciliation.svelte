<!--
  Statement reconciliation. The user enters what the bank or card statement
  says; Rust compares it with recorded postings. A confirmed period guards
  later changes: if entries inside it change, the session turns "stale".
-->
<script lang="ts">
  import { app, resource, WriteSession } from '../lib/app.svelte';
  import {
    confirmReconciliation,
    createReconciliation,
    getLedgerQuality,
    getReconciliation,
    getReconciliationAudit,
    previewReconciliation,
    reopenReconciliation,
    setReconciledPosting,
  } from '../lib/finance';
  import { addDays, formatDate, formatInstant, monthStart } from '../lib/money';
  import { focusFirstInvalid, parseFor } from '../lib/forms';
  import type { Id, Reconciliation, ReconciliationInput } from '../lib/types';
  import AsyncView from '../lib/components/AsyncView.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';
  import FormStatus from '../lib/components/FormStatus.svelte';
  import Money from '../lib/components/Money.svelte';
  import StateTag from '../lib/components/StateTag.svelte';
  import YenField from '../lib/components/YenField.svelte';

  let { reconciliationId }: { reconciliationId: Id | null } = $props();

  const STATUS: Record<string, { label: string; state: 'actual' | 'warning' | 'neutral' }> = {
    draft: { label: 'Draft', state: 'neutral' },
    confirmed: { label: 'Confirmed', state: 'actual' },
    stale: { label: 'Changed since confirmed', state: 'warning' },
  };

  // ---- New session -------------------------------------------------------
  let accountId = $state('');
  let periodStart = $state('');
  let periodEnd = $state('');
  let openingRaw = $state('');
  let closingRaw = $state('');
  let attempted = $state(false);
  let preview = $state<Reconciliation | null>(null);
  let previewError = $state<string | null>(null);
  const createSession = new WriteSession();
  let form: HTMLFormElement | undefined = $state();

  const reconcilable = $derived(app.activeAccounts);
  $effect(() => {
    if (!accountId && reconcilable[0]) accountId = reconcilable[0].id;
    if (!periodStart && app.today) {
      periodStart = monthStart(addDays(monthStart(app.today), -1));
      periodEnd = addDays(monthStart(app.today), -1);
    }
  });

  const opening = $derived(parseFor('signed', openingRaw));
  const closing = $derived(parseFor('signed', closingRaw));
  const errors = $derived.by(() => {
    const e: Record<string, string> = {};
    if (!accountId) e.accountId = 'Add an account first.';
    if (!periodStart || !periodEnd) e.periodEnd = 'Choose the statement period.';
    else if (periodEnd < periodStart) e.periodEnd = 'The period must end after it starts.';
    if (!opening.ok) e.statementOpening = opening.reason;
    if (!closing.ok) e.statementClosing = closing.reason;
    return e;
  });
  const show = (f: string) => createSession.fieldMessage(f) ?? (attempted ? errors[f] : undefined);

  function input(): ReconciliationInput {
    return {
      accountId,
      periodStart,
      periodEnd,
      statementOpening: opening.ok ? opening.value : 0,
      statementClosing: closing.ok ? closing.value : 0,
      selectedPostingIds: null,
    };
  }

  async function check(e: Event) {
    e.preventDefault();
    attempted = true;
    previewError = null;
    if (Object.keys(errors).length) return focusFirstInvalid(form);
    try {
      preview = await previewReconciliation(input());
    } catch (err) {
      preview = null;
      previewError = (err as { message?: string }).message ?? 'The statement could not be checked.';
    }
  }

  async function startSession() {
    const r = await createSession.run(input(), createReconciliation);
    if (r) {
      preview = null;
      openingRaw = '';
      closingRaw = '';
      attempted = false;
      app.go({ name: 'reconciliation', reconciliationId: r.result.id });
    }
  }

  // ---- Existing session --------------------------------------------------
  const quality = resource(() => getLedgerQuality(app.today));
  const openSessions = $derived((quality.current.data ?? []).filter((i) => i.kind.startsWith('reconciliation_')));
  const session = resource(() => (reconciliationId ? getReconciliation(reconciliationId) : Promise.resolve(null)));
  const audit = resource(() => (reconciliationId ? getReconciliationAudit(reconciliationId) : Promise.resolve([])));
  // Every posting in the period, so included ones can be shown and unticked.
  const allPostings = resource(async () => {
    const s = session.current.data;
    if (!s) return [];
    const full = await previewReconciliation({
      accountId: s.accountId,
      periodStart: s.periodStart,
      periodEnd: s.periodEnd,
      statementOpening: s.statementOpening,
      statementClosing: s.statementClosing,
      selectedPostingIds: [],
    });
    return full.unmatchedPostings;
  });
  const action = new WriteSession();

  async function toggle(postingId: Id, included: boolean) {
    if (!reconciliationId) return;
    await action.run({ reconciliationId, postingId, included }, setReconciledPosting);
  }

  async function confirm() {
    if (!reconciliationId) return;
    const r = await action.run({ reconciliationId }, confirmReconciliation);
    if (r && !r.isReplay) app.toast('Statement period confirmed.');
  }

  async function reopen() {
    if (!reconciliationId) return;
    const r = await action.run({ reconciliationId }, reopenReconciliation);
    if (r && !r.isReplay) app.toast('Period reopened for review.');
  }
</script>

<header class="page-head">
  <div>
    <h1 tabindex="-1">Reconciliation</h1>
    <p class="muted">Check recorded entries against a bank or card statement. Confirming a period protects it: later changes inside it are flagged for review.</p>
  </div>
  {#if reconciliationId}<button class="btn" onclick={() => app.go({ name: 'reconciliation', reconciliationId: null })}>New check</button>{/if}
</header>

{#if reconciliationId}
  <AsyncView state={session.current} isEmpty={(s) => s === null} onretry={session.reload} label="reconciliation">
    {#snippet empty()}<EmptyState title="Reconciliation not found" body="It may have been created on another copy of the data." />{/snippet}
    {#snippet children(s)}
      {#if s}
        {@const st = STATUS[s.status] ?? { label: s.status, state: 'neutral' as const }}
        {@const selected = new Set(s.selectedPostingIds)}
        <div class="stack">
          <section class="panel" aria-labelledby="rs-title">
            <div class="panel-head">
              <div>
                <h2 id="rs-title">{app.accountName(s.accountId)} · {formatDate(s.periodStart)} – {formatDate(s.periodEnd)}</h2>
                <p class="muted small">Captured {formatInstant(s.capturedAt)}</p>
              </div>
              <StateTag state={st.state} label={st.label} />
            </div>
            <div class="panel-body cols-3">
              <div><p class="eyebrow">Statement closing</p><Money value={s.statementClosing} size="lg" /></div>
              <div><p class="eyebrow">Ticked entries give</p><Money value={s.calculatedClosing} size="lg" /></div>
              <div><p class="eyebrow">Difference</p><Money value={s.difference} size="lg" signed tone={s.difference ? 'debt' : 'plain'} />{#if s.difference === 0}<StateTag state="actual" label="Balanced" />{/if}</div>
            </div>
            {#if s.openingMismatch}
              <p class="panel-body notice">The statement opening (<Money value={s.statementOpening} />) differs from the recorded balance the day before (<Money value={s.baselineBeforePeriod} />). An earlier entry may be missing or wrong.</p>
            {/if}
            {#if s.status === 'stale'}
              <p class="panel-body notice">Entries in this confirmed period changed afterwards. Reopen it, review the changes, and confirm again.</p>
            {/if}
            <FormStatus session={action} />
            <div class="row end panel-body">
              {#if s.status === 'draft'}
                <button class="btn primary" disabled={action.submitting || s.difference !== 0} onclick={confirm}>Confirm period</button>
                {#if s.difference !== 0}<span class="muted small">Confirm once the difference is ¥0.</span>{/if}
              {:else}
                <button class="btn" disabled={action.submitting} onclick={reopen}>Reopen for review…</button>
                <span class="muted small">Reopening removes the protection until you confirm again.</span>
              {/if}
            </div>
          </section>

          <section class="panel" aria-labelledby="rp-title">
            <div class="panel-head"><h2 id="rp-title">Entries in this period</h2><span class="muted small">{selected.size} ticked · {s.unmatchedPostings.length} not on statement</span></div>
            <AsyncView state={allPostings.current} isEmpty={(p) => p.length === 0} onretry={allPostings.reload} label="entries">
              {#snippet empty()}<EmptyState title="No entries in this period" body="The calculated closing equals the statement opening." />{/snippet}
              {#snippet children(postings)}
                <table class="data">
                  <caption class="sr-only">Postings; tick those that appear on the statement</caption>
                  <thead><tr><th scope="col">On statement</th><th scope="col">Posted</th><th scope="col" class="right">Change</th></tr></thead>
                  <tbody>
                    {#each postings as p (p.id)}
                      <tr>
                        <td><label class="check"><input type="checkbox" checked={selected.has(p.id)} disabled={s.status !== 'draft' || action.submitting} onchange={(e) => toggle(p.id, e.currentTarget.checked)} /> <span class="sr-only">Entry posted {p.effectiveDate}, {p.delta} yen</span>{selected.has(p.id) ? 'Yes' : 'No'}</label></td>
                        <td>{formatDate(p.effectiveDate)}</td>
                        <td class="right"><Money value={p.delta} signed tone="direction" /></td>
                      </tr>
                    {/each}
                  </tbody>
                </table>
              {/snippet}
            </AsyncView>
          </section>

          <section class="panel" aria-labelledby="ra-title">
            <div class="panel-head"><h2 id="ra-title">Audit trail</h2></div>
            <ul class="list-plain panel-body audit">
              {#each audit.current.data ?? [] as a, i (i)}<li><span class="muted">{formatInstant(a.recordedAt)}</span> {a.action.replaceAll('_', ' ')}</li>{/each}
            </ul>
          </section>
        </div>
      {/if}
    {/snippet}
  </AsyncView>
{:else}
  <div class="cols-2">
    <section class="panel" aria-labelledby="new-title">
      <div class="panel-head"><h2 id="new-title">Check a statement</h2></div>
      {#if app.accounts !== undefined && reconcilable.length === 0}
        <div class="panel-body"><EmptyState title="No accounts to reconcile" body="Add an account and record some entries first." /></div>
      {:else}
        <form bind:this={form} class="stacked panel-body" novalidate onsubmit={check}>
          <div class="form-grid">
            <div class="field wide"><label class="field-label"><span>Account</span><select bind:value={accountId} onchange={() => (preview = null)}>{#each reconcilable as a (a.id)}<option value={a.id}>{a.name}</option>{/each}</select></label></div>
            <div class="field"><label class="field-label"><span>Statement from</span><input type="date" bind:value={periodStart} onchange={() => (preview = null)} /></label></div>
            <div class="field"><label class="field-label"><span>to</span><input type="date" bind:value={periodEnd} onchange={() => (preview = null)} aria-invalid={show('periodEnd') ? 'true' : undefined} /></label>{#if show('periodEnd')}<span class="error">{show('periodEnd')}</span>{/if}</div>
            <YenField label="Opening balance on statement" bind:value={openingRaw} mode="signed" error={show('statementOpening')} />
            <YenField label="Closing balance on statement" bind:value={closingRaw} mode="signed" error={show('statementClosing')} />
          </div>
          <p class="muted small">For a card, enter amounts owed as negative balances, as the ledger stores them.</p>
          {#if previewError}<p class="notice danger" role="alert">{previewError} Nothing was saved.</p>{/if}
          <div class="row end"><button class="btn" type="submit">Check</button></div>
        </form>
        {#if preview}
          <div class="panel-body preview" aria-live="polite">
            <dl class="dl">
              <dt>Recorded entries give</dt><dd><Money value={preview.calculatedClosing} /></dd>
              <dt>Statement says</dt><dd><Money value={preview.statementClosing} /></dd>
              <dt>Difference</dt><dd><Money value={preview.difference} signed />{#if preview.difference === 0} <StateTag state="actual" label="Balanced" />{/if}</dd>
              <dt>Ledger balance before period</dt><dd><Money value={preview.baselineBeforePeriod} />{#if preview.openingMismatch} <StateTag state="warning" label="Differs from statement opening" />{/if}</dd>
            </dl>
            <FormStatus session={createSession} fields={['statementOpening', 'statementClosing', 'periodEnd']} />
            <div class="row end"><button class="btn primary" disabled={createSession.submitting} onclick={startSession}>Save and review entries</button></div>
          </div>
        {/if}
      {/if}
    </section>

    <section class="panel" aria-labelledby="open-title">
      <div class="panel-head"><h2 id="open-title">Needs attention</h2></div>
      {#if openSessions.length === 0}
        <div class="panel-body"><EmptyState title="Nothing waiting" body="Drafts and periods that changed after confirmation appear here." /></div>
      {:else}
        <ul class="list-plain panel-body">
          {#each openSessions as i (i.recordId)}
            <li class="row"><StateTag state={i.kind.endsWith('stale') ? 'warning' : 'neutral'} label={i.kind.endsWith('stale') ? 'Changed since confirmed' : 'Draft'} />
              <button class="link" onclick={() => app.go({ name: 'reconciliation', reconciliationId: i.recordId })}>Open session</button></li>
          {/each}
        </ul>
      {/if}
    </section>
  </div>
{/if}

<style>
  .preview { border-top: 1px solid var(--border); display: grid; gap: var(--space-3); }
  .audit { display: grid; gap: var(--space-1); font-size: var(--text-sm); }
</style>
