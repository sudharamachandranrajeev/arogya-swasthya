<script lang="ts">
  // Charts of every vital sign, numeric test result and symptom recorded at
  // least twice, for one time span chosen above them all.
  import { Card, Segmented } from '@swasthya/ui'
  import { api } from './api.ts'
  import Chart from './Chart.svelte'
  import { series, within, type Series, type Span } from './model/trends.ts'
  import { session } from './session.svelte.ts'

  let all = $state<Series[]>([])
  let span = $state<Span>('1y')

  $effect(() => {
    void session.version
    const person = session.current
    if (!person) return
    api.records(person, 'Observation').then((r) => (all = series(r)), session.fail.bind(session))
  })

  const shown = $derived(all.map((s) => ({ s, points: within(s, span) })).filter((x) => x.points.length >= 2))
  const groups = [
    { id: 'vital', title: 'Vital signs' },
    { id: 'result', title: 'Test results' },
    { id: 'symptom', title: 'Symptoms' },
  ] as const
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

<style>
  .filters { margin-bottom: 18px; }
  h2 { font-size: var(--text-lead); margin: 22px 0 10px; }
  .grid { display: grid; gap: 14px; grid-template-columns: repeat(auto-fill, minmax(min(420px, 100%), 1fr)); }
</style>
