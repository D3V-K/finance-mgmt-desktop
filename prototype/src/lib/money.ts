// Display rules for JPY amounts. Amounts are whole yen (integers); `null`
// means the value is unknown and must never be rendered or summed as zero.

export type Yen = number;
export type MaybeYen = Yen | null;

/** Where a figure comes from. Drives labels, color, and chart patterns. */
export type ValueState = 'actual' | 'forecast' | 'incomplete';

export const STATE_LABEL: Record<ValueState, string> = {
  actual: 'Actual',
  forecast: 'Forecast',
  incomplete: 'Incomplete'
};

const grouping = new Intl.NumberFormat('ja-JP', { maximumFractionDigits: 0 });
const MINUS = '−';

export function assertYen(value: number): asserts value is Yen {
  if (!Number.isSafeInteger(value)) throw new RangeError(`Not a whole-yen amount: ${value}`);
}

/** `¥1,234` or `−¥1,234`. Pass `signed` to prefix positive values with `+`. */
export function formatYen(value: Yen, { signed = false } = {}): string {
  assertYen(value);
  const body = `¥${grouping.format(Math.abs(value))}`;
  if (value < 0) return `${MINUS}${body}`;
  if (signed && value > 0) return `+${body}`;
  return body;
}

/** Screen-reader phrasing that does not rely on the minus glyph being announced. */
export function spokenYen(value: MaybeYen, { signed = false } = {}): string {
  if (value === null) return 'unknown amount';
  const n = `${grouping.format(Math.abs(value))} yen`;
  if (value < 0) return `minus ${n}`;
  if (signed && value > 0) return `plus ${n}`;
  return n;
}

/**
 * Sum that keeps unknowns visible. `total` is only set when every input is
 * known; otherwise callers get the known subtotal and how many were excluded.
 */
export interface PartialSum {
  total: Yen | null;
  knownSubtotal: Yen;
  unknownCount: number;
  count: number;
}

export function sumKnown(values: readonly MaybeYen[]): PartialSum {
  let knownSubtotal = 0;
  let unknownCount = 0;
  for (const v of values) {
    if (v === null) unknownCount += 1;
    else {
      assertYen(v);
      knownSubtotal += v;
    }
  }
  assertYen(knownSubtotal);
  return { total: unknownCount === 0 ? knownSubtotal : null, knownSubtotal, unknownCount, count: values.length };
}

/**
 * Parses user input for a positive whole-yen amount. Accepts `1,200`, `¥1200`,
 * full-width digits; rejects decimals, signs and anything over the safe range.
 */
export function parseYenInput(raw: string): { ok: true; value: Yen } | { ok: false; reason: string } {
  const normalized = raw
    .normalize('NFKC')
    .trim()
    .replace(/^[¥￥]/, '')
    .replace(/[,\s]/g, '');
  if (normalized === '') return { ok: false, reason: 'Enter an amount.' };
  if (!/^\d+$/.test(normalized)) {
    return { ok: false, reason: /[.]/.test(normalized) ? 'Use whole yen — no decimals.' : 'Use digits only, like 1,200.' };
  }
  const value = Number(normalized);
  if (!Number.isSafeInteger(value)) return { ok: false, reason: 'That amount is too large.' };
  if (value === 0) return { ok: false, reason: 'Amount must be more than ¥0.' };
  return { ok: true, value };
}

const dateFmt = new Intl.DateTimeFormat('en-GB', { day: 'numeric', month: 'short', year: 'numeric', timeZone: 'UTC' });
const monthFmt = new Intl.DateTimeFormat('en-GB', { month: 'short', year: 'numeric', timeZone: 'UTC' });

/** ISO calendar date (`YYYY-MM-DD`), interpreted as a local calendar day, not an instant. */
export function formatDate(iso: string | null): string {
  if (iso === null) return 'Date unknown';
  return dateFmt.format(new Date(`${iso}T00:00:00Z`));
}

export function formatMonth(isoMonth: string): string {
  return monthFmt.format(new Date(`${isoMonth}-01T00:00:00Z`));
}
