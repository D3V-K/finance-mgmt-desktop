import type { AccountKind, Balance, DateBasis, EventKind } from '../lib/types';

export const KIND_LABEL: Record<EventKind, string> = {
  INCOME: 'Income',
  EXPENSE: 'Spending',
  TRANSFER: 'Transfer',
  CARD_REPAYMENT: 'Card payment',
  REFUND: 'Refund',
  BALANCE_ADJUSTMENT: 'Adjustment',
};

/** Movements between your own accounts: never income or spending. */
export const MOVED: ReadonlySet<EventKind> = new Set(['TRANSFER', 'CARD_REPAYMENT']);

export const ACCOUNT_KIND_LABEL: Record<AccountKind, string> = {
  CASH: 'Cash',
  BANK: 'Bank',
  CREDIT_CARD: 'Credit card',
  BROKERAGE_CASH: 'Brokerage cash',
};

export const DATE_BASIS_LABEL: Record<DateBasis, string> = {
  known: 'Date known',
  confirmed_purchase_date: 'Purchase date confirmed later',
  legacy_recorded_date: 'Earlier-record date, not the purchase date',
};

export function balanceReason(b: Pick<Balance, 'status' | 'openingAsOfDate'>): string | null {
  if (b.status === 'complete') return null;
  if (b.status === 'incomplete_legacy_mapping') return 'Ledger start or earlier history is not finished';
  return b.openingAsOfDate ? `Opening starts ${b.openingAsOfDate}` : 'No opening balance recorded';
}

export const QUALITY_LABEL: Record<string, { title: string; help: string }> = {
  unknown_purchase_date: {
    title: 'Purchase date unknown',
    help: 'Spending from earlier records without a purchase date. It is left out of monthly and budget reports.',
  },
  unresolved_mapping: {
    title: 'Earlier record not mapped',
    help: 'An earlier Cash/Bank record has not been assigned to a named account. Balances stay incomplete until it is.',
  },
  missing_opening: {
    title: 'Opening balance missing',
    help: 'This account has no opening balance, so its balance is unknown.',
  },
  missing_settlement_account: {
    title: 'Card has no payment account',
    help: 'A card payment schedule exists but the card has no settlement account, so forecasts cannot place the payment.',
  },
  valuation_missing: {
    title: 'No securities valuation',
    help: 'A brokerage account has no market value recorded, so net worth is incomplete.',
  },
  valuation_stale: {
    title: 'Securities valuation out of date',
    help: 'Holdings changed after the last valuation. Record a new market value.',
  },
  reconciliation_draft: {
    title: 'Reconciliation not confirmed',
    help: 'A statement check was started but not confirmed.',
  },
  reconciliation_stale: {
    title: 'Reconciliation changed',
    help: 'Entries in a confirmed statement period changed afterwards. Review and confirm again.',
  },
};
