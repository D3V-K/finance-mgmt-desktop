<script lang="ts">
  import { STATE_LABEL, type ValueState } from '../money';

  interface Props { state: ValueState | 'warning' | 'neutral'; label?: string }
  let { state, label }: Props = $props();

  const GLYPH = { actual: '●', forecast: '◌', incomplete: '◐', warning: '!', neutral: '·' } as const;
  const text = $derived(label ?? (state === 'warning' || state === 'neutral' ? '' : STATE_LABEL[state]));
</script>

<span class="tag {state}"><span aria-hidden="true" class="glyph">{GLYPH[state]}</span>{text}</span>

<style>
  .tag {
    display: inline-flex; align-items: center; gap: 5px;
    padding: 1px 8px; border-radius: 999px;
    font-size: var(--text-xs); font-weight: 600; line-height: 18px; white-space: nowrap;
    border: 1px solid transparent;
  }
  .glyph { font-size: 0.7em; }
  .actual { background: var(--state-actual-bg); color: var(--ink-2); }
  .forecast { background: var(--state-forecast-bg); color: var(--state-forecast); border-style: dashed; border-color: currentColor; }
  .incomplete, .warning { background: var(--state-incomplete-bg); color: var(--state-incomplete); }
  .neutral { background: var(--surface-sunken); color: var(--ink-2); }
</style>
