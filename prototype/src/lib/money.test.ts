import { formatYen, parseYenInput, spokenYen, sumKnown } from './money';

describe('formatYen', () => {
  it('groups digits and uses a real minus sign', () => {
    expect(formatYen(1_234_567)).toBe('¥1,234,567');
    expect(formatYen(-86_420)).toBe('−¥86,420');
    expect(formatYen(500, { signed: true })).toBe('+¥500');
    expect(formatYen(0, { signed: true })).toBe('¥0');
  });

  it('handles very large whole-yen amounts', () => {
    expect(formatYen(9_007_199_254_740_991)).toBe('¥9,007,199,254,740,991');
  });

  it('rejects non-integer amounts instead of rounding them', () => {
    expect(() => formatYen(10.5)).toThrow(RangeError);
  });

  it('speaks unknown and negative amounts in words', () => {
    expect(spokenYen(null)).toBe('unknown amount');
    expect(spokenYen(-1200)).toBe('minus 1,200 yen');
  });
});

describe('sumKnown', () => {
  it('returns a total only when every value is known', () => {
    expect(sumKnown([100, 200])).toEqual({ total: 300, knownSubtotal: 300, unknownCount: 0, count: 2 });
  });

  it('never treats unknown as zero', () => {
    const s = sumKnown([100, null, 50]);
    expect(s.total).toBeNull();
    expect(s.knownSubtotal).toBe(150);
    expect(s.unknownCount).toBe(1);
  });
});

describe('parseYenInput', () => {
  it.each([
    ['1200', 1200],
    ['1,200', 1200],
    ['¥3,000', 3000],
    ['１２００', 1200],
    [' 45 ', 45]
  ])('accepts %j', (raw, value) => {
    expect(parseYenInput(raw)).toEqual({ ok: true, value });
  });

  it.each([
    ['', 'Enter an amount.'],
    ['12.5', 'Use whole yen — no decimals.'],
    ['-50', 'Use digits only, like 1,200.'],
    ['0', 'Amount must be more than ¥0.'],
    ['99999999999999999999', 'That amount is too large.']
  ])('rejects %j', (raw, reason) => {
    expect(parseYenInput(raw)).toEqual({ ok: false, reason });
  });
});
