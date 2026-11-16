import { invoke } from '@tauri-apps/api/core';
import type { AppError, SystemInfo, StorageStatus } from './types';

export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

let mockInvoker: ((cmd: string, args?: Record<string, unknown>) => Promise<unknown>) | null = null;

export function setMockInvoker(
  invoker: ((cmd: string, args?: Record<string, unknown>) => Promise<unknown>) | null
): void {
  mockInvoker = invoker;
}

export async function runCommand<T>(
  command: string,
  args?: Record<string, unknown>
): Promise<T> {
  if (mockInvoker) {
    return (await mockInvoker(command, args)) as T;
  }

  if (!isTauri()) {
    // Development/test mock fallback when running outside Tauri webview
    if (command === 'get_system_info') {
      return {
        appVersion: '0.1.0-dev',
        appDataDir: 'LocalMockAppData',
        timeZone: Intl.DateTimeFormat().resolvedOptions().timeZone || 'Asia/Tokyo',
        today: new Date().toISOString().split('T')[0] ?? '2026-11-04',
        osTarget: 'browser-mock',
      } as T;
    }
    if (command === 'get_storage_status') {
      return {
        status: 'ready',
        databasePath: 'mock.db',
        isWalEnabled: true,
        migrationVersion: 1,
      } as T;
    }
    throw {
      code: 'TAURI_NOT_AVAILABLE',
      message: `Tauri runtime not available for command: ${command}`,
      category: 'internal',
    } satisfies AppError;
  }

  try {
    return await invoke<T>(command, args);
  } catch (err: unknown) {
    if (err && typeof err === 'object' && 'code' in err && 'category' in err) {
      throw err as AppError;
    }
    throw {
      code: 'COMMAND_FAILED',
      message: typeof err === 'string' ? err : 'Command failed with unexpected error',
      category: 'internal',
    } satisfies AppError;
  }
}

export async function getSystemInfo(): Promise<SystemInfo> {
  return runCommand<SystemInfo>('get_system_info');
}

export async function getStorageStatus(): Promise<StorageStatus> {
  return runCommand<StorageStatus>('get_storage_status');
}
