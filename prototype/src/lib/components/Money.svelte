<!--
  The only way the UI renders an amount. Unknown values render as words,
  negative values carry a real minus sign plus spoken text, and state
  (actual / forecast / incomplete) is labelled, not just colored.
-->
<script lang="ts">
  import { formatYen, spokenYen, type MaybeYen, type ValueState } from '../money';

  interface Props {
    value: MaybeYen;
    state?: ValueState;
    signed?: boolean;
    size?: 'md' | 'lg' | 'figure';
    /** `direction` colors money in green; `debt` renders a positive owed amount in the debt color. */
    tone?: 'plain' | 'direction' | 'debt';
    unknownLabel?: string;
  }

  let { value, state = 'actual', signed = false, size = 'md', tone = 'plain', unknownLabel = 'Unknown' }: Props = $props();

  const text = $derived(value === null ? unknownLabel : formatYen(value, { signed }));
  const spoken = $derived(
    value === null ? unknownLabel : `${spokenYen(value, { signed })}${state === 'actual' ? '' : `, ${state}`}`
  );
</script>

<span
  class="money num {size}"
  class:unknown={value === null}
  class:in={tone === 'direction' && value !== null && value > 0}
  class:debt={tone === 'debt'}
  class:forecast={state === 'forecast'}
  class:incomplete={state === 'incomplete'}
  data-state={state}
><span aria-hidden="true">{text}</span><span class="sr-only">{spoken}</span></span>

<style>
  .money { white-space: nowrap; font-weight: 560; }
  .lg { font-size: var(--text-lg); font-weight: 620; }
  .figure { font-size: var(--text-figure); font-weight: 650; letter-spacing: -0.02em; line-height: 1.1; }
  .in { color: var(--money-in); }
  .debt { color: var(--money-debt); }
  .forecast { color: var(--state-forecast); }
  .incomplete { text-decoration: underline dotted var(--state-incomplete) 2px; text-underline-offset: 0.2em; }
  .unknown { color: var(--state-incomplete); font-weight: 600; font-style: normal; }
  .unknown.figure { font-size: var(--text-xl); }
</style>
