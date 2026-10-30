<script lang="ts">
  // Bring in records from a file from a clinic, hospital or another app
  // (FHIR JSON, such as an International Patient Summary, or C-CDA XML). Nothing is kept
  // until the user ticks what they want; one Undo takes the whole import back.
  import { Button, Card, Row } from '@swasthya/ui'
  import { api, inApp, type LinkInfo, type Pending } from './api.ts'
  import { kindOf, type Resource } from './model/fhir.ts'
  import { session } from './session.svelte.ts'

  let pending = $state<Pending | null>(null)
  let ticked = $state<boolean[]>([])
  let busy = $state(false)

  const show = (p: Pending | null) => {
    pending = p
    // Ones that look like records already kept start unticked.
    ticked = p?.candidates.map((c) => !c.duplicateOf) ?? []
  }
  api.importPending().then(show, session.fail.bind(session))

  async function pick() {
    busy = true
    try {
      const p = await api.importPick(session.current)
      if (p) show(p)
    } catch (e) {
      session.fail(e)
    } finally {
      busy = false
    }
  }

  // ---- a SMART Health Link ----
  let link = $state('')
  let passcode = $state('')
  let info = $state<LinkInfo | null>(null)
  let linkProblem = $state('')
  $effect(() => {
    const l = link.trim()
    info = null
    linkProblem = ''
    if (!l || !inApp) return
    api.linkInfo(l).then((i) => (info = i), (e) => (linkProblem = String(e)))
  })
  const expired = $derived(!!info?.exp && info.exp * 1000 < Date.now())

  async function fetchLink(e: SubmitEvent) {
    e.preventDefault()
    busy = true
    try {
      const p = await api.importLink(session.current, link.trim(), info?.needsPasscode ? passcode : null)
      if (p) show(p)
      link = ''
      passcode = ''
    } catch (err) {
      session.fail(err)
    } finally {
      busy = false
    }
  }

  async function accept() {
    busy = true
    try {
      const picks = ticked.flatMap((t, i) => (t ? [i] : []))
      const n = await api.importAccept(picks)
      show(null)
      await session.changed(n ? `Imported ${n} record${n === 1 ? '' : 's'}.` : 'Nothing was imported.')
    } catch (e) {
      session.fail(e)
    } finally {
      busy = false
    }
  }

  async function discard() {
    try {
      await api.importDiscard()
      show(null)
    } catch (e) {
      session.fail(e)
    }
  }

  const count = $derived(ticked.filter(Boolean).length)
  const forName = $derived.by(() => {
    const p = session.people.find((x) => x.id === pending?.person)
    return (p?.name as { text?: string }[] | undefined)?.[0]?.text ?? 'someone in the trash'
  })
  const describe = (r: Resource) => {
    const k = kindOf(r)
    const s = k?.summary(r)
    const code = (r.code as { text?: string } | undefined)?.text
    return { date: s?.date ?? '', title: s?.title || code || r.resourceType, kind: k?.one.replace(/^an? /, '') ?? 'report' }
  }
</script>

{#if !pending}
  <Card title="Import a file" explain="A health record file you were given or downloaded, such as a patient summary from a clinic or another app. You see everything in it before anything is kept.">
    <p>Choose a <strong>FHIR JSON</strong> file, such as an <strong>International Patient Summary</strong> (IPS), or a <strong>C-CDA</strong> summary (an XML file from many US hospitals and patient portals). The records go into <strong>{session.personName}'s</strong> records once you choose which to keep.</p>
    <p class="muted small">The file is read on this device. Nothing is sent anywhere.</p>
    <div class="actions">
      <Button disabled={busy} onclick={pick}>{busy ? 'Reading…' : 'Choose a file'}</Button>
    </div>
  </Card>
  <div class="gap"></div>
  <Card title="Open a SMART Health Link" explain="A link (often shown as a QR code) to records kept encrypted on a server. The key is in the link, so the server cannot read them.">
    <form onsubmit={fetchLink} novalidate>
      <div class="field">
        <label for="shl">The link</label>
        <input id="shl" bind:value={link} placeholder="shlink:/… or a web address ending in #shlink:/…" autocomplete="off" spellcheck="false" />
      </div>
      {#if linkProblem}
        <p class="muted small" role="alert">{linkProblem}</p>
      {:else if info}
        <p class="small">{info.label ?? 'Health records'} · from {new URL(info.url).host}{info.exp ? ` · ${expired ? 'expired' : 'works until'} ${new Date(info.exp * 1000).toLocaleDateString()}` : ''}</p>
        {#if info.needsPasscode}
          <div class="field">
            <label for="shl-pass">Passcode (given to you with the link)</label>
            <input id="shl-pass" type="password" bind:value={passcode} autocomplete="off" />
          </div>
        {/if}
      {/if}
      <p class="muted small">This contacts the server named in the link, and only that server.</p>
      <div class="actions">
        <Button type="submit" disabled={busy || !info || expired || (info.needsPasscode && !passcode)}>{busy ? 'Fetching…' : 'Fetch and review'}</Button>
      </div>
    </form>
  </Card>
{:else}
  <Card title={`From ${pending.fileName}`}>
    {#snippet action()}<Button variant="ghost" disabled={busy} onclick={discard}>Discard</Button>{/snippet}
    <p>Tick what to keep in <strong>{forName}'s</strong> records. You can change any of them later, and one Undo takes the whole import back.</p>
    {#if pending.person !== session.current}
      <p class="note">This file was read for {forName}, not {session.personName}. To import it for {session.personName}, discard it and choose the file again.</p>
    {/if}
    <div class="list">
      {#each pending.candidates as c, i (i)}
        {@const d = describe(c.resource)}
        <label class="pick">
          <input type="checkbox" bind:checked={ticked[i]} />
          <Row lead={d.date.slice(0, 4)} title={d.title} sub={c.duplicateOf ? `${d.kind} · looks like one already kept` : d.kind} />
        </label>
      {/each}
    </div>
    {#if pending.skipped.length}
      <p class="muted small">Not kept here: {pending.skipped.map(([t, n]) => `${n} ${t}`).join(', ')}.</p>
    {/if}
    <div class="actions">
      <Button disabled={busy || count === 0} onclick={accept}>{busy ? 'Saving…' : `Keep ${count} record${count === 1 ? '' : 's'}`}</Button>
      <Button variant="ghost" disabled={busy} onclick={() => (ticked = ticked.map(() => count === 0))}>{count === 0 ? 'Tick all' : 'Untick all'}</Button>
    </div>
  </Card>
{/if}

<style>
  .list { display: grid; gap: 2px; margin: 12px 0; }
  .pick { display: flex; align-items: center; gap: 10px; cursor: pointer; padding-left: 4px; }
  .pick :global(.row) { flex: 1; }
  .small { font-size: var(--text-small); }
  .note { padding: 8px 12px; border-radius: var(--r-sm); background: var(--c-accent-soft); }
  .gap { height: 18px; }
  .actions { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 12px; }
</style>
