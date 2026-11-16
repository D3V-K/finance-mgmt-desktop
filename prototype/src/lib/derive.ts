// Presentation-only aggregation over prototype fixtures. Real figures will be
// computed in Rust (#7–#11); the UI must not re-derive financial results.

import type { Account, LedgerEntry } from './fixtures';
import { sumKnown, type PartialSum, type Yen } from './money';

const LIQUID = new Set<Account['kind']>(['BANK', 'CASH', 'BROKERAGE_CASH']);

export function cashPosition(accounts: Account[]): PartialSum {
  return sumKnown(accounts.filter((a) => LIQUID.has(a.kind)).map((a) => a.balance));
}

/** Card balances are negative when money is owed; returns the positive amount owed. */
export function cardOwed(accounts: Account[]): PartialSum {
  const sum = sumKnown(accounts.filter((a) => a.kind === 'CREDIT_CARD').map((a) => (a.balance === null ? null : -a.balance)));
  return sum;
}

/** Income and net spending for a calendar month. Transfers and card repayments move money; they are not spending. */
export function monthFlow(entries: LedgerEntry[], month: string): { moneyIn: Yen; spending: Yen; excludedMoves: number } {
  let moneyIn = 0;
  let spending = 0;
  let excludedMoves = 0;
  for (const e of entries) {
    if (!e.effectiveDate.startsWith(month)) continue;
    const amount = Math.abs(e.postings[0]?.delta ?? 0);
    if (e.kind === 'INCOME') moneyIn += amount;
    else if (e.kind === 'EXPENSE') spending += amount;
    else if (e.kind === 'REFUND') spending -= amount;
    else excludedMoves += 1;
  }
  return { moneyIn, spending, excludedMoves };
}

/** The signed change an entry made to one account. */
export function deltaFor(entry: LedgerEntry, accountId: string | null): Yen {
  if (accountId === null) {
    // Across all accounts a transfer nets to zero; show the amount moved instead.
    if (entry.postings.length > 1) return Math.abs(entry.postings[0]!.delta);
    return entry.postings[0]?.delta ?? 0;
  }
  return entry.postings.filter((p) => p.accountId === accountId).reduce((s, p) => s + p.delta, 0);
}
