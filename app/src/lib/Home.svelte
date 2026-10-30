<script lang="ts">
  // The first screen: who is being looked at, how much is recorded, what
  // changed lately, and the emergency number for where they are.
  import { Card, Stat } from '@swasthya/ui'
  import { api } from './api.ts'
  import { kindOf, kinds } from './model/fhir.ts'
  import { emergencyLine } from './model/places.ts'
  import { session } from './session.svelte.ts'
  import Timeline from './Timeline.svelte'

  let counts = $state<Record<string, number>>({})
  $effect(() => {
    void session.version
    const person = session.current
    if (!person) return
    const types = [...new Set(kinds.map((k) => k.type))]
    Promise.all(types.map((t) => api.records(person, t))).then((lists) => {
      const c: Record<string, number> = {}
      for (const r of lists.flat()) {
        const k = kindOf(r)
        if (k) c[k.title] = (c[k.title] ?? 0) + 1
      }
      counts = c
    }, session.fail.bind(session))
  })
</script>

<h1>{session.personName || 'Overview'}</h1>
<p class="lead">{emergencyLine(session.places)}</p>

{#if session.status?.recoveryPending}
  <Card title="Keep your recovery key safe">
    <p>You have not confirmed that your recovery key is written down. Without it, a lost device means lost records.</p>
    <p><a href="#/settings">Open Settings → Security</a></p>
  </Card>
  <div class="gap"></div>
{/if}

<div class="stats">
  {#each kinds as k (k.title)}
    <a href={`#/${k.page}`} class="stat">
      <Stat label={k.title} value={String(counts[k.title] ?? 0)} explain={k.explain} />
    </a>
  {/each}
</div>
<div class="gap"></div>
<Card title="Lately" explain="The newest entries in this record, by the date they happened (not the date you typed them).">
  {#snippet action()}<a href="#/timeline">Whole timeline</a>{/snippet}
  <Timeline limit={8} />
</Card>

<style>
  .stats { display: grid; gap: 12px; grid-template-columns: repeat(auto-fill, minmax(min(170px, 100%), 1fr)); }
  .stat { color: inherit; text-decoration: none; }
  .gap { height: 18px; }
</style>
