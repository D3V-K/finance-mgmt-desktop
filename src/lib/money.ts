// Display rules for JPY amounts. Amounts are whole yen (integers); `null`
// means the value is unknown and must never be rendered or summed as zero.

import type { BillingCurrency } from './types';

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

/**
 * Signed whole-yen input for balances (openings, statement balances, signed
 * flows). Accepts a leading `-`, `−` or `▲` (Japanese bank notation) and zero.
 */
export function parseSignedYenInput(raw: string): { ok: true; value: Yen } | { ok: false; reason: string } {
  const trimmed = raw.normalize('NFKC').trim();
  const negative = /^[-−▲△]/.test(trimmed);
  const body = negative ? trimmed.slice(1) : trimmed.replace(/^\+/, '');
  if (body.replace(/^[¥￥]/, '').replace(/[,\s]/g, '') === '0') return { ok: true, value: 0 };
  const parsed = parseYenInput(body);
  if (!parsed.ok) return parsed;
  return { ok: true, value: negative ? -parsed.value : parsed.value };
}

/** Whole-yen input where zero is meaningful (an empty card, a zero budget). */
export function parseNonNegativeYenInput(raw: string): { ok: true; value: Yen } | { ok: false; reason: string } {
  const parsed = parseSignedYenInput(raw);
  if (parsed.ok && parsed.value < 0) return { ok: false, reason: 'Enter zero or more.' };
  return parsed;
}

const stampFmt = new Intl.DateTimeFormat('en-GB', { dateStyle: 'medium', timeStyle: 'short' });

/** UTC RFC 3339 instant (audit time), shown in the viewer's clock. */
export function formatInstant(rfc3339: string): string {
  const d = new Date(rfc3339);
  return Number.isNaN(d.getTime()) ? rfc3339 : stampFmt.format(d);
}

/** Calendar arithmetic on ISO dates without touching time zones. */
export function addDays(iso: string, days: number): string {
  const d = new Date(`${iso}T00:00:00Z`);
  d.setUTCDate(d.getUTCDate() + days);
  return d.toISOString().slice(0, 10);
}

export function addMonths(iso: string, months: number): string {
  const [y, m, day] = iso.split('-').map(Number) as [number, number, number];
  const first = new Date(Date.UTC(y, m - 1 + months, 1));
  const last = new Date(Date.UTC(first.getUTCFullYear(), first.getUTCMonth() + 1, 0)).getUTCDate();
  first.setUTCDate(Math.min(day, last));
  return first.toISOString().slice(0, 10);
}

export function monthStart(iso: string): string {
  return `${iso.slice(0, 7)}-01`;
}

// ---- Subscription billing currencies (JPY whole yen, USD integer cents) ----

const usdGrouping = new Intl.NumberFormat('en-US', {
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});

export type ParseAmountResult =
  | { ok: true; value: number }
  | { ok: false; reason: string };

export function formatUsdCents(cents: number): string {
  if (!Number.isSafeInteger(cents)) {
    throw new RangeError(`Not an integer USD cents amount: ${cents}`);
  }
  const abs = Math.abs(cents);
  const dollars = Math.trunc(abs / 100);
  const rem = abs % 100;
  const formatted = `${usdGrouping.format(dollars).replace(/\.00$/, '')}.${String(rem).padStart(2, '0')}`;
  return cents < 0 ? `${MINUS}$${formatted}` : `$${formatted}`;
}

export function formatCurrencyMinor(currency: BillingCurrency, minor: number): string {
  return currency === 'JPY' ? formatYen(minor) : formatUsdCents(minor);
}

export function spokenCurrencyMinor(currency: BillingCurrency, minor: number): string {
  if (!Number.isSafeInteger(minor)) {
    throw new RangeError(`Not a safe integer minor amount: ${minor}`);
  }
  if (currency === 'JPY') {
    return `${grouping.format(Math.abs(minor))} yen`;
  }
  const abs = Math.abs(minor);
  const dollars = Math.trunc(abs / 100);
  const cents = abs % 100;
  const dollarLabel = dollars === 1 ? 'dollar' : 'dollars';
  if (cents === 0) return `${dollars} US ${dollarLabel}`;
  const centLabel = cents === 1 ? 'cent' : 'cents';
  return `${dollars} US ${dollarLabel} ${cents} ${centLabel}`;
}

/**
 * Parses positive USD input (`$19.99`, `20`, `１９．９９`) into integer cents without floats.
 */
export function parseUsdInput(raw: string): ParseAmountResult {
  const normalized = raw
    .normalize('NFKC')
    .trim()
    .replace(/^\$/, '')
    .replace(/[,\s]/g, '');
  if (normalized === '') return { ok: false, reason: 'Enter a USD amount.' };
  const match = /^(\d+)(?:\.(\d+))?$/.exec(normalized);
  if (!match) {
    return { ok: false, reason: 'Enter a valid USD amount, like 19.99.' };
  }
  const [, wholePart, fracPart = ''] = match;
  if (fracPart.length > 2) {
    return { ok: false, reason: 'Use at most two decimal places for USD.' };
  }
  const paddedFrac = fracPart.padEnd(2, '0');
  const cents = Number(wholePart) * 100 + Number(paddedFrac);
  if (!Number.isSafeInteger(cents)) return { ok: false, reason: 'That amount is too large.' };
  if (cents === 0) return { ok: false, reason: 'Amount must be greater than $0.00.' };
  return { ok: true, value: cents };
}

export function parseCurrencyInput(currency: BillingCurrency, raw: string): ParseAmountResult {
  return currency === 'JPY' ? parseYenInput(raw) : parseUsdInput(raw);
}
