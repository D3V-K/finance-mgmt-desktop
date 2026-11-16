import { describe, it, expect, beforeEach } from 'vitest';
import { getAppStatus, getProfile, getStorageStatus, runCommand, setMockInvoker, toAppError } from './commands';
import type { AppError } from './types';

describe('command transport', () => {
  beforeEach(() => setMockInvoker(null));

  it('dev preview outside Tauri reports ready with an IANA zone', async () => {
    expect(await getAppStatus()).toEqual({ state: 'ready', error: null });
    const profile = await getProfile();
    expect(profile.timeZone).toBe('Asia/Tokyo');
    expect(profile.today).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });

  it('commands without a dev fallback fail clearly outside Tauri', async () => {
    await expect(runCommand('list_accounts')).rejects.toMatchObject({ code: 'TAURI_NOT_AVAILABLE' });
  });

  it('passes structured AppErrors through unchanged', async () => {
    const err: AppError = { code: 'STORAGE_NOT_READY', message: 'Local storage is not available.', category: 'storage' };
    setMockInvoker(async () => {
      throw err;
    });
    await expect(getStorageStatus()).rejects.toEqual(err);
  });

  it('never surfaces raw invoke strings, which can echo argument values', () => {
    const invalid = toAppError('invalid args `request` for command `create_account`: invalid type: string "Secret Bank"');
    expect(invalid).toMatchObject({ code: 'INVALID_REQUEST', category: 'validation' });
    expect(invalid.message).not.toContain('Secret');

    const denied = toAppError('read_file not allowed. Permissions associated with this command: []');
    expect(denied).toMatchObject({ code: 'COMMAND_FAILED', category: 'internal' });
    expect(denied.message).not.toContain('read_file');
  });
});
