<script lang="ts">
  // Find records by the words in them, including text read from documents.
  // Searches the person being shown, or everyone.
  import { Card, Row } from '@swasthya/ui'
  import { api, type Hit } from './api.ts'
  import { kindOf } from './model/fhir.ts'
  import { session } from './session.svelte.ts'

  let words = $state('')
  let everyone = $state(false)
  let hits = $state<Hit[]>([])
  let searched = $state('')

  // Search as the user types, a moment after they stop.
  $effect(() => {
    void session.version
    const w = words.trim()
    const person = everyone ? null : session.current || null
    const t = setTimeout(() => {
      if (!w) return void ((hits = []), (searched = ''))
      api.search(person, w).then((h) => ((hits = h), (searched = w)), session.fail.bind(session))
    }, 200)
    return () => clearTimeout(t)
  })

  /** The snippet in plain and marked pieces; never parsed as HTML. */
  const pieces = (s: string) =>
    s.split('\u0002').flatMap((part, i) => {
      if (i === 0) return [{ text: part, mark: false }]
      const [hit, rest = ''] = part.split('\u0003')
      return [{ text: hit, mark: true }, { text: rest, mark: false }]
    })
  const nameOf = (id: string) => session.people.find((p) => p.id === id)
  const personName = (r: Hit['resource']) => {
    const ref = JSON.stringify(r).match(/"reference":"Patient\/([^"]+)"/)?.[1] ?? ''
    return ((nameOf(ref)?.name as { text?: string }[] | undefined)?.[0]?.text) ?? ''
  }
</script>

<Card>
  <div class="bar">
    <div class="field grow">
      <label class="sr" for="q">Search the records</label>
      <input id="q" type="search" bind:value={words} placeholder="Words to find, such as asthma or ferritin" autocomplete="off" />
    </div>
    {#if session.people.length > 1}
      <label class="who"><input type="checkbox" bind:checked={everyone} /> Everyone</label>
    {/if}
  </div>
  {#if searched && hits.length === 0}
    <p class="muted">Nothing found for “{searched}”{everyone ? '' : ` in ${session.personName}'s records`}.</p>
  {:else}
    <div class="list">
      {#each hits as h (h.resource.id)}
        {@const k = kindOf(h.resource)}
        {@const s = k?.summary(h.resource)}
        <a class="hit" href={`#/${k?.page ?? 'records'}`}>
          <Row lead={s?.date ?? ''} title={s?.title || h.resource.resourceType} sub={[k?.one.replace(/^an? /, ''), everyone ? personName(h.resource) : ''].filter(Boolean).join(' · ')} />
          {#if h.snippet}
            <p class="snip">{#each pieces(h.snippet) as p, i (i)}{#if p.mark}<mark>{p.text}</mark>{:else}{p.text}{/if}{/each}</p>
          {/if}
        </a>
      {/each}
    </div>
  {/if}
</Card>

<style>
  .bar { display: flex; flex-wrap: wrap; gap: 12px; align-items: center; margin-bottom: 12px; }
  .grow { flex: 1 1 260px; }
  .who { display: inline-flex; gap: 6px; align-items: center; }
  .sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); }
  .list { display: grid; gap: 4px; }
  .hit { color: inherit; text-decoration: none; display: block; border-radius: var(--r-sm); }
  .hit:hover, .hit:focus-visible { background: var(--c-accent-soft); }
  .snip { margin: -4px 0 8px; padding: 0 12px; color: var(--c-muted); font-size: var(--text-small); }
  mark { background: var(--c-accent-soft); color: inherit; font-weight: 600; border-radius: 3px; padding: 0 2px; }
</style>
