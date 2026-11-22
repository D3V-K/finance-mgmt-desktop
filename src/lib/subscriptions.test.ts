import { beforeEach, describe, expect, it } from 'vitest';
import { setMockInvoker } from './commands';
import {
  createSubscription,
  newWrite,
  reconcileSubscriptionBatch,
} from './subscriptions';

describe('subscriptions typed command wrappers', () => {
  beforeEach(() => setMockInvoker(null));

  it('wraps writes in { request } and rejects non-positive minor units client-side', async () => {
    let seenCmd = '';
    let seenArgs: Record<string, unknown> | undefined;
    setMockInvoker(async (cmd, args) => {
      seenCmd = cmd;
      seenArgs = args;
      return { isReplay: false, result: { id: 'sub-1' } };
    });

    const req = newWrite({
      name: 'ChatGPT Plus',
      accountId: '00000000-0000-4000-8000-000000000001',
      categoryId: '00000000-0000-4000-8000-000000000002',
      currency: 'USD' as const,
      amountMinor: 2000,
      cadence: 'monthly' as const,
      startDate: '2026-11-01',
    });
    await createSubscription(req);
    expect(seenCmd).toBe('create_subscription');
    expect(seenArgs).toEqual({ request: req });

    expect(() =>
      createSubscription(
        newWrite({
          ...req.payload,
          amountMinor: 0,
        }),
      ),
    ).toThrow(/INVALID_AMOUNT|positive/i);
  });

  it('validates whole-yen actualJpy in batch reconciliation', () => {
    expect(() =>
      reconcileSubscriptionBatch(
        newWrite({
          items: [
            {
              subscriptionId: '00000000-0000-4000-8000-000000000001',
              scheduledDate: '2026-11-15',
              actualJpy: 3142.5,
              settledOn: '2026-11-17',
            },
          ],
        }),
      ),
    ).toThrow(/whole-yen/i);
  });
});
