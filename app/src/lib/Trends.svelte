<script lang="ts">
  // Charts of every vital sign, numeric test result and symptom recorded at
  // least twice, for one time span chosen above them all.
  import { Card, Pill, Segmented } from '@swasthya/ui'
  import { api } from './api.ts'
  import Chart from './Chart.svelte'
  import type { Resource } from './model/fhir.ts'
  import { insights, MIN_DAYS } from './model/insights.ts'
  import { series, within, type Series, type Span } from './model/trends.ts'
  import { session } from './session.svelte.ts'

  let all = $state<Series[]>([])
  let meds = $state<Resource[]>([])
  let span = $state<Span>('1y')

  $effect(() => {
    void session.version
    const person = session.current
    if (!person) return
    Promise.all([api.records(person, 'Observation'), api.records(person, 'MedicationStatement')]).then(([r, m]) => {
      all = series(r)
      meds = m
    }, session.fail.bind(session))
  })

  const shown = $derived(all.map((s) => ({ s, points: within(s, span) })).filter((x) => x.points.length >= 2))
  const groups = [
    { id: 'vital', title: 'Vital signs' },
    { id: 'result', title: 'Test results' },
    { id: 'symptom', title: 'Symptoms' },
  ] as const
  // Patterns use every reading, whatever the span above.
  const found = $derived(insights(all, meds))
  const unitOf = (s: Series) => (s.group === 'symptom' ? '0 to 10' : s.unit)
</script>

<div class="filters">
  <Segmented
    label="Time span"
    options={[
      { value: '3m', label: 'Last 3 months' },
      { value: '1y', label: 'Last year' },
      { value: 'all', label: 'All' },
    ]}
    value={span}
    onchange={(v) => (span = v as Span)}
  />
</div>

{#if !shown.length}
  <Card>
    <p class="muted">
      {all.some((s) => s.points.length >= 2)
        ? 'Nothing was recorded more than once in this time span. Try “All”.'
        : `A chart appears once something is recorded at least twice: a vital sign or symptom on Track, or a test result with a number.`}
    </p>
  </Card>
{/if}

{#each groups as g (g.id)}
  {@const items = shown.filter((x) => x.s.group === g.id)}
  {#if items.length}
    <h2>{g.title}</h2>
    <div class="grid">
      {#each items as { s, points } (s.id)}
        <Card title={unitOf(s) ? `${s.title} (${unitOf(s)})` : s.title}>
          <Chart {s} {points} />
        </Card>
      {/each}
    </div>
  {/if}
{/each}

{#if found.length}
  <h2>Patterns</h2>
  <p class="muted small">Only for things that plausibly go together, from at least {MIN_DAYS} days of readings. A pattern is something to ask a clinician about: it does not show what caused what.</p>
  <div class="grid">
    {#each found as f (f.id)}
      <Card title={f.title}>
        <p>{f.text}</p>
        <p class="meta">
          {#if f.size}<Pill tone="accent">{f.size}</Pill>{/if}
          <span class="muted small">{f.enough ? `Based on ${f.n} days` : 'Keep recording'}</span>
        </p>
      </Card>
    {/each}
  </div>
{/if}

<style>
  .filters { margin-bottom: 18px; }
  .meta { display: flex; gap: 8px; align-items: center; margin: 8px 0 0; }
  .small { font-size: var(--text-small); }
  .muted { color: var(--c-muted); }
  h2 { font-size: var(--text-lead); margin: 22px 0 10px; }
  .grid { display: grid; gap: 14px; grid-template-columns: repeat(auto-fill, minmax(min(420px, 100%), 1fr)); }
</style>
