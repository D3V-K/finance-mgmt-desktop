/**
 * Typed v1 core-ledger commands (docs/contracts/v1-core-ledger.md).
 *
 * Writes take a `Write<T>` built once per user intent with `newWrite`; a retry of
 * the same intent must resend the same object so Rust can return `isReplay`
 * instead of posting twice. Rust remains the validator of record; the checks here
 * only stop values that cannot cross the wire exactly (unsafe integers).
 * Rust handlers land with #7; until then these reject with COMMAND_FAILED.
 */
import { runCommand } from './commands';
import type {
  Account,
  AccountKind,
  AppError,
  Balance,
  Category,
  DateOnly,
  EventInput,
  FinancialEvent,
  Id,
  MonthlyPoint,
  Opening,
  Page,
  Write,
  WriteResult,
  Yen,
} from './types';

export function newIdempotencyKey(): Id {
  return crypto.randomUUID();
}

export function newWrite<T>(payload: T): Write<T> {
  return { idempotencyKey: newIdempotencyKey(), payload };
}

export function assertYen(value: unknown, field: string): asserts value is Yen {
  if (typeof value !== 'number' || !Number.isSafeInteger(value)) {
    throw {
      code: 'INVALID_AMOUNT',
      message: 'Enter a whole-yen amount.',
      category: 'validation',
      field,
    } satisfies AppError;
  }
}

function write<T, R>(command: string, request: Write<T>): Promise<WriteResult<R>> {
  return runCommand<WriteResult<R>>(command, { request });
}

export const listCategories = (includeArchived = false) =>
  runCommand<Category[]>('list_categories', { includeArchived });

export const createCategory = (
  request: Write<{ name: string; type: Category['type']; parentId?: Id }>,
) => write<typeof request.payload, Category>('create_category', request);

export const listAccounts = (includeArchived = false) =>
  runCommand<Account[]>('list_accounts', { includeArchived });

export const createAccount = (request: Write<{ name: string; kind: AccountKind }>) =>
  write<typeof request.payload, Account>('create_account', request);

export function setAccountOpening(
  request: Write<{ accountId: Id; amount: Yen; asOfDate: DateOnly; reason: string }>,
) {
  assertYen(request.payload.amount, 'amount');
  return write<typeof request.payload, Opening>('set_account_opening', request);
}

export const getAccountBalance = (accountId: Id, asOfDate: DateOnly) =>
  runCommand<Balance>('get_account_balance', { accountId, asOfDate });

export function createFinancialEvent(request: Write<EventInput>) {
  assertYen(request.payload.amount, 'amount');
  if (request.payload.adjustmentDelta !== undefined) {
    assertYen(request.payload.adjustmentDelta, 'adjustmentDelta');
  }
  return write<EventInput, FinancialEvent>('create_financial_event', request);
}

export const listFinancialEvents = (args: { accountId?: Id; page: number; pageSize: number }) =>
  runCommand<Page<FinancialEvent>>('list_financial_events', args);

export const getCanonicalMonthly = (range: { from?: DateOnly; to?: DateOnly } = {}) =>
  runCommand<MonthlyPoint[]>('get_canonical_monthly', range);
