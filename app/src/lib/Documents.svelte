<script lang="ts">
  // One person's documents: add a file with a few words about it, look at it
  // inside the app, save a copy out, edit what it is, delete (with Undo).
  import { Button, Card, Explain, Row } from '@swasthya/ui'
  import { api, inApp } from './api.ts'
  import DateField from './DateField.svelte'
  import { attachment, check, document, toFhir, type Resource, type Values } from './model/fhir.ts'
  import RecordForm from './RecordForm.svelte'
  import { session } from './session.svelte.ts'

  let rows = $state<Resource[]>([])
  let adding = $state(false)
  let editing = $state<Resource | null>(null)
  let values = $state<Values>({})
  let problems = $state<string[]>([])
  let busy = $state(false)

  $effect(() => {
    void session.version
    const person = session.current
    if (!person) return
    api.records(person, document.type).then((r) => (rows = r), session.fail.bind(session))
  })

  async function add(e: SubmitEvent) {
    e.preventDefault()
    problems = check(document, values)
    if (problems.length) return
    busy = true
    try {
      const saved = await api.addDocument(toFhir(document, values, session.current))
      if (!saved) return
      adding = false
      values = {}
      await session.changed(`Added ${document.summary(saved).title}.`)
    } catch (err) {
      session.fail(err)
    } finally {
      busy = false
    }
  }

  async function remove(r: Resource) {
    try {
      await api.remove(r.id!)
      await session.changed(`Moved ${document.summary(r).title || 'the document'} to the trash.`)
    } catch (e) {
      session.fail(e)
    }
  }

  // ---- looking at one ----
  type Shown = { id: string; title: string; type: string; url: string | null; text: string | null }
  let shown = $state<Shown | null>(null)
  // WebView2 (Windows) shows PDFs itself; the Linux and Android web views do not.
  const pdfInside = $derived(session.status?.platform === 'windows')
  const showable = (type: string) =>
    /^(image\/(png|jpeg|gif|webp)|audio\/|video\/|text\/)/.test(type) || (type === 'application/pdf' && pdfInside)

  async function view(r: Resource) {
    const a = attachment(r)
    if (!a) return
    close()
    if (!showable(a.type)) {
      shown = { id: a.id, title: document.summary(r).title, type: a.type, url: null, text: null }
      return
    }
    try {
      const bytes = await api.documentBytes(a.id)
      const blob = new Blob([bytes], { type: a.type })
      shown = {
        id: a.id,
        title: document.summary(r).title,
        type: a.type,
        url: a.type.startsWith('text/') ? null : URL.createObjectURL(blob),
        text: a.type.startsWith('text/') ? await blob.text() : null,
      }
    } catch (e) {
      session.fail(e)
    }
  }
  function close() {
    if (shown?.url) URL.revokeObjectURL(shown.url)
    shown = null
  }
  $effect(() => close)

  async function saveCopy(r: Resource) {
    const a = attachment(r)
    if (!a) return
    try {
      const { save } = await import('@tauri-apps/plugin-dialog')
      const target = await save({ defaultPath: a.name, title: 'Save a copy of the document' })
      if (!target) return
      await api.saveDocumentTo(a.id, target)
      session.say({ text: 'Copy saved. That copy is not encrypted: keep it somewhere private.' })
    } catch (e) {
      session.fail(e)
    }
  }

  const size = (b: number) => (b < 1_048_576 ? `${Math.max(1, Math.round(b / 1024))} KB` : `${(b / 1_048_576).toFixed(1)} MB`)
  const uid = Math.random().toString(36).slice(2, 8)
</script>

<Card title={document.title} explain={document.explain}>
  {#snippet action()}
    {#if !adding && !editing}<Button variant="quiet" disabled={!inApp} onclick={() => (adding = true)}>Add a document</Button>{/if}
  {/snippet}
  {#if adding}
    <form onsubmit={add} novalidate>
      <div class="fields">
        {#each document.fields as f (f.key)}
          {@const id = `${uid}-${f.key}`}
          <div class="field">
            <span class="label">
              <label for={id}>{f.label}{f.required ? '' : ' (optional)'}</label>
              {#if f.explain}<Explain title={f.label} text={f.explain} />{/if}
            </span>
            {#if f.type === 'date'}
              <DateField {id} bind:value={values[f.key]} />
            {:else}
              <input {id} bind:value={values[f.key]} list={f.list ? `${id}-list` : undefined} autocomplete="off" />
              {#if f.list}<datalist id={`${id}-list`}>{#each f.list as o (o)}<option value={o}></option>{/each}</datalist>{/if}
            {/if}
          </div>
        {/each}
      </div>
      {#if problems.length}<ul class="problems" role="alert">{#each problems as p (p)}<li>{p}</li>{/each}</ul>{/if}
      <p class="muted small">Next, choose the file: a PDF, photo, scan, Word document, recording or video. It is encrypted as it is added.</p>
      <div class="actions">
        <Button type="submit" disabled={busy}>{busy ? 'Adding…' : 'Choose the file and add'}</Button>
        <Button variant="ghost" disabled={busy} onclick={() => ((adding = false), (problems = []))}>Cancel</Button>
      </div>
    </form>
  {:else if editing}
    {#key editing}
      <RecordForm kind={document} person={session.current} record={editing} ondone={() => (editing = null)} />
    {/key}
  {:else if rows.length === 0}
    <p class="muted">No documents yet for {session.personName}. Choose “Add a document” to keep a letter, report, scan or photo here.</p>
  {:else}
    <div class="list">
      {#each rows as r (r.id)}
        {@const s = document.summary(r)}
        {@const a = attachment(r)}
        <Row lead={s.date ?? ''} title={s.title} sub={a ? `${s.sub} · ${size(a.size)}` : s.sub}>
          {#snippet trailing()}
            <span class="tools">
              {#if a}
                <button class="link" onclick={() => view(r)}>Open</button>
                <button class="link" onclick={() => saveCopy(r)}>Save a copy</button>
              {/if}
              <button class="link" onclick={() => (editing = r)}>Edit</button>
              <button class="link" onclick={() => remove(r)}>Delete</button>
            </span>
          {/snippet}
        </Row>
      {/each}
    </div>
  {/if}
</Card>

{#if shown}
  <div class="gap"></div>
  <Card title={shown.title}>
    {#snippet action()}<Button variant="ghost" onclick={close}>Close</Button>{/snippet}
    {#if shown.text !== null}
      <pre class="text">{shown.text}</pre>
    {:else if shown.url && shown.type.startsWith('image/')}
      <img src={shown.url} alt={shown.title} />
    {:else if shown.url && shown.type.startsWith('audio/')}
      <audio controls src={shown.url}></audio>
    {:else if shown.url && shown.type.startsWith('video/')}
      <!-- svelte-ignore a11y_media_has_caption -->
      <video controls src={shown.url}></video>
    {:else if shown.url}
      <iframe src={shown.url} title={shown.title}></iframe>
    {:else}
      <p class="muted">This kind of file cannot be shown inside the app. Use “Save a copy” to open it in another program.</p>
    {/if}
  </Card>
{/if}

<style>
  .list { display: grid; gap: 2px; }
  .tools { display: inline-flex; flex-wrap: wrap; gap: 12px; margin-left: 10px; font-size: var(--text-small); }
  .gap { height: 18px; }
  .small { font-size: var(--text-small); }
  img, video, iframe { display: block; width: 100%; border-radius: var(--r-sm); }
  img { height: auto; max-height: 75vh; object-fit: contain; }
  iframe { height: 75vh; border: 1px solid var(--c-line); }
  audio { width: 100%; }
  .text { white-space: pre-wrap; font: inherit; max-height: 60vh; overflow: auto; }
</style>
