import { render, screen, waitFor } from '@testing-library/svelte';
import { describe, it, expect, beforeEach } from 'vitest';
import App from './App.svelte';
import { setMockInvoker } from './lib/commands';
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
  beforeEach(() => setMockInvoker(null));

  it('renders local status once boot is ready', async () => {
    render(App);
    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('Finance Desktop');

    expect(await screen.findByTestId('storage-grid')).toBeInTheDocument();
    expect(screen.getByText('SQLite (WAL mode: Enabled)')).toBeInTheDocument();
    expect(screen.getByText('Offline schema ready')).toBeInTheDocument();
    expect(screen.getByTestId('system-meta')).toHaveTextContent('(Asia/Tokyo)');
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

  it('shows a retryable banner for a storage error after a ready boot', async () => {
    setMockInvoker(async (cmd) => {
      if (cmd === 'get_app_status') return { state: 'ready', error: null };
      if (cmd === 'get_system_info') return { appVersion: '0.1.0', osTarget: 'macos' };
      if (cmd === 'get_profile') return { timeZone: 'Asia/Tokyo', today: '2026-11-16' };
      const err: AppError = {
        code: 'STORAGE_UNAVAILABLE',
        message: 'Local storage is busy. Try again in a moment.',
        category: 'storage',
      };
      throw err;
    });

    render(App);

    expect(await screen.findByTestId('error-banner')).toBeInTheDocument();
    expect(screen.getByText('Error: STORAGE_UNAVAILABLE')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Try again' })).toBeInTheDocument();
  });
});
