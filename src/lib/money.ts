/**
 * Strict integer money formatting and input parsing for JPY (whole yen) and
 * USD (integer cents). Never uses floating-point arithmetic for stored values.
 */
import type { BillingCurrency, Yen } from './types';

const jpyGrouping = new Intl.NumberFormat('ja-JP', { maximumFractionDigits: 0 });
const usdGrouping = new Intl.NumberFormat('en-US', {
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});
const MINUS = '−';

export type ParseAmountResult =
  | { ok: true; value: number }
  | { ok: false; reason: string };

export function formatYen(value: Yen, { signed = false } = {}): string {
  if (!Number.isSafeInteger(value)) {
    throw new RangeError(`Not a whole-yen amount: ${value}`);
  }
  const body = `¥${jpyGrouping.format(Math.abs(value))}`;
  if (value < 0) return `${MINUS}${body}`;
  if (signed && value > 0) return `+${body}`;
  return body;
}

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
    return `${jpyGrouping.format(Math.abs(minor))} yen`;
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
 * Parses positive whole-yen input (`1,200`, `¥1200`, full-width digits).
 */
export function parseYenInput(raw: string): ParseAmountResult {
  const normalized = raw
    .normalize('NFKC')
    .trim()
    .replace(/^[¥￥]/, '')
    .replace(/[,\s]/g, '');
  if (normalized === '') return { ok: false, reason: 'Enter a whole-yen amount.' };
  if (!/^\d+$/.test(normalized)) {
    return {
      ok: false,
      reason: normalized.includes('.')
        ? 'Use whole yen — no decimals.'
        : 'Use digits only, like 3,142.',
    };
  }
  const value = Number(normalized);
  if (!Number.isSafeInteger(value)) return { ok: false, reason: 'That amount is too large.' };
  if (value === 0) return { ok: false, reason: 'Amount must be greater than ¥0.' };
  return { ok: true, value };
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
