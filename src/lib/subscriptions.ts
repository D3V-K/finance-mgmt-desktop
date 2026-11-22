/**
 * Typed v2 Subscription Plan commands (docs/contracts/v2-subscriptions.md).
 */
import { runCommand } from './commands';
import { assertYen, newWrite } from './ledger';
import type {
  AppError,
  CreateSubscriptionInput,
  DateOnly,
  Id,
  MatchOccurrenceInput,
  OccurrenceOverrideInput,
  PlanRevision,
  PriceRevisionInput,
  ReconcileBatchInput,
  ReconcileOccurrenceInput,
  ReconciledCharge,
  StatusChangeInput,
  SubscriptionDetail,
  SubscriptionListItem,
  SubscriptionOccurrence,
  SubscriptionOverview,
  UnlinkOccurrenceInput,
  UpdateSubscriptionInput,
  Write,
  WriteResult,
} from './types';

export { newWrite };

function assertPositiveMinor(value: unknown, field: string): asserts value is number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value <= 0) {
    throw {
      code: 'INVALID_AMOUNT',
      message: 'Enter a valid positive amount.',
      category: 'validation',
      field,
    } satisfies AppError;
  }
}

function write<T, R>(command: string, request: Write<T>): Promise<WriteResult<R>> {
  return runCommand<WriteResult<R>>(command, { request });
}

export const listSubscriptions = (args: { includeCanceled?: boolean; asOfDate?: DateOnly } = {}) =>
  runCommand<SubscriptionOverview>('list_subscriptions', args);

export const getSubscription = (args: {
  subscriptionId: Id;
  from?: DateOnly;
  to?: DateOnly;
  asOfDate?: DateOnly;
}) => runCommand<SubscriptionDetail>('get_subscription', args);

export function createSubscription(request: Write<CreateSubscriptionInput>) {
  assertPositiveMinor(request.payload.amountMinor, 'amountMinor');
  return write<CreateSubscriptionInput, SubscriptionListItem>('create_subscription', request);
}

export const updateSubscription = (request: Write<UpdateSubscriptionInput>) =>
  write<UpdateSubscriptionInput, SubscriptionListItem>('update_subscription', request);

export function reviseSubscriptionPrice(request: Write<PriceRevisionInput>) {
  assertPositiveMinor(request.payload.amountMinor, 'amountMinor');
  return write<PriceRevisionInput, PlanRevision>('revise_subscription_price', request);
}

export const changeSubscriptionStatus = (request: Write<StatusChangeInput>) =>
  write<StatusChangeInput, SubscriptionListItem>('change_subscription_status', request);

export const listSubscriptionOccurrences = (args: {
  from: DateOnly;
  to: DateOnly;
  asOfDate?: DateOnly;
  subscriptionId?: Id;
  unreconciledOnly?: boolean;
}) => runCommand<SubscriptionOccurrence[]>('list_subscription_occurrences', args);

export const overrideSubscriptionOccurrence = (request: Write<OccurrenceOverrideInput>) =>
  write<OccurrenceOverrideInput, SubscriptionOccurrence>('override_subscription_occurrence', request);

export function reconcileSubscriptionOccurrence(request: Write<ReconcileOccurrenceInput>) {
  if (request.payload.actualJpy !== undefined) {
    assertYen(request.payload.actualJpy, 'actualJpy');
  }
  if (request.payload.billedAmountMinor !== undefined) {
    assertPositiveMinor(request.payload.billedAmountMinor, 'billedAmountMinor');
  }
  return write<ReconcileOccurrenceInput, ReconciledCharge>(
    'reconcile_subscription_occurrence',
    request,
  );
}

export function reconcileSubscriptionBatch(request: Write<ReconcileBatchInput>) {
  for (const item of request.payload.items) {
    if (item.actualJpy !== undefined) {
      assertYen(item.actualJpy, 'actualJpy');
    }
    if (item.billedAmountMinor !== undefined) {
      assertPositiveMinor(item.billedAmountMinor, 'billedAmountMinor');
    }
  }
  return write<ReconcileBatchInput, ReconciledCharge[]>('reconcile_subscription_batch', request);
}

export function matchSubscriptionOccurrence(request: Write<MatchOccurrenceInput>) {
  if (request.payload.billedAmountMinor !== undefined) {
    assertPositiveMinor(request.payload.billedAmountMinor, 'billedAmountMinor');
  }
  return write<MatchOccurrenceInput, SubscriptionOccurrence>(
    'match_subscription_occurrence',
    request,
  );
}

export const unlinkSubscriptionOccurrence = (request: Write<UnlinkOccurrenceInput>) =>
  write<UnlinkOccurrenceInput, SubscriptionOccurrence>('unlink_subscription_occurrence', request);
