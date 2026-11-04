<script lang="ts">
  import Dialog from '../lib/components/Dialog.svelte';
  import { NAV } from '../lib/app.svelte';

  let { open, mod, onclose }: { open: boolean; mod: string; onclose: () => void } = $props();

  const rows = $derived([
    { keys: [`${mod}`, 'N'], action: 'New entry' },
    ...NAV.map((n) => ({ keys: [mod, n.shortcut], action: `Go to ${n.label}` })),
    { keys: ['/'], action: 'Search history (Accounts)' },
    { keys: ['↑', '↓'], action: 'Move through the account list' },
    { keys: ['←', '→'], action: 'Move between months in a chart' },
    { keys: [mod, 'Enter'], action: 'Save the open form' },
    { keys: ['Esc'], action: 'Close a dialog without saving' },
    { keys: ['?'], action: 'Show this list' }
  ]);
</script>

<Dialog {open} title="Keyboard shortcuts" width="420px" {onclose}>
  <table class="data">
    <caption class="sr-only">Keyboard shortcuts</caption>
    <thead><tr><th scope="col">Keys</th><th scope="col">Action</th></tr></thead>
    <tbody>
      {#each rows as r (r.action)}
        <tr><td>{#each r.keys as k, i (i)}{#if i}<span aria-hidden="true"> </span>{/if}<kbd>{k}</kbd>{/each}</td><td>{r.action}</td></tr>
      {/each}
    </tbody>
  </table>
  <div><button class="btn" onclick={onclose} data-autofocus>Done</button></div>
</Dialog>
