import { untrack } from 'svelte';
import { toAppError } from './commands';
import { listAccounts, listCategories } from './ledger';
import type { Account, AppError, Category, Id, Write, WriteResult } from './types';

export type Route =
  | { name: 'overview' }
  | { name: 'accounts'; accountId: Id | null }
  | { name: 'cashflow' }
  | { name: 'categories' }
  | { name: 'budgets' }
  | { name: 'cards'; cardId: Id | null }
  | { name: 'forecast' }
  | { name: 'subscriptions' }
  | { name: 'reconciliation'; reconciliationId: Id | null }
  | { name: 'investments' }
  | { name: 'quality' }
  | { name: 'start' };
export type RouteName = Route['name'];
export type Theme = 'system' | 'light' | 'dark';

export interface NavItem {
  name: RouteName;
  label: string;
  shortcut?: string;
}

export const NAV_GROUPS: { group: string; items: NavItem[] }[] = [
  {
    group: 'Track',
    items: [
      { name: 'overview', label: 'Overview', shortcut: '1' },
      { name: 'accounts', label: 'Accounts', shortcut: '2' },
      { name: 'cashflow', label: 'Cash flow', shortcut: '3' },
      { name: 'categories', label: 'Categories', shortcut: '4' },
    ],
  },
  {
    group: 'Plan',
    items: [
      { name: 'budgets', label: 'Budgets & savings', shortcut: '5' },
      { name: 'cards', label: 'Cards & statements', shortcut: '6' },
      { name: 'forecast', label: 'Forecast', shortcut: '7' },
      { name: 'subscriptions', label: 'Subscriptions' },
    ],
  },
  {
    group: 'Review',
    items: [
      { name: 'reconciliation', label: 'Reconciliation', shortcut: '8' },
      { name: 'investments', label: 'Investments', shortcut: '9' },
      { name: 'quality', label: 'Data quality' },
    ],
  },
  {
    group: 'Data',
    items: [{ name: 'start', label: 'Ledger start & history' }],
  },
];
export const NAV: NavItem[] = NAV_GROUPS.flatMap((g) => g.items);
/** Owned by #14/#15; listed so the information architecture stays complete. */
export const PLANNED = ['Import', 'Backup & restore'];

export function routeFor(name: RouteName): Route {
  switch (name) {
    case 'accounts':
      return { name, accountId: null };
    case 'cards':
      return { name, cardId: null };
    case 'reconciliation':
      return { name, reconciliationId: null };
    default:
      return { name };
  }
}

export interface Toast {
  id: number;
  message: string;
  tone: 'ok' | 'error';
}

class AppState {
  route = $state<Route>({ name: 'overview' });
  theme = $state<Theme>('system');
  /** Profile calendar day from Rust; never the webview clock. */
  today = $state('');
  timeZone = $state('');
  /** Bumped after every write so open views refetch. */
  revision = $state(0);
  entryOpen = $state(false);
  helpOpen = $state(false);
  toasts = $state<Toast[]>([]);
  /** A storage-level failure: one app banner instead of an error per panel. */
  storageIssue = $state<AppError | null>(null);
  accounts = $state<Account[] | undefined>(undefined);
  categories = $state<Category[] | undefined>(undefined);
  #toastId = 0;

  go(route: Route) {
    this.route = route;
  }

  setTheme(theme: Theme) {
    this.theme = theme;
    if (theme === 'system') document.documentElement.removeAttribute('data-theme');
    else document.documentElement.dataset.theme = theme;
  }

  changed() {
    this.revision += 1;
  }

  async loadReferences() {
    try {
      [this.accounts, this.categories] = await Promise.all([listAccounts(true), listCategories(true)]);
    } catch (err) {
      this.noteError(toAppError(err));
    }
  }

  noteError(error: AppError) {
    if (error.category === 'storage') this.storageIssue = error;
  }

  account(id: Id | null | undefined): Account | undefined {
    return id ? this.accounts?.find((a) => a.id === id) : undefined;
  }

  accountName(id: Id | null | undefined): string {
    return this.account(id)?.name ?? 'Unknown account';
  }

  categoryName(id: Id | null | undefined): string {
    if (!id) return 'Uncategorized';
    return this.categories?.find((c) => c.id === id)?.name ?? 'Unknown category';
  }

  get activeAccounts(): Account[] {
    return (this.accounts ?? []).filter((a) => a.archivedAt === null);
  }

  toast(message: string, tone: Toast['tone'] = 'ok') {
    const id = ++this.#toastId;
    this.toasts = [...this.toasts, { id, message, tone }];
    setTimeout(() => this.dismiss(id), 6_000);
  }

  dismiss(id: number) {
    this.toasts = this.toasts.filter((t) => t.id !== id);
  }
}

export const app = new AppState();

export type ResourceState<T> =
  | { status: 'loading'; data: T | undefined }
  | { status: 'ready'; data: T }
  | { status: 'error'; error: AppError; data: T | undefined };

/**
 * Loads `fetcher` whenever its reactive inputs (or `app.revision`) change.
 * Stale responses from superseded requests are ignored. Must be created during
 * component initialisation.
 */
export function resource<T>(fetcher: () => Promise<T>) {
  let state = $state<ResourceState<T>>({ status: 'loading', data: undefined });
  let request = 0;

  function settle(promise: Promise<T>) {
    const mine = ++request;
    const previous = state.data;
    state = { status: 'loading', data: previous };
    promise.then(
      (data) => {
        if (mine === request) state = { status: 'ready', data };
      },
      (err: unknown) => {
        if (mine !== request) return;
        const error = toAppError(err);
        app.noteError(error);
        state = { status: 'error', error, data: previous };
      },
    );
  }

  $effect(() => {
    void app.revision;
    const promise = fetcher();
    untrack(() => settle(promise));
  });

  return {
    get current() {
      return state;
    },
    reload: () => settle(fetcher()),
  };
}

/**
 * One form session = one user intent = one idempotency key. A retry after a
 * failure resends the same key, so Rust replays a write that did commit
 * instead of posting twice. The key is renewed only after success or when the
 * user explicitly starts over.
 */
export class WriteSession {
  #key = crypto.randomUUID();
  submitting = $state(false);
  failure = $state<AppError | null>(null);
  fieldError = $state<{ field: string; message: string } | null>(null);

  get conflict() {
    return this.failure?.code === 'IDEMPOTENCY_CONFLICT';
  }

  async run<T, R>(
    payload: T,
    send: (request: Write<T>) => Promise<WriteResult<R>>,
  ): Promise<WriteResult<R> | null> {
    if (this.submitting) return null;
    this.submitting = true;
    this.failure = null;
    this.fieldError = null;
    try {
      const result = await send({ idempotencyKey: this.#key, payload });
      this.#key = crypto.randomUUID();
      app.changed();
      if (result.isReplay) app.toast('Already saved — nothing was recorded twice.');
      return result;
    } catch (err) {
      const error = toAppError(err);
      app.noteError(error);
      if (error.category === 'validation' && error.field) {
        this.fieldError = { field: error.field, message: error.message };
      } else {
        this.failure = error;
      }
      return null;
    } finally {
      this.submitting = false;
    }
  }

  /** Field message from the last server rejection, matched by wire name. */
  fieldMessage(field: string): string | undefined {
    const f = this.fieldError;
    if (!f) return undefined;
    return f.field === field || f.field.endsWith(`.${field}`) ? f.message : undefined;
  }

  /** Validation errors the form cannot attach to a visible field. */
  get orphanFieldError(): string | undefined {
    return this.fieldError?.message;
  }

  reset() {
    this.#key = crypto.randomUUID();
    this.failure = null;
    this.fieldError = null;
  }
}
