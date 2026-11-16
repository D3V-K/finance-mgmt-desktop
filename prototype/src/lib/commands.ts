// In-memory stand-in for the Tauri command boundary.
//
// This is NOT a contract proposal: command names, payloads and the error
// shape will be agreed in #4 (transport) and #7 (domain). It exists so the
// prototype exercises the states a real local command produces — latency,
// structured failure, empty results and idempotent writes — and so screens
// depend on an async interface rather than on fixtures directly.

import { accounts, cashFlow, entries, TODAY, type Account, type CashFlowMonth, type EventKind, type LedgerEntry } from './fixtures';
import type { Yen } from './money';

export type DemoMode = 'ready' | 'empty' | 'slow' | 'error';

export interface CommandError {
  /** Stable machine code; the UI maps it to copy. */
  code: 'storage_unavailable' | 'validation' | 'not_found';
  /** Safe to show; contains no file paths or record contents. */
  message: string;
  field?: string;
}

export class CommandFailure extends Error {
  constructor(readonly error: CommandError) {
    super(error.message);
  }
}

export interface NewEntryInput {
  /** Generated once per form session so a retried submit can't post twice. */
  idempotencyKey: string;
  kind: Extract<EventKind, 'EXPENSE' | 'INCOME' | 'TRANSFER'>;
  amount: Yen;
  description: string;
  category: string | null;
  occurredOn: string;
  accountId: string;
  toAccountId?: string;
}

const config = { mode: 'ready' as DemoMode, latencyMs: 180 };
let ledger: LedgerEntry[] = [...entries];
const seenKeys = new Map<string, LedgerEntry>();

export function setDemoMode(mode: DemoMode) {
  config.mode = mode;
}
export function setLatency(ms: number) {
  config.latencyMs = ms;
}
export function resetPrototypeData() {
  ledger = [...entries];
  seenKeys.clear();
}

async function respond<T>(value: () => T, emptyValue: T): Promise<T> {
  const wait = config.mode === 'slow' ? 2_500 : config.latencyMs;
  if (wait > 0) await new Promise((r) => setTimeout(r, wait));
  if (config.mode === 'error') {
    throw new CommandFailure({ code: 'storage_unavailable', message: 'The local data file could not be opened.' });
  }
  return config.mode === 'empty' ? emptyValue : structuredClone(value());
}

function applyPostings(list: Account[]): Account[] {
  const added = ledger.filter((e) => e.id.startsWith('new-'));
  return list.map((a) => {
    if (a.balance === null) return a;
    const delta = added.flatMap((e) => e.postings).filter((p) => p.accountId === a.id).reduce((s, p) => s + p.delta, 0);
    return { ...a, balance: a.balance + delta };
  });
}

export const commands = {
  listAccounts: () => respond(() => applyPostings(accounts), [] as Account[]),

  listEntries: (filter: { accountId?: string } = {}) =>
    respond(
      () =>
        ledger
          .filter((e) => !filter.accountId || e.postings.some((p) => p.accountId === filter.accountId))
          .sort((a, b) => b.effectiveDate.localeCompare(a.effectiveDate)),
      [] as LedgerEntry[]
    ),

  cashFlowReport: () => respond(() => cashFlow, [] as CashFlowMonth[]),

  async createEntry(input: NewEntryInput): Promise<{ entry: LedgerEntry; replayed: boolean }> {
    const replay = seenKeys.get(input.idempotencyKey);
    // A replay returns the original result even if the store is now failing,
    // mirroring a write that committed before the response was lost.
    if (replay) return { entry: structuredClone(replay), replayed: true };

    await respond(() => null, null);
    if (input.kind === 'TRANSFER' && (!input.toAccountId || input.toAccountId === input.accountId)) {
      throw new CommandFailure({ code: 'validation', field: 'toAccountId', message: 'Choose a different account to move money to.' });
    }
    if (input.occurredOn > TODAY) {
      throw new CommandFailure({ code: 'validation', field: 'occurredOn', message: 'Future-dated entries belong in a plan, not the ledger.' });
    }
    const postings =
      input.kind === 'TRANSFER'
        ? [{ accountId: input.accountId, delta: -input.amount }, { accountId: input.toAccountId!, delta: input.amount }]
        : [{ accountId: input.accountId, delta: input.kind === 'INCOME' ? input.amount : -input.amount }];
    const entry: LedgerEntry = {
      id: `new-${seenKeys.size + 1}`,
      kind: input.kind,
      description: input.description,
      category: input.kind === 'TRANSFER' ? null : input.category,
      occurredOn: input.occurredOn,
      effectiveDate: input.occurredOn,
      dateBasis: 'known',
      postings
    };
    ledger = [entry, ...ledger];
    seenKeys.set(input.idempotencyKey, entry);
    return { entry: structuredClone(entry), replayed: false };
  }
};

export type Commands = typeof commands;
