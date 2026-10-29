<script lang="ts">
  // One person's records of one kind: list, add, edit, delete (with Undo).
  import { Button, Card, Row } from '@swasthya/ui'
  import { api } from './api.ts'
  import type { Kind, Resource } from './model/fhir.ts'
  import RecordForm from './RecordForm.svelte'
  import { session } from './session.svelte.ts'

  let { kind }: { kind: Kind } = $props()

  let rows = $state<Resource[]>([])
  let editing = $state<Resource | 'new' | null>(null)

  $effect(() => {
    void session.version
    const person = session.current
    if (!person) return
    api.records(person, kind.type).then((r) => (rows = r), session.fail.bind(session))
  })

  async function remove(r: Resource) {
    try {
      await api.remove(r.id!)
      await session.changed(`Moved ${kind.summary(r).title || kind.one} to the trash.`)
    } catch (e) {
      session.fail(e)
    }
  }

</script>

<Card title={kind.title} explain={kind.explain}>
  {#snippet action()}
    {#if editing === null}<Button variant="quiet" onclick={() => (editing = 'new')}>Add</Button>{/if}
  {/snippet}
  {#if editing !== null}
    {#key editing}
      <RecordForm {kind} person={session.current} record={editing === 'new' ? undefined : editing} ondone={() => (editing = null)} />
    {/key}
  {:else if rows.length === 0}
    <p class="muted">Nothing here yet for {session.personName}. Choose “Add” to put in {kind.one}.</p>
  {:else}
    <div class="list">
      {#each rows as r (r.id)}
        {@const s = kind.summary(r)}
        <Row lead={s.date ?? ''} title={s.title} sub={s.sub}>
          {#snippet trailing()}
            <span class="tools">
              <button class="link" onclick={() => (editing = r)}>Edit</button>
              <button class="link" onclick={() => remove(r)}>Delete</button>
            </span>
          {/snippet}
        </Row>
      {/each}
    </div>
  {/if}
</Card>

<style>
  .list { display: grid; gap: 2px; }
  .tools { display: inline-flex; gap: 12px; margin-left: 10px; font-size: var(--text-small); }
</style>
