<script lang="ts">
  import { resource } from '../lib/app.svelte';
  import { commands } from '../lib/commands';
  import type { CashFlowMonth } from '../lib/fixtures';
  import { formatMonth, spokenYen, STATE_LABEL } from '../lib/money';
  import AsyncView from '../lib/components/AsyncView.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';
  import Money from '../lib/components/Money.svelte';
  import StateTag from '../lib/components/StateTag.svelte';

  const report = resource(() => commands.cashFlowReport());

  let view = $state<'chart' | 'table'>('chart');
  let active = $state(0);
  let chartEl: SVGSVGElement | undefined = $state();

  const W = 860;
  const H = 300;
  const PAD = { top: 16, right: 16, bottom: 44, left: 72 };
  const innerW = W - PAD.left - PAD.right;
  const innerH = H - PAD.top - PAD.bottom;

  function scale(months: CashFlowMonth[]) {
    const max = Math.max(...months.flatMap((m) => [m.moneyIn, m.spending]));
    const step = max > 400_000 ? 200_000 : 100_000;
    const top = Math.ceil(max / step) * step;
    const ticks = Array.from({ length: top / step + 1 }, (_, i) => i * step);
    return { y: (v: number) => innerH - (v / top) * innerH, ticks };
  }

  function describe(m: CashFlowMonth) {
    return `${formatMonth(m.month)}, ${STATE_LABEL[m.state]}: money in ${spokenYen(m.moneyIn)}, spending ${spokenYen(m.spending)}${m.note ? `. ${m.note}` : ''}`;
  }

  function onChartKeydown(e: KeyboardEvent, count: number) {
    const next = e.key === 'ArrowRight' ? active + 1 : e.key === 'ArrowLeft' ? active - 1 : e.key === 'Home' ? 0 : e.key === 'End' ? count - 1 : null;
    if (next === null) return;
    e.preventDefault();
    active = Math.max(0, Math.min(count - 1, next));
    chartEl?.querySelectorAll<SVGGElement>('g.month')[active]?.focus();
  }

  function averages(months: CashFlowMonth[]) {
    const actual = months.filter((m) => m.state === 'actual');
    const avg = actual.length ? Math.round(actual.reduce((s, m) => s + m.spending, 0) / actual.length) : null;
    return { avg, actualCount: actual.length, incomplete: months.filter((m) => m.state === 'incomplete').length };
  }
</script>

<header class="page-head">
  <div>
    <h1 tabindex="-1">Cash flow</h1>
    <p class="muted">Money in and spending by month. Transfers and card repayments are excluded.</p>
  </div>
  <div class="toggle" role="group" aria-label="Display as">
    <button class="btn small" aria-pressed={view === 'chart'} onclick={() => (view = 'chart')}>Chart</button>
    <button class="btn small" aria-pressed={view === 'table'} onclick={() => (view = 'table')}>Table</button>
  </div>
</header>

<section class="panel">
  <AsyncView state={report.current} isEmpty={(r) => r.length === 0} onretry={report.reload} label="cash flow" skeletonRows={6}>
    {#snippet empty()}
      <EmptyState title="Not enough history to chart" body="Cash flow appears after your first month of recorded income or spending. Forecast months are added once a payday is set." />
    {/snippet}
    {#snippet children(months)}
      {@const s = scale(months)}
      {@const a = averages(months)}
      {@const slot = innerW / months.length}
      {@const bw = Math.min(28, slot / 3)}
      {@const firstForecast = months.findIndex((m) => m.state === 'forecast')}

      <div class="summary">
        <div>
          <p class="eyebrow">Average monthly spending</p>
          <Money value={a.avg} size="lg" />
          <p class="muted small">From {a.actualCount} complete actual months{a.incomplete ? ` · ${a.incomplete} incomplete month excluded` : ''}</p>
        </div>
        <ul class="legend" aria-label="Legend">
          <li><svg width="14" height="14" aria-hidden="true"><rect width="14" height="14" fill="var(--chart-in)" rx="2" /></svg> Money in</li>
          <li><svg width="14" height="14" aria-hidden="true"><rect width="14" height="14" fill="var(--chart-out)" rx="2" /></svg> Spending</li>
          <li><svg width="14" height="14" aria-hidden="true"><rect width="14" height="14" fill="url(#hatch-legend)" stroke="var(--chart-forecast)" stroke-dasharray="3 2" rx="2" /></svg> Forecast</li>
          <li><svg width="14" height="14" aria-hidden="true"><rect x="1" y="1" width="12" height="12" fill="none" stroke="var(--state-incomplete)" stroke-width="2" stroke-dasharray="2 2" rx="2" /></svg> Incomplete</li>
        </ul>
      </div>

      {#if view === 'chart'}
        <div class="chart-wrap">
          <svg
            bind:this={chartEl}
            viewBox="0 0 {W} {H}"
            role="group"
            aria-label="Monthly cash flow chart. Use left and right arrow keys to move between months."
            class="chart"
          >
            <defs>
              <pattern id="hatch-legend" width="4" height="4" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
                <rect width="4" height="4" fill="var(--state-forecast-bg)" /><line x1="0" y1="0" x2="0" y2="4" stroke="var(--chart-forecast)" stroke-width="1.5" />
              </pattern>
              <pattern id="hatch-in" width="5" height="5" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
                <rect width="5" height="5" fill="var(--state-forecast-bg)" /><line x1="0" y1="0" x2="0" y2="5" stroke="var(--chart-in)" stroke-width="2" />
              </pattern>
              <pattern id="hatch-out" width="5" height="5" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
                <rect width="5" height="5" fill="var(--state-forecast-bg)" /><line x1="0" y1="0" x2="0" y2="5" stroke="var(--chart-out)" stroke-width="2" />
              </pattern>
            </defs>
            <g transform="translate({PAD.left},{PAD.top})">
              {#each s.ticks as t (t)}
                <line x1="0" x2={innerW} y1={s.y(t)} y2={s.y(t)} stroke="var(--chart-grid)" />
                <text x="-10" y={s.y(t)} dy="0.32em" text-anchor="end" class="tick" aria-hidden="true">{t === 0 ? '¥0' : `¥${t / 10_000}万`}</text>
              {/each}
              {#if firstForecast > 0}
                <rect x={firstForecast * slot} y="0" width={innerW - firstForecast * slot} height={innerH} fill="var(--state-forecast-bg)" opacity="0.5" />
                <text x={firstForecast * slot + 6} y="12" class="zone" aria-hidden="true">Forecast →</text>
              {/if}
              {#each months as m, i (m.month)}
                {@const cx = i * slot + slot / 2}
                {@const f = m.state === 'forecast'}
                <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
                <g
                  class="month"
                  class:active={i === active}
                  role="img"
                  tabindex={i === active ? 0 : -1}
                  aria-label={describe(m)}
                  onkeydown={(e) => onChartKeydown(e, months.length)}
                  onfocus={() => (active = i)}
                  onpointerenter={() => (active = i)}
                >
                  <rect class="hit" x={i * slot} y="0" width={slot} height={innerH} fill="transparent" />
                  <rect x={cx - bw - 2} y={s.y(m.moneyIn)} width={bw} height={innerH - s.y(m.moneyIn)} rx="2"
                    fill={f ? 'url(#hatch-in)' : 'var(--chart-in)'} stroke={f ? 'var(--chart-in)' : 'none'} stroke-dasharray={f ? '4 2' : undefined} />
                  <rect x={cx + 2} y={s.y(m.spending)} width={bw} height={innerH - s.y(m.spending)} rx="2"
                    fill={f ? 'url(#hatch-out)' : 'var(--chart-out)'} stroke={f ? 'var(--chart-out)' : 'none'} stroke-dasharray={f ? '4 2' : undefined} />
                  {#if m.state === 'incomplete'}
                    <rect x={cx - bw - 6} y={Math.min(s.y(m.moneyIn), s.y(m.spending)) - 4} width={bw * 2 + 12} height={innerH - Math.min(s.y(m.moneyIn), s.y(m.spending)) + 4}
                      fill="none" stroke="var(--state-incomplete)" stroke-width="2" stroke-dasharray="3 3" rx="4" />
                  {/if}
                  <text x={cx} y={innerH + 18} text-anchor="middle" class="xlab" aria-hidden="true">{formatMonth(m.month).replace(/ \d{4}$/, '')}</text>
                  <text x={cx} y={innerH + 34} text-anchor="middle" class="xstate" aria-hidden="true">{m.state === 'actual' ? '' : STATE_LABEL[m.state]}</text>
                </g>
              {/each}
            </g>
          </svg>
        </div>
        {#if months[active]}
          {@const m = months[active]!}
          <div class="detail" aria-live="polite">
            <strong>{formatMonth(m.month)}</strong>
            <StateTag state={m.state} />
            <span><span class="muted">In</span> <Money value={m.moneyIn} state={m.state} signed /></span>
            <span><span class="muted">Spending</span> <Money value={-m.spending} state={m.state} /></span>
            <span><span class="muted">Difference</span> <Money value={m.moneyIn - m.spending} state={m.state} signed tone="direction" /></span>
            {#if m.note}<span class="muted">{m.note}</span>{/if}
          </div>
        {/if}
      {:else}
        <table class="data">
          <caption class="sr-only">Monthly cash flow</caption>
          <thead>
            <tr><th scope="col">Month</th><th scope="col">Status</th><th scope="col" class="right">Money in</th><th scope="col" class="right">Spending</th><th scope="col" class="right">Difference</th><th scope="col">Note</th></tr>
          </thead>
          <tbody>
            {#each months as m (m.month)}
              <tr class:forecast-row={m.state === 'forecast'}>
                <th scope="row" class="rowhead">{formatMonth(m.month)}</th>
                <td><StateTag state={m.state} /></td>
                <td class="right"><Money value={m.moneyIn} state={m.state} /></td>
                <td class="right"><Money value={-m.spending} state={m.state} /></td>
                <td class="right"><Money value={m.moneyIn - m.spending} state={m.state} signed tone="direction" /></td>
                <td class="muted">{m.note ?? ''}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
      <p class="muted small foot">
        Forecast months are estimates from your payday and recent spending; they don’t change balances. A “difference” is not savings —
        money moved to savings is a transfer.
      </p>
    {/snippet}
  </AsyncView>
</section>

<style>
  .page-head { display: flex; justify-content: space-between; align-items: end; margin-bottom: var(--space-5); }
  h1:focus { outline: none; }
  .toggle { display: flex; gap: 2px; }
  .toggle .btn[aria-pressed='true'] { background: var(--ink); color: var(--bg); border-color: var(--ink); }
  .summary { display: flex; justify-content: space-between; align-items: end; padding: var(--space-4) var(--space-5); border-bottom: 1px solid var(--border); gap: var(--space-4); }
  .legend { list-style: none; display: flex; gap: var(--space-4); margin: 0; padding: 0; font-size: var(--text-sm); color: var(--ink-2); }
  .legend li { display: flex; align-items: center; gap: 6px; }
  .small { font-size: var(--text-xs); }
  .chart-wrap { padding: var(--space-4) var(--space-3) 0; }
  .chart { width: 100%; height: auto; display: block; }
  .tick, .xlab { font-size: 12px; fill: var(--ink-3); font-variant-numeric: tabular-nums; }
  .xstate { font-size: 11px; fill: var(--state-forecast); font-weight: 600; }
  .zone { font-size: 11px; fill: var(--state-forecast); font-weight: 600; }
  .month { outline: none; cursor: default; }
  .month.active .hit { fill: var(--surface-sunken); }
  .month:focus-visible .hit { stroke: var(--focus); stroke-width: 2; }
  .detail { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-4); padding: var(--space-3) var(--space-5); border-top: 1px solid var(--border); min-height: 52px; font-size: var(--text-sm); }
  .rowhead { text-align: left; font-weight: 560; text-transform: none; letter-spacing: 0; font-size: var(--text-md); color: var(--ink); position: static; }
  .foot { padding: var(--space-3) var(--space-5) var(--space-4); border-top: 1px solid var(--border); }
</style>
