import { CommandFailure, commands, resetPrototypeData, setDemoMode, setLatency, type NewEntryInput } from './commands';

const input = (over: Partial<NewEntryInput> = {}): NewEntryInput => ({
  idempotencyKey: 'k1', kind: 'EXPENSE', amount: 1200, description: 'Lunch', category: 'Dining',
  occurredOn: '2026-11-04', accountId: 'acc-everyday', ...over
});

beforeEach(() => {
  setLatency(0);
  setDemoMode('ready');
  resetPrototypeData();
});

describe('mock command boundary', () => {
  it('replays a write with the same idempotency key instead of posting twice', async () => {
    const before = (await commands.listEntries()).length;
    const first = await commands.createEntry(input());
    const again = await commands.createEntry(input());
    expect(again.replayed).toBe(true);
    expect(again.entry.id).toBe(first.entry.id);
    expect(await commands.listEntries()).toHaveLength(before + 1);
  });

  it('applies a new entry to the account balance', async () => {
    await commands.createEntry(input({ amount: 380 }));
    const everyday = (await commands.listAccounts()).find((a) => a.id === 'acc-everyday')!;
    expect(everyday.balance).toBe(412_380 - 380);
  });

  it('fails with a structured, path-free error when storage is unavailable', async () => {
    setDemoMode('error');
    await expect(commands.listAccounts()).rejects.toBeInstanceOf(CommandFailure);
    await expect(commands.createEntry(input())).rejects.toMatchObject({ error: { code: 'storage_unavailable' } });
  });

  it('rejects future-dated ledger entries with a field-level error', async () => {
    await expect(commands.createEntry(input({ occurredOn: '2026-12-01' }))).rejects.toMatchObject({ error: { field: 'occurredOn' } });
  });
});
