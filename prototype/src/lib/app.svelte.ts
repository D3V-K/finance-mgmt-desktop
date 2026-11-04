import { untrack } from 'svelte';
import { CommandFailure, resetPrototypeData, setDemoMode, type CommandError, type DemoMode } from './commands';

export type Route = { name: 'overview' } | { name: 'accounts'; accountId: string | null } | { name: 'cashflow' };
export type Theme = 'system' | 'light' | 'dark';

export const NAV = [
  { name: 'overview', label: 'Overview', shortcut: '1' },
  { name: 'accounts', label: 'Accounts', shortcut: '2' },
  { name: 'cashflow', label: 'Cash flow', shortcut: '3' }
] as const satisfies ReadonlyArray<{ name: Route['name']; label: string; shortcut: string }>;

/** Sections the full app will need; shown so the IA can be reviewed, not navigable yet. */
export const PLANNED = [
  { group: 'Plan', items: ['Budgets & payday', 'Cards & statements', 'Forecast'] },
  { group: 'Review', items: ['Reconciliation', 'Investments', 'Data quality'] },
  { group: 'Data', items: ['Import', 'Backup & restore'] }
];

export interface Toast {
  id: number;
  message: string;
  tone: 'ok' | 'error';
}

class AppState {
  route = $state<Route>({ name: 'overview' });
  demoMode = $state<DemoMode>('ready');
  theme = $state<Theme>('system');
  /** Bumped after every write so open views refetch. */
  revision = $state(0);
  entryOpen = $state(false);
  helpOpen = $state(false);
  toasts = $state<Toast[]>([]);
  #toastId = 0;

  go(route: Route) {
    this.route = route;
  }

  setMode(mode: DemoMode) {
    this.demoMode = mode;
    setDemoMode(mode);
    this.revision += 1;
  }

  setTheme(theme: Theme) {
    this.theme = theme;
    if (theme === 'system') document.documentElement.removeAttribute('data-theme');
    else document.documentElement.dataset.theme = theme;
  }

  resetData() {
    resetPrototypeData();
    this.revision += 1;
    this.toast('Sample data restored');
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
  | { status: 'error'; error: CommandError; data: T | undefined };

/**
 * Loads `fetcher` whenever its reactive inputs (or `app.revision`) change.
 * Stale responses from superseded requests are ignored.
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
        const error: CommandError =
          err instanceof CommandFailure ? err.error : { code: 'storage_unavailable', message: 'Something went wrong reading local data.' };
        state = { status: 'error', error, data: previous };
      }
    );
  }

  const load = () => settle(fetcher());

  // Only the fetcher's synchronous reads (e.g. a selected account id) and the
  // write revision are dependencies; bookkeeping on `state` is untracked.
  $effect(() => {
    void app.revision;
    const promise = fetcher();
    untrack(() => settle(promise));
  });

  return {
    get current() {
      return state;
    },
    reload: load
  };
}
