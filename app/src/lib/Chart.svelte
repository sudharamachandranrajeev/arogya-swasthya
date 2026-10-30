<script lang="ts">
  // One series over time: a 2px line with dots (blood pressure: a bar from
  // the bottom to the top number per reading), the lab's normal range shaded
  // behind it, a crosshair with a readout on hover or arrow keys, and the
  // same numbers as a table.
  import { ticks, type Point, type Series } from './model/trends.ts'

  let { s, points }: { s: Series; points: Point[] } = $props()

  const H = 190
  const top = 12
  const bottom = 26
  const left = 44
  const right = 64
  let width = $state(520)
  let sel = $state<number | null>(null)

  const values = $derived(points.flatMap((p) => (p.lo != null ? [p.v, p.lo] : [p.v])))
  const yTicks = $derived(
    s.domain
      ? ticks(s.domain[0], s.domain[1], 5)
      : ticks(
          Math.min(...values, s.range?.low ?? Infinity),
          Math.max(...values, s.range?.high ?? -Infinity),
        ),
  )
  const y0 = $derived(yTicks[0])
  const y1 = $derived(yTicks[yTicks.length - 1])
  const t0 = $derived(points.length > 1 ? points[0].t : points[0].t - 86_400_000)
  const t1 = $derived(points.length > 1 ? points[points.length - 1].t : points[0].t + 86_400_000)
  const x = (t: number) => left + ((t - t0) / (t1 - t0)) * (width - left - right)
  const y = (v: number) => top + (1 - (v - y0) / (y1 - y0)) * (H - top - bottom)

  const days = $derived((t1 - t0) / 86_400_000)
  const fmt = $derived(
    new Intl.DateTimeFormat(undefined, days > 730 ? { year: 'numeric' } : days > 120 ? { month: 'short', year: 'numeric' } : { day: 'numeric', month: 'short' }),
  )
  // First, middle and last date, without repeating a label.
  const xTicks = $derived(
    (points.length > 2 ? [points[0], points[Math.floor(points.length / 2)], points[points.length - 1]] : points).filter(
      (p, i, all) => i === 0 || fmt.format(new Date(p.t)) !== fmt.format(new Date(all[i - 1].t)),
    ),
  )
  const day = new Intl.DateTimeFormat(undefined, { day: 'numeric', month: 'short', year: 'numeric' })
  const when = (p: Point) => (p.date.length > 7 ? day.format(new Date(p.t)) : p.date)
  const value = (p: Point) => (p.lo != null ? `${p.v}/${p.lo}` : `${p.v}`)
  const bandWords = (p: Point) => (p.band === 'low' ? 'Below range' : p.band === 'high' ? 'Above range' : p.band === 'in' ? 'In range' : '')

  const line = $derived(s.paired ? '' : points.map((p, i) => `${i ? 'L' : 'M'}${x(p.t).toFixed(1)},${y(p.v).toFixed(1)}`).join(''))
  const last = $derived(points[points.length - 1])
  const unitShown = $derived(s.group === 'symptom' ? '' : s.unit)

  function nearest(px: number) {
    let best = 0
    for (let i = 1; i < points.length; i++) if (Math.abs(x(points[i].t) - px) < Math.abs(x(points[best].t) - px)) best = i
    return best
  }
  function move(e: PointerEvent) {
    const box = (e.currentTarget as SVGElement).getBoundingClientRect()
    sel = nearest(e.clientX - box.left)
  }
  function key(e: KeyboardEvent) {
    if (e.key === 'ArrowLeft') sel = Math.max(0, (sel ?? points.length) - 1)
    else if (e.key === 'ArrowRight') sel = Math.min(points.length - 1, (sel ?? -1) + 1)
    else if (e.key === 'Escape') sel = null
    else return
    e.preventDefault()
  }
  const summary = $derived(
    `${s.title}: ${points.length} readings from ${when(points[0])} to ${when(last)}, latest ${value(last)} ${unitShown}. Arrow keys step through them.`,
  )
</script>

<div class="chart" bind:clientWidth={width}>
  <svg
    {width}
    height={H}
    role="slider"
    aria-roledescription="chart"
    aria-label={summary}
    aria-valuemin={0}
    aria-valuemax={points.length - 1}
    aria-valuenow={sel ?? points.length - 1}
    aria-valuetext={`${when(points[sel ?? points.length - 1])}: ${value(points[sel ?? points.length - 1])} ${unitShown}`}
    tabindex="0"
    onpointermove={move}
    onpointerleave={() => (sel = null)}
    onkeydown={key}
    onblur={() => (sel = null)}
  >
    {#if s.range && (s.range.low != null || s.range.high != null)}
      {@const hi = y(Math.min(s.range.high ?? y1, y1))}
      {@const lo = y(Math.max(s.range.low ?? y0, y0))}
      <rect class="band" x={left} y={hi} width={width - left - right} height={Math.max(0, lo - hi)} />
    {/if}
    {#each yTicks as t (t)}
      <line class="grid" x1={left} x2={width - right} y1={y(t)} y2={y(t)} />
      <text class="tick" x={left - 8} y={y(t)} dy="0.32em" text-anchor="end">{t}</text>
    {/each}
    {#each xTicks as p, i (p.t + ':' + i)}
      <text class="tick" x={x(p.t)} y={H - 6} text-anchor={i === 0 && xTicks.length > 1 ? 'start' : i === xTicks.length - 1 && xTicks.length > 1 ? 'end' : 'middle'}>{fmt.format(new Date(p.t))}</text>
    {/each}
    {#if sel !== null}
      <line class="cross" x1={x(points[sel].t)} x2={x(points[sel].t)} y1={top} y2={H - bottom} />
    {/if}
    {#if s.paired}
      {#each points as p, i (i)}
        <line class="bar" x1={x(p.t)} x2={x(p.t)} y1={y(p.v)} y2={y(p.lo!)} />
        <circle class="dot" cx={x(p.t)} cy={y(p.v)} r="4" />
        <circle class="dot" cx={x(p.t)} cy={y(p.lo!)} r="4" />
      {/each}
    {:else}
      <path class="line" d={line} />
      {#each points as p, i (i)}
        {#if points.length <= 60 || i === points.length - 1 || i === sel}
          <circle class="dot" class:off={p.band === 'low' || p.band === 'high'} cx={x(p.t)} cy={y(p.v)} r="4" />
        {/if}
      {/each}
    {/if}
    <text class="end" x={x(last.t) + 10} y={y(last.v)} dy="0.32em">{value(last)}</text>
  </svg>
  {#if sel !== null}
    {@const p = points[sel]}
    <div class="tip" style:left={`${Math.min(Math.max(x(p.t), 70), width - 70)}px`} aria-live="polite">
      <strong>{value(p)} {unitShown}</strong>
      <span>{when(p)}{bandWords(p) ? ` · ${bandWords(p)}` : ''}</span>
    </div>
  {/if}
</div>
<p class="foot">
  {#if s.range && (s.range.low != null || s.range.high != null)}Shaded: the lab's normal range ({s.range.low ?? '…'} to {s.range.high ?? '…'}{s.unit ? ` ${s.unit}` : ''}).{/if}
  {#if s.otherUnits}{s.otherUnits} reading{s.otherUnits === 1 ? '' : 's'} in another unit not shown.{/if}
</p>
<details>
  <summary>Show as a table</summary>
  <table>
    <thead><tr><th scope="col">Date</th><th scope="col">{s.title}{unitShown ? ` (${unitShown})` : ''}</th>{#if s.group === 'result'}<th scope="col">Range</th>{/if}</tr></thead>
    <tbody>
      {#each [...points].reverse() as p, i (i)}
        <tr><td>{when(p)}</td><td class="num">{value(p)}</td>{#if s.group === 'result'}<td>{bandWords(p)}</td>{/if}</tr>
      {/each}
    </tbody>
  </table>
</details>

<style>
  .chart { position: relative; width: 100%; }
  svg { display: block; overflow: visible; touch-action: pan-y; }
  svg:focus-visible { outline: 2px solid var(--c-focus); outline-offset: 4px; border-radius: 4px; }
  .grid { stroke: var(--c-line); stroke-width: 1; }
  .cross { stroke: var(--c-muted); stroke-width: 1; }
  .band { fill: var(--c-ok-soft); }
  .tick { fill: var(--c-muted); font-size: 11px; font-variant-numeric: tabular-nums; }
  .end { fill: var(--c-ink); font-size: 12px; font-weight: 600; }
  .line { fill: none; stroke: var(--c-chart, var(--c-accent)); stroke-width: 2; stroke-linejoin: round; stroke-linecap: round; }
  .bar { stroke: var(--c-chart, var(--c-accent)); stroke-width: 2; stroke-linecap: round; }
  .dot { fill: var(--c-chart, var(--c-accent)); stroke: var(--c-panel); stroke-width: 2; }
  .dot.off { fill: var(--c-warn); }
  .tip {
    position: absolute; top: -6px; transform: translate(-50%, -100%); pointer-events: none;
    background: var(--c-panel); border: 1px solid var(--c-line); border-radius: 8px; padding: 6px 10px;
    display: grid; gap: 2px; font-size: var(--text-small); white-space: nowrap; box-shadow: 0 4px 14px rgb(0 0 0 / 0.08);
  }
  .tip strong { color: var(--c-ink); }
  .tip span { color: var(--c-muted); }
  .foot { color: var(--c-muted); font-size: var(--text-small); margin: 6px 0 0; min-height: 0; }
  .foot:empty { display: none; }
  details { margin-top: 6px; font-size: var(--text-small); }
  summary { cursor: pointer; color: var(--c-accent); }
  table { border-collapse: collapse; margin-top: 8px; }
  th, td { text-align: left; padding: 4px 16px 4px 0; border-bottom: 1px solid var(--c-line); }
  .num { font-variant-numeric: tabular-nums; }
</style>
