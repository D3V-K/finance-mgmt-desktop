<script lang="ts">
  import { onMount } from 'svelte';
  import { listAccounts, listCategories } from '../../lib/ledger';
  import {
    changeSubscriptionStatus,
    getSubscription,
    listSubscriptionOccurrences,
    listSubscriptions,
    newWrite,
    overrideSubscriptionOccurrence,
    reconcileSubscriptionBatch,
    reconcileSubscriptionOccurrence,
    unlinkSubscriptionOccurrence,
  } from '../../lib/subscriptions';
  import { toAppError } from '../../lib/commands';
  import {
    formatCurrencyMinor,
    formatUsdCents,
    formatYen,
    parseYenInput,
    spokenCurrencyMinor,
  } from '../../lib/money';
  import type {
    Account,
    AppError,
    BillingCurrency,
    Category,
    DateOnly,
    ReconcileBatchInput,
    ReconcileOccurrenceInput,
    SubscriptionDetail,
    SubscriptionListItem,
    SubscriptionOccurrence,
    SubscriptionOverview,
    SubscriptionStatus,
    Write,
  } from '../../lib/types';
  import SubscriptionFormDialog from './SubscriptionFormDialog.svelte';
  import PriceRevisionDialog from './PriceRevisionDialog.svelte';
  import MatchExpenseDialog from './MatchExpenseDialog.svelte';

  interface Props {
    today: DateOnly;
    onpendingchange?: (count: number) => void;
  }

  let { today, onpendingchange }: Props = $props();

  type SubTab = 'reconcile' | 'all' | 'upcoming';

  let activeTab = $state<SubTab>('reconcile');
  let loading = $state(true);
  let error = $state<AppError | null>(null);
  let statusBanner = $state<string | null>(null);

  let overview = $state<SubscriptionOverview | null>(null);
  let accounts = $state<Account[]>([]);
  let categories = $state<Category[]>([]);

  // Month-end reconciliation state
  let reconcileMonth = $state('');
  let showAllUnreconciled = $state(false);
  let includeJpyInReconcile = $state(false);
  let reconcileRows = $state<SubscriptionOccurrence[]>([]);
  let jpyInputs = $state<Record<string, string>>({});
  let settledDateInputs = $state<Record<string, string>>({});
  let cardScheduleInputs = $state<Record<string, boolean>>({});
  let rowErrors = $state<Record<string, string>>({});
  let batchSubmitting = $state(false);
  let pendingBatchWrite = $state<Write<ReconcileBatchInput> | null>(null);

  // Upcoming 60-day state
  let upcomingRows = $state<SubscriptionOccurrence[]>([]);

  // All subscriptions master-detail state
  let includeCanceled = $state(false);
  let currencyFilter = $state<'ALL' | BillingCurrency>('ALL');
  let selectedSubId = $state<string | null>(null);
  let detail = $state<SubscriptionDetail | null>(null);
  let detailLoading = $state(false);

  // Dialogs
  let formOpen = $state(false);
  let editingSub = $state<SubscriptionListItem | null>(null);
  let revOpen = $state(false);
  let matchOpen = $state(false);
  let matchingOccurrence = $state<SubscriptionOccurrence | null>(null);

  function rowKey(occ: SubscriptionOccurrence): string {
    return `${occ.subscriptionId}:${occ.scheduledDate}`;
  }

  function monthRange(ym: string): { from: DateOnly; to: DateOnly } {
    const parts = ym.split('-');
    const y = Number(parts[0] ?? 2026);
    const m = Number(parts[1] ?? 1);
    const lastDay = new Date(Date.UTC(y, m, 0)).getUTCDate();
    return {
      from: `${ym}-01`,
      to: `${ym}-${String(lastDay).padStart(2, '0')}`,
    };
  }

  function addDays(iso: DateOnly, delta: number): DateOnly {
    const d = new Date(`${iso}T00:00:00Z`);
    d.setUTCDate(d.getUTCDate() + delta);
    return d.toISOString().slice(0, 10);
  }

  const filteredSubscriptions = $derived(
    (overview?.items ?? []).filter((item) => {
      if (currencyFilter !== 'ALL' && item.currency !== currencyFilter) return false;
      return true;
    }),
  );

  const visibleReconcileRows = $derived(
    reconcileRows.filter((occ) => {
      if (occ.status === 'reconciled' || occ.status === 'skipped') return false;
      if (!includeJpyInReconcile && occ.billedCurrency === 'JPY') return false;
      return true;
    }),
  );

  const enteredCount = $derived(
    visibleReconcileRows.filter((occ) => (jpyInputs[rowKey(occ)] ?? '').trim().length > 0).length,
  );

  async function loadAll() {
    loading = true;
    error = null;
    try {
      const activeMonth = reconcileMonth || today.slice(0, 7);
      if (!reconcileMonth) reconcileMonth = activeMonth;
      const range = showAllUnreconciled
        ? { from: addDays(today, -365), to: monthRange(today.slice(0, 7)).to }
        : monthRange(activeMonth);
      const upcomingEnd = addDays(today, 60);

      const [ov, accs, cats, recOccs, upOccs] = await Promise.all([
        listSubscriptions({ includeCanceled, asOfDate: today }),
        listAccounts(false),
        listCategories(false),
        listSubscriptionOccurrences({
          from: range.from,
          to: range.to,
          asOfDate: today,
          unreconciledOnly: true,
        }),
        listSubscriptionOccurrences({
          from: today,
          to: upcomingEnd,
          asOfDate: today,
        }),
      ]);

      overview = ov;
      accounts = accs;
      categories = cats;
      reconcileRows = recOccs;
      upcomingRows = upOccs;
      onpendingchange?.(ov.pendingReconciliationCount);

      // Seed defaults for newly shown reconciliation rows without clobbering user input
      const nextDates = { ...settledDateInputs };
      const nextCards = { ...cardScheduleInputs };
      const nextJpy = { ...jpyInputs };
      for (const occ of recOccs) {
        const k = rowKey(occ);
        if (!nextDates[k]) nextDates[k] = occ.effectiveDate;
        if (nextCards[k] === undefined) nextCards[k] = occ.hasCardSettings;
        if (nextJpy[k] === undefined && occ.billedCurrency === 'JPY') {
          nextJpy[k] = String(occ.billedAmountMinor);
        }
      }
      settledDateInputs = nextDates;
      cardScheduleInputs = nextCards;
      jpyInputs = nextJpy;

      if (selectedSubId && ov.items.some((i) => i.id === selectedSubId)) {
        await loadDetail(selectedSubId);
      } else {
        const first = ov.items[0];
        if (first) {
          selectedSubId = first.id;
          await loadDetail(first.id);
        } else {
          selectedSubId = null;
          detail = null;
        }
      }
    } catch (err) {
      error = toAppError(err);
    } finally {
      loading = false;
    }
  }

  async function loadDetail(subscriptionId: string) {
    detailLoading = true;
    try {
      detail = await getSubscription({ subscriptionId, asOfDate: today });
    } catch (err) {
      error = toAppError(err);
    } finally {
      detailLoading = false;
    }
  }

  async function handleBatchReconcile() {
    error = null;
    statusBanner = null;
    rowErrors = {};

    const items: ReconcileOccurrenceInput[] = [];
    const nextRowErrors: Record<string, string> = {};

    for (const occ of visibleReconcileRows) {
      const k = rowKey(occ);
      const raw = (jpyInputs[k] ?? '').trim();
      if (!raw) continue;
      const parsed = parseYenInput(raw);
      if (!parsed.ok) {
        nextRowErrors[k] = parsed.reason;
        continue;
      }
      const settledOn = settledDateInputs[k] || occ.effectiveDate;
      items.push({
        subscriptionId: occ.subscriptionId,
        scheduledDate: occ.scheduledDate,
        actualJpy: parsed.value,
        settledOn,
        occurredOn: occ.effectiveDate,
        createCardSchedule: occ.hasCardSettings ? (cardScheduleInputs[k] ?? true) : undefined,
      });
    }

    if (Object.keys(nextRowErrors).length > 0) {
      rowErrors = nextRowErrors;
      return;
    }
    if (items.length === 0) {
      error = {
        code: 'INVALID_REQUEST',
        message: 'Enter the actual JPY deducted for at least one row before reconciling.',
        category: 'validation',
      };
      return;
    }

    batchSubmitting = true;
    try {
      const canReusePending =
        pendingBatchWrite !== null &&
        JSON.stringify(pendingBatchWrite.payload.items) === JSON.stringify(items);
      const req = canReusePending ? pendingBatchWrite! : newWrite<ReconcileBatchInput>({ items });
      pendingBatchWrite = req;
      const res = await reconcileSubscriptionBatch(req);
      pendingBatchWrite = null;

      // Clear reconciled inputs
      const cleared = { ...jpyInputs };
      for (const item of items) {
        delete cleared[`${item.subscriptionId}:${item.scheduledDate}`];
      }
      jpyInputs = cleared;
      statusBanner = `Reconciled ${res.result.length} subscription charge(s) into the JPY ledger.`;
      await loadAll();
    } catch (err) {
      error = toAppError(err);
    } finally {
      batchSubmitting = false;
    }
  }

  async function handleQuickRecordJpy(occ: SubscriptionOccurrence) {
    error = null;
    statusBanner = null;
    try {
      const req = newWrite<ReconcileOccurrenceInput>({
        subscriptionId: occ.subscriptionId,
        scheduledDate: occ.scheduledDate,
        actualJpy: occ.billedAmountMinor,
        settledOn: occ.effectiveDate,
        occurredOn: occ.effectiveDate,
        createCardSchedule: occ.hasCardSettings ? true : undefined,
      });
      await reconcileSubscriptionOccurrence(req);
      statusBanner = `Recorded ${occ.subscriptionName} (${formatYen(occ.billedAmountMinor)}).`;
      await loadAll();
    } catch (err) {
      error = toAppError(err);
    }
  }

  async function handleSkip(occ: SubscriptionOccurrence, skipped: boolean) {
    error = null;
    pendingBatchWrite = null;
    try {
      await overrideSubscriptionOccurrence(
        newWrite({
          subscriptionId: occ.subscriptionId,
          scheduledDate: occ.scheduledDate,
          skipped,
        }),
      );
      await loadAll();
    } catch (err) {
      error = toAppError(err);
    }
  }

  async function handleUnlink(occ: SubscriptionOccurrence) {
    error = null;
    try {
      await unlinkSubscriptionOccurrence(
        newWrite({
          subscriptionId: occ.subscriptionId,
          scheduledDate: occ.scheduledDate,
        }),
      );
      statusBanner = `Unlinked ${occ.subscriptionName} (${occ.scheduledDate}) for re-reconciliation.`;
      await loadAll();
    } catch (err) {
      error = toAppError(err);
    }
  }

  async function handleStatusChange(sub: SubscriptionListItem, status: SubscriptionStatus) {
    error = null;
    try {
      await changeSubscriptionStatus(
        newWrite({
          subscriptionId: sub.id,
          status,
          effectiveOn: today,
        }),
      );
      await loadAll();
    } catch (err) {
      error = toAppError(err);
    }
  }

  onMount(() => {
    void loadAll();
  });
</script>

<section class="subscriptions-view" aria-label="Subscriptions">
  <header class="view-header">
    <div>
      <h1>Subscriptions</h1>
      <p class="lead">
        Fixed JPY plans project into cash flow automatically; foreign-currency plans (USD) are
        reconciled with the actual JPY deducted during month-end bookkeeping.
      </p>
    </div>
    <button
      type="button"
      class="btn-primary"
      onclick={() => {
        editingSub = null;
        formOpen = true;
      }}
    >
      + New Subscription
    </button>
  </header>

  {#if error}
    <div class="banner banner-error" role="alert" data-testid="sub-error-banner">
      <div>
        <strong>{error.code}:</strong> {error.message}
      </div>
      <button type="button" class="btn-secondary" onclick={loadAll}>Try again</button>
    </div>
  {/if}

  {#if statusBanner}
    <div class="banner banner-ok" role="status" data-testid="sub-status-banner">
      <span>{statusBanner}</span>
      <button type="button" class="link-btn" onclick={() => (statusBanner = null)}>Dismiss</button>
    </div>
  {/if}

  {#if overview}
    <div class="summary-strip" data-testid="sub-summary-strip">
      <div class="summary-card">
        <span class="card-label">JPY Subscriptions (Confirmed)</span>
        <strong class="card-figure">{formatYen(overview.jpyMonthlyNormalized)} / mo</strong>
        <span class="card-meta">Due this month: {formatYen(overview.jpyDueThisMonth)}</span>
      </div>

      <div class="summary-card">
        <span class="card-label">Foreign Subscriptions (USD)</span>
        {#if overview.foreignMonthlyNormalized[0]}
          {@const foreignSummary = overview.foreignMonthlyNormalized[0]}
          <strong class="card-figure">
            {formatUsdCents(foreignSummary.totalMinor)} / mo
          </strong>
          <span class="card-meta">
            Reconciled at month-end · Last month actual: {formatYen(overview.lastMonthReconciledForeignJpy)}
          </span>
        {:else}
          <strong class="card-figure">$0.00 / mo</strong>
          <span class="card-meta">No active foreign subscriptions</span>
        {/if}
      </div>

      <div class="summary-card" class:attention={overview.pendingReconciliationCount > 0}>
        <span class="card-label">Month-End Reconciliation</span>
        <strong class="card-figure">
          {overview.pendingReconciliationCount}
          {overview.pendingReconciliationCount === 1 ? 'charge' : 'charges'} pending
        </strong>
        <span class="card-meta">Enter actual JPY deducted from bank or card statements</span>
      </div>
    </div>
  {/if}

  <div class="sub-tabs" role="tablist" aria-label="Subscription sections">
    <button
      type="button"
      role="tab"
      aria-selected={activeTab === 'reconcile'}
      class:active={activeTab === 'reconcile'}
      onclick={() => (activeTab = 'reconcile')}
    >
      Month-End Reconciliation
      {#if (overview?.pendingReconciliationCount ?? 0) > 0}
        <span class="pill">{overview?.pendingReconciliationCount}</span>
      {/if}
    </button>
    <button
      type="button"
      role="tab"
      aria-selected={activeTab === 'all'}
      class:active={activeTab === 'all'}
      onclick={() => (activeTab = 'all')}
    >
      All Subscriptions ({overview?.items.length ?? 0})
    </button>
    <button
      type="button"
      role="tab"
      aria-selected={activeTab === 'upcoming'}
      class:active={activeTab === 'upcoming'}
      onclick={() => (activeTab = 'upcoming')}
    >
      Upcoming (60 days)
    </button>
  </div>

  {#if loading}
    <p class="loading" role="status">Loading subscriptions …</p>
  {:else if activeTab === 'reconcile'}
    <section class="panel" aria-label="Month-End Reconciliation" data-testid="reconcile-panel">
      <div class="toolbar">
        <label class="inline-field">
          <span>Bookkeeping month</span>
          <input
            type="month"
            bind:value={reconcileMonth}
            disabled={showAllUnreconciled}
            onchange={() => {
              pendingBatchWrite = null;
              void loadAll();
            }}
          />
        </label>
        <label class="checkbox-label">
          <input
            type="checkbox"
            bind:checked={showAllUnreconciled}
            onchange={() => {
              pendingBatchWrite = null;
              void loadAll();
            }}
          />
          <span>Show all unreconciled (past 12 months)</span>
        </label>
        <label class="checkbox-label">
          <input
            type="checkbox"
            bind:checked={includeJpyInReconcile}
            onchange={() => (pendingBatchWrite = null)}
          />
          <span>Include unrecorded JPY subscriptions</span>
        </label>
      </div>

      {#if visibleReconcileRows.length === 0}
        <div class="empty-box" data-testid="reconcile-empty">
          <h3>All caught up</h3>
          <p>
            No unreconciled foreign-currency subscription charges found for
            {showAllUnreconciled ? 'the past 12 months' : reconcileMonth}.
          </p>
        </div>
      {:else}
        <table class="data-table">
          <thead>
            <tr>
              <th>Subscription</th>
              <th>Billing Date</th>
              <th>Billed Price</th>
              <th>Last Actual JPY</th>
              <th>Account</th>
              <th>Actual JPY Deducted</th>
              <th>Deduction Date</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            {#each visibleReconcileRows as occ (rowKey(occ))}
              {@const k = rowKey(occ)}
              <tr>
                <td>
                  <div class="cell-title">{occ.subscriptionName}</div>
                  <span class="tag tag-pending">Needs JPY Reconciliation</span>
                </td>
                <td class="num">{occ.effectiveDate}</td>
                <td class="num">
                  <span aria-hidden="true">
                    {formatCurrencyMinor(occ.billedCurrency, occ.billedAmountMinor)}
                  </span>
                  <span class="sr-only">
                    {spokenCurrencyMinor(occ.billedCurrency, occ.billedAmountMinor)}
                  </span>
                </td>
                <td class="num muted">
                  {occ.lastReconciledJpy !== null ? formatYen(occ.lastReconciledJpy) : 'First cycle'}
                </td>
                <td>
                  <div>{occ.accountName}</div>
                  {#if occ.hasCardSettings}
                    <label class="mini-check">
                      <input
                        type="checkbox"
                        checked={cardScheduleInputs[k] ?? true}
                        onchange={(e) => {
                          cardScheduleInputs = {
                            ...cardScheduleInputs,
                            [k]: (e.currentTarget as HTMLInputElement).checked,
                          };
                          pendingBatchWrite = null;
                        }}
                      />
                      <span>Add card schedule</span>
                    </label>
                  {/if}
                </td>
                <td>
                  <input
                    type="text"
                    inputmode="numeric"
                    class="jpy-input"
                    class:invalid={!!rowErrors[k]}
                    aria-label={`Actual JPY deducted for ${occ.subscriptionName} on ${occ.effectiveDate}`}
                    aria-invalid={!!rowErrors[k]}
                    placeholder="e.g. 3,142"
                    value={jpyInputs[k] ?? ''}
                    oninput={(e) => {
                      jpyInputs = { ...jpyInputs, [k]: (e.currentTarget as HTMLInputElement).value };
                      pendingBatchWrite = null;
                      if (rowErrors[k]) {
                        const copy = { ...rowErrors };
                        delete copy[k];
                        rowErrors = copy;
                      }
                    }}
                  />
                  {#if rowErrors[k]}
                    <div class="field-err" role="alert">{rowErrors[k]}</div>
                  {/if}
                </td>
                <td>
                  <input
                    type="date"
                    class="date-input"
                    aria-label={`Deduction date for ${occ.subscriptionName} on ${occ.effectiveDate}`}
                    value={settledDateInputs[k] ?? occ.effectiveDate}
                    onchange={(e) => {
                      settledDateInputs = {
                        ...settledDateInputs,
                        [k]: (e.currentTarget as HTMLInputElement).value,
                      };
                      pendingBatchWrite = null;
                    }}
                  />
                </td>
                <td class="actions-cell">
                  <button
                    type="button"
                    class="link-btn"
                    onclick={() => {
                      pendingBatchWrite = null;
                      matchingOccurrence = occ;
                      matchOpen = true;
                    }}
                  >
                    Match
                  </button>
                  <button type="button" class="link-btn muted" onclick={() => handleSkip(occ, true)}>
                    Skip
                  </button>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>

        <footer class="batch-bar">
          <span>
            {enteredCount} of {visibleReconcileRows.length} row(s) have actual JPY entered.
          </span>
          <button
            type="button"
            class="btn-primary"
            disabled={batchSubmitting || enteredCount === 0}
            onclick={handleBatchReconcile}
          >
            {#if batchSubmitting}
              Reconciling …
            {:else if pendingBatchWrite}
              Try again ({enteredCount})
            {:else}
              Reconcile Entered ({enteredCount})
            {/if}
          </button>
        </footer>
      {/if}
    </section>
  {:else if activeTab === 'all'}
    <section class="master-detail" aria-label="All Subscriptions">
      <div class="list-pane">
        <div class="toolbar">
          <select aria-label="Filter by currency" bind:value={currencyFilter}>
            <option value="ALL">All currencies</option>
            <option value="JPY">JPY only</option>
            <option value="USD">USD only</option>
          </select>
          <label class="checkbox-label">
            <input
              type="checkbox"
              bind:checked={includeCanceled}
              onchange={() => void loadAll()}
            />
            <span>Include canceled</span>
          </label>
        </div>

        {#if filteredSubscriptions.length === 0}
          <p class="empty-inline">No subscriptions yet. Add your first JPY or USD subscription.</p>
        {:else}
          <ul class="sub-list">
            {#each filteredSubscriptions as item (item.id)}
              <li>
                <button
                  type="button"
                  class="sub-row"
                  class:selected={selectedSubId === item.id}
                  onclick={() => {
                    selectedSubId = item.id;
                    void loadDetail(item.id);
                  }}
                >
                  <div>
                    <strong>{item.name}</strong>
                    <div class="row-meta">
                      <span>{item.accountName}</span> · <span>{item.cadence}</span>
                      {#if item.canceledEffectiveOn && item.canceledEffectiveOn > today}
                        · <span class="tag tag-muted">cancels {item.canceledEffectiveOn}</span>
                      {:else if item.pausedFrom && item.pausedFrom > today}
                        · <span class="tag tag-muted">pauses {item.pausedFrom}</span>
                      {:else if item.status !== 'active'}
                        · <span class="tag tag-muted">{item.status}</span>
                      {/if}
                    </div>
                  </div>
                  <div class="row-right num">
                    <strong>{formatCurrencyMinor(item.currency, item.currentAmountMinor)}</strong>
                    <div class="row-meta">
                      {#if item.currency === 'USD'}
                        {item.lastReconciledJpy !== null
                          ? `Last: ${formatYen(item.lastReconciledJpy)}`
                          : 'Month-end JPY'}
                      {:else if item.nextBillingDate}
                        Next: {item.nextBillingDate}
                      {/if}
                    </div>
                  </div>
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </div>

      <div class="detail-pane">
        {#if detailLoading}
          <p class="loading">Loading details …</p>
        {:else if detail}
          {@const sub = detail.subscription}
          <header class="detail-header">
            <div>
              <h2>{sub.name}</h2>
              <p class="row-meta">
                {sub.merchant ? `${sub.merchant} · ` : ''}{sub.categoryName} · {sub.accountName} ·
                {sub.cadence}
              </p>
            </div>
            <div class="detail-actions">
              <button
                type="button"
                class="btn-secondary"
                onclick={() => {
                  editingSub = sub;
                  formOpen = true;
                }}
              >
                Edit
              </button>
              <button type="button" class="btn-secondary" onclick={() => (revOpen = true)}>
                Change Price
              </button>
              {#if sub.status === 'active'}
                <button
                  type="button"
                  class="btn-secondary"
                  onclick={() => handleStatusChange(sub, 'paused')}
                >
                  Pause
                </button>
              {:else if sub.status === 'paused'}
                <button
                  type="button"
                  class="btn-secondary"
                  onclick={() => handleStatusChange(sub, 'active')}
                >
                  Resume
                </button>
              {/if}
              {#if sub.status !== 'canceled'}
                <button
                  type="button"
                  class="btn-danger"
                  onclick={() => handleStatusChange(sub, 'canceled')}
                >
                  Cancel
                </button>
              {/if}
            </div>
          </header>

          <h3>Plan Price History</h3>
          <ul class="rev-list">
            {#each detail.revisions as rev (rev.id)}
              <li>
                <span class="num">#{rev.revision} · {rev.effectiveFrom}</span>
                <strong class="num">{formatCurrencyMinor(sub.currency, rev.amountMinor)}</strong>
                <span class="muted">{rev.reason}</span>
              </li>
            {/each}
          </ul>

          <h3>Billing & Reconciliation History</h3>
          <table class="data-table compact">
            <thead>
              <tr>
                <th>Cycle</th>
                <th>Billed</th>
                <th>Actual JPY</th>
                <th>Settled</th>
                <th>Status</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              {#each detail.occurrences as occ (occ.scheduledDate)}
                <tr>
                  <td class="num">{occ.effectiveDate}</td>
                  <td class="num">{formatCurrencyMinor(occ.billedCurrency, occ.billedAmountMinor)}</td>
                  <td class="num">
                    {occ.actualJpy !== null ? formatYen(occ.actualJpy) : 'Pending'}
                  </td>
                  <td class="num">{occ.settledOn ?? '—'}</td>
                  <td><span class="tag">{occ.status.replace('_', ' ')}</span></td>
                  <td>
                    {#if occ.status === 'reconciled'}
                      <button type="button" class="link-btn muted" onclick={() => handleUnlink(occ)}>
                        Unlink
                      </button>
                    {:else if occ.status === 'skipped'}
                      <button type="button" class="link-btn" onclick={() => handleSkip(occ, false)}>
                        Restore
                      </button>
                    {/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        {:else}
          <p class="empty-inline">Select a subscription to view its price history and billing log.</p>
        {/if}
      </div>
    </section>
  {:else}
    <section class="panel" aria-label="Upcoming Subscriptions">
      {#if upcomingRows.length === 0}
        <div class="empty-box">
          <h3>No upcoming charges in the next 60 days</h3>
          <p>Active subscriptions will appear here as their billing dates approach.</p>
        </div>
      {:else}
        <table class="data-table">
          <thead>
            <tr>
              <th>Date</th>
              <th>Subscription</th>
              <th>Account</th>
              <th>Billed</th>
              <th>Status</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            {#each upcomingRows as occ (rowKey(occ))}
              <tr>
                <td class="num">{occ.effectiveDate}</td>
                <td>{occ.subscriptionName}</td>
                <td>{occ.accountName}</td>
                <td class="num">
                  {formatCurrencyMinor(occ.billedCurrency, occ.billedAmountMinor)}
                </td>
                <td>
                  {#if occ.billedCurrency === 'USD' && occ.status !== 'reconciled' && occ.status !== 'skipped'}
                    <span class="tag tag-pending">Reconcile at month-end</span>
                  {:else}
                    <span class="tag">{occ.status.replace('_', ' ')}</span>
                  {/if}
                </td>
                <td class="actions-cell">
                  {#if occ.status !== 'reconciled' && occ.status !== 'skipped'}
                    {#if occ.billedCurrency === 'JPY'}
                      <button type="button" class="link-btn" onclick={() => handleQuickRecordJpy(occ)}>
                        Record JPY
                      </button>
                    {/if}
                    <button
                      type="button"
                      class="link-btn"
                      onclick={() => {
                        matchingOccurrence = occ;
                        matchOpen = true;
                      }}
                    >
                      Match
                    </button>
                    <button type="button" class="link-btn muted" onclick={() => handleSkip(occ, true)}>
                      Skip
                    </button>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </section>
  {/if}

  <SubscriptionFormDialog
    open={formOpen}
    editing={editingSub}
    {accounts}
    {categories}
    {today}
    onclose={() => (formOpen = false)}
    onsaved={() => void loadAll()}
  />

  <PriceRevisionDialog
    open={revOpen}
    subscription={detail?.subscription ?? null}
    {today}
    onclose={() => (revOpen = false)}
    onsaved={() => void loadAll()}
  />

  <MatchExpenseDialog
    open={matchOpen}
    occurrence={matchingOccurrence}
    onclose={() => (matchOpen = false)}
    onmatched={() => void loadAll()}
  />
</section>

<style>
  .view-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 1rem;
    margin-bottom: 1.25rem;
  }
  .view-header h1 {
    margin: 0;
    font-size: 1.4rem;
  }
  .lead {
    margin: 0.35rem 0 0;
    color: #475569;
    font-size: 0.9rem;
    max-width: 68ch;
  }
  .banner {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem 1rem;
    border-radius: 8px;
    margin-bottom: 1rem;
    font-size: 0.875rem;
  }
  .banner-error {
    background: #fef2f2;
    border: 1px solid #fecaca;
    color: #991b1b;
  }
  .banner-ok {
    background: #ecfdf5;
    border: 1px solid #a7f3d0;
    color: #065f46;
  }
  .summary-strip {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 1rem;
    margin-bottom: 1.25rem;
  }
  .summary-card {
    background: #ffffff;
    border: 1px solid #e2e8f0;
    border-radius: 8px;
    padding: 1rem 1.15rem;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .summary-card.attention {
    border-color: #f59e0b;
    background: #fffbeb;
  }
  .card-label {
    font-size: 0.75rem;
    font-weight: 700;
    color: #64748b;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .card-figure {
    font-size: 1.25rem;
    font-variant-numeric: tabular-nums;
    color: #0f172a;
  }
  .card-meta {
    font-size: 0.8rem;
    color: #475569;
  }
  .sub-tabs {
    display: flex;
    gap: 0.5rem;
    border-bottom: 1px solid #e2e8f0;
    margin-bottom: 1rem;
  }
  .sub-tabs button {
    border: none;
    background: transparent;
    padding: 0.65rem 1rem;
    font-weight: 600;
    color: #475569;
    cursor: pointer;
    border-bottom: 2px solid transparent;
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
  }
  .sub-tabs button.active {
    color: #1d4ed8;
    border-bottom-color: #1d4ed8;
  }
  .pill {
    background: #d97706;
    color: #ffffff;
    font-size: 0.72rem;
    padding: 0.1rem 0.45rem;
    border-radius: 999px;
  }
  .panel {
    background: #ffffff;
    border: 1px solid #e2e8f0;
    border-radius: 8px;
    padding: 1rem 1.25rem;
  }
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    gap: 1rem;
    align-items: center;
    margin-bottom: 1rem;
    font-size: 0.875rem;
  }
  .inline-field {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    font-weight: 600;
    color: #334155;
  }
  .inline-field input,
  .toolbar select {
    padding: 0.35rem 0.55rem;
    border: 1px solid #cbd5e1;
    border-radius: 6px;
  }
  .checkbox-label {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    color: #334155;
    cursor: pointer;
  }
  .data-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.875rem;
  }
  .data-table th {
    text-align: left;
    font-size: 0.75rem;
    text-transform: uppercase;
    color: #64748b;
    border-bottom: 1px solid #e2e8f0;
    padding: 0.55rem 0.5rem;
  }
  .data-table td {
    padding: 0.65rem 0.5rem;
    border-bottom: 1px solid #f1f5f9;
    vertical-align: top;
  }
  .data-table.compact td {
    padding: 0.45rem 0.4rem;
  }
  .num {
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .cell-title {
    font-weight: 600;
    color: #0f172a;
  }
  .tag {
    display: inline-block;
    font-size: 0.72rem;
    font-weight: 600;
    padding: 0.1rem 0.45rem;
    border-radius: 4px;
    background: #f1f5f9;
    color: #334155;
    text-transform: capitalize;
  }
  .tag-pending {
    background: #fef3c7;
    color: #92400e;
  }
  .tag-muted {
    background: #e2e8f0;
    color: #475569;
  }
  .jpy-input,
  .date-input {
    padding: 0.4rem 0.5rem;
    border: 1px solid #cbd5e1;
    border-radius: 6px;
    font-size: 0.875rem;
    font-variant-numeric: tabular-nums;
  }
  .jpy-input {
    width: 110px;
  }
  .jpy-input.invalid {
    border-color: #dc2626;
    background: #fef2f2;
  }
  .field-err {
    color: #b91c1c;
    font-size: 0.75rem;
    margin-top: 0.25rem;
  }
  .mini-check {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.75rem;
    color: #475569;
    margin-top: 0.2rem;
  }
  .actions-cell {
    display: flex;
    gap: 0.6rem;
    align-items: center;
  }
  .batch-bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding-top: 1rem;
    margin-top: 0.5rem;
    border-top: 1px solid #e2e8f0;
    font-size: 0.875rem;
  }
  .master-detail {
    display: grid;
    grid-template-columns: 360px 1fr;
    gap: 1rem;
  }
  .list-pane,
  .detail-pane {
    background: #ffffff;
    border: 1px solid #e2e8f0;
    border-radius: 8px;
    padding: 1rem;
  }
  .sub-list {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .sub-row {
    width: 100%;
    text-align: left;
    display: flex;
    justify-content: space-between;
    gap: 0.75rem;
    padding: 0.65rem 0.75rem;
    border: 1px solid transparent;
    border-radius: 6px;
    background: transparent;
    cursor: pointer;
  }
  .sub-row:hover {
    background: #f8fafc;
  }
  .sub-row.selected {
    background: #eff6ff;
    border-color: #bfdbfe;
  }
  .row-right {
    text-align: right;
  }
  .row-meta {
    font-size: 0.78rem;
    color: #64748b;
    margin-top: 0.15rem;
  }
  .detail-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 1rem;
    border-bottom: 1px solid #e2e8f0;
    padding-bottom: 0.85rem;
    margin-bottom: 1rem;
  }
  .detail-header h2 {
    margin: 0;
    font-size: 1.15rem;
  }
  .detail-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.45rem;
  }
  .rev-list {
    list-style: none;
    padding: 0;
    margin: 0 0 1.25rem;
    font-size: 0.875rem;
  }
  .rev-list li {
    display: flex;
    gap: 0.85rem;
    padding: 0.35rem 0;
    border-bottom: 1px solid #f1f5f9;
  }
  .empty-box {
    text-align: center;
    padding: 2rem 1rem;
    color: #475569;
  }
  .empty-box h3 {
    margin: 0 0 0.35rem;
    color: #0f172a;
  }
  .empty-inline,
  .loading {
    color: #64748b;
    font-size: 0.9rem;
    padding: 1rem 0;
  }
  .muted {
    color: #64748b;
  }
  .btn-primary {
    background: #1d4ed8;
    color: #ffffff;
    border: none;
    padding: 0.55rem 1rem;
    border-radius: 6px;
    font-weight: 600;
    cursor: pointer;
  }
  .btn-secondary {
    background: #ffffff;
    color: #334155;
    border: 1px solid #cbd5e1;
    padding: 0.45rem 0.85rem;
    border-radius: 6px;
    font-weight: 600;
    cursor: pointer;
    font-size: 0.825rem;
  }
  .btn-danger {
    background: #ffffff;
    color: #b91c1c;
    border: 1px solid #fecaca;
    padding: 0.45rem 0.85rem;
    border-radius: 6px;
    font-weight: 600;
    cursor: pointer;
    font-size: 0.825rem;
  }
  .link-btn {
    border: none;
    background: transparent;
    color: #1d4ed8;
    font-weight: 600;
    font-size: 0.825rem;
    cursor: pointer;
    padding: 0;
  }
  .link-btn.muted {
    color: #64748b;
  }
  button:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }
</style>
