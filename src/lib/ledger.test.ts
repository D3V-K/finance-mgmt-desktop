import { describe, it, expect, beforeEach } from 'vitest';
import { setMockInvoker } from './commands';
import {
  assertYen,
  createAccount,
  createFinancialEvent,
  getAccountBalance,
  newWrite,
} from './ledger';
import type { EventInput } from './types';

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

describe('v1 ledger transport', () => {
  let calls: { cmd: string; args?: Record<string, unknown> }[];

  beforeEach(() => {
    calls = [];
    setMockInvoker(async (cmd, args) => {
      calls.push({ cmd, args });
      return { isReplay: calls.length > 1, result: { id: 'x' } };
    });
  });

  it('wraps writes in a single `request` argument with a UUID key', async () => {
    const req = newWrite({ name: 'Wallet', kind: 'CASH' as const });
    await createAccount(req);
    expect(calls[0]).toEqual({ cmd: 'create_account', args: { request: req } });
    expect(req.idempotencyKey).toMatch(UUID);
  });

  it('resending the same write reuses its key so Rust can replay', async () => {
    const req = newWrite({ name: 'Wallet', kind: 'CASH' as const });
    await createAccount(req);
    const retry = await createAccount(req);
    expect(calls[0]?.args).toEqual(calls[1]?.args);
    expect(retry.isReplay).toBe(true);
    expect(newWrite({}).idempotencyKey).not.toBe(req.idempotencyKey);
  });

  it('reads use camelCase named arguments', async () => {
    await getAccountBalance('a', '2026-11-16');
    expect(calls[0]).toEqual({
      cmd: 'get_account_balance',
      args: { accountId: 'a', asOfDate: '2026-11-16' },
    });
  });

  it('rejects unsafe or fractional yen before invoking', async () => {
    const input: EventInput = {
      kind: 'EXPENSE',
      amount: 12.5,
      accountId: 'a',
      occurredOn: '2026-11-16',
      effectiveDate: '2026-11-16',
    };
    await expect(async () => createFinancialEvent(newWrite(input))).rejects.toMatchObject({
      code: 'INVALID_AMOUNT',
      field: 'amount',
    });
    expect(calls).toHaveLength(0);
    expect(() => assertYen(Number.MAX_SAFE_INTEGER + 1, 'amount')).toThrow();
    expect(() => assertYen(-1200, 'delta')).not.toThrow();
  });
});
