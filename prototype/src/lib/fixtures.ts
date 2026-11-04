// Synthetic data for the UI prototype (#3).
//
// These shapes are illustrative only. They are NOT the #2 parity fixtures and
// NOT a proposed Tauri command contract — the real types will come from the
// agreed contract in #4/#7. Semantics they deliberately exercise:
//   - whole-yen integers; `null` money = unknown, never zero
//   - card debt is negative on the account balance; "owed" is shown positive
//   - a legacy record may have no known purchase date
//   - transfers and card repayments are not spending
//   - forecast months are separate from actual months

import type { MaybeYen, Yen } from './money';

export type AccountKind = 'BANK' | 'CASH' | 'CREDIT_CARD' | 'BROKERAGE_CASH';
export type BalanceStatus = 'complete' | 'incomplete';

export interface Account {
  id: string;
  name: string;
  kind: AccountKind;
  /** Signed balance as of `asOf`. Card debt is negative. */
  balance: MaybeYen;
  status: BalanceStatus;
  asOf: string;
  /** Why a balance is incomplete, in plain language. */
  statusNote?: string;
}

export type EventKind = 'INCOME' | 'EXPENSE' | 'TRANSFER' | 'CARD_REPAYMENT' | 'REFUND';
export type DateBasis = 'known' | 'legacy_recorded_date';

export interface LedgerEntry {
  id: string;
  kind: EventKind;
  description: string;
  category: string | null;
  /** Purchase / occurrence date. `null` when a legacy record never had one. */
  occurredOn: string | null;
  /** Date the money moved on the account (always known). */
  effectiveDate: string;
  dateBasis: DateBasis;
  /** Signed change per account. Transfers have two postings. */
  postings: Array<{ accountId: string; delta: Yen }>;
}

export interface CashFlowMonth {
  month: string; // YYYY-MM
  state: 'actual' | 'forecast' | 'incomplete';
  moneyIn: Yen;
  /** Spending only — excludes transfers and card repayments. */
  spending: Yen;
  note?: string;
}

export const TODAY = '2026-11-04';

export const KIND_LABEL: Record<AccountKind, string> = {
  BANK: 'Bank',
  CASH: 'Cash',
  CREDIT_CARD: 'Credit card',
  BROKERAGE_CASH: 'Brokerage cash'
};

export const EVENT_LABEL: Record<EventKind, string> = {
  INCOME: 'Income',
  EXPENSE: 'Spending',
  TRANSFER: 'Transfer',
  CARD_REPAYMENT: 'Card repayment',
  REFUND: 'Refund'
};

export const accounts: Account[] = [
  { id: 'acc-everyday', name: 'Everyday', kind: 'BANK', balance: 412_380, status: 'complete', asOf: TODAY },
  { id: 'acc-savings', name: 'Rainy-day savings', kind: 'BANK', balance: 1_850_000, status: 'complete', asOf: TODAY },
  { id: 'acc-card', name: 'Travel card', kind: 'CREDIT_CARD', balance: -86_420, status: 'complete', asOf: TODAY },
  {
    id: 'acc-wallet',
    name: 'Wallet',
    kind: 'CASH',
    balance: null,
    status: 'incomplete',
    asOf: TODAY,
    statusNote: 'No opening balance recorded'
  },
  { id: 'acc-broker', name: 'Brokerage cash', kind: 'BROKERAGE_CASH', balance: 120_500, status: 'complete', asOf: '2026-10-31' }
];

export const categories = ['Groceries', 'Dining', 'Transport', 'Home', 'Utilities', 'Health', 'Leisure', 'Salary', 'Other'];

let n = 0;
const id = () => `evt-${String(++n).padStart(3, '0')}`;
const expense = (date: string, description: string, category: string, accountId: string, amount: Yen): LedgerEntry => ({
  id: id(), kind: 'EXPENSE', description, category, occurredOn: date, effectiveDate: date, dateBasis: 'known',
  postings: [{ accountId, delta: -amount }]
});

export const entries: LedgerEntry[] = [
  expense('2026-11-04', 'Supermarket', 'Groceries', 'acc-everyday', 6_240),
  expense('2026-11-03', 'Train pass top-up', 'Transport', 'acc-card', 5_000),
  { id: id(), kind: 'INCOME', description: 'October salary', category: 'Salary', occurredOn: '2026-10-25', effectiveDate: '2026-10-25', dateBasis: 'known', postings: [{ accountId: 'acc-everyday', delta: 318_400 }] },
  {
    id: id(), kind: 'TRANSFER', description: 'Monthly savings', category: null, occurredOn: '2026-10-26', effectiveDate: '2026-10-26', dateBasis: 'known',
    postings: [{ accountId: 'acc-everyday', delta: -60_000 }, { accountId: 'acc-savings', delta: 60_000 }]
  },
  {
    id: id(), kind: 'CARD_REPAYMENT', description: 'Travel card — October statement', category: null, occurredOn: '2026-10-27', effectiveDate: '2026-10-27', dateBasis: 'known',
    postings: [{ accountId: 'acc-everyday', delta: -71_930 }, { accountId: 'acc-card', delta: 71_930 }]
  },
  expense('2026-10-29', 'Dinner with friends', 'Dining', 'acc-card', 8_800),
  expense('2026-10-30', 'Electricity', 'Utilities', 'acc-everyday', 9_412),
  { id: id(), kind: 'REFUND', description: 'Returned headphones', category: 'Leisure', occurredOn: '2026-10-31', effectiveDate: '2026-10-31', dateBasis: 'known', postings: [{ accountId: 'acc-card', delta: 12_980 }] },
  expense('2026-11-01', 'Pharmacy', 'Health', 'acc-everyday', 2_310),
  expense('2026-11-02', 'Coffee beans', 'Groceries', 'acc-wallet', 1_680),
  {
    id: id(), kind: 'EXPENSE', description: 'Imported: card payment line', category: 'Other', occurredOn: null, effectiveDate: '2026-09-27', dateBasis: 'legacy_recorded_date',
    postings: [{ accountId: 'acc-everyday', delta: -23_100 }]
  },
  {
    id: id(), kind: 'EXPENSE', description: 'Imported: Home centre', category: 'Home', occurredOn: null, effectiveDate: '2026-09-12', dateBasis: 'legacy_recorded_date',
    postings: [{ accountId: 'acc-everyday', delta: -14_520 }]
  }
];

export const cashFlow: CashFlowMonth[] = [
  { month: '2026-05', state: 'actual', moneyIn: 318_400, spending: 241_300 },
  { month: '2026-06', state: 'actual', moneyIn: 512_400, spending: 268_900, note: 'Includes summer bonus' },
  { month: '2026-07', state: 'actual', moneyIn: 318_400, spending: 296_150 },
  { month: '2026-08', state: 'incomplete', moneyIn: 318_400, spending: 204_700, note: 'Wallet spending not recorded' },
  { month: '2026-09', state: 'actual', moneyIn: 318_400, spending: 252_880 },
  { month: '2026-10', state: 'actual', moneyIn: 318_400, spending: 239_640 },
  { month: '2026-11', state: 'forecast', moneyIn: 318_400, spending: 247_000, note: 'Month in progress' },
  { month: '2026-12', state: 'forecast', moneyIn: 612_400, spending: 301_000, note: 'Assumes winter bonus' },
  { month: '2027-01', state: 'forecast', moneyIn: 318_400, spending: 255_000 }
];
