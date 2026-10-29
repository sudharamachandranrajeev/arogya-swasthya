<script lang="ts">
  // People, places, security, backups and the trash.
  import { Button, Card, Row, Segmented } from '@swasthya/ui'
  import { api, inApp, message, type Backups, type Trashed } from './api.ts'
  import { kindOf, patient, type Resource } from './model/fhir.ts'
  import PlacesEditor from './PlacesEditor.svelte'
  import RecordForm from './RecordForm.svelte'
  import { session } from './session.svelte.ts'

  type Tab = 'people' | 'places' | 'security' | 'backups' | 'trash'
  let tab = $state<Tab>('people')
  const tabs: { value: Tab; label: string }[] = [
    { value: 'people', label: 'People' },
    { value: 'places', label: 'Places' },
    { value: 'security', label: 'Security' },
    { value: 'backups', label: 'Backups' },
    { value: 'trash', label: 'Trash' },
  ]
  const phone = $derived(['android', 'ios'].includes(session.status?.platform ?? ''))

  async function act(f: () => Promise<unknown>, done?: string) {
    try {
      await f()
      if (done) session.say({ text: done })
    } catch (e) {
      session.fail(e)
    }
  }

  // ---- people ----
  let editing = $state<Resource | 'new' | null>(null)
  async function removePerson(p: Resource) {
    const name = patient.summary(p).title
    if (!confirm(`Move ${name} and all their records to the trash? You can restore them for 30 days.`)) return
    await act(async () => {
      await api.remove(p.id!)
      await session.changed(`Moved ${name} and their records to the trash.`)
    })
  }

  // ---- security ----
  let key = $state('')
  let pin = $state('')
  let pin2 = $state('')
  let changingPin = $state(false)
  async function savePin() {
    if (!/^\d{4,}$/.test(pin)) throw 'Use at least 4 digits.'
    if (pin !== pin2) throw 'The two PINs are different.'
    await api.setPin(pin)
    pin = pin2 = ''
    changingPin = false
    session.status = await api.status()
  }
  const STORAGE = { keyring: "this computer's keyring", file: 'a private file on this computer (no keyring was available)', androidKeystore: "the phone's secure keystore" }
  const storage = $derived(session.status?.keyStorage ? STORAGE[session.status.keyStorage] : 'this device')

  // ---- backups ----
  let backups = $state<Backups | null>(null)
  const loadBackups = () => api.backups().then((b) => (backups = b), session.fail.bind(session))
  $effect(() => {
    if (tab === 'backups') loadBackups()
  })
  async function chooseFolder() {
    const { open } = await import('@tauri-apps/plugin-dialog')
    const picked = await open({ directory: true, title: 'Choose a folder for backup copies' })
    if (typeof picked !== 'string') return
    await api.setBackupFolder(picked)
    await loadBackups()
  }
  async function saveCopy() {
    const { save } = await import('@tauri-apps/plugin-dialog')
    const target = await save({ defaultPath: `swasthya-${new Date().toISOString().slice(0, 10)}.db`, title: 'Save a backup' })
    if (!target) return
    await api.saveBackupTo(target)
    session.say({ text: 'Backup saved.' })
  }

  // ---- trash ----
  let trash = $state<Trashed[]>([])
  $effect(() => {
    void session.version
    if (tab === 'trash') api.trash().then((t) => (trash = t), session.fail.bind(session))
  })
  const daysLeft = (at: string) => Math.max(0, 30 - Math.floor((Date.now() - Date.parse(at)) / 86_400_000))
  const describe = (r: Resource) => {
    const k = kindOf(r.resourceType)
    return { title: k?.summary(r).title || r.resourceType, what: k?.one.replace(/^an? /, '') ?? r.resourceType }
  }
</script>

<h1>Settings</h1>
<div class="tabs"><Segmented label="Settings section" options={tabs} value={tab} onchange={(v) => (tab = v)} /></div>

{#if tab === 'people'}
  <Card title="People" explain={patient.explain}>
    {#snippet action()}{#if editing === null}<Button variant="quiet" onclick={() => (editing = 'new')}>Add a person</Button>{/if}{/snippet}
    {#if editing !== null}
      {#key editing}
        <RecordForm
          kind={patient}
          person=""
          record={editing === 'new' ? undefined : editing}
          ondone={(saved) => {
            editing = null
            if (saved && !session.current) session.choose(saved.id!)
          }}
        />
      {/key}
    {:else}
      {#each session.people as p (p.id)}
        {@const s = patient.summary(p)}
        <Row title={s.title} sub={s.sub}>
          {#snippet trailing()}
            <span class="tools">
              {#if p.id === session.current}<span class="muted">Showing</span>{:else}<button class="link" onclick={() => session.choose(p.id!)}>Show</button>{/if}
              <button class="link" onclick={() => (editing = p)}>Edit</button>
              <button class="link" onclick={() => removePerson(p)}>Delete</button>
            </span>
          {/snippet}
        </Row>
      {/each}
    {/if}
  </Card>

{:else if tab === 'places'}
  <Card title="Places" explain="Emergency numbers on every screen follow “Where you are now”. Switch it when you travel.">
    <PlacesEditor places={session.places} onchange={(p) => act(() => session.setPlaces(p))} />
  </Card>

{:else if tab === 'security'}
  <Card title="Encryption" explain="Your records are encrypted with a random key. This device keeps that key; the recovery key is a copy of it for you.">
    <p>Your records are encrypted. The key is kept in {storage}.</p>
    {#if key}
      <code class="key">{key}</code>
      <div class="actions">
        <Button variant="quiet" onclick={() => (key = '')}>Hide</Button>
        {#if session.status?.recoveryPending}<Button onclick={() => act(async () => { await api.recoverySaved(); session.status = await api.status() }, 'Thank you. Keep it somewhere safe.')}>I have written it down</Button>{/if}
      </div>
    {:else}
      <div class="actions"><Button variant="quiet" onclick={() => act(async () => (key = await api.recoveryKey()))}>Show recovery key</Button></div>
    {/if}
  </Card>
  <div class="gap"></div>
  <Card title="App lock" explain="Asks for a PIN{phone ? ' or your fingerprint or face' : ''} each time the app opens. If you forget the PIN, the recovery key opens the app.">
    <p>{session.status?.hasPin ? 'A PIN is set.' : 'No PIN is set.'}{session.status?.biometric ? ' Fingerprint or face unlock is on.' : ''}</p>
    {#if changingPin}
      <form onsubmit={(e) => (e.preventDefault(), act(savePin, 'PIN saved.'))}>
        <div class="fields">
          <div class="field"><label class="label" for="s-pin1">New PIN (at least 4 digits)</label><input id="s-pin1" type="password" inputmode="numeric" bind:value={pin} autocomplete="new-password" /></div>
          <div class="field"><label class="label" for="s-pin2">Same PIN again</label><input id="s-pin2" type="password" inputmode="numeric" bind:value={pin2} autocomplete="new-password" /></div>
        </div>
        <div class="actions"><Button type="submit">Save PIN</Button><Button variant="ghost" onclick={() => (changingPin = false)}>Cancel</Button></div>
      </form>
    {:else}
      <div class="actions">
        <Button variant="quiet" onclick={() => (changingPin = true)}>{session.status?.hasPin ? 'Change PIN' : 'Set a PIN'}</Button>
        {#if session.status?.hasPin}<Button variant="ghost" onclick={() => act(async () => { await api.setPin(null); session.status = await api.status() }, 'PIN removed.')}>Remove PIN</Button>{/if}
        {#if phone}
          <Button variant="quiet" onclick={() => act(async () => { await api.setBiometric(!session.status?.biometric); session.status = await api.status() })}>
            {session.status?.biometric ? 'Turn off fingerprint or face' : 'Use fingerprint or face'}
          </Button>
        {/if}
        {#if session.status?.hasPin || session.status?.biometric}<Button variant="ghost" onclick={() => act(async () => { await api.lock(); await session.load() })}>Lock now</Button>{/if}
      </div>
    {/if}
  </Card>

{:else if tab === 'backups'}
  <Card title="Backups" explain="Each backup is an encrypted copy of every record. It opens only with this device's key or your recovery key. One is made each day; the newest 14 are kept in each folder.">
    {#if backups}
      <p><strong>On this device:</strong> {backups.files.length} backup{backups.files.length === 1 ? '' : 's'}{backups.files[0] ? `, newest ${backups.files[0].replace(/^swasthya-|\.db$/g, '')}` : ''}.</p>
      <p class="muted small">{backups.folder}</p>
      {#if !phone}
        <p><strong>Second copy:</strong> {backups.userFolder ? `${backups.userFolder} (${backups.userFiles.length} backups)` : 'none'}</p>
      {/if}
    {/if}
    <div class="actions">
      <Button variant="quiet" onclick={() => act(async () => { await api.backupNow(); await loadBackups() }, 'Backup made.')}>Back up now</Button>
      {#if inApp && !phone}
        <Button variant="quiet" onclick={() => act(chooseFolder, 'Folder saved, and a backup was written to it.')}>{backups?.userFolder ? 'Change folder' : 'Choose a folder for a second copy'}</Button>
        {#if backups?.userFolder}<Button variant="ghost" onclick={() => act(async () => { await api.setBackupFolder(null); await loadBackups() })}>Stop copying there</Button>{/if}
      {/if}
      {#if inApp}<Button variant="quiet" onclick={() => act(saveCopy)}>Save a backup as…</Button>{/if}
    </div>
  </Card>

{:else if tab === 'trash'}
  <Card title="Trash" explain="Deleted records stay here for 30 days, then are removed for good. Restoring a person brings back the records deleted with them.">
    {#if trash.length === 0}
      <p class="muted">The trash is empty.</p>
    {:else}
      {#each trash as t (t.resource.id)}
        {@const d = describe(t.resource)}
        <Row title={d.title} sub={`${d.what} · ${daysLeft(t.deletedAt)} days left`}>
          {#snippet trailing()}
            <button class="link" onclick={() => act(async () => { await api.restore(t.resource.id!); await session.changed(`Restored ${d.title}.`) })}>Restore</button>
          {/snippet}
        </Row>
      {/each}
    {/if}
  </Card>
{/if}

<style>
  .tabs { margin-bottom: 16px; overflow-x: auto; }
  .tools { display: inline-flex; gap: 12px; font-size: var(--text-small); }
  .gap { height: 18px; }
  .small { font-size: var(--text-small); overflow-wrap: anywhere; }
  .key {
    display: block; margin: 12px 0; padding: 14px; border-radius: var(--r-sm); background: var(--c-accent-soft);
    font-family: var(--font-mono, monospace); line-height: 1.7; overflow-wrap: anywhere;
  }
</style>
