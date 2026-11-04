import { render, screen, waitFor } from '@testing-library/svelte';
import { describe, it, expect, beforeEach } from 'vitest';
import App from './App.svelte';
import { setMockInvoker } from './lib/commands';
import type { AppError } from './lib/types';

describe('App shell component', () => {
  beforeEach(() => {
    setMockInvoker(null);
  });

  it('renders application title and loading state then status cards', async () => {
    render(App);

    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('Finance Desktop');

    await waitFor(() => {
      expect(screen.getByTestId('storage-grid')).toBeInTheDocument();
    });

    expect(screen.getByText('SQLite (WAL mode: Enabled)')).toBeInTheDocument();
    expect(screen.getByText('Offline schema ready')).toBeInTheDocument();
  });

  it('renders app-level error banner when command fails with storage error', async () => {
    setMockInvoker(async (cmd) => {
      if (cmd === 'get_system_info') {
        return {
          appVersion: '0.1.0-dev',
          appDataDir: 'test-dir',
          timeZone: 'Asia/Tokyo',
          today: '2026-11-04',
          osTarget: 'macOS',
        };
      }
      const err: AppError = {
        code: 'STORAGE_LOCKED',
        message: 'Database is locked by another process',
        category: 'storage',
      };
      throw err;
    });

    render(App);

    await waitFor(() => {
      expect(screen.getByTestId('error-banner')).toBeInTheDocument();
    });

    expect(screen.getByText('Error: STORAGE_LOCKED')).toBeInTheDocument();
    expect(screen.getByText('Database is locked by another process')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Try again' })).toBeInTheDocument();
  });
});
