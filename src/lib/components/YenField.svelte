<!--
  Whole-yen input. `mode` decides what is valid: positive event amounts,
  nonnegative values (budgets, card debt) or signed balances. The parent owns
  parsing (via `parseFor`) so submit logic and the inline message agree.
-->
<script lang="ts">
  import { formatYen } from '../money';
  import { parseFor, type YenMode } from '../forms';

  interface Props {
    label: string;
    value: string;
    mode?: YenMode;
    error?: string;
    hint?: string;
    autofocus?: boolean;
    large?: boolean;
  }
  let { label, value = $bindable(), mode = 'positive', error, hint, autofocus = false, large = false }: Props = $props();
  const id = `yen-${Math.random().toString(36).slice(2, 8)}`;
  const parsed = $derived(parseFor(mode, value));
  const defaultHint = $derived(mode === 'signed' ? 'Whole yen; use − for a negative amount' : 'Whole yen, e.g. 1,200');
</script>

<div class="field">
  <label for={id}>{label}</label>
  <span class="yen-input" class:large>
    <span aria-hidden="true">¥</span>
    <input
      {id}
      bind:value
      inputmode={mode === 'signed' ? 'text' : 'numeric'}
      autocomplete="off"
      placeholder="0"
      data-autofocus={autofocus ? '' : undefined}
      aria-invalid={error ? 'true' : undefined}
      aria-describedby="{id}-msg"
    />
  </span>
  {#if error}
    <span id="{id}-msg" class="error">{error}</span>
  {:else}
    <span id="{id}-msg" class="hint num">{parsed.ok && value.trim() ? formatYen(parsed.value) : (hint ?? defaultHint)}</span>
  {/if}
</div>

<style>
  .yen-input { display: flex; align-items: center; border: 1px solid var(--border-strong); border-radius: var(--radius-md); background: var(--surface); padding-left: var(--space-3); }
  .yen-input:has(input[aria-invalid='true']) { border-color: var(--state-error); box-shadow: 0 0 0 1px var(--state-error); }
  .yen-input:focus-within { outline: 2px solid var(--focus); outline-offset: 2px; }
  .field .yen-input input { border: 0; flex: 1; min-width: 0; font-variant-numeric: tabular-nums; outline: none; box-shadow: none; background: transparent; }
  .large input { font-size: var(--text-lg); }
</style>
