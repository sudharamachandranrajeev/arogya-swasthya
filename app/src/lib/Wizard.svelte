<script lang="ts">
  // First run: where you live, the terms, the recovery key (shown once, then
  // checked), an optional app lock, backups, and the first person. Coming back
  // after the terms change shows only the terms.
  import { Button, Card, Explain } from '@swasthya/ui'
  import { api, inApp, message } from './api.ts'
  import DateField from './DateField.svelte'
  import { check, patient, toFhir, type Values } from './model/fhir.ts'
  import { emergencyLine } from './model/places.ts'
  import { TERMS_VERSION, terms } from './model/terms.ts'
  import PlacesEditor from './PlacesEditor.svelte'
  import { session } from './session.svelte.ts'

  type Step = 'welcome' | 'places' | 'terms' | 'recovery' | 'lock' | 'backups' | 'person'
  const all: Step[] = ['welcome', 'places', 'terms', 'recovery', 'lock', 'backups', 'person']

  // Someone who finished setup before only needs to accept new terms.
  const returning = session.people.length > 0 && !session.status?.recoveryPending
  const steps: Step[] = returning ? ['terms'] : all
  let at = $state(0)
  const step = $derived(steps[at])
  const next = () => (at = Math.min(at + 1, steps.length - 1))
  const back = () => (at = Math.max(at - 1, 0))

  let error = $state('')
  let busy = $state(false)
  async function run(f: () => Promise<unknown>, thenNext = true) {
    busy = true
    error = ''
    try {
      await f()
      if (thenNext) next()
    } catch (e) {
      error = message(e)
    } finally {
      busy = false
    }
  }

  const phone = $derived(['android', 'ios'].includes(session.status?.platform ?? ''))

  // ---- terms ----
  let agree = $state(false)
  const t = $derived(terms(session.places))
  async function accept() {
    await api.putSetting('terms.accepted', { version: TERMS_VERSION, at: new Date().toISOString() })
    if (returning) await finish()
  }

  // ---- recovery key ----
  let key = $state('')
  let shown = $state(false)
  let copied = $state(false)
  let confirming = $state(false)
  let typed = $state('')
  // Ask for one group of four, picked at random, to show it was written down.
  const group = 1 + Math.floor(Math.random() * 16)
  $effect(() => {
    if (step === 'recovery' && !key) api.recoveryKey().then((k) => (key = k), (e) => (error = message(e)))
  })
  async function copy() {
    await navigator.clipboard.writeText(key)
    copied = true
  }
  async function confirmKey() {
    const want = key.split('-')[group - 1]
    if (typed.trim().toLowerCase() !== want) throw `That is not group ${group}. Look at what you wrote down and try again.`
    await api.recoverySaved()
  }

  // ---- app lock ----
  let pin = $state('')
  let pin2 = $state('')
  async function savePin() {
    if (!/^\d{4,}$/.test(pin)) throw 'Use at least 4 digits.'
    if (pin !== pin2) throw 'The two PINs are different.'
    await api.setPin(pin)
  }

  // ---- backups ----
  let folder = $state<string | null>(null)
  async function chooseFolder() {
    const { open } = await import('@tauri-apps/plugin-dialog')
    const picked = await open({ directory: true, title: 'Choose a folder for backup copies' })
    if (typeof picked !== 'string') return
    await api.setBackupFolder(picked)
    folder = picked
  }

  // ---- first person ----
  let person = $state<Values>({})
  let problems = $state<string[]>([])
  async function addPerson() {
    problems = check(patient, person)
    if (problems.length) return
    const saved = await api.save(toFhir(patient, person, ''))
    await api.putSetting('person.current', saved.id)
    await finish()
  }

  async function finish() {
    await api.putSetting('setup.done', true)
    await session.load()
  }
</script>

<main>
  <div class="wrap">
    {#if steps.length > 1}
      <ol class="progress" aria-label="Setup steps">
        {#each steps as s, i (s)}<li class:done={i < at} class:now={i === at}><span class="sr">{s}</span></li>{/each}
      </ol>
    {/if}

    {#if step === 'welcome'}
      <Card title="Welcome to Arogya Swasthya">
        <p>A private place for your health records, and for the people you look after.</p>
        <ul class="points">
          <li>Everything is encrypted and stays on this device.</li>
          <li>Backups are made every day, automatically.</li>
          <li>Nothing is sent anywhere unless you choose to send it.</li>
        </ul>
        <p class="muted">Setup takes about two minutes.</p>
        <div class="actions"><Button onclick={next}>Start</Button></div>
      </Card>

    {:else if step === 'places'}
      <Card title="Where do you live?" explain="Your country sets which emergency numbers the app shows. If you split your year between countries, add each one; when you travel, switch “Where you are now”.">
        <PlacesEditor places={session.places} onchange={(p) => run(() => session.setPlaces(p), false)} />
        <div class="actions">
          <Button disabled={!session.places.current} onclick={next}>Continue</Button>
          <Button variant="ghost" onclick={back}>Back</Button>
        </div>
      </Card>

    {:else if step === 'terms'}
      <Card title={t.heading}>
        {#if returning}<p class="muted">The terms have changed since you last accepted them. Please read them again.</p>{/if}
        <ol class="points">{#each t.points as p (p)}<li>{p}</li>{/each}</ol>
        <label class="agree"><input type="checkbox" bind:checked={agree} /> I have read these terms and I accept them.</label>
        <div class="actions">
          <Button disabled={!agree || busy} onclick={() => run(accept)}>Accept and continue</Button>
          {#if !returning}<Button variant="ghost" onclick={back}>Back</Button>{/if}
        </div>
      </Card>

    {:else if step === 'recovery'}
      <Card title="Your recovery key" explain="This key unlocks your records if this device's own key is lost, for example after reinstalling the system or moving to a new phone. The app cannot show it to anyone else, and nobody can reset it.">
        {#if !confirming}
          <p>Write this key on paper, or keep it in a password manager. You need it to open your records on a new device.</p>
          {#if shown}
            <code class="key">{key}</code>
            <div class="actions">
              <Button variant="quiet" onclick={copy}>{copied ? 'Copied' : 'Copy'}</Button>
            </div>
            <p class="muted small">If you copy it, paste it somewhere safe now: the clipboard is not private.</p>
          {:else}
            <div class="actions"><Button variant="quiet" onclick={() => (shown = true)}>Show my recovery key</Button></div>
          {/if}
          <div class="actions">
            <Button disabled={!shown} onclick={() => (confirming = true)}>I have kept it safe</Button>
            <Button variant="ghost" onclick={back}>Back</Button>
          </div>
        {:else}
          <form class="field" onsubmit={(e) => (e.preventDefault(), run(confirmKey))}>
            <label class="label" for="grp">Type group {group} of your key (the {group}{group === 1 ? 'st' : group === 2 ? 'nd' : group === 3 ? 'rd' : 'th'} set of four characters)</label>
            <input id="grp" bind:value={typed} maxlength="4" autocomplete="off" spellcheck="false" />
            <div class="actions">
              <Button type="submit" disabled={busy || typed.trim().length !== 4}>Check</Button>
              <Button variant="ghost" onclick={() => ((confirming = false), (typed = ''))}>Show the key again</Button>
            </div>
          </form>
        {/if}
      </Card>

    {:else if step === 'lock'}
      <Card title="Lock the app?" explain="Your records are always encrypted. An app lock also stops someone who picks up your unlocked {phone ? 'phone' : 'computer'} from opening them.">
        <form onsubmit={(e) => (e.preventDefault(), run(savePin))}>
          <div class="fields">
            <div class="field"><label class="label" for="pin1">PIN (at least 4 digits)</label><input id="pin1" type="password" inputmode="numeric" bind:value={pin} autocomplete="new-password" /></div>
            <div class="field"><label class="label" for="pin2">Same PIN again</label><input id="pin2" type="password" inputmode="numeric" bind:value={pin2} autocomplete="new-password" /></div>
          </div>
          <div class="actions">
            <Button type="submit" disabled={busy || !pin}>Use this PIN</Button>
            {#if phone}<Button variant="quiet" disabled={busy} onclick={() => run(() => api.setBiometric(true))}>Use fingerprint or face</Button>{/if}
            <Button variant="ghost" onclick={next}>Not now</Button>
          </div>
        </form>
        <p class="muted small">If you forget the PIN, your recovery key opens the app.</p>
      </Card>

    {:else if step === 'backups'}
      <Card title="Backups" explain="A backup is an encrypted copy of all your records. It opens only with your recovery key or this device's key.">
        <p>Arogya Swasthya makes an encrypted backup every day and keeps the last 14.</p>
        {#if phone}
          <p class="muted">You can also save a copy anywhere you like, such as Google Drive, from Settings → Backups.</p>
        {:else if inApp}
          <p>Keep a second copy in a folder of your choice, such as a USB drive or a synced cloud folder, so one lost disk cannot lose everything.</p>
          {#if folder}<p><strong>Copies go to:</strong> {folder}</p>{/if}
          <div class="actions"><Button variant="quiet" disabled={busy} onclick={() => run(chooseFolder, false)}>{folder ? 'Choose another folder' : 'Choose a folder'}</Button></div>
        {/if}
        <div class="actions">
          <Button onclick={next}>{folder ? 'Continue' : 'Continue without a second copy'}</Button>
          <Button variant="ghost" onclick={back}>Back</Button>
        </div>
      </Card>

    {:else if step === 'person'}
      <Card title="Whose records first?" explain="Usually you. You can add family members or anyone you care for later, in Settings → People.">
        <form onsubmit={(e) => (e.preventDefault(), run(addPerson, false))} novalidate>
          <div class="fields">
            {#each patient.fields as f (f.key)}
              <div class="field">
                <span class="label"><label for={`p-${f.key}`}>{f.label}{f.required ? '' : ' (optional)'}</label>{#if f.explain}<Explain title={f.label} text={f.explain} />{/if}</span>
                {#if f.type === 'select'}
                  <select id={`p-${f.key}`} bind:value={person[f.key]}>
                    <option value="">—</option>
                    {#each f.options ?? [] as [v, l] (v)}<option value={v}>{l}</option>{/each}
                  </select>
                {:else if f.type === 'date'}
                  <DateField id={`p-${f.key}`} bind:value={person[f.key]} />
                {:else}
                  <input id={`p-${f.key}`} bind:value={person[f.key]} autocomplete="off" />
                {/if}
              </div>
            {/each}
          </div>
          {#if problems.length}<ul class="problems" role="alert">{#each problems as p (p)}<li>{p}</li>{/each}</ul>{/if}
          <div class="actions">
            <Button type="submit" disabled={busy}>Finish</Button>
            <Button variant="ghost" onclick={back}>Back</Button>
          </div>
        </form>
      </Card>
    {/if}

    {#if error}<p class="problems" role="alert">{error}</p>{/if}
    {#if session.places.current && step !== 'places'}<p class="sos">{emergencyLine(session.places)}</p>{/if}
  </div>
</main>

<style>
  main { min-height: 100dvh; display: grid; place-items: start center; padding: 6vh 16px 32px; background: var(--c-bg); }
  .wrap { width: min(640px, 100%); display: grid; gap: 14px; }
  .progress { display: flex; gap: 6px; list-style: none; padding: 0; margin: 0; }
  .progress li { flex: 1; height: 4px; border-radius: 2px; background: var(--c-line); }
  .progress li.done, .progress li.now { background: var(--c-accent); }
  .progress li.now { opacity: .6; }
  .sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); }
  .points { display: grid; gap: 8px; padding-left: 20px; margin: 10px 0; }
  .agree { display: flex; gap: 10px; align-items: flex-start; margin-top: 12px; font-weight: 600; }
  .agree input { width: 18px; height: 18px; margin-top: 2px; }
  .key {
    display: block; margin: 12px 0; padding: 14px; border-radius: var(--r-sm); background: var(--c-accent-soft);
    font-family: var(--font-mono, monospace); font-size: 1.05rem; line-height: 1.7; word-spacing: 2px; overflow-wrap: anywhere;
  }
  .small { font-size: var(--text-small); }
  .sos { color: var(--c-muted); font-size: var(--text-small); text-align: center; }
</style>
