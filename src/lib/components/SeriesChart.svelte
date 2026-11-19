<!--
  Accessible SVG chart with an equivalent table. One tab stop; ←/→/Home/End
  move between points and each point has a full spoken description. Unknown
  values (`null`) are never plotted and read as "Unknown". Forecast points are
  hatched and incomplete points get a dotted outline, never color alone.
-->
<script lang="ts" module>
  import type { ValueState } from '../money';

  export interface ChartPoint {
    key: string;
    label: string;
    /** Longer label for the table and spoken text. */
    fullLabel?: string;
    values: (number | null)[];
    state: ValueState;
    note?: string;
  }
  export interface ChartSeries {
    name: string;
    color: string;
  }
</script>

<script lang="ts">
  import { spokenYen, STATE_LABEL } from '../money';
  import Money from './Money.svelte';
  import StateTag from './StateTag.svelte';

  interface Props {
    caption: string;
    points: ChartPoint[];
    series: ChartSeries[];
    kind?: 'bars' | 'line';
    height?: number;
  }
  let { caption, points, series, kind = 'bars', height = 280 }: Props = $props();

  let view = $state<'chart' | 'table'>('chart');
  let active = $state(0);
  let chartEl: SVGSVGElement | undefined = $state();
  const uid = `c${Math.random().toString(36).slice(2, 8)}`;

  const W = 860;
  const PAD = { top: 16, right: 16, bottom: 40, left: 84 };
  const innerW = W - PAD.left - PAD.right;
  const innerH = $derived(height - PAD.top - PAD.bottom);

  function niceStep(span: number) {
    const raw = span / 4;
    const mag = 10 ** Math.floor(Math.log10(Math.max(raw, 1)));
    const n = raw / mag;
    return (n <= 1 ? 1 : n <= 2 ? 2 : n <= 5 ? 5 : 10) * mag;
  }

  const scale = $derived.by(() => {
    const known = points.flatMap((p) => p.values.filter((v): v is number => v !== null));
    const lo = Math.min(0, ...known);
    const hi = Math.max(0, ...known, 1);
    const step = niceStep(hi - lo);
    const bottom = Math.floor(lo / step) * step;
    const top = Math.ceil(hi / step) * step;
    const ticks: number[] = [];
    for (let t = bottom; t <= top; t += step) ticks.push(t);
    const y = (v: number) => innerH - ((v - bottom) / (top - bottom || 1)) * innerH;
    return { y, ticks };
  });

  const slot = $derived(innerW / Math.max(points.length, 1));
  const bw = $derived(Math.max(3, Math.min(24, (slot * 0.7) / series.length)));
  const firstForecast = $derived(points.findIndex((p) => p.state === 'forecast'));

  function tick(v: number) {
    if (v === 0) return '¥0';
    const abs = Math.abs(v);
    const body = abs >= 100_000_000 ? `${abs / 100_000_000}億` : abs >= 10_000 ? `${abs / 10_000}万` : `${abs}`;
    return `${v < 0 ? '−' : ''}¥${body}`;
  }

  function describe(p: ChartPoint) {
    const parts = series.map((s, i) => `${s.name} ${spokenYen(p.values[i] ?? null)}`);
    return `${p.fullLabel ?? p.label}, ${STATE_LABEL[p.state]}: ${parts.join(', ')}${p.note ? `. ${p.note}` : ''}`;
  }

  function onKeydown(e: KeyboardEvent) {
    const last = points.length - 1;
    const next =
      e.key === 'ArrowRight' ? active + 1 : e.key === 'ArrowLeft' ? active - 1 : e.key === 'Home' ? 0 : e.key === 'End' ? last : null;
    if (next === null) return;
    e.preventDefault();
    active = Math.max(0, Math.min(last, next));
    chartEl?.querySelectorAll<SVGGElement>('g.point')[active]?.focus();
  }

  function linePath(i: number) {
    let d = '';
    let pen = false;
    points.forEach((p, j) => {
      const v = p.values[i];
      if (v === null || v === undefined) {
        pen = false;
        return;
      }
      d += `${pen ? 'L' : 'M'}${j * slot + slot / 2},${scale.y(v)}`;
      pen = true;
    });
    return d;
  }

  const labelEvery = $derived(Math.max(1, Math.ceil(points.length / 14)));
  $effect(() => {
    if (active > points.length - 1) active = Math.max(0, points.length - 1);
  });
</script>

<div class="chart-block">
  <div class="bar">
    <ul class="legend" aria-label="Legend">
      {#each series as s (s.name)}
        <li><svg width="14" height="14" aria-hidden="true"><rect width="14" height="14" fill={s.color} rx="2" /></svg> {s.name}</li>
      {/each}
      {#if points.some((p) => p.state === 'forecast')}
        <li><svg width="14" height="14" aria-hidden="true"><rect width="14" height="14" fill="url(#{uid}-hatch)" stroke="var(--chart-forecast)" stroke-dasharray="3 2" rx="2" /></svg> Forecast</li>
      {/if}
      {#if points.some((p) => p.state === 'incomplete')}
        <li><svg width="14" height="14" aria-hidden="true"><rect x="1" y="1" width="12" height="12" fill="none" stroke="var(--state-incomplete)" stroke-width="2" stroke-dasharray="2 2" rx="2" /></svg> Incomplete</li>
      {/if}
    </ul>
    <div class="toggle" role="group" aria-label="Display {caption} as">
      <button type="button" class="btn small" aria-pressed={view === 'chart'} onclick={() => (view = 'chart')}>Chart</button>
      <button type="button" class="btn small" aria-pressed={view === 'table'} onclick={() => (view = 'table')}>Table</button>
    </div>
  </div>

  {#if view === 'chart'}
    <svg bind:this={chartEl} viewBox="0 0 {W} {height}" role="group" aria-label="{caption}. Use left and right arrow keys to move between points." class="chart">
      <defs>
        <pattern id="{uid}-hatch" width="5" height="5" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
          <rect width="5" height="5" fill="var(--state-forecast-bg)" /><line x1="0" y1="0" x2="0" y2="5" stroke="var(--chart-forecast)" stroke-width="1.5" />
        </pattern>
      </defs>
      <g transform="translate({PAD.left},{PAD.top})">
        {#each scale.ticks as t (t)}
          <line x1="0" x2={innerW} y1={scale.y(t)} y2={scale.y(t)} stroke={t === 0 ? 'var(--border-strong)' : 'var(--chart-grid)'} />
          <text x="-10" y={scale.y(t)} dy="0.32em" text-anchor="end" class="tick" aria-hidden="true">{tick(t)}</text>
        {/each}
        {#if firstForecast >= 0}
          <rect x={firstForecast * slot} y="0" width={innerW - firstForecast * slot} height={innerH} fill="var(--state-forecast-bg)" opacity="0.5" />
          <text x={firstForecast * slot + 6} y="12" class="zone" aria-hidden="true">Forecast →</text>
        {/if}
        {#if kind === 'line'}
          {#each series as s, i (s.name)}
            <path d={linePath(i)} fill="none" stroke={s.color} stroke-width="2" stroke-dasharray={firstForecast >= 0 ? '5 3' : undefined} />
          {/each}
        {/if}
        {#each points as p, j (p.key)}
          {@const cx = j * slot + slot / 2}
          {@const f = p.state === 'forecast'}
          <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
          <g class="point" class:active={j === active} role="img" tabindex={j === active ? 0 : -1} aria-label={describe(p)}
            onkeydown={onKeydown} onfocus={() => (active = j)} onpointerenter={() => (active = j)}>
            <rect class="hit" x={j * slot} y="0" width={slot} height={innerH} fill="transparent" />
            {#each series as s, i (s.name)}
              {@const v = p.values[i]}
              {#if v !== null && v !== undefined}
                {#if kind === 'bars'}
                  {@const x = cx - (bw * series.length) / 2 + i * bw}
                  {@const y0 = scale.y(0)}
                  {@const y1 = scale.y(v)}
                  <rect x={x + 1} y={Math.min(y0, y1)} width={bw - 2} height={Math.max(1, Math.abs(y1 - y0))} rx="2"
                    fill={f ? `url(#${uid}-hatch)` : s.color} stroke={f ? s.color : 'none'} stroke-dasharray={f ? '4 2' : undefined} />
                {:else}
                  <circle cx={cx} cy={scale.y(v)} r={j === active ? 4.5 : 2.5} fill={s.color} />
                {/if}
              {/if}
            {/each}
            {#if p.state === 'incomplete'}
              <rect x={j * slot + 2} y="1" width={slot - 4} height={innerH - 2} fill="none" stroke="var(--state-incomplete)" stroke-width="2" stroke-dasharray="3 3" rx="4" />
            {/if}
            {#if j % labelEvery === 0}
              <text x={cx} y={innerH + 18} text-anchor="middle" class="xlab" aria-hidden="true">{p.label}</text>
            {/if}
          </g>
        {/each}
      </g>
    </svg>
    {#if points[active]}
      {@const p = points[active]!}
      <div class="detail" aria-live="polite">
        <strong>{p.fullLabel ?? p.label}</strong>
        <StateTag state={p.state} />
        {#each series as s, i (s.name)}
          <span><span class="muted">{s.name}</span> <Money value={p.values[i] ?? null} state={p.state} /></span>
        {/each}
        {#if p.note}<span class="muted">{p.note}</span>{/if}
      </div>
    {/if}
  {:else}
    <div class="table-wrap">
      <table class="data">
        <caption class="sr-only">{caption}</caption>
        <thead>
          <tr>
            <th scope="col">Period</th><th scope="col">Status</th>
            {#each series as s (s.name)}<th scope="col" class="right">{s.name}</th>{/each}
            <th scope="col">Note</th>
          </tr>
        </thead>
        <tbody>
          {#each points as p (p.key)}
            <tr>
              <th scope="row" class="rowhead">{p.fullLabel ?? p.label}</th>
              <td><StateTag state={p.state} /></td>
              {#each series as s, i (s.name)}
                <td class="right"><Money value={p.values[i] ?? null} state={p.state} /></td>
              {/each}
              <td class="muted">{p.note ?? ''}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<style>
  .chart-block { display: grid; }
  .bar { display: flex; justify-content: space-between; align-items: center; gap: var(--space-4); padding: var(--space-3) var(--space-5); border-bottom: 1px solid var(--border); }
  .legend { list-style: none; display: flex; flex-wrap: wrap; gap: var(--space-4); margin: 0; padding: 0; font-size: var(--text-sm); color: var(--ink-2); }
  .legend li { display: flex; align-items: center; gap: 6px; }
  .toggle { display: flex; gap: 2px; }
  .toggle .btn[aria-pressed='true'] { background: var(--ink); color: var(--bg); border-color: var(--ink); }
  .chart { width: 100%; height: auto; display: block; padding: var(--space-3) var(--space-3) 0; }
  .tick, .xlab { font-size: 12px; fill: var(--ink-3); font-variant-numeric: tabular-nums; }
  .zone { font-size: 11px; fill: var(--state-forecast); font-weight: 600; }
  .point { outline: none; }
  .point.active .hit { fill: var(--surface-sunken); }
  .point:focus-visible .hit { stroke: var(--focus); stroke-width: 2; }
  .detail { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-4); padding: var(--space-3) var(--space-5); border-top: 1px solid var(--border); min-height: 52px; font-size: var(--text-sm); }
  .table-wrap { max-height: 420px; overflow-y: auto; }
  .rowhead { text-align: left; font-weight: 560; text-transform: none; letter-spacing: 0; font-size: var(--text-md); color: var(--ink); position: static; }
</style>
