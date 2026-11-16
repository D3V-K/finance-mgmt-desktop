import { describe, it, expect, beforeEach } from 'vitest';
import { getSystemInfo, getStorageStatus, setMockInvoker, runCommand } from './commands';
import type { AppError } from './types';

describe('command transport', () => {
  beforeEach(() => {
    setMockInvoker(null);
  });

  it('returns mock system info outside tauri', async () => {
    const info = await getSystemInfo();
    expect(info.appVersion).toBe('0.1.0-dev');
    expect(info.osTarget).toBe('browser-mock');
    expect(info.today).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });

  it('returns mock storage status outside tauri', async () => {
    const status = await getStorageStatus();
    expect(status.status).toBe('ready');
    expect(status.isWalEnabled).toBe(true);
  });

  it('allows injecting custom invoker for tests', async () => {
    setMockInvoker(async (cmd) => {
      if (cmd === 'custom_test') {
        return { ok: true };
      }
      throw new Error('unknown');
    });

    const res = await runCommand<{ ok: boolean }>('custom_test');
    expect(res.ok).toBe(true);
  });

  it('handles structured errors properly', async () => {
    setMockInvoker(async () => {
      const err: AppError = {
        code: 'STORAGE_UNAVAILABLE',
        message: 'Could not open database',
        category: 'storage',
      };
      throw err;
    });

    await expect(getStorageStatus()).rejects.toEqual({
      code: 'STORAGE_UNAVAILABLE',
      message: 'Could not open database',
      category: 'storage',
    });
  });
});
