import { invoke } from '@tauri-apps/api/core';
import type { AppError, AppStatus, Profile, StorageStatus, SystemInfo } from './types';

type Invoker = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

let mockInvoker: Invoker | null = null;

/** Test/dev hook. Production code paths always go through Tauri `invoke`. */
export function setMockInvoker(invoker: Invoker | null): void {
  mockInvoker = invoker;
}

export function isAppError(value: unknown): value is AppError {
  return (
    !!value &&
    typeof value === 'object' &&
    typeof (value as AppError).code === 'string' &&
    typeof (value as AppError).category === 'string'
  );
}

/**
 * Normalizes non-AppError rejections (ACL denial, malformed arguments). The raw
 * text can echo argument values, so it is never shown to the user.
 */
export function toAppError(err: unknown): AppError {
  if (isAppError(err)) return err;
  const raw = typeof err === 'string' ? err : '';
  if (raw.includes('invalid args')) {
    return {
      code: 'INVALID_REQUEST',
      message: 'The request was not in the expected format.',
      category: 'validation',
    };
  }
  return {
    code: 'COMMAND_FAILED',
    message: 'The action could not be completed.',
    category: 'internal',
  };
}

/** Browser-only dev preview (`npm run dev` outside Tauri). Never used in a build. */
const devFallback: Record<string, () => unknown> = {
  get_app_status: () => ({ state: 'ready', error: null }) satisfies AppStatus,
  get_system_info: () => ({ appVersion: '0.1.0-dev', osTarget: 'browser-dev' }) satisfies SystemInfo,
  get_storage_status: () =>
    ({ status: 'ready', databasePath: 'dev-preview', isWalEnabled: true, migrationVersion: 0 }) satisfies StorageStatus,
  get_profile: () => ({ timeZone: 'Asia/Tokyo', today: tokyoToday() }) satisfies Profile,
};

function tokyoToday(): string {
  // en-CA formats as YYYY-MM-DD.
  return new Intl.DateTimeFormat('en-CA', { timeZone: 'Asia/Tokyo' }).format(new Date());
}

export async function runCommand<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (mockInvoker) {
    try {
      return (await mockInvoker(command, args)) as T;
    } catch (err) {
      throw toAppError(err);
    }
  }

  if (!isTauri()) {
    const fallback = import.meta.env.DEV ? devFallback[command] : undefined;
    if (fallback) return fallback() as T;
    throw {
      code: 'TAURI_NOT_AVAILABLE',
      message: 'This action is only available in the desktop app.',
      category: 'internal',
    } satisfies AppError;
  }

  try {
    return await invoke<T>(command, args);
  } catch (err: unknown) {
    throw toAppError(err);
  }
}

export const getAppStatus = () => runCommand<AppStatus>('get_app_status');
export const getSystemInfo = () => runCommand<SystemInfo>('get_system_info');
export const getStorageStatus = () => runCommand<StorageStatus>('get_storage_status');
export const getProfile = () => runCommand<Profile>('get_profile');
