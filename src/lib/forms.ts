import { parseNonNegativeYenInput, parseSignedYenInput, parseYenInput, type Yen } from './money';

export type YenMode = 'positive' | 'nonnegative' | 'signed';
export type Parsed = { ok: true; value: Yen } | { ok: false; reason: string };

export function parseFor(mode: YenMode, raw: string): Parsed {
  if (mode === 'signed') return parseSignedYenInput(raw);
  if (mode === 'nonnegative') return parseNonNegativeYenInput(raw);
  return parseYenInput(raw);
}

/** Focus the first invalid control in `root` after a blocked submit. */
export function focusFirstInvalid(root: ParentNode = document) {
  root.querySelector<HTMLElement>('[aria-invalid="true"]')?.focus();
}

export function isUuid(value: string): boolean {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value);
}
