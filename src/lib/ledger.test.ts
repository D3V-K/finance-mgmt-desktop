import { afterEach, describe, expect, it } from 'vitest';
import { setMockInvoker } from './commands';
import { createAccount, createFinancialEvent, getAccountBalance, listFinancialEvents, setAccountOpening } from './ledger';

describe('v1 ledger transport', () => {
  afterEach(() => setMockInvoker(null));

  it('passes the idempotent envelope as a single request argument', async () => {
    const calls: unknown[] = [];
    setMockInvoker(async (command, args) => {
      calls.push([command, args]);
      return { isReplay: false, result: { id: 'account-id' } };
    });
    const request = { idempotencyKey: 'key', payload: { name: 'Wallet', kind: 'CASH' as const } };
    await createAccount(request);
    expect(calls).toEqual([['create_account', { request }]]);
  });

  it('preserves signed amount and date fields without financial logic in the UI', async () => {
    const calls: unknown[] = [];
    setMockInvoker(async (command, args) => {
      calls.push([command, args]);
      return {};
    });
    const request = { idempotencyKey: 'key', payload: {
      kind: 'BALANCE_ADJUSTMENT' as const, amount: 5, adjustmentDelta: -5,
      accountId: 'account', occurredOn: '2026-11-16', effectiveDate: '2026-11-16',
    } };
    await createFinancialEvent(request);
    await getAccountBalance('account', '2026-11-16');
    await listFinancialEvents(1, 20, 'account');
    expect(calls).toEqual([
      ['create_financial_event', { request }],
      ['get_account_balance', { accountId: 'account', asOfDate: '2026-11-16' }],
      ['list_financial_events', { accountId: 'account', page: 1, pageSize: 20 }],
    ]);
  });

  it('rejects unsafe yen before crossing the wire', async () => {
    let invoked = false;
    setMockInvoker(async () => { invoked = true; return {}; });
    await expect(setAccountOpening({ idempotencyKey: 'key', payload: {
      accountId: 'account', amount: Number.MAX_SAFE_INTEGER + 1,
      asOfDate: '2026-11-16', reason: 'opening',
    } })).rejects.toMatchObject({ code: 'INVALID_AMOUNT', field: 'amount' });
    await expect(createFinancialEvent({ idempotencyKey: 'key', payload: {
      kind: 'BALANCE_ADJUSTMENT', amount: 5, adjustmentDelta: 0.5,
      accountId: 'account', occurredOn: '2026-11-16', effectiveDate: '2026-11-16',
    } })).rejects.toMatchObject({ code: 'INVALID_AMOUNT', field: 'adjustmentDelta' });
    expect(invoked).toBe(false);
  });
});
