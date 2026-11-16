<script lang="ts">
  import { onMount } from 'svelte';
  import { getAppStatus, getProfile, getStorageStatus, getSystemInfo, toAppError } from './lib/commands';
  import type { AppError, Profile, StorageStatus, SystemInfo } from './lib/types';

  type Phase = 'starting' | 'blocked' | 'ready';

  /** Startup failures are terminal for this launch: copy explains the safe next step. */
  const BLOCKED_COPY: Record<string, { title: string; body: string }> = {
    INSTANCE_LOCKED: {
      title: 'Finance Desktop is already open',
      body: 'Only one copy can use your data at a time. Switch to the open window, or quit it and open the app again.',
    },
    MIGRATION_FAILED: {
      title: 'Your data could not be upgraded',
      body: 'No changes were applied to your data. Quit and reopen the app. If this keeps happening, restore from a backup.',
    },
    DATABASE_TOO_NEW: {
      title: 'This data needs a newer version',
      body: 'Your data was saved by a newer version of Finance Desktop. Install that version to open it; this version will not change it.',
    },
  };
  const DEFAULT_BLOCKED = {
    title: 'Local storage is unavailable',
    body: 'Finance Desktop could not open its data on this computer. Quit and reopen the app. Nothing was sent anywhere.',
  };

  let phase = $state<Phase>('starting');
  let startupError = $state<AppError | null>(null);
  let systemInfo = $state<SystemInfo | null>(null);
  let profile = $state<Profile | null>(null);
  let storageStatus = $state<StorageStatus | null>(null);
  let error = $state<AppError | null>(null);
  let isLoading = $state(false);

  const blockedCopy = $derived(
    startupError ? (BLOCKED_COPY[startupError.code] ?? DEFAULT_BLOCKED) : DEFAULT_BLOCKED,
  );

  async function start() {
    try {
      const status = await getAppStatus();
      if (status.state !== 'ready') {
        startupError = status.error;
        phase = 'blocked';
        return;
      }
    } catch (err) {
      startupError = toAppError(err);
      phase = 'blocked';
      return;
    }
    phase = 'ready';
    await loadState();
  }

  async function loadState() {
    isLoading = true;
    error = null;
    try {
      [systemInfo, profile, storageStatus] = await Promise.all([
        getSystemInfo(),
        getProfile(),
        getStorageStatus(),
      ]);
    } catch (err: unknown) {
      error = toAppError(err);
    } finally {
      isLoading = false;
    }
  }

  onMount(() => {
    void start();
  });
</script>

<main class="shell">
  <header class="app-header">
    <div class="header-brand">
      <span class="badge">Offline</span>
      <h1 class="app-title">Finance Desktop</h1>
    </div>
    {#if systemInfo && profile}
      <div class="system-meta" data-testid="system-meta">
        <span>v{systemInfo.appVersion}</span>
        <span class="divider">·</span>
        <span>{profile.today} ({profile.timeZone})</span>
      </div>
    {/if}
  </header>

  {#if phase === 'starting'}
    <div class="loading-state" role="status" data-testid="loading-state">
      <p>Opening your local data …</p>
    </div>
  {:else if phase === 'blocked'}
    <section class="blocked" role="alert" aria-labelledby="blocked-title" data-testid="startup-blocked">
      <h2 id="blocked-title">{blockedCopy.title}</h2>
      <p>{blockedCopy.body}</p>
      {#if startupError}
        <p class="category-tag">Reference: {startupError.code}</p>
      {/if}
    </section>
  {:else}
    {#if error}
      <div class="banner banner-error" role="alert" data-testid="error-banner">
        <div class="banner-content">
          <strong>Error: {error.code}</strong>
          <p>{error.message}</p>
          <span class="category-tag">Category: {error.category}</span>
        </div>
        <button class="retry-btn" onclick={loadState}>Try again</button>
      </div>
    {/if}

    <section class="content" aria-busy={isLoading}>
      {#if isLoading}
        <div class="loading-state" data-testid="loading-state">
          <p>Loading local status …</p>
        </div>
      {:else if storageStatus}
        <div class="status-grid" data-testid="storage-grid">
          <div class="status-card">
            <span class="card-label">Storage Engine</span>
            <strong class="card-value">SQLite (WAL mode: {storageStatus.isWalEnabled ? 'Enabled' : 'Disabled'})</strong>
            <span class="card-subtext">Status: {storageStatus.status}</span>
          </div>

          <div class="status-card">
            <span class="card-label">Migration Level</span>
            <strong class="card-value">Version {storageStatus.migrationVersion}</strong>
            <span class="card-subtext">Offline schema ready</span>
          </div>

          {#if systemInfo}
            <div class="status-card">
              <span class="card-label">Platform Target</span>
              <strong class="card-value">{systemInfo.osTarget}</strong>
              <span class="card-subtext">Data stays on this computer</span>
            </div>
          {/if}
        </div>

        <div class="welcome-box">
          <h2>Local Application Runtime Active</h2>
          <p>
            Finance Desktop operates fully offline without remote AWS cloud services.
            All data transactions and financial calculations are processed locally.
          </p>
        </div>
      {/if}
    </section>
  {/if}
</main>

<style>
  :global(body) {
    margin: 0;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Hiragino Sans", "Noto Sans JP", sans-serif;
    background-color: #f7f9fa;
    color: #1a202c;
    min-width: 960px;
  }

  .shell {
    max-width: 1200px;
    margin: 0 auto;
    padding: 2rem;
  }

  .app-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid #e2e8f0;
    padding-bottom: 1rem;
    margin-bottom: 1.5rem;
  }

  .header-brand {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .badge {
    background-color: #047857;
    color: #ffffff;
    font-size: 0.75rem;
    font-weight: 600;
    padding: 0.2rem 0.5rem;
    border-radius: 9999px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .app-title {
    font-size: 1.5rem;
    margin: 0;
    font-weight: 700;
  }

  .system-meta {
    font-size: 0.875rem;
    color: #64748b;
  }

  .divider {
    margin: 0 0.5rem;
  }

  .banner {
    padding: 1rem;
    border-radius: 8px;
    margin-bottom: 1.5rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .banner-error {
    background-color: #fef2f2;
    border: 1px solid #fecaca;
    color: #991b1b;
  }

  .banner-content p {
    margin: 0.25rem 0;
  }

  .category-tag {
    font-size: 0.75rem;
    opacity: 0.85;
  }

  .retry-btn {
    background-color: #dc2626;
    color: white;
    border: none;
    padding: 0.5rem 1rem;
    border-radius: 6px;
    cursor: pointer;
    font-weight: 600;
  }

  .status-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 1rem;
    margin-bottom: 1.5rem;
  }

  .status-card {
    background: #ffffff;
    border: 1px solid #e2e8f0;
    border-radius: 8px;
    padding: 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .card-label {
    font-size: 0.8125rem;
    color: #64748b;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .card-value {
    font-size: 1.125rem;
    color: #0f172a;
  }

  .card-subtext {
    font-size: 0.8125rem;
    color: #94a3b8;
  }

  .welcome-box {
    background: #ffffff;
    border: 1px solid #e2e8f0;
    border-radius: 8px;
    padding: 1.5rem;
  }

  .welcome-box h2 {
    margin-top: 0;
    font-size: 1.125rem;
  }

  .welcome-box p {
    color: #475569;
    line-height: 1.5;
    margin-bottom: 0;
  }

  .blocked {
    background: #ffffff;
    border: 1px solid #fecaca;
    border-left: 4px solid #dc2626;
    border-radius: 8px;
    padding: 1.5rem;
    max-width: 40rem;
  }

  .blocked h2 {
    margin-top: 0;
    font-size: 1.25rem;
  }

  .blocked p {
    color: #475569;
    line-height: 1.5;
  }

  .loading-state {
    padding: 3rem;
    text-align: center;
    color: #64748b;
  }
</style>
