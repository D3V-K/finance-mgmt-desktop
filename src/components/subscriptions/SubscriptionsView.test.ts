import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { beforeEach, describe, expect, it } from 'vitest';
import { setMockInvoker } from '../../lib/commands';
import type {
  Account,
  AppError,
  Category,
  SubscriptionDetail,
  SubscriptionOccurrence,
  SubscriptionOverview,
} from '../../lib/types';
import SubscriptionsView from './SubscriptionsView.svelte';

const CARD_ID = '00000000-0000-4000-8000-000000000001';
const CAT_ID = '00000000-0000-4000-8000-000000000002';
const SUB_USD_ID = '00000000-0000-4000-8000-000000000010';
const SUB_JPY_ID = '00000000-0000-4000-8000-000000000011';

const sampleAccounts: Account[] = [
  { id: CARD_ID, name: 'Visa Card', kind: 'CREDIT_CARD', currency: 'JPY', archivedAt: null },
];

const sampleCategories: Category[] = [
  { id: CAT_ID, name: 'Subscriptions', type: 'expense', parentId: null, archivedAt: null },
];

function makeOverview(pendingCount = 1): SubscriptionOverview {
  return {
    asOfDate: '2026-11-22',
    items: [
      {
        id: SUB_USD_ID,
        name: 'ChatGPT Plus',
        merchant: 'OpenAI',
        accountId: CARD_ID,
        categoryId: CAT_ID,
        currency: 'USD',
        cadence: 'monthly',
        billingDay: 15,
        billingMonth: null,
        startDate: '2026-01-15',
        status: 'active',
        pausedFrom: null,
        canceledEffectiveOn: null,
        note: null,
        createdAt: '2026-01-15T00:00:00Z',
        updatedAt: '2026-01-15T00:00:00Z',
        currentAmountMinor: 2000,
        nextBillingDate: '2026-12-15',
        lastReconciledJpy: 3120,
        lastReconciledOn: '2026-10-17',
        accountName: 'Visa Card',
        accountKind: 'CREDIT_CARD',
        categoryName: 'Subscriptions',
      },
      {
        id: SUB_JPY_ID,
        name: 'iCloud+',
        merchant: 'Apple',
        accountId: CARD_ID,
        categoryId: CAT_ID,
        currency: 'JPY',
        cadence: 'monthly',
        billingDay: 5,
        billingMonth: null,
        startDate: '2026-01-05',
        status: 'active',
        pausedFrom: null,
        canceledEffectiveOn: null,
        note: null,
        createdAt: '2026-01-05T00:00:00Z',
        updatedAt: '2026-01-05T00:00:00Z',
        currentAmountMinor: 1300,
        nextBillingDate: '2026-12-05',
        lastReconciledJpy: 1300,
        lastReconciledOn: '2026-11-05',
        accountName: 'Visa Card',
        accountKind: 'CREDIT_CARD',
        categoryName: 'Subscriptions',
      },
    ],
    jpyMonthlyNormalized: 1300,
    jpyDueThisMonth: 1300,
    foreignMonthlyNormalized: [{ currency: 'USD', count: 1, totalMinor: 2000 }],
    lastMonthReconciledForeignJpy: 3120,
    pendingReconciliationCount: pendingCount,
  };
}

function makePendingOccurrence(): SubscriptionOccurrence {
  return {
    subscriptionId: SUB_USD_ID,
    subscriptionName: 'ChatGPT Plus',
    merchant: 'OpenAI',
    accountId: CARD_ID,
    accountName: 'Visa Card',
    accountKind: 'CREDIT_CARD',
    hasCardSettings: true,
    categoryId: CAT_ID,
    categoryName: 'Subscriptions',
    scheduledDate: '2026-11-15',
    overrideDate: null,
    effectiveDate: '2026-11-15',
    status: 'needs_reconciliation',
    billedCurrency: 'USD',
    billedAmountMinor: 2000,
    actualJpy: null,
    settledOn: null,
    eventId: null,
    scheduleId: null,
    lastReconciledJpy: 3120,
  };
}

describe('SubscriptionsView', () => {
  beforeEach(() => setMockInvoker(null));

  it('displays separate JPY and USD totals and reuses idempotencyKey when retrying month-end batch reconciliation', async () => {
    let reconciled = false;
    let attempt = 0;
    const seenKeys: string[] = [];

    setMockInvoker(async (cmd, args) => {
      if (cmd === 'list_subscriptions') return makeOverview(reconciled ? 0 : 1);
      if (cmd === 'list_accounts') return sampleAccounts;
      if (cmd === 'list_categories') return sampleCategories;
      if (cmd === 'list_subscription_occurrences') {
        return reconciled ? [] : [makePendingOccurrence()];
      }
      if (cmd === 'get_subscription') {
        const sub = makeOverview(reconciled ? 0 : 1).items[0]!;
        return {
          subscription: sub,
          revisions: [],
          occurrences: [],
        } satisfies SubscriptionDetail;
      }
      if (cmd === 'reconcile_subscription_batch') {
        const req = (args as { request: { idempotencyKey: string; payload: { items: unknown[] } } })
          .request;
        seenKeys.push(req.idempotencyKey);
        attempt += 1;
        if (attempt === 1) {
          throw {
            code: 'STORAGE_UNAVAILABLE',
            message: 'Local storage is busy. Try again in a moment.',
            category: 'storage',
          } satisfies AppError;
        }
        reconciled = true;
        return {
          isReplay: false,
          result: [{ occurrence: { ...makePendingOccurrence(), status: 'reconciled', actualJpy: 3142 }, event: {} }],
        };
      }
      throw new Error(`unexpected command: ${cmd}`);
    });

    render(SubscriptionsView, { props: { today: '2026-11-22' } });

    const strip = await screen.findByTestId('sub-summary-strip');
    expect(strip).toHaveTextContent('¥1,300 / mo');
    expect(strip).toHaveTextContent('$20.00 / mo');
    expect(strip).toHaveTextContent('1 charge pending');

    const jpyInput = (await screen.findByLabelText(
      'Actual JPY deducted for ChatGPT Plus on 2026-11-15',
    )) as HTMLInputElement;

    // Decimal JPY is rejected inline before calling backend
    await fireEvent.input(jpyInput, { target: { value: '3142.5' } });
    await fireEvent.click(screen.getByRole('button', { name: /Reconcile Entered/i }));
    expect(await screen.findByText('Use whole yen — no decimals.')).toBeInTheDocument();
    expect(seenKeys).toHaveLength(0);

    // Valid full-width IME yen input -> first attempt fails -> retry reuses exact same idempotencyKey
    await fireEvent.input(jpyInput, { target: { value: '３，１４２' } });
    await fireEvent.click(screen.getByRole('button', { name: /Reconcile Entered/i }));

    expect(await screen.findByTestId('sub-error-banner')).toHaveTextContent('STORAGE_UNAVAILABLE');
    const retryBtn = screen.getByRole('button', { name: /Try again \(1\)/i });
    await fireEvent.click(retryBtn);

    await waitFor(() => {
      expect(screen.getByTestId('sub-status-banner')).toHaveTextContent(
        'Reconciled 1 subscription charge(s)',
      );
    });
    expect(seenKeys).toHaveLength(2);
    expect(seenKeys[0]).toBe(seenKeys[1]);
  });
});
