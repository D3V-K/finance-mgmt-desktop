<script lang="ts">
  import { createSubscription, newWrite, updateSubscription } from '../../lib/subscriptions';
  import { toAppError } from '../../lib/commands';
  import { formatCurrencyMinor, parseCurrencyInput } from '../../lib/money';
  import type {
    Account,
    AppError,
    BillingCurrency,
    Category,
    DateOnly,
    SubscriptionCadence,
    SubscriptionListItem,
    Write,
    CreateSubscriptionInput,
    UpdateSubscriptionInput,
  } from '../../lib/types';

  interface Props {
    open: boolean;
    editing: SubscriptionListItem | null;
    accounts: Account[];
    categories: Category[];
    today: DateOnly;
    onclose: () => void;
    onsaved: () => void;
  }

  let { open, editing, accounts, categories, today, onclose, onsaved }: Props = $props();

  let dialogEl = $state<HTMLDialogElement | null>(null);
  let name = $state('');
  let merchant = $state('');
  let currency = $state<BillingCurrency>('JPY');
  let amountText = $state('');
  let cadence = $state<SubscriptionCadence>('monthly');
  let startDate = $state('');
  let billingDay = $state('');
  let billingMonth = $state('');
  let accountId = $state('');
  let categoryId = $state('');
  let note = $state('');

  let saving = $state(false);
  let formError = $state<AppError | null>(null);
  let fieldError = $state<string | null>(null);
  let pendingCreate = $state<Write<CreateSubscriptionInput> | null>(null);
  let pendingUpdate = $state<Write<UpdateSubscriptionInput> | null>(null);

  const expenseCategories = $derived(
    categories.filter((c) => c.type === 'expense' && c.archivedAt === null),
  );
  const activeAccounts = $derived(accounts.filter((a) => a.archivedAt === null));

  $effect(() => {
    if (open) {
      formError = null;
      fieldError = null;
      pendingCreate = null;
      pendingUpdate = null;
      if (editing) {
        name = editing.name;
        merchant = editing.merchant ?? '';
        currency = editing.currency;
        amountText = formatCurrencyMinor(editing.currency, editing.currentAmountMinor);
        cadence = editing.cadence;
        startDate = editing.startDate;
        billingDay = editing.billingDay ? String(editing.billingDay) : '';
        billingMonth = editing.billingMonth ? String(editing.billingMonth) : '';
        accountId = editing.accountId;
        categoryId = editing.categoryId;
        note = editing.note ?? '';
      } else {
        name = '';
        merchant = '';
        currency = 'USD';
        amountText = '';
        cadence = 'monthly';
        startDate = today;
        billingDay = String(Number(today.slice(8, 10)) || 1);
        billingMonth = String(Number(today.slice(5, 7)) || 1);
        accountId = activeAccounts[0]?.id ?? '';
        categoryId = expenseCategories[0]?.id ?? '';
        note = '';
      }
      if (dialogEl && !dialogEl.open) {
        if (typeof dialogEl.showModal === 'function') {
          dialogEl.showModal();
        } else {
          dialogEl.setAttribute('open', '');
        }
      }
    } else if (dialogEl?.open) {
      if (typeof dialogEl.close === 'function') {
        dialogEl.close();
      } else {
        dialogEl.removeAttribute('open');
      }
    }
  });

  function clearPending() {
    pendingCreate = null;
    pendingUpdate = null;
  }

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    formError = null;
    fieldError = null;

    if (!name.trim()) {
      fieldError = 'Enter a subscription name.';
      return;
    }
    if (!accountId || !categoryId) {
      fieldError = 'Select a payment account and an expense category.';
      return;
    }

    saving = true;
    try {
      if (editing) {
        const req =
          pendingUpdate ??
          newWrite<UpdateSubscriptionInput>({
            subscriptionId: editing.id,
            name: name.trim(),
            merchant: merchant.trim() || undefined,
            accountId,
            categoryId,
            note: note.trim() || undefined,
          });
        pendingUpdate = req;
        await updateSubscription(req);
      } else {
        const parsed = parseCurrencyInput(currency, amountText);
        if (!parsed.ok) {
          fieldError = parsed.reason;
          saving = false;
          return;
        }
        const dayNum = cadence === 'weekly' ? undefined : Number(billingDay) || undefined;
        const monthNum = cadence === 'yearly' ? Number(billingMonth) || undefined : undefined;
        const req =
          pendingCreate ??
          newWrite<CreateSubscriptionInput>({
            name: name.trim(),
            merchant: merchant.trim() || undefined,
            accountId,
            categoryId,
            currency,
            amountMinor: parsed.value,
            cadence,
            billingDay: dayNum,
            billingMonth: monthNum,
            startDate,
            note: note.trim() || undefined,
          });
        pendingCreate = req;
        await createSubscription(req);
      }
      onsaved();
      onclose();
    } catch (err) {
      formError = toAppError(err);
    } finally {
      saving = false;
    }
  }
</script>

{#if open}
  <dialog
    bind:this={dialogEl}
    class="sub-dialog"
    aria-labelledby="sub-form-title"
    oncancel={(e) => {
      e.preventDefault();
      if (!saving) onclose();
    }}
  >
    <form method="dialog" onsubmit={handleSubmit}>
      <header class="dialog-header">
        <h2 id="sub-form-title">{editing ? 'Edit Subscription' : 'New Subscription'}</h2>
        <button type="button" class="close-btn" onclick={onclose} disabled={saving} aria-label="Close">
          ×
        </button>
      </header>

      {#if formError}
        <div class="alert" role="alert">
          <strong>{formError.code}:</strong> {formError.message} Nothing was saved.
        </div>
      {/if}
      {#if fieldError}
        <div class="alert" role="alert">{fieldError}</div>
      {/if}

      <div class="form-grid">
        <label class="field full">
          <span>Name</span>
          <input
            type="text"
            bind:value={name}
            oninput={clearPending}
            placeholder="e.g. ChatGPT Plus, iCloud+"
            required
          />
        </label>

        <label class="field full">
          <span>Merchant (optional)</span>
          <input
            type="text"
            bind:value={merchant}
            oninput={clearPending}
            placeholder="e.g. OpenAI, Apple"
          />
        </label>

        {#if !editing}
          <fieldset class="field full currency-group">
            <legend>Billing currency</legend>
            <div class="segmented">
              <label class:selected={currency === 'JPY'}>
                <input
                  type="radio"
                  name="currency"
                  value="JPY"
                  checked={currency === 'JPY'}
                  onchange={() => {
                    currency = 'JPY';
                    clearPending();
                  }}
                />
                <span>JPY (¥ fixed)</span>
              </label>
              <label class:selected={currency === 'USD'}>
                <input
                  type="radio"
                  name="currency"
                  value="USD"
                  checked={currency === 'USD'}
                  onchange={() => {
                    currency = 'USD';
                    clearPending();
                  }}
                />
                <span>USD ($ reconciled at month-end)</span>
              </label>
            </div>
          </fieldset>

          <label class="field">
            <span>{currency === 'JPY' ? 'Amount (JPY)' : 'Billed Price (USD)'}</span>
            <input
              type="text"
              inputmode={currency === 'JPY' ? 'numeric' : 'decimal'}
              bind:value={amountText}
              oninput={clearPending}
              placeholder={currency === 'JPY' ? '1,300' : '20.00'}
              required
            />
          </label>

          <label class="field">
            <span>Cadence</span>
            <select bind:value={cadence} onchange={clearPending}>
              <option value="monthly">Monthly</option>
              <option value="yearly">Yearly</option>
              <option value="quarterly">Quarterly</option>
              <option value="weekly">Weekly</option>
            </select>
          </label>

          <label class="field">
            <span>Start date</span>
            <input type="date" bind:value={startDate} onchange={clearPending} required />
          </label>

          {#if cadence !== 'weekly'}
            <label class="field">
              <span>Billing day (1–31)</span>
              <input
                type="number"
                min="1"
                max="31"
                bind:value={billingDay}
                oninput={clearPending}
              />
            </label>
          {/if}

          {#if cadence === 'yearly'}
            <label class="field">
              <span>Billing month (1–12)</span>
              <input
                type="number"
                min="1"
                max="12"
                bind:value={billingMonth}
                oninput={clearPending}
              />
            </label>
          {/if}
        {/if}

        <label class="field">
          <span>Payment account</span>
          <select bind:value={accountId} onchange={clearPending} required>
            {#each activeAccounts as acc (acc.id)}
              <option value={acc.id}>{acc.name} ({acc.kind})</option>
            {/each}
          </select>
        </label>

        <label class="field">
          <span>Expense category</span>
          <select bind:value={categoryId} onchange={clearPending} required>
            {#each expenseCategories as cat (cat.id)}
              <option value={cat.id}>{cat.name}</option>
            {/each}
          </select>
        </label>

        <label class="field full">
          <span>Note (optional)</span>
          <input type="text" bind:value={note} oninput={clearPending} placeholder="Plan tier or reminder" />
        </label>
      </div>

      {#if !editing && currency === 'USD'}
        <p class="fx-hint">
          USD subscriptions are flagged automatically for month-end bookkeeping so you can enter the
          exact JPY deducted from your bank or credit card statement without tracking FX rates.
        </p>
      {/if}

      <footer class="dialog-footer">
        <button type="button" class="btn-secondary" onclick={onclose} disabled={saving}>
          Cancel
        </button>
        <button type="submit" class="btn-primary" disabled={saving}>
          {#if saving}
            Saving …
          {:else if formError}
            Try again
          {:else}
            {editing ? 'Save Changes' : 'Create Subscription'}
          {/if}
        </button>
      </footer>
    </form>
  </dialog>
{/if}

<style>
  .sub-dialog {
    width: min(560px, 92vw);
    border: 1px solid #cbd5e1;
    border-radius: 10px;
    padding: 0;
    background: #ffffff;
    color: #0f172a;
    box-shadow: 0 20px 50px rgba(15, 23, 42, 0.2);
  }
  .sub-dialog::backdrop {
    background: rgba(15, 23, 42, 0.45);
  }
  form {
    padding: 1.25rem 1.5rem;
  }
  .dialog-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }
  .dialog-header h2 {
    margin: 0;
    font-size: 1.15rem;
  }
  .close-btn {
    border: none;
    background: transparent;
    font-size: 1.4rem;
    cursor: pointer;
    color: #475569;
  }
  .alert {
    background: #fef2f2;
    border: 1px solid #fecaca;
    color: #991b1b;
    padding: 0.65rem 0.85rem;
    border-radius: 6px;
    font-size: 0.875rem;
    margin-bottom: 1rem;
  }
  .form-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.85rem;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    font-size: 0.85rem;
  }
  .field.full {
    grid-column: 1 / -1;
  }
  .field span,
  .currency-group legend {
    font-weight: 600;
    color: #334155;
    font-size: 0.8rem;
  }
  .field input,
  .field select {
    padding: 0.5rem 0.6rem;
    border: 1px solid #cbd5e1;
    border-radius: 6px;
    font-size: 0.9rem;
    font-variant-numeric: tabular-nums;
  }
  .currency-group {
    border: none;
    padding: 0;
    margin: 0;
  }
  .segmented {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.5rem;
    margin-top: 0.3rem;
  }
  .segmented label {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    padding: 0.5rem 0.65rem;
    border: 1px solid #cbd5e1;
    border-radius: 6px;
    cursor: pointer;
    background: #f8fafc;
  }
  .segmented label.selected {
    border-color: #2563eb;
    background: #eff6ff;
    color: #1d4ed8;
    font-weight: 600;
  }
  .fx-hint {
    margin: 0.9rem 0 0;
    font-size: 0.8125rem;
    color: #475569;
    background: #f8fafc;
    border: 1px solid #e2e8f0;
    padding: 0.6rem 0.75rem;
    border-radius: 6px;
  }
  .dialog-footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.6rem;
    margin-top: 1.25rem;
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
    padding: 0.55rem 1rem;
    border-radius: 6px;
    font-weight: 600;
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
</style>
