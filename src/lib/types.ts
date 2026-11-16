/**
 * Error categories matching Rust backend error taxonomy.
 * - storage: SQLite or file system issues. Renders as app-level banner.
 * - validation: user input invalid. Renders field-level inline errors.
 * - not_found: missing entity.
 * - conflict: concurrency, constraint or idempotency collision.
 * - internal: unexpected failure. Redacts sensitive paths/details.
 */
export type ErrorCategory = 'storage' | 'validation' | 'not_found' | 'conflict' | 'internal';

export interface AppError {
  code: string;
  message: string;
  category: ErrorCategory;
  field?: string;
}

export interface SystemInfo {
  appVersion: string;
  appDataDir: string;
  timeZone: string;
  today: string;
  osTarget: string;
}

export interface StorageStatus {
  status: 'ready' | 'uninitialized' | 'error';
  databasePath: string;
  isWalEnabled: boolean;
  migrationVersion: number;
}

export interface IdempotentRequest<T = unknown> {
  idempotencyKey: string;
  payload: T;
}

export interface IdempotentResponse<T = unknown> {
  isReplay: boolean;
  result: T;
}
