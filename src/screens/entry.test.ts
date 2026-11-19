import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it } from 'vitest';
import { app } from '../lib/app.svelte';
import type { FinancialEvent } from '../lib/types';
import { backend, BANK, CARD, FOOD, readyApp, TODAY, written } from '../test/backend';
import EventDialog from './EventDialog.svelte';
import QuickEntry from './QuickEntry.svelte';

function saved(payload: object, isReplay = false) {
  return { isReplay, result: { id: 'e1', postings: [], ...payload } };
}

const purchase: FinancialEvent = {
  id: '55555555-5555-4555-8555-555555555555',
  kind: 'EXPENSE',
  amount: 1200,
  categoryId: FOOD.id,
  description: 'Supermarket',
  occurredOn: '2026-11-18',
  dateBasis: 'known',
  recordedAt: '2026-11-18T03:00:00Z',
  originalEventId: null,
  reversesEventId: null,
  replacesEventId: null,
  isReversal: false,
  postings: [{ accountId: BANK.id, delta: -1200, effectiveDate: '2026-11-18' }],
};

describe('quick entry', () => {
  beforeEach(() => readyApp());

  it('records spending with explicit account, category and both dates', async () => {
    const api = backend({ create_financial_event: (args) => saved((args!.request as { payload: object }).payload) });
    const onclose = vi.fn();
    render(QuickEntry, { open: true, onclose });
    const user = userEvent.setup();

    await user.type(screen.getByLabelText('Amount'), '1,200');
    await user.type(screen.getByLabelText(/Description/), 'Supermarket');
    await user.click(screen.getByRole('button', { name: 'Save entry' }));

    await waitFor(() => expect(onclose).toHaveBeenCalled());
    expect(written(api.of('create_financial_event')[0])?.payload).toEqual({
      kind: 'EXPENSE',
      amount: 1200,
      accountId: BANK.id,
      categoryId: FOOD.id,
      description: 'Supermarket',
      occurredOn: TODAY,
      effectiveDate: TODAY,
    });
    expect(app.toasts.at(-1)?.message).toBe('Saved: spending ¥1,200');
  });

  it('keeps the form after a failed save and retries with the same key so nothing posts twice', async () => {
    const api = backend({
      create_financial_event: (args, n) => {
        if (n === 1) throw { code: 'STORAGE_FAILURE', message: 'Local storage could not be written.', category: 'storage' };
        return saved((args!.request as { payload: object }).payload, true);
      },
    });
    const onclose = vi.fn();
    render(QuickEntry, { open: true, onclose });
    const user = userEvent.setup();
    await user.type(screen.getByLabelText('Amount'), '980');
    await user.click(screen.getByRole('button', { name: 'Save entry' }));

    expect(await screen.findByText('Nothing was saved.')).toBeInTheDocument();
    expect(screen.getByLabelText('Amount')).toHaveValue('980');
    expect(onclose).not.toHaveBeenCalled();

    await user.click(screen.getByRole('button', { name: 'Try again' }));
    await waitFor(() => expect(onclose).toHaveBeenCalled());
    const [first, retry] = api.of('create_financial_event').map(written);
    expect(retry?.idempotencyKey).toBe(first?.idempotencyKey);
    expect(retry?.payload).toEqual(first?.payload);
    expect(app.toasts.at(-1)?.message).toMatch(/Already saved/);
  });

  it('disables submit while saving so a double click sends one write', async () => {
    let resolve: (v: unknown) => void = () => {};
    const api = backend({ create_financial_event: () => new Promise((r) => (resolve = r)) });
    render(QuickEntry, { open: true, onclose: () => {} });
    const user = userEvent.setup();
    await user.type(screen.getByLabelText('Amount'), '500');
    const button = screen.getByRole('button', { name: 'Save entry' });
    await user.click(button);
    expect(screen.getByRole('button', { name: 'Saving…' })).toBeDisabled();
    await user.click(screen.getByRole('button', { name: 'Saving…' }));
    expect(api.of('create_financial_event')).toHaveLength(1);
    resolve(saved({ amount: 500 }));
  });

  it('shows Rust field errors on the field and blocks invalid amounts locally', async () => {
    backend({
      create_financial_event: () => {
        throw { code: 'INVALID_DATE', message: 'Occurrence cannot precede confirmed cutover.', category: 'validation', field: 'occurredOn' };
      },
    });
    render(QuickEntry, { open: true, onclose: () => {} });
    const user = userEvent.setup();
    await user.type(screen.getByLabelText('Amount'), '12.5');
    await user.click(screen.getByRole('button', { name: 'Save entry' }));
    expect(screen.getByText('Use whole yen — no decimals.')).toBeInTheDocument();
    expect(screen.getByLabelText('Amount')).toHaveAttribute('aria-invalid', 'true');

    await user.clear(screen.getByLabelText('Amount'));
    await user.type(screen.getByLabelText('Amount'), '120');
    await user.click(screen.getByRole('button', { name: 'Save entry' }));
    expect(await screen.findByText('Occurrence cannot precede confirmed cutover.')).toBeInTheDocument();
    expect(screen.getByLabelText('Date it happened')).toHaveAttribute('aria-invalid', 'true');
  });

  it('offers only cards as the destination of a card payment and never a category', async () => {
    const api = backend({ create_financial_event: (args) => saved((args!.request as { payload: object }).payload) });
    render(QuickEntry, { open: true, onclose: () => {} });
    const user = userEvent.setup();
    await user.click(screen.getByLabelText('Card payment'));
    const cardSelect = screen.getByLabelText('Card');
    expect(within(cardSelect).getAllByRole('option').map((o) => o.textContent)).toEqual([CARD.name]);
    expect(within(screen.getByLabelText('Paid from')).queryByText(/Visa/)).toBeNull();
    expect(screen.queryByLabelText('Category')).toBeNull();
    await user.type(screen.getByLabelText('Amount'), '30000');
    await user.click(screen.getByRole('button', { name: 'Save entry' }));
    await waitFor(() => expect(api.of('create_financial_event')).toHaveLength(1));
    expect(written(api.of('create_financial_event')[0])?.payload).toMatchObject({
      kind: 'CARD_REPAYMENT',
      accountId: BANK.id,
      destinationAccountId: CARD.id,
    });
    expect(written(api.of('create_financial_event')[0])?.payload).not.toHaveProperty('categoryId');
  });

  it('requires a reason for an adjustment and sends a signed delta', async () => {
    const api = backend({ create_financial_event: (args) => saved((args!.request as { payload: object }).payload) });
    render(QuickEntry, { open: true, onclose: () => {} });
    const user = userEvent.setup();
    await user.click(screen.getByLabelText('Adjustment'));
    await user.type(screen.getByLabelText('Amount'), '300');
    await user.click(screen.getByLabelText('Decrease balance'));
    await user.click(screen.getByRole('button', { name: 'Save entry' }));
    expect(screen.getByText('Say why the balance is being adjusted.')).toBeInTheDocument();
    await user.type(screen.getByLabelText(/Reason for adjustment/), 'Bank fee missed');
    await user.click(screen.getByRole('button', { name: 'Save entry' }));
    await waitFor(() => expect(api.of('create_financial_event')).toHaveLength(1));
    expect(written(api.of('create_financial_event')[0])?.payload).toMatchObject({ kind: 'BALANCE_ADJUSTMENT', amount: 300, adjustmentDelta: -300 });
  });
});

describe('reversal and correction', () => {
  beforeEach(() => readyApp());

  it('explains a reversal and requires a reason before reversing', async () => {
    const api = backend({ reverse_financial_event: { isReplay: false, result: { ...purchase, id: 'r1', isReversal: true } } });
    const onclose = vi.fn();
    render(EventDialog, { event: purchase, reversedIds: new Set<string>(), onclose });
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Reverse…' }));
    expect(screen.getByText(/exactly cancels this one/)).toBeInTheDocument();
    expect(screen.getByText(/nothing is deleted/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Reverse entry' }));
    expect(api.of('reverse_financial_event')).toHaveLength(0);
    await user.type(screen.getByLabelText('Reason'), 'Duplicate');
    await user.click(screen.getByRole('button', { name: 'Reverse entry' }));
    await waitFor(() => expect(onclose).toHaveBeenCalled());
    expect(written(api.of('reverse_financial_event')[0])?.payload).toEqual({ eventId: purchase.id, reason: 'Duplicate' });
  });

  it('corrects through one replace command that keeps the original linked', async () => {
    const api = backend({ replace_financial_event: (args) => ({ isReplay: false, result: { ...purchase, id: 'n1', ...(args!.request as { payload: { replacement: object } }).payload.replacement, replacesEventId: purchase.id } }) });
    const onclose = vi.fn();
    render(EventDialog, { event: purchase, reversedIds: new Set<string>(), onclose });
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Correct…' }));
    expect(screen.getByText(/keeps the original entry/)).toBeInTheDocument();
    expect(screen.getByLabelText('Amount')).toHaveValue('1200');
    await user.clear(screen.getByLabelText('Amount'));
    await user.type(screen.getByLabelText('Amount'), '1250');
    await user.type(screen.getByLabelText('Why is this being corrected?'), 'Receipt said 1,250');
    await user.click(screen.getByRole('button', { name: 'Save correction' }));
    await waitFor(() => expect(onclose).toHaveBeenCalled());
    const req = written(api.of('replace_financial_event')[0]);
    expect(req?.payload).toMatchObject({ eventId: purchase.id, reason: 'Receipt said 1,250', replacement: { kind: 'EXPENSE', amount: 1250, accountId: BANK.id, categoryId: FOOD.id, occurredOn: '2026-11-18' } });
    expect(api.of('create_financial_event')).toHaveLength(0);
  });

  it('offers no reverse or correct on a reversal or an already-reversed entry', () => {
    backend({});
    render(EventDialog, { event: purchase, reversedIds: new Set([purchase.id]), onclose: () => {} });
    expect(screen.getByText(/has been reversed/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Reverse…' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Correct…' })).toBeNull();
  });
});
