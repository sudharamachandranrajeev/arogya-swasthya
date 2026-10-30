<script lang="ts">
  // Add or edit one record of any kind. Saves through the shell, then reports
  // with an Undo offer.
  import { Button, Explain } from '@swasthya/ui'
  import { api } from './api.ts'
  import DateField from './DateField.svelte'
  import { check, fromFhir, toFhir, type Kind, type Resource, type Values } from './model/fhir.ts'
  import { session } from './session.svelte.ts'

  let {
    kind,
    person,
    record,
    ondone,
  }: { kind: Kind; person: string; record?: Resource; ondone: (saved?: Resource) => void } = $props()

  const today = new Date().toISOString().slice(0, 10)
  const start = (): Values =>
    record ? fromFhir(kind, record) : Object.fromEntries(kind.fields.filter((f) => f.today).map((f) => [f.key, today]))
  let values = $state<Values>(start())
  let problems = $state<string[]>([])
  let busy = $state(false)
  const uid = Math.random().toString(36).slice(2, 8)

  async function save(e: SubmitEvent) {
    e.preventDefault()
    problems = check(kind, values)
    if (problems.length) return
    busy = true
    try {
      const saved = await api.save(toFhir(kind, values, person, record))
      await session.changed(`${record ? 'Saved' : 'Added'} ${kind.summary(saved).title || kind.one}.`)
      ondone(saved)
    } catch (err) {
      session.fail(err)
    } finally {
      busy = false
    }
  }
</script>

<form onsubmit={save} novalidate>
  <div class="fields">
    {#each kind.fields as f (f.key)}
      {@const id = `${uid}-${f.key}`}
      <div class="field" class:wide={f.type === 'note'}>
        <span class="label">
          <label for={id}>{f.label}{f.required ? '' : ' (optional)'}</label>
          {#if f.explain}<Explain title={f.label} text={f.explain} />{/if}
        </span>
        {#if f.type === 'select'}
          <select {id} bind:value={values[f.key]}>
            <option value="">{f.required ? 'Choose…' : '—'}</option>
            {#each f.options ?? [] as [v, l] (v)}<option value={v}>{l}</option>{/each}
          </select>
        {:else if f.type === 'note'}
          <textarea {id} bind:value={values[f.key]}></textarea>
        {:else if f.type === 'date'}
          <DateField {id} bind:value={values[f.key]} />
        {:else}
          <input
            {id}
            bind:value={values[f.key]}
            inputmode={f.type === 'number' ? 'decimal' : undefined}
            placeholder={f.type === 'times' ? '08:00, 20:00' : undefined}
            list={f.list ? `${id}-list` : undefined}
            autocomplete="off"
          />
          {#if f.list}
            <datalist id={`${id}-list`}>{#each f.list as o (o)}<option value={o}></option>{/each}</datalist>
          {/if}
        {/if}
      </div>
    {/each}
  </div>
  {#if problems.length}
    <ul class="problems" role="alert">{#each problems as p (p)}<li>{p}</li>{/each}</ul>
  {/if}
  <div class="actions">
    <Button type="submit" disabled={busy}>{record ? 'Save changes' : `Add ${kind.one.replace(/^an? /, '')}`}</Button>
    <Button variant="ghost" onclick={() => ondone()}>Cancel</Button>
  </div>
</form>
