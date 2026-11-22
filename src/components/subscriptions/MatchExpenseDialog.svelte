<script lang="ts">
  import { listFinancialEvents } from '../../lib/ledger';
  import { matchSubscriptionOccurrence, newWrite } from '../../lib/subscriptions';
  import { toAppError } from '../../lib/commands';
  import { formatCurrencyMinor, formatYen } from '../../lib/money';
  import type {
    AppError,
    FinancialEvent,
    MatchOccurrenceInput,
    SubscriptionOccurrence,
    Write,
  } from '../../lib/types';

  interface Props {
    open: boolean;
    occurrence: SubscriptionOccurrence | null;
    onclose: () => void;
    onmatched: () => void;
  }

  let { open, occurrence, onclose, onmatched }: Props = $props();

  let dialogEl = $state<HTMLDialogElement | null>(null);
  let candidates = $state<FinancialEvent[]>([]);
  let loading = $state(false);
  let saving = $state(false);
  let error = $state<AppError | null>(null);
  let search = $state('');
  let selectedEventId = $state<string | null>(null);
  let pendingWrite = $state<Write<MatchOccurrenceInput> | null>(null);

  const filtered = $derived(
    candidates.filter((e) => {
      if (!search.trim()) return true;
      const q = search.trim().toLowerCase();
      return (
        (e.description ?? '').toLowerCase().includes(q) ||
        String(e.amount).includes(q) ||
        (e.occurredOn ?? '').includes(q)
      );
    }),
  );

  async function loadCandidates(occ: SubscriptionOccurrence) {
    loading = true;
    error = null;
    try {
      const page = await listFinancialEvents({
        accountId: occ.accountId,
        page: 1,
        pageSize: 50,
      });
      candidates = page.items.filter((e) => e.kind === 'EXPENSE' && !e.isReversal);
    } catch (err) {
      error = toAppError(err);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    if (open && occurrence) {
      search = '';
      selectedEventId = null;
      pendingWrite = null;
      void loadCandidates(occurrence);
      if (dialogEl && !dialogEl.open) {
        if (typeof dialogEl.showModal === 'function') dialogEl.showModal();
        else dialogEl.setAttribute('open', '');
      }
    } else if (dialogEl?.open) {
      if (typeof dialogEl.close === 'function') dialogEl.close();
      else dialogEl.removeAttribute('open');
    }
  });

  async function handleMatch() {
    if (!occurrence || !selectedEventId) return;
    saving = true;
    error = null;
    try {
      const req =
        pendingWrite ??
        newWrite<MatchOccurrenceInput>({
          subscriptionId: occurrence.subscriptionId,
          scheduledDate: occurrence.scheduledDate,
          eventId: selectedEventId,
        });
      pendingWrite = req;
      await matchSubscriptionOccurrence(req);
      onmatched();
      onclose();
    } catch (err) {
      error = toAppError(err);
    } finally {
      saving = false;
    }
  }
</script>

{#if open && occurrence}
  <dialog
    bind:this={dialogEl}
    class="match-dialog"
    aria-labelledby="match-title"
    oncancel={(e) => {
      e.preventDefault();
      if (!saving) onclose();
    }}
  >
    <div class="dialog-body">
      <header class="dialog-header">
        <h2 id="match-title">Match Existing Expense</h2>
        <button type="button" class="close-btn" onclick={onclose} disabled={saving} aria-label="Close">
          ×
        </button>
      </header>
      <p class="context">
        Link an already-recorded expense on <strong>{occurrence.accountName}</strong> to
        <strong>{occurrence.subscriptionName}</strong> ({occurrence.effectiveDate} ·
        {formatCurrencyMinor(occurrence.billedCurrency, occurrence.billedAmountMinor)}).
      </p>

      {#if error}
        <div class="alert" role="alert"><strong>{error.code}:</strong> {error.message}</div>
      {/if}

      <input
        type="search"
        class="search-input"
        placeholder="Filter by description, amount, or date"
        bind:value={search}
        aria-label="Filter expenses"
      />

      {#if loading}
        <p class="empty">Loading recent expenses …</p>
      {:else if filtered.length === 0}
        <p class="empty">No matching expense entries found on {occurrence.accountName}.</p>
      {:else}
        <ul class="event-list" role="listbox" aria-label="Candidate expenses">
          {#each filtered as ev (ev.id)}
            {@const posting = ev.postings.find((p) => p.accountId === occurrence.accountId)}
            <li>
              <label class:selected={selectedEventId === ev.id}>
                <input
                  type="radio"
                  name="matchEvent"
                  value={ev.id}
                  checked={selectedEventId === ev.id}
                  onchange={() => {
                    selectedEventId = ev.id;
                    pendingWrite = null;
                  }}
                />
                <span class="ev-date">{posting?.effectiveDate ?? ev.occurredOn ?? 'Unknown'}</span>
                <span class="ev-desc">{ev.description ?? 'Expense'}</span>
                <strong class="ev-amt">{formatYen(ev.amount)}</strong>
              </label>
            </li>
          {/each}
        </ul>
      {/if}

      <footer class="dialog-footer">
        <button type="button" class="btn-secondary" onclick={onclose} disabled={saving}>Cancel</button>
        <button
          type="button"
          class="btn-primary"
          onclick={handleMatch}
          disabled={saving || !selectedEventId}
        >
          {saving ? 'Matching …' : error ? 'Try again' : 'Link Expense'}
        </button>
      </footer>
    </div>
  </dialog>
{/if}

<style>
  .match-dialog {
    width: min(560px, 92vw);
    border: 1px solid #cbd5e1;
    border-radius: 10px;
    padding: 0;
    background: #ffffff;
  }
  .dialog-body {
    padding: 1.25rem 1.5rem;
  }
  .dialog-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .dialog-header h2 {
    margin: 0;
    font-size: 1.05rem;
  }
  .close-btn {
    border: none;
    background: transparent;
    font-size: 1.4rem;
    cursor: pointer;
  }
  .context {
    margin: 0.45rem 0 0.9rem;
    font-size: 0.875rem;
    color: #475569;
  }
  .alert {
    background: #fef2f2;
    border: 1px solid #fecaca;
    color: #991b1b;
    padding: 0.6rem 0.8rem;
    border-radius: 6px;
    font-size: 0.85rem;
    margin-bottom: 0.75rem;
  }
  .search-input {
    width: 100%;
    box-sizing: border-box;
    padding: 0.5rem 0.65rem;
    border: 1px solid #cbd5e1;
    border-radius: 6px;
    margin-bottom: 0.75rem;
  }
  .event-list {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 260px;
    overflow-y: auto;
    border: 1px solid #e2e8f0;
    border-radius: 6px;
  }
  .event-list li + li {
    border-top: 1px solid #f1f5f9;
  }
  .event-list label {
    display: grid;
    grid-template-columns: auto 95px 1fr auto;
    gap: 0.6rem;
    align-items: center;
    padding: 0.55rem 0.7rem;
    cursor: pointer;
    font-size: 0.875rem;
  }
  .event-list label.selected {
    background: #eff6ff;
  }
  .ev-date,
  .ev-amt {
    font-variant-numeric: tabular-nums;
  }
  .empty {
    color: #64748b;
    font-size: 0.875rem;
    text-align: center;
    padding: 1.5rem 0;
  }
  .dialog-footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.6rem;
    margin-top: 1rem;
  }
  .btn-primary {
    background: #1d4ed8;
    color: #ffffff;
    border: none;
    padding: 0.5rem 0.95rem;
    border-radius: 6px;
    font-weight: 600;
    cursor: pointer;
  }
  .btn-secondary {
    background: #ffffff;
    color: #334155;
    border: 1px solid #cbd5e1;
    padding: 0.5rem 0.95rem;
    border-radius: 6px;
    font-weight: 600;
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
</style>
