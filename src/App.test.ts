import { render, screen, waitFor } from '@testing-library/svelte';
import { describe, it, expect, beforeEach } from 'vitest';
import userEvent from '@testing-library/user-event';
import { backend } from './test/backend';
import App from './App.svelte';
import { setMockInvoker } from './lib/commands';
import { app } from './lib/app.svelte';
import type { AppError, AppStatus } from './lib/types';

function failedBoot(code: string, category: AppError['category'] = 'storage') {
  const calls: string[] = [];
  setMockInvoker(async (cmd) => {
    calls.push(cmd);
    if (cmd === 'get_app_status') {
      return { state: 'failed', error: { code, message: 'x', category } } satisfies AppStatus;
    }
    throw new Error(`unexpected command ${cmd}`);
  });
  return calls;
}

describe('App startup gate', () => {
  beforeEach(() => {
    setMockInvoker(null);
    app.today = '';
    app.storageIssue = null;
    app.route = { name: 'overview' };
  });

  it('opens the app shell on the profile day once boot is ready', async () => {
    const api = backend({
      get_dashboard: () => new Promise(() => {}),
    });
    render(App);

    expect(await screen.findByRole('navigation', { name: 'Sections' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: /Overview/ })).toHaveAttribute('aria-current', 'page');
    expect(screen.getByTestId('system-meta')).toHaveTextContent('Today 2026-11-19 (Asia/Tokyo)');
    for (const section of ['Accounts', 'Cash flow', 'Categories', 'Budgets & savings', 'Cards & statements', 'Forecast', 'Reconciliation', 'Investments', 'Data quality', 'Ledger start & history']) {
      expect(screen.getByRole('link', { name: new RegExp(section) })).toBeInTheDocument();
    }
    await waitFor(() => expect(api.of('get_dashboard')[0]?.args).toEqual({ asOfDate: '2026-11-19' }));
  });

  it('switches sections with the keyboard and moves focus to the new heading', async () => {
    backend({ get_dashboard: () => new Promise(() => {}), get_canonical_monthly: [], get_canonical_by_category: [] });
    render(App);
    await screen.findByRole('navigation', { name: 'Sections' });
    const user = userEvent.setup();
    const isMac = /Mac|iPhone|iPad/.test(navigator.platform);
    await user.keyboard(isMac ? '{Meta>}3{/Meta}' : '{Control>}3{/Control}');
    const heading = await screen.findByRole('heading', { level: 1, name: 'Cash flow' });
    await waitFor(() => expect(heading).toHaveFocus());
  });

  it.each([
    ['INSTANCE_LOCKED', 'conflict', 'Finance Desktop is already open'],
    ['MIGRATION_FAILED', 'storage', 'Your data could not be upgraded'],
    ['DATABASE_TOO_NEW', 'storage', 'This data needs a newer version'],
    ['STORAGE_UNAVAILABLE', 'storage', 'Local storage is unavailable'],
  ] as const)('blocks financial screens on %s and never calls data commands', async (code, category, title) => {
    const calls = failedBoot(code, category);
    render(App);

    const alert = await screen.findByTestId('startup-blocked');
    expect(alert).toHaveAttribute('role', 'alert');
    expect(screen.getByRole('heading', { level: 2 })).toHaveTextContent(title);
    expect(alert).toHaveTextContent(`Reference: ${code}`);
    expect(screen.queryByTestId('storage-grid')).not.toBeInTheDocument();
    expect(calls).toEqual(['get_app_status']);
  });

  it('shows one retryable message when the profile cannot be read after boot', async () => {
    setMockInvoker(async (cmd) => {
      if (cmd === 'get_app_status') return { state: 'ready', error: null };
      const err: AppError = {
        code: 'STORAGE_UNAVAILABLE',
        message: 'Local storage is busy. Try again in a moment.',
        category: 'storage',
      };
      throw err;
    });

    render(App);

    const banner = await screen.findByTestId('error-banner');
    expect(banner).toHaveTextContent('Reference: STORAGE_UNAVAILABLE');
    expect(screen.getByRole('button', { name: 'Try again' })).toBeInTheDocument();
    expect(screen.queryByRole('navigation')).not.toBeInTheDocument();
  });

  it('reports a storage failure once in an app banner instead of per panel', async () => {
    backend({
      get_dashboard: () => {
        throw { code: 'STORAGE_FAILURE', message: 'Local storage could not be read.', category: 'storage' };
      },
    });
    render(App);
    const banner = await screen.findByTestId('storage-banner');
    expect(banner).toHaveTextContent('Local storage is not responding.');
    expect(banner).toHaveTextContent('STORAGE_FAILURE');
  });
});
