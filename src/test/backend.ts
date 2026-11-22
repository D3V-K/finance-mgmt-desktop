/**
 * Fake command backend for component tests. Routes `runCommand` calls to
 * per-command handlers (or fixed values) and records every call, so tests can
 * assert the exact command names and wire payloads the UI sends to Rust.
 * Financial results are fixtures, never computed here.
 */
import { setMockInvoker } from '../lib/commands';
import { app } from '../lib/app.svelte';
import type { Account, Balance, Category } from '../lib/types';

export type Handler = (args: Record<string, unknown> | undefined, call: number) => unknown;
export interface Call {
  cmd: string;
  args: Record<string, unknown> | undefined;
}

export const TODAY = '2026-11-19';

export const BANK: Account = { id: '11111111-1111-4111-8111-111111111111', name: 'Everyday', kind: 'BANK', currency: 'JPY', archivedAt: null };
export const CARD: Account = { id: '22222222-2222-4222-8222-222222222222', name: 'Visa', kind: 'CREDIT_CARD', currency: 'JPY', archivedAt: null };
export const FOOD: Category = { id: '33333333-3333-4333-8333-333333333333', name: '食費', type: 'expense', parentId: null, archivedAt: null };
export const SALARY: Category = { id: '44444444-4444-4444-8444-444444444444', name: 'Salary', type: 'income', parentId: null, archivedAt: null };

export function balance(accountId: string, value: number | null, status: Balance['status'] = 'complete'): Balance {
  const card = accountId === CARD.id;
  return {
    accountId,
    asOfDate: TODAY,
    balance: value,
    amountOwed: card && value !== null ? Math.max(0, -value) : null,
    creditBalance: card && value !== null ? Math.max(0, value) : null,
    status,
    openingAsOfDate: status === 'unavailable' ? null : '2026-01-01',
    reason: status === 'complete' ? null : status,
  };
}

export type Fixture = object | string | number | boolean | null;

export function backend(routes: Record<string, Handler | Fixture>) {
  const calls: Call[] = [];
  const counts = new Map<string, number>();
  const defaults: Record<string, unknown> = {
    get_app_status: { state: 'ready', error: null },
    get_profile: { timeZone: 'Asia/Tokyo', today: TODAY },
    get_storage_status: { status: 'ready', databasePath: 'x', isWalEnabled: true, migrationVersion: 2 },
    list_accounts: [BANK, CARD],
    list_categories: [FOOD, SALARY],
  };
  setMockInvoker(async (cmd, args) => {
    calls.push({ cmd, args });
    const n = (counts.get(cmd) ?? 0) + 1;
    counts.set(cmd, n);
    const route = cmd in routes ? routes[cmd] : defaults[cmd];
    if (route === undefined) throw { code: 'NOT_FOUND', message: `No fixture for ${cmd}`, category: 'not_found' };
    return typeof route === 'function' ? (route as Handler)(args, n) : structuredClone(route);
  });
  return {
    calls,
    of: (cmd: string) => calls.filter((c) => c.cmd === cmd),
  };
}

/** Puts the shared app state into a ready, loaded profile for screen tests. */
export function readyApp(accounts: Account[] = [BANK, CARD], categories: Category[] = [FOOD, SALARY]) {
  app.today = TODAY;
  app.timeZone = 'Asia/Tokyo';
  app.accounts = accounts;
  app.categories = categories;
  app.storageIssue = null;
  app.entryOpen = false;
  app.helpOpen = false;
  app.toasts = [];
  app.route = { name: 'overview' };
}

export function written(call: Call | undefined) {
  return (call?.args?.request as { idempotencyKey: string; payload: Record<string, unknown> } | undefined) ?? null;
}
