import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it } from 'vitest';
import type { Dashboard, FinancialEvent, Forecast, Position, Schedule } from '../lib/types';
import { backend, balance, BANK, CARD, FOOD, readyApp, TODAY, written } from '../test/backend';
import Budgets from './Budgets.svelte';
import Cards from './Cards.svelte';
import ForecastScreen from './Forecast.svelte';
import Overview from './Overview.svelte';

const position: Position = {
  asOfDate: TODAY,
  currency: 'JPY',
  liquidCash: 200_000,
  brokerageCash: 0,
  securitiesValue: null,
  signedCardBalances: -30_000,
  netWorth: null,
  coverage: 'incomplete',
  accounts: [
    { accountId: BANK.id, name: BANK.name, kind: 'BANK', balance: 200_000, status: 'complete', securities: null },
    { accountId: CARD.id, name: CARD.name, kind: 'CREDIT_CARD', balance: -30_000, status: 'complete', securities: null },
  ],
  cumulativeContributions: 0,
  cumulativeWithdrawals: 0,
  userEnteredRealizedGain: 0,
  valuationChange: null,
  investmentReturn: null,
};

const forecast: Forecast = {
  from: TODAY,
  to: '2027-01-18',
  currency: 'JPY',
  scenario: 'base',
  days: [
    { accountId: BANK.id, date: TODAY, startBalance: 200_000, endBalance: 200_000, movements: [], intradayOrderUncertain: false, shortfall: false, coverage: 'complete' },
    {
      accountId: BANK.id,
      date: '2026-12-10',
      startBalance: 200_000,
      endBalance: 170_000,
      movements: [{ id: 's1', amount: -30_000, source: 'card_schedule', label: 'Visa payment', certainty: 'confirmed' }],
      intradayOrderUncertain: false,
      shortfall: false,
      coverage: 'complete',
    },
  ],
  lowestByAccount: { [BANK.id]: { date: '2026-12-10', balance: 170_000 } },
  coverage: 'partial_assumptions',
  overdueScheduleIds: [],
  warnings: ['Finance settings are unconfirmed'],
};

const dashboard: Dashboard = {
  asOfDate: TODAY,
  availableCash: {
    asOfDate: TODAY,
    horizon: '2026-12-19',
    currency: 'JPY',
    accounts: [{ accountId: BANK.id, liquidBalance: 200_000, fundedEarmarks: 50_000, unfundedKnownDue: 30_000, rawAvailable: 150_000, spendable: 120_000, reserveShortfall: 0, coverage: 'complete' }],
    remainingDiscretionaryBudget: null,
    coverage: 'incomplete',
    overdueScheduleIds: [],
    warnings: ['Finance settings are unconfirmed'],
  },
  livingBudget: { start: '2026-10-25', endExclusive: '2026-11-25', budgets: [], unknownPurchaseDateCount: 0, coverage: 'complete' },
  forecast,
  financialPosition: position,
  monthlyActuals: [{ month: '2026-11-01', income: 0, grossSpending: 30_000, refunds: 0, netSpending: 30_000, dateBasis: 'known_only' }],
};

describe('overview keeps unlike figures apart', () => {
  beforeEach(() => readyApp());

  it('shows cash, spendable, card debt and net worth separately and never as one total', async () => {
    backend({ get_dashboard: dashboard });
    render(Overview);
    const cash = (await screen.findByText('Cash in accounts')).closest('section')!;
    expect(cash).toHaveTextContent('¥200,000');
    expect(cash).not.toHaveTextContent('170,000');

    const owed = screen.getByText('Owed on cards').closest('section')!;
    expect(owed).toHaveTextContent('¥30,000');
    expect(owed).toHaveTextContent('Shown separately from cash');

    const spendable = screen.getByText(/Spendable after commitments/).closest('section')!;
    expect(spendable).toHaveTextContent('¥120,000');
    expect(spendable).toHaveTextContent('Set aside ¥50,000');

    const worth = screen.getByText('Net worth').closest('section')!;
    expect(within(worth).getByText('Incomplete', { selector: '[aria-hidden="true"]' })).toBeInTheDocument();
    expect(worth).not.toHaveTextContent('¥170,000');

    expect(screen.getByText('Lowest projected balance').closest('section')).toHaveTextContent('Forecast');
    expect(screen.getByRole('note')).toHaveTextContent('Finance settings are unconfirmed');
    expect(screen.getByText('Finish setting up')).toBeInTheDocument();
  });
});

describe('forecast', () => {
  beforeEach(() => readyApp());

  it('surfaces Rust warnings, keeps projections labelled, and asks Rust for what-if flows explicitly', async () => {
    const api = backend({
      get_cash_forecast: forecast,
      get_available_cash: dashboard.availableCash,
      list_expected_flows: [],
    });
    render(ForecastScreen);
    expect(await screen.findByTestId('forecast-warnings')).toHaveTextContent('Finance settings are unconfirmed');
    expect(api.of('get_cash_forecast')[0]?.args).toEqual({ from: TODAY, to: '2027-01-18', scenario: false });

    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Table' }));
    const table = screen.getByRole('table', { name: /Projected end-of-day balance/ });
    expect(within(table).getAllByText('Forecast').length).toBeGreaterThan(0);
    expect(table).toHaveTextContent('¥170,000');

    await user.click(screen.getByLabelText('Include what-if flows'));
    await waitFor(() => expect(api.of('get_cash_forecast').at(-1)?.args).toMatchObject({ scenario: true }));
    expect(screen.getByText(/never count as income or balances/)).toBeInTheDocument();
  });

  it('moves between chart points with the arrow keys', async () => {
    backend({ get_cash_forecast: forecast, get_available_cash: dashboard.availableCash, list_expected_flows: [] });
    render(ForecastScreen);
    const chart = await screen.findByRole('group', { name: /Projected end-of-day balance.*arrow keys/ });
    const points = within(chart).getAllByRole('img');
    points[0]!.focus();
    const user = userEvent.setup();
    await user.keyboard('{ArrowRight}');
    expect(points[1]).toHaveFocus();
    expect(points[1]).toHaveAccessibleName(/10 Dec 2026, Forecast: End of day 170,000 yen/);
  });
});

const purchase: FinancialEvent = {
  id: '66666666-6666-4666-8666-666666666666',
  kind: 'EXPENSE',
  amount: 30_000,
  categoryId: FOOD.id,
  description: 'Laptop bag',
  occurredOn: '2026-11-10',
  dateBasis: 'known',
  recordedAt: '2026-11-10T00:00:00Z',
  originalEventId: null,
  reversesEventId: null,
  replacesEventId: null,
  isReversal: false,
  postings: [{ accountId: CARD.id, delta: -30_000, effectiveDate: '2026-11-10' }],
};
const repayment: FinancialEvent = {
  ...purchase,
  id: '77777777-7777-4777-8777-777777777777',
  kind: 'CARD_REPAYMENT',
  categoryId: null,
  description: null,
  occurredOn: '2026-12-10',
  postings: [
    { accountId: BANK.id, delta: -30_000, effectiveDate: '2026-12-10' },
    { accountId: CARD.id, delta: 30_000, effectiveDate: '2026-12-10' },
  ],
};
const schedule: Schedule = {
  id: '88888888-8888-4888-8888-888888888888',
  cardId: CARD.id,
  purchaseEventId: purchase.id,
  openingObligation: null,
  dueDate: '2026-12-10',
  principalDue: 30_000,
  source: 'estimate',
  statementId: null,
  paid: 0,
  refunded: 0,
  outstanding: 30_000,
  status: 'planned',
  fundedReserve: 0,
  unfunded: 30_000,
};

function cardBackend(extra: Parameters<typeof backend>[0] = {}) {
  return backend({
    get_card_settings: { cardId: CARD.id, settlementAccountId: BANK.id, closingDay: 15, paymentDay: 10 },
    list_card_schedules: [schedule],
    list_card_statements: [],
    get_account_balance: balance(CARD.id, -30_000),
    list_financial_events: { items: [repayment, purchase], total: 2, page: 1, pageSize: 200 },
    ...extra,
  });
}

describe('cards: purchase to repayment and statement review', () => {
  beforeEach(() => readyApp());

  it('schedules a card purchase and applies the recorded repayment to it', async () => {
    const api = cardBackend({
      list_card_schedules: [],
      create_card_schedule: (args) => ({ isReplay: false, result: { ...schedule, ...((args!.request as { payload: object }).payload) } }),
    });
    render(Cards, { cardId: CARD.id });
    const user = userEvent.setup();
    await screen.findByText('No payments scheduled');
    await user.click(screen.getByRole('button', { name: 'Schedule payment' }));
    expect(screen.getByLabelText('Purchase')).toHaveDisplayValue(/Laptop bag/);
    expect(screen.getByLabelText('Amount due')).toHaveValue('30000');
    const due = screen.getByLabelText('Due date');
    await user.clear(due);
    await user.type(due, '2026-12-10');
    await user.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(api.of('create_card_schedule')).toHaveLength(1));
    expect(written(api.of('create_card_schedule')[0])?.payload).toEqual({
      cardId: CARD.id,
      purchaseEventId: purchase.id,
      openingObligation: null,
      dueDate: '2026-12-10',
      principalDue: 30_000,
      source: 'estimate',
    });
  });

  it('applies a repayment event to the schedule; the repayment is not spending', async () => {
    const api = cardBackend({ apply_card_event: { isReplay: false, result: { ...schedule, paid: 30_000, outstanding: 0, status: 'paid' } } });
    render(Cards, { cardId: CARD.id });
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Apply payment…' }));
    const select = screen.getByLabelText('Payment or refund');
    expect(within(select).getAllByRole('option').map((o) => o.textContent)).toEqual([expect.stringMatching(/^Payment · 10 Dec 2026 · ¥30,000/)]);
    await user.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(api.of('apply_card_event')).toHaveLength(1));
    expect(written(api.of('apply_card_event')[0])?.payload).toEqual({ scheduleId: schedule.id, eventId: repayment.id, amount: 30_000 });
  });

  it('reviews a statement against the schedules and confirms only when totals match', async () => {
    const api = cardBackend({
      confirm_card_statement: (args) => ({ isReplay: false, result: { id: 'st1', ...((args!.request as { payload: object }).payload) } }),
    });
    render(Cards, { cardId: CARD.id });
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Confirm statement…' }));
    await user.type(screen.getByLabelText('Closing date'), '2026-11-15');
    await user.type(screen.getByLabelText('Total on the statement'), '31,000');
    expect(screen.getByRole('status')).toHaveTextContent('The statement differs from the scheduled payments');
    await user.click(screen.getByRole('button', { name: 'Confirm statement' }));
    expect(api.of('confirm_card_statement')).toHaveLength(0);
    expect(screen.getByText('Must match the selected payments.')).toBeInTheDocument();

    await user.clear(screen.getByLabelText('Total on the statement'));
    await user.type(screen.getByLabelText('Total on the statement'), '30000');
    expect(screen.getByText('Matches')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Confirm statement' }));
    await waitFor(() => expect(api.of('confirm_card_statement')).toHaveLength(1));
    expect(written(api.of('confirm_card_statement')[0])?.payload).toEqual({
      cardId: CARD.id,
      closingOn: '2026-11-15',
      dueDate: '2026-12-10',
      principalConfirmed: 30_000,
      scheduleIds: [schedule.id],
    });
  });
});

describe('savings allocation', () => {
  beforeEach(() => readyApp());

  it('explains that setting money aside keeps the balance and sends a bucket funding', async () => {
    const bucket = { id: '99999999-9999-4999-8999-999999999999', name: 'Trip fund', goalId: null, funded: 0, archivedAt: null };
    const api = backend({
      get_finance_settings: { timeZone: 'Asia/Tokyo', paydayDay: 25, defaultAccountId: null },
      get_budget_summary: dashboard.livingBudget,
      list_savings_goals: [],
      list_savings_buckets: [bucket],
      get_allocation_capacity: [{ balance: balance(BANK.id, 200_000), funded: 0, allocatable: 200_000, shortfall: null }],
      fund_savings_bucket: { isReplay: false, result: { ...bucket, funded: 50_000 } },
    });
    render(Budgets);
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Set money aside for Trip fund' }));
    expect(screen.getByText(/the balance does not change/)).toBeInTheDocument();
    await user.type(screen.getByLabelText('Amount'), '50000');
    await user.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(api.of('fund_savings_bucket')).toHaveLength(1));
    expect(written(api.of('fund_savings_bucket')[0])?.payload).toEqual({ bucketId: bucket.id, accountId: BANK.id, amount: 50_000, asOfDate: TODAY });
    expect(screen.getByRole('heading', { name: 'Free to allocate' }).closest('section')).toHaveTextContent('Balances are unchanged by setting money aside.');
  });
});
