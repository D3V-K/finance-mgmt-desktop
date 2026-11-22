import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it } from 'vitest';
import { app } from '../lib/app.svelte';
import type { Balance, FinancialEvent, LegacyRecords } from '../lib/types';
import { backend, balance, BANK, CARD, FOOD, readyApp, TODAY, written } from '../test/backend';
import Accounts from './Accounts.svelte';
import LedgerStart from './LedgerStart.svelte';

const legacyImported: FinancialEvent = {
  id: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
  kind: 'EXPENSE',
  amount: 800,
  categoryId: FOOD.id,
  description: 'Lunch',
  occurredOn: null,
  dateBasis: 'legacy_recorded_date',
  recordedAt: '2026-01-05T00:00:00Z',
  originalEventId: null,
  reversesEventId: null,
  replacesEventId: null,
  isReversal: false,
  postings: [{ accountId: BANK.id, delta: -800, effectiveDate: '2026-01-05' }],
};

describe('accounts', () => {
  beforeEach(() => readyApp());

  it('shows an unknown opening as Unknown with its reason, never ¥0', async () => {
    const balances: Record<string, Balance> = {
      [BANK.id]: balance(BANK.id, null, 'unavailable'),
      [CARD.id]: balance(CARD.id, -30_000),
    };
    backend({
      get_account_balance: (args) => balances[args!.accountId as string],
      list_financial_events: { items: [], total: 0, page: 1, pageSize: 50 },
      list_opening_revisions: [],
    });
    render(Accounts, { selectedId: BANK.id });
    expect(await screen.findByText('No opening balance recorded.')).toBeInTheDocument();
    const detail = screen.getByRole('region', { name: 'Account detail' });
    expect(within(detail).getAllByText('Unknown').length).toBeGreaterThan(0);
    expect(detail).not.toHaveTextContent('¥0');
    expect(screen.getByRole('button', { name: 'Set opening…' })).toBeInTheDocument();
    const list = screen.getByRole('region', { name: 'Account list' });
    await waitFor(() => expect(within(list).getByText('owed')).toBeInTheDocument());
  });

  it('labels a purchase with no known date instead of guessing one', async () => {
    backend({
      get_account_balance: balance(BANK.id, 100_000),
      list_financial_events: { items: [legacyImported], total: 1, page: 1, pageSize: 50 },
      list_opening_revisions: [],
    });
    render(Accounts, { selectedId: BANK.id });
    const row = (await screen.findByRole('button', { name: 'Lunch' })).closest('tr')!;
    expect(row).toHaveTextContent('Purchase date unknown');
    expect(row).toHaveTextContent('Posted 5 Jan 2026');
    expect(row).toHaveTextContent('Earlier-record date, not the purchase date');
  });

  it('moves through the account list with the arrow keys', async () => {
    backend({
      get_account_balance: (args) => balance(args!.accountId as string, 0),
      list_financial_events: { items: [], total: 0, page: 1, pageSize: 50 },
      list_opening_revisions: [],
    });
    render(Accounts, { selectedId: null });
    const all = await screen.findByRole('button', { name: /All accounts/ });
    all.focus();
    const user = userEvent.setup();
    await user.keyboard('{ArrowDown}');
    expect(app.route).toEqual({ name: 'accounts', accountId: BANK.id });
  });
});

describe('ledger start', () => {
  beforeEach(() => readyApp());

  const records: LegacyRecords = {
    transactions: [
      { id: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb', amount: 500, description: 'Old coffee', transactionDate: '2026-10-01', categoryId: FOOD.id, accountType: 'CASH' },
      { id: 'cccccccc-cccc-4ccc-8ccc-cccccccccccc', amount: 900, description: 'New lunch', transactionDate: '2026-11-19', categoryId: FOOD.id, accountType: 'BANK' },
    ],
    transfers: [],
  };

  function startBackend() {
    return backend({
      list_legacy_records: records,
      get_legacy_reports: { monthly: [], netBalanceSeries: [], balances: { cashBalance: 0, bankBalance: 0, totalBalance: 0 } },
      list_legacy_provenance: [],
      get_account_balance: (args) => balance(args!.accountId as string, null, 'incomplete_legacy_mapping'),
      get_legacy_balance: 12_000,
      preview_cutover: (args) => ({
        asOfDate: (args!.input as { asOfDate: string }).asOfDate,
        legacyTransactionCount: 2,
        legacyTransferCount: 0,
        categorySums: {},
        legacyDateMin: '2026-10-01',
        legacyDateMax: '2026-11-19',
        unresolvedTransactionIds: [records.transactions[1]!.id],
        unresolvedTransferIds: [],
        unresolvedCount: 1,
        mappedCount: 0,
        openingTotal: 170_000,
        legacyAccountTypeIsUnverified: true,
        purchaseDateBasis: 'unknown_without_confirmation',
      }),
      confirm_cutover: { isReplay: false, result: { id: 'c1', asOfDate: TODAY, unresolvedCount: 1, status: 'incomplete' } },
    });
  }

  it('needs an explicit opening for every account and never maps earlier records by guessing', async () => {
    const api = startBackend();
    render(LedgerStart);
    const user = userEvent.setup();
    await screen.findByRole('heading', { name: 'Start the ledger' });

    // Only the record on/after the start date needs mapping; it defaults to unresolved.
    const mapping = await screen.findByRole('table', { name: 'Earlier records to map' });
    expect(within(mapping).getAllByRole('row')).toHaveLength(2);
    expect(within(mapping).getByLabelText(/Account for record/)).toHaveValue('');
    expect(screen.getByText(/not applied automatically/)).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Review start…' }));
    expect(screen.getByText('Enter an opening for every account (0 is fine).')).toBeInTheDocument();
    expect(api.of('preview_cutover')).toHaveLength(0);

    await user.type(screen.getByLabelText('Everyday — balance'), '200000');
    await user.type(screen.getByLabelText('Visa — amount owed'), '30000');
    await user.click(screen.getByRole('button', { name: 'Review start…' }));
    expect(await screen.findByText('Check before starting')).toBeInTheDocument();
    expect(screen.getByText(/cannot be undone/)).toBeInTheDocument();
    expect(screen.getByText('Balances stay incomplete')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Start the ledger' }));
    await waitFor(() => expect(api.of('confirm_cutover')).toHaveLength(1));
    expect(written(api.of('confirm_cutover')[0])?.payload).toEqual({
      asOfDate: TODAY,
      openings: [
        { accountId: BANK.id, amount: 200_000 },
        { accountId: CARD.id, amount: -30_000 },
      ],
      transactions: [],
      transfers: [],
    });
  });
});
