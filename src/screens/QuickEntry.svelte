<script lang="ts">
  import { app } from '../lib/app.svelte';
  import Dialog from '../lib/components/Dialog.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';
  import EventForm from './EventForm.svelte';

  let { open, onclose }: { open: boolean; onclose: () => void } = $props();
  let keepOpen = $state(false);
  let formKey = $state(0);
  const preset = $derived(app.route.name === 'accounts' ? app.route.accountId : null);
</script>

<Dialog {open} title="New entry" description="Saved on this device only. Amounts are whole yen." width="640px" {onclose}>
  {#if app.activeAccounts.length === 0}
    <EmptyState title="Add an account first" body="Entries are recorded against a named account such as a wallet, bank account or card.">
      {#snippet action()}
        <button class="btn primary" data-autofocus onclick={() => { onclose(); app.go({ name: 'accounts', accountId: null }); }}>Go to Accounts</button>
      {/snippet}
    </EmptyState>
  {:else}
    {#key formKey}
      <EventForm
        presetAccountId={preset}
        onsaved={() => {
          if (keepOpen) formKey += 1;
          else onclose();
        }}
        oncancel={onclose}
      />
    {/key}
    <label class="check"><input type="checkbox" bind:checked={keepOpen} /> Add another after saving</label>
  {/if}
</Dialog>
