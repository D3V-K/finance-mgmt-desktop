<!--
  Uniform loading / error / empty / ready handling for any resource. Stale
  data stays visible (dimmed) while a refresh is in flight.
-->
<script lang="ts" generics="T">
  import type { Snippet } from 'svelte';
  import type { ResourceState } from '../app.svelte';

  interface Props {
    state: ResourceState<T>;
    isEmpty: (data: T) => boolean;
    onretry: () => void;
    label: string;
    empty: Snippet;
    children: Snippet<[T]>;
    skeletonRows?: number;
  }
  let { state, isEmpty, onretry, label, empty, children, skeletonRows = 4 }: Props = $props();
</script>

{#if state.status === 'error'}
  <div class="state error" role="alert">
    <p class="title">Couldn’t load {label}</p>
    <p class="muted">{state.error.message} Nothing has been changed.</p>
    <button class="btn small" onclick={onretry}>Try again</button>
  </div>
{:else if state.data === undefined}
  <div class="loading" role="status" aria-live="polite">
    <span class="sr-only">Loading {label}…</span>
    {#each Array(skeletonRows) as _, i (i)}<div class="skeleton" style:width="{90 - i * 12}%"></div>{/each}
  </div>
{:else if isEmpty(state.data)}
  <div class="state">{@render empty()}</div>
{:else}
  <div class="ready" class:stale={state.status === 'loading'} aria-busy={state.status === 'loading'}>
    {@render children(state.data)}
  </div>
{/if}

<style>
  .state { padding: var(--space-6) var(--space-5); display: grid; gap: var(--space-2); justify-items: start; }
  .state.error { border-left: 3px solid var(--state-error); background: var(--state-error-bg); border-radius: var(--radius-md); }
  .title { font-weight: 620; }
  .loading { padding: var(--space-4) var(--space-5); display: grid; gap: var(--space-3); }
  .skeleton { height: 14px; border-radius: 4px; background: linear-gradient(90deg, var(--surface-sunken), var(--border), var(--surface-sunken)); background-size: 200% 100%; animation: shimmer 1.2s linear infinite; }
  .stale { opacity: 0.6; transition: opacity var(--duration); }
  @keyframes shimmer { to { background-position: -200% 0; } }
  @media (prefers-reduced-motion: reduce) { .skeleton { animation: none; } }
</style>
