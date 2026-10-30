<script lang="ts">
  // Everything in one person's record, newest first, grouped by year.
  import { Card, Row } from '@swasthya/ui'
  import { api } from './api.ts'
  import { kindOf, type Resource } from './model/fhir.ts'
  import { session } from './session.svelte.ts'

  let { limit }: { limit?: number } = $props()
  let rows = $state<Resource[]>([])

  $effect(() => {
    void session.version
    const person = session.current
    if (!person) return
    api.timeline(person).then((r) => (rows = limit ? r.slice(0, limit) : r), session.fail.bind(session))
  })

  const items = $derived(
    rows.map((r) => {
      const k = kindOf(r)
      const s = k?.summary(r) ?? { title: r.resourceType, sub: '' }
      return { id: r.id!, year: s.date?.slice(0, 4) || 'No date', what: k?.title.replace(/s$/, '') ?? r.resourceType, ...s }
    }),
  )
  const years = $derived([...new Set(items.map((i) => i.year))])
</script>

{#if items.length === 0}
  <p class="muted">Nothing recorded yet.</p>
{:else}
  {#each years as y (y)}
    <h3>{y}</h3>
    <div class="list">
      {#each items.filter((i) => i.year === y) as i (i.id)}
        <Row lead={i.date?.slice(5) || '—'} title={i.title} sub={[i.what, i.sub].filter(Boolean).join(' · ')} />
      {/each}
    </div>
  {/each}
{/if}

<style>
  h3 { font-size: var(--text-small); color: var(--c-muted); margin: 14px 0 4px; letter-spacing: .04em; }
  h3:first-child { margin-top: 0; }
  .list { display: grid; gap: 2px; }
</style>
