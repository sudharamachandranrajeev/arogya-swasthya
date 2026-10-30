<script lang="ts">
  // Vital signs: put in several readings at once (one Undo takes them all
  // back), list them, edit or delete one.
  import { Button, Card, Explain, Row } from '@swasthya/ui'
  import { api } from './api.ts'
  import DateField from './DateField.svelte'
  import { kindOf, vital, type Resource } from './model/fhir.ts'
  import { checkReading, filled, show, toObservation, toReading, vitalOf, vitals, type Reading, type Vital } from './model/vitals.ts'
  import { session } from './session.svelte.ts'

  let rows = $state<Resource[]>([])
  let open = $state(false)
  let editing = $state<Resource | null>(null)
  let date = $state('')
  let readings = $state<Record<string, Reading>>({})
  let problems = $state<string[]>([])
  let busy = $state(false)
  const uid = Math.random().toString(36).slice(2, 8)

  $effect(() => {
    void session.version
    const person = session.current
    if (!person) return
    api.records(person, vital.type).then((r) => (rows = r.filter((x) => kindOf(x) === vital)), session.fail.bind(session))
  })

  /** Empty readings, each in the unit last used for it. */
  function blank(): Record<string, Reading> {
    const out: Record<string, Reading> = {}
    for (const v of vitals) {
      const last = rows.find((r) => vitalOf(r) === v)
      out[v.key] = { unit: last ? toReading(v, last).unit : v.units[0].label, values: {} }
    }
    return out
  }

  function start(r?: Resource) {
    const v = r && vitalOf(r)
    if (r && !v) return
    editing = r ?? null
    readings = blank()
    if (r && v) readings[v.key] = toReading(v, r)
    date = r ? String(r.effectiveDateTime ?? '').slice(0, 10) : new Date().toISOString().slice(0, 10)
    problems = []
    open = true
  }

  const shown = $derived(editing ? vitals.filter((v) => v === vitalOf(editing!)) : vitals)

  async function save(e: SubmitEvent) {
    e.preventDefault()
    const chosen = shown.filter((v) => filled(v, readings[v.key]))
    problems = shown.flatMap((v) => checkReading(v, readings[v.key]))
    if (!/^\d{4}-\d{2}-\d{2}$/.test(date)) problems.push('Date: choose the day these were taken.')
    else if (date > new Date().toISOString().slice(0, 10)) problems.push('Date is in the future.')
    if (!chosen.length) problems.push('Fill in at least one reading.')
    if (problems.length) return
    const person = session.current
    busy = true
    try {
      const obs = chosen.map((v) => toObservation(v, readings[v.key], person, date, editing ?? undefined))
      if (editing) {
        await api.save(obs[0])
        await session.changed(`Saved ${shown[0].label.toLowerCase()}.`)
      } else {
        await api.saveAll(obs, `add ${obs.length} vital sign reading${obs.length === 1 ? '' : 's'}`)
        await session.changed(`Added ${obs.length} reading${obs.length === 1 ? '' : 's'}.`)
      }
      open = false
    } catch (err) {
      session.fail(err)
    } finally {
      busy = false
    }
  }

  async function remove(r: Resource) {
    try {
      await api.remove(r.id!)
      await session.changed(`Moved ${vital.summary(r).title.toLowerCase()} to the trash.`)
    } catch (e) {
      session.fail(e)
    }
  }

  const parts = (v: Vital) => v.parts ?? [{ key: 'value', label: v.label, display: v.label, loinc: v.loinc }]
</script>

<Card title={vital.title} explain={vital.explain}>
  {#snippet action()}
    {#if !open}<Button variant="quiet" onclick={() => start()}>Add readings</Button>{/if}
  {/snippet}
  {#if open}
    <form onsubmit={save} novalidate>
      <div class="when field">
        <span class="label"><label for={`${uid}-date`}>Date taken</label></span>
        <DateField id={`${uid}-date`} bind:value={date} />
      </div>
      <p class="muted small">{editing ? 'Change the reading.' : 'Fill in only what you measured. The rest stays empty.'}</p>
      <div class="grid">
        {#each shown as v (v.key)}
          {@const r = readings[v.key]}
          <fieldset class="vital field">
            <legend class="label">{v.label} <Explain title={v.label} text={v.explain} /></legend>
            <div class="inputs">
              {#each parts(v) as p, i (p.key)}
                {#if i > 0}<span class="slash" aria-hidden="true">/</span>{/if}
                <input
                  id={`${uid}-${v.key}-${p.key}`}
                  aria-label={v.parts ? `${v.label}, ${p.label.toLowerCase()}` : v.label}
                  inputmode="decimal"
                  autocomplete="off"
                  bind:value={r.values[p.key]}
                />
              {/each}
              {#if v.units.length > 1}
                <select aria-label={`${v.label} unit`} bind:value={r.unit}>
                  {#each v.units as u (u.label)}<option value={u.label}>{u.label}</option>{/each}
                </select>
              {:else}
                <span class="unit">{v.units[0].label}</span>
              {/if}
            </div>
          </fieldset>
        {/each}
      </div>
      {#if problems.length}
        <ul class="problems" role="alert">{#each problems as p (p)}<li>{p}</li>{/each}</ul>
      {/if}
      <div class="actions">
        <Button type="submit" disabled={busy}>{editing ? 'Save changes' : 'Add readings'}</Button>
        <Button variant="ghost" onclick={() => (open = false)}>Cancel</Button>
      </div>
    </form>
  {:else if rows.length === 0}
    <p class="muted">No readings yet for {session.personName}. Choose “Add readings” to put in blood pressure, weight and more.</p>
  {:else}
    <div class="list">
      {#each rows as r (r.id)}
        {@const s = vital.summary(r)}
        <Row lead={s.date ?? ''} title={s.title} sub={show(r)}>
          {#snippet trailing()}
            <span class="tools">
              {#if vitalOf(r)}<button class="link" onclick={() => start(r)}>Edit</button>{/if}
              <button class="link" onclick={() => remove(r)}>Delete</button>
            </span>
          {/snippet}
        </Row>
      {/each}
    </div>
  {/if}
</Card>

<style>
  .when { max-width: 260px; margin-bottom: 8px; }
  .grid { display: grid; gap: 12px; grid-template-columns: repeat(auto-fill, minmax(min(240px, 100%), 1fr)); margin: 12px 0; }
  .vital { border: 0; padding: 0; margin: 0; min-width: 0; }
  .vital legend { display: inline-flex; align-items: center; gap: 6px; margin-bottom: 6px; padding: 0; font-weight: 600; font-size: var(--text-small); }
  .inputs { display: flex; align-items: center; gap: 6px; }
  .inputs input { width: 5.5em; min-width: 0; }
  .inputs select { width: auto; }
  .unit, .slash { color: var(--c-muted); font-size: var(--text-small); }
  .list { display: grid; gap: 2px; }
  .tools { display: inline-flex; gap: 12px; margin-left: 10px; font-size: var(--text-small); }
  .small { font-size: var(--text-small); }
</style>
