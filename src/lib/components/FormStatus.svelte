<!--
  Write outcome for a form: nothing-saved failures, duplicate-key conflicts and
  server validation errors that do not belong to a visible field. The form
  keeps every input intact; the submit button becomes "Try again".
-->
<script lang="ts">
  import type { WriteSession } from '../app.svelte';

  interface Props {
    session: WriteSession;
    /** Wire field names the form renders inline; others are shown here. */
    fields?: string[];
  }
  let { session, fields = [] }: Props = $props();

  const orphan = $derived(
    session.fieldError && !fields.some((f) => session.fieldMessage(f)) ? session.fieldError.message : null,
  );
</script>

{#if session.conflict}
  <div class="status" role="alert">
    <strong>This was already saved with different details.</strong>
    Nothing new was recorded. Check the history before saving again, or start a new entry.
    <button type="button" class="btn small" onclick={() => session.reset()}>Start a new entry</button>
  </div>
{:else if session.failure}
  <div class="status" role="alert">
    <strong>Nothing was saved.</strong>
    {session.failure.message} Your input is still here — try again when ready.
  </div>
{:else if orphan}
  <div class="status" role="alert"><strong>Check this form.</strong> {orphan}</div>
{/if}

<style>
  .status {
    display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-2);
    padding: var(--space-3) var(--space-4); border-radius: var(--radius-md);
    background: var(--state-error-bg); color: var(--ink); border-left: 3px solid var(--state-error);
    font-size: var(--text-sm);
  }
</style>
