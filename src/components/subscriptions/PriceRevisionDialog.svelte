<script lang="ts">
  import { newWrite, reviseSubscriptionPrice } from '../../lib/subscriptions';
  import { toAppError } from '../../lib/commands';
  import { parseCurrencyInput } from '../../lib/money';
  import type {
    AppError,
    DateOnly,
    PriceRevisionInput,
    SubscriptionListItem,
    Write,
  } from '../../lib/types';

  interface Props {
    open: boolean;
    subscription: SubscriptionListItem | null;
    today: DateOnly;
    onclose: () => void;
    onsaved: () => void;
  }

  let { open, subscription, today, onclose, onsaved }: Props = $props();

  let dialogEl = $state<HTMLDialogElement | null>(null);
  let effectiveFrom = $state('');
  let amountText = $state('');
  let reason = $state('');
  let saving = $state(false);
  let formError = $state<AppError | null>(null);
  let fieldError = $state<string | null>(null);
  let pendingWrite = $state<Write<PriceRevisionInput> | null>(null);

  $effect(() => {
    if (open && subscription) {
      effectiveFrom = today;
      amountText = '';
      reason = '';
      formError = null;
      fieldError = null;
      pendingWrite = null;
      if (dialogEl && !dialogEl.open) {
        if (typeof dialogEl.showModal === 'function') dialogEl.showModal();
        else dialogEl.setAttribute('open', '');
      }
    } else if (dialogEl?.open) {
      if (typeof dialogEl.close === 'function') dialogEl.close();
      else dialogEl.removeAttribute('open');
    }
  });

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    if (!subscription) return;
    formError = null;
    fieldError = null;
    const parsed = parseCurrencyInput(subscription.currency, amountText);
    if (!parsed.ok) {
      fieldError = parsed.reason;
      return;
    }
    if (!reason.trim()) {
      fieldError = 'Enter a reason for the price change.';
      return;
    }
    saving = true;
    try {
      const req =
        pendingWrite ??
        newWrite<PriceRevisionInput>({
          subscriptionId: subscription.id,
          effectiveFrom,
          amountMinor: parsed.value,
          reason: reason.trim(),
        });
      pendingWrite = req;
      await reviseSubscriptionPrice(req);
      onsaved();
      onclose();
    } catch (err) {
      formError = toAppError(err);
    } finally {
      saving = false;
    }
  }
</script>

{#if open && subscription}
  <dialog
    bind:this={dialogEl}
    class="rev-dialog"
    aria-labelledby="rev-title"
    oncancel={(e) => {
      e.preventDefault();
      if (!saving) onclose();
    }}
  >
    <form method="dialog" onsubmit={handleSubmit}>
      <header class="dialog-header">
        <h2 id="rev-title">Change Plan Price — {subscription.name}</h2>
        <button type="button" class="close-btn" onclick={onclose} disabled={saving} aria-label="Close">
          ×
        </button>
      </header>
      <p class="subtitle">
        Appends a new price revision effective on the selected date. Prior billing cycles keep their
        original price.
      </p>

      {#if formError}
        <div class="alert" role="alert"><strong>{formError.code}:</strong> {formError.message}</div>
      {/if}
      {#if fieldError}
        <div class="alert" role="alert">{fieldError}</div>
      {/if}

      <div class="fields">
        <label>
          <span>Effective from</span>
          <input
            type="date"
            bind:value={effectiveFrom}
            onchange={() => (pendingWrite = null)}
            required
          />
        </label>
        <label>
          <span>New price ({subscription.currency})</span>
          <input
            type="text"
            inputmode={subscription.currency === 'JPY' ? 'numeric' : 'decimal'}
            bind:value={amountText}
            oninput={() => (pendingWrite = null)}
            placeholder={subscription.currency === 'JPY' ? '1,500' : '25.00'}
            required
          />
        </label>
        <label>
          <span>Reason</span>
          <input
            type="text"
            bind:value={reason}
            oninput={() => (pendingWrite = null)}
            placeholder="e.g. Annual price increase"
            required
          />
        </label>
      </div>

      <footer class="dialog-footer">
        <button type="button" class="btn-secondary" onclick={onclose} disabled={saving}>Cancel</button>
        <button type="submit" class="btn-primary" disabled={saving}>
          {saving ? 'Saving …' : formError ? 'Try again' : 'Save Revision'}
        </button>
      </footer>
    </form>
  </dialog>
{/if}

<style>
  .rev-dialog {
    width: min(440px, 92vw);
    border: 1px solid #cbd5e1;
    border-radius: 10px;
    padding: 0;
    background: #ffffff;
  }
  form {
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
  .subtitle {
    margin: 0.4rem 0 1rem;
    font-size: 0.85rem;
    color: #475569;
  }
  .alert {
    background: #fef2f2;
    border: 1px solid #fecaca;
    color: #991b1b;
    padding: 0.6rem 0.8rem;
    border-radius: 6px;
    font-size: 0.85rem;
    margin-bottom: 0.85rem;
  }
  .fields {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }
  .fields label {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.85rem;
  }
  .fields span {
    font-weight: 600;
    color: #334155;
  }
  .fields input {
    padding: 0.5rem 0.6rem;
    border: 1px solid #cbd5e1;
    border-radius: 6px;
    font-size: 0.9rem;
  }
  .dialog-footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.6rem;
    margin-top: 1.1rem;
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
</style>
