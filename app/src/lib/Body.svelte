<script lang="ts">
  // The Body page: conditions and procedures on a 3D body, with the same
  // records as a list beside it. The viewer (and three.js) loads only here.
  import { Card, Pill } from '@swasthya/ui'
  import { api } from './api.ts'
  import { byRegion, firstYear, labelOf, place, type Tone } from './model/body.ts'
  import { condition, kindOf, procedure, type Resource } from './model/fhir.ts'
  import { session } from './session.svelte.ts'

  let records = $state<Resource[]>([])
  let selected = $state<string | null>(null)
  let skin = $state(true)
  const thisYear = new Date().getFullYear()
  let year = $state(thisYear)

  $effect(() => {
    void session.version
    const person = session.current
    if (!person) return
    Promise.all([api.records(person, 'Condition'), api.records(person, 'Procedure')]).then(([c, p]) => {
      records = [...c, ...p]
    }, session.fail.bind(session))
  })

  const from = $derived(firstYear(records))
  const placed = $derived(place(records, year < thisYear ? String(year) : undefined))
  const regions = $derived(byRegion(placed))
  const unplaced = $derived(
    records.filter((r) => (kindOf(r) === condition || kindOf(r) === procedure) && !placed.some((p) => p.record === r)),
  )
  const gender = $derived(String(session.person?.gender ?? ''))
  const hidden = $derived(gender === 'female' ? ['prostate'] : gender === 'male' ? ['uterus', 'ovary-r', 'ovary-l'] : [])
  const here = $derived(selected ? regions.get(selected) : undefined)

  const TONES: [Tone, string][] = [
    ['danger', 'Active'],
    ['warn', 'Comes and goes'],
    ['ok', 'Over or in remission'],
    ['accent', 'Operation or procedure'],
  ]
  const toneLabel = (t: Tone) => TONES.find(([k]) => k === t)?.[1] ?? ''
  const summary = (r: Resource) => (kindOf(r) ?? condition).summary(r)
</script>

<div class="controls">
  {#if from && from < thisYear}
    <label class="year">
      <span>As of <strong>{year < thisYear ? year : 'now'}</strong></span>
      <input type="range" min={from} max={thisYear} step="1" bind:value={year} aria-label="Show the body as of a year" />
    </label>
  {/if}
  <label class="check"><input type="checkbox" bind:checked={skin} /> Show skin</label>
  <div class="legend" aria-label="Colours">
    {#each TONES as [tone, text] (tone)}<Pill {tone}>{text}</Pill>{/each}
  </div>
</div>

<div class="layout">
  <div class="viewer">
    {#await import('./Anatomy.svelte') then m}
      <m.default lit={regions} {selected} {hidden} {skin} onselect={(id) => (selected = id)} />
    {/await}
  </div>

  <div class="side">
    {#if selected}
      <Card title={labelOf(selected)}>
        {#snippet action()}<button class="link" onclick={() => (selected = null)}>Back to all</button>{/snippet}
        {#if here}
          <ul class="items">
            {#each here.items as p (p.record.id)}
              {@const s = summary(p.record)}
              <li>
                <span class="dot {p.tone}" aria-hidden="true"></span>
                <span class="text">
                  <strong>{s.title}</strong>
                  <span class="muted small">{[toneLabel(p.tone), s.date?.slice(0, 10)].filter(Boolean).join(' · ')}</span>
                  {#if p.byName}<span class="muted small">Placed by its name. Set "Part of the body" in Records to change it.</span>{/if}
                </span>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="muted">Nothing recorded here{year < thisYear ? ` as of ${year}` : ''}.</p>
        {/if}
        <p><a href="#/records">Open Records</a></p>
      </Card>
    {:else}
      <Card title="On the body" explain="Each condition and operation with a part of the body. Tap a part on the body, or a name here, to see its records. Conditions without a part are placed by name when the name points to one place, such as asthma and the lungs.">
        {#if regions.size}
          <ul class="regions">
            {#each [...regions] as [id, r] (id)}
              <li>
                <button onclick={() => (selected = id)}>
                  <span class="dot {r.tone}" aria-hidden="true"></span>
                  <span>{labelOf(id)}</span>
                  <span class="muted small">{r.items.length} {r.items.length === 1 ? 'record' : 'records'}</span>
                </button>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="muted">Nothing on the body{year < thisYear ? ` as of ${year}` : ' yet'}. Add a condition or operation in Records and choose "Part of the body".</p>
        {/if}
      </Card>
      {#if unplaced.length}
        <div class="gap"></div>
        <Card title="Not placed" explain="These have no part of the body, and their names do not point to one place. Add a part of the body to show them.">
          <ul class="plain">
            {#each unplaced as r (r.id)}<li>{summary(r).title}</li>{/each}
          </ul>
        </Card>
      {/if}
    {/if}
    <p class="muted small credit">Model from BodyParts3D, © The Database Center for Life Science (CC BY 4.0).</p>
  </div>
</div>

<style>
  .controls { display: flex; flex-wrap: wrap; gap: 12px 20px; align-items: center; margin-bottom: 14px; }
  .year { display: grid; gap: 2px; min-width: min(260px, 100%); }
  .year input { width: 100%; accent-color: var(--c-accent); }
  .check { display: flex; gap: 6px; align-items: center; }
  .legend { display: flex; flex-wrap: wrap; gap: 6px; }
  .layout { display: grid; gap: 18px; grid-template-columns: minmax(0, 1.4fr) minmax(0, 1fr); align-items: start; }
  @media (max-width: 760px) { .layout { grid-template-columns: 1fr; } }
  .items, .regions, .plain { list-style: none; margin: 0; padding: 0; display: grid; gap: 8px; }
  .items li { display: flex; gap: 10px; align-items: flex-start; }
  .text { display: grid; gap: 2px; }
  .regions button { width: 100%; display: flex; gap: 10px; align-items: center; font: inherit; color: inherit; text-align: left; background: none; border: 1px solid var(--c-line); border-radius: var(--r-control); padding: 8px 10px; cursor: pointer; }
  .regions button:hover { background: var(--c-panel-2); }
  .regions button:focus-visible, .link:focus-visible { outline: 2px solid var(--c-focus); outline-offset: 2px; }
  .regions .small { margin-left: auto; }
  .dot { width: 12px; height: 12px; border-radius: 50%; flex: 0 0 auto; margin-top: 3px; }
  .regions .dot { margin-top: 0; }
  .dot.danger { background: var(--c-danger); }
  .dot.warn { background: var(--c-warn); }
  .dot.ok { background: var(--c-ok); }
  .dot.accent { background: var(--c-accent); }
  .muted { color: var(--c-muted); }
  .small { font-size: var(--text-small); }
  .link { font: inherit; color: var(--c-accent); background: none; border: 0; cursor: pointer; padding: 0; }
  .gap { height: 18px; }
  .credit { margin-top: 12px; }
</style>
