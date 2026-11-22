import { describe, expect, it } from 'vitest';
import {
  formatCurrencyMinor,
  formatUsdCents,
  formatYen,
  parseUsdInput,
  parseYenInput,
  spokenCurrencyMinor,
} from './money';

describe('money formatting and parsing', () => {
  it('formats JPY and USD integer minor units cleanly', () => {
    expect(formatYen(3142)).toBe('¥3,142');
    expect(formatYen(-1200)).toBe('−¥1,200');
    expect(formatUsdCents(2000)).toBe('$20.00');
    expect(formatUsdCents(1999)).toBe('$19.99');
    expect(formatUsdCents(105)).toBe('$1.05');
    expect(formatUsdCents(123456)).toBe('$1,234.56');
    expect(formatCurrencyMinor('JPY', 1300)).toBe('¥1,300');
    expect(formatCurrencyMinor('USD', 999)).toBe('$9.99');
  });

  it('produces screen-reader-friendly spoken strings', () => {
    expect(spokenCurrencyMinor('JPY', 3142)).toBe('3,142 yen');
    expect(spokenCurrencyMinor('USD', 2000)).toBe('20 US dollars');
    expect(spokenCurrencyMinor('USD', 1999)).toBe('19 US dollars 99 cents');
    expect(spokenCurrencyMinor('USD', 101)).toBe('1 US dollar 1 cent');
  });

  it('parses JPY whole-yen inputs including Japanese full-width digits', () => {
    expect(parseYenInput('3,142')).toEqual({ ok: true, value: 3142 });
    expect(parseYenInput('¥1200')).toEqual({ ok: true, value: 1200 });
    expect(parseYenInput('￥３，１４２')).toEqual({ ok: true, value: 3142 });
    expect(parseYenInput('12.5').ok).toBe(false);
    expect(parseYenInput('0').ok).toBe(false);
    expect(parseYenInput('-500').ok).toBe(false);
    expect(parseYenInput('').ok).toBe(false);
  });

  it('parses USD inputs into exact integer cents including full-width IME digits', () => {
    expect(parseUsdInput('20')).toEqual({ ok: true, value: 2000 });
    expect(parseUsdInput('$19.99')).toEqual({ ok: true, value: 1999 });
    expect(parseUsdInput('1,234.5')).toEqual({ ok: true, value: 123450 });
    expect(parseUsdInput('１９．９９')).toEqual({ ok: true, value: 1999 });
    expect(parseUsdInput('19.999').ok).toBe(false);
    expect(parseUsdInput('0.00').ok).toBe(false);
    expect(parseUsdInput('-10').ok).toBe(false);
    expect(parseUsdInput('').ok).toBe(false);
  });
});
