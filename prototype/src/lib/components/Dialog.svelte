<!--
  Native <dialog> in modal mode: the platform handles inertness, Escape and
  top-layer stacking in WebKit (macOS/Linux) and WebView2 (Windows). Focus is
  returned to whatever opened the dialog.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    open: boolean;
    title: string;
    description?: string;
    width?: string;
    onclose: () => void;
    children: Snippet;
  }
  let { open, title, description, width = '520px', onclose, children }: Props = $props();

  let dialog: HTMLDialogElement | undefined = $state();
  let returnFocus: HTMLElement | null = null;
  const id = `dlg-${Math.random().toString(36).slice(2, 8)}`;

  $effect(() => {
    if (!dialog) return;
    if (open && !dialog.open) {
      returnFocus = document.activeElement as HTMLElement | null;
      dialog.showModal();
      (dialog.querySelector<HTMLElement>('[data-autofocus]') ?? dialog.querySelector<HTMLElement>('input, select, button:not(.close)'))?.focus();
    } else if (!open && dialog.open) {
      dialog.close();
      returnFocus?.focus?.();
    }
  });
</script>

<dialog
  bind:this={dialog}
  aria-labelledby="{id}-title"
  aria-describedby={description ? `${id}-desc` : undefined}
  style:width
  oncancel={(e) => { e.preventDefault(); onclose(); }}
  onclick={(e) => { if (e.target === dialog) onclose(); }}
>
  {#if open}
    <div class="inner">
      <header>
        <div>
          <h2 id="{id}-title">{title}</h2>
          {#if description}<p id="{id}-desc" class="muted">{description}</p>{/if}
        </div>
        <button type="button" class="btn ghost small close" aria-label="Close" onclick={onclose}>✕</button>
      </header>
      {@render children()}
    </div>
  {/if}
</dialog>

<style>
  dialog {
    max-width: calc(100vw - 48px); padding: 0; border: 1px solid var(--border);
    border-radius: var(--radius-lg); background: var(--surface-raised); color: var(--ink);
    box-shadow: 0 24px 64px rgb(0 0 0 / 0.25);
  }
  dialog::backdrop { background: rgb(20 20 18 / 0.45); }
  .inner { padding: var(--space-5); display: grid; gap: var(--space-4); }
  header { display: flex; justify-content: space-between; align-items: start; gap: var(--space-4); }
  header p { margin-top: var(--space-1); font-size: var(--text-sm); }
</style>
