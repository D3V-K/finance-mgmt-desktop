import { accounts, entries } from './fixtures';
import { cardOwed, cashPosition, deltaFor, monthFlow } from './derive';

describe('presentation aggregates', () => {
  it('reports cash as partial while any liquid balance is unknown', () => {
    const cash = cashPosition(accounts);
    expect(cash.total).toBeNull();
    expect(cash.unknownCount).toBe(1);
    expect(cash.knownSubtotal).toBe(412_380 + 1_850_000 + 120_500);
  });

  it('keeps card debt out of cash and reports it as a positive amount owed', () => {
    expect(cardOwed(accounts).total).toBe(86_420);
  });

  it('excludes transfers and card repayments from spending; refunds reduce it', () => {
    const oct = monthFlow(entries, '2026-10');
    expect(oct.moneyIn).toBe(318_400);
    expect(oct.spending).toBe(8_800 + 9_412 - 12_980);
    expect(oct.excludedMoves).toBe(2);
  });

  it('shows the per-account side of a transfer, and the amount moved across all accounts', () => {
    const transfer = entries.find((e) => e.kind === 'TRANSFER')!;
    expect(deltaFor(transfer, 'acc-everyday')).toBe(-60_000);
    expect(deltaFor(transfer, 'acc-savings')).toBe(60_000);
    expect(deltaFor(transfer, null)).toBe(60_000);
  });
});
